//! End-to-end tests of the core against the fake Herdr server.

use mock_herdr::MockHerdr;
use starfleet_core::env::HostEnv;
use starfleet_core::herdr::Endpoint;
use starfleet_core::paths::Paths;
use starfleet_core::view::{BoardView, SessionStatus};
use starfleet_core::{DispatchRequest, Options, Starfleet};
use std::sync::Arc;
use std::time::Duration;

struct Rig {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    socket: std::path::PathBuf,
    mock: MockHerdr,
}

async fn rig() -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let socket = root.join("herdr.sock");
    let mock = MockHerdr::serve(&socket, 150).await.unwrap();
    Rig {
        _dir: dir,
        root,
        socket,
        mock,
    }
}

impl Rig {
    fn app_with(&self, endpoint: Endpoint) -> Arc<Starfleet> {
        let app = Starfleet::new(Options {
            paths: Paths::at(&self.root.join("starfleet")),
            endpoint: Some(endpoint),
            env: Some(HostEnv::Native),
        })
        .unwrap();
        tokio::spawn(app.clone().run_monitor());
        app
    }

    fn app(&self) -> Arc<Starfleet> {
        self.app_with(Endpoint::Unix(self.socket.clone()))
    }

    fn workdir(&self, name: &str) -> String {
        let dir = self.root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        dir.to_string_lossy().into_owned()
    }
}

/// Wait until the view satisfies `cond`, driven by the monitor's change signal.
async fn until(app: &Starfleet, what: &str, cond: impl Fn(&BoardView) -> bool) -> BoardView {
    let mut changes = app.changes();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let view = app.view();
        if cond(&view) {
            return view;
        }
        if tokio::time::timeout_at(deadline, changes.changed())
            .await
            .is_err()
        {
            panic!(
                "timed out waiting for: {what}\nview: {}",
                serde_json::to_string_pretty(&view).unwrap()
            );
        }
    }
}

fn request(card_id: &str, station: &str, prompt: &str) -> DispatchRequest {
    DispatchRequest {
        card_id: card_id.to_string(),
        station_id: station.to_string(),
        prompt: prompt.to_string(),
        agent: None,
        model: None,
    }
}

#[tokio::test]
async fn dispatch_runs_a_session_and_tracks_it_to_done() {
    let rig = rig().await;
    let app = rig.app();
    until(&app, "connected", |v| v.connected).await;

    let card = app
        .create_card("Cache research", &rig.workdir("repo"), "")
        .await
        .unwrap();
    let result = app
        .dispatch(request(
            &card,
            "daystrom.research",
            "Can the library delete keys?",
        ))
        .await
        .unwrap();
    assert!(result.warning.is_none());

    // The station's base prompt and the user's prompt arrive as one message.
    let prompts = rig.mock.prompts_received(&result.pane_id);
    assert_eq!(prompts.len(), 1);
    assert!(prompts[0].contains("You are a research agent"));
    assert!(prompts[0].ends_with("Can the library delete keys?"));

    // Status changes arrive by event, with no further calls from the test.
    let view = until(&app, "session done", |v| {
        v.cards
            .first()
            .and_then(|c| c.sessions.first())
            .map(|s| s.status)
            == Some(SessionStatus::Done)
    })
    .await;
    let session = &view.cards[0].sessions[0];
    assert_eq!(
        session.station_name.as_deref(),
        Some("Dr. Richard Daystrom")
    );
    assert_eq!(session.workspace.as_deref(), Some("Cache research"));
    assert_eq!(view.counts.done, 1);
    assert!(view.unassigned.is_empty());

    let tail = app.tail(&result.pane_id, 50).await.unwrap();
    assert!(tail.contains("simulated work complete"));
}

#[tokio::test]
async fn second_dispatch_reuses_the_cards_workspace_and_applies_overrides() {
    let rig = rig().await;
    let app = rig.app();
    let card = app
        .create_card("Feature", &rig.workdir("repo"), "")
        .await
        .unwrap();

    let first = app
        .dispatch(request(&card, "enterprise.picard", "plan it"))
        .await
        .unwrap();
    let mut second = request(&card, "enterprise.riker", "build it");
    second.agent = Some("opencode".into());
    second.model = Some("openai/gpt-5".into());
    let second = app.dispatch(second).await.unwrap();

    let workspace = |pane: &str| pane.split(':').next().unwrap().to_string();
    assert_eq!(workspace(&first.pane_id), workspace(&second.pane_id));
    assert_ne!(first.pane_id, second.pane_id);

    let tail = app.tail(&second.pane_id, 50).await.unwrap();
    assert!(
        tail.contains("$ opencode --model openai/gpt-5"),
        "tail was: {tail}"
    );
    assert_eq!(app.view().cards[0].sessions.len(), 2);
}

#[tokio::test]
async fn a_blocked_agent_holds_the_prompt_until_answered() {
    let rig = rig().await;
    let app = rig.app();
    let card = app
        .create_card("Risky", &rig.workdir("untrusted-repo"), "")
        .await
        .unwrap();

    let result = app
        .dispatch(request(&card, "enterprise.laforge", "fix the bug"))
        .await
        .unwrap();
    assert!(result
        .warning
        .as_deref()
        .unwrap()
        .contains("waiting on a question"));
    assert!(rig.mock.prompts_received(&result.pane_id).is_empty());

    let view = until(&app, "blocked session", |v| v.counts.blocked == 1).await;
    assert!(view.cards[0].sessions[0].pending_prompt.is_some());

    // Follow-ups are refused while blocked, with a usable message.
    let refused = app.follow_up(&result.pane_id, "hello").await.unwrap_err();
    assert!(refused.to_string().contains("waiting on a question"));

    // Answer the dialog, then deliver the held prompt.
    app.reply_keys(&result.pane_id, Some("1".into()), vec![])
        .await
        .unwrap();
    until(&app, "unblocked", |v| v.counts.blocked == 0).await;
    app.send_pending(&result.link_id).await.unwrap();
    assert_eq!(rig.mock.prompts_received(&result.pane_id).len(), 1);
    until(&app, "pending cleared and done", |v| {
        let s = &v.cards[0].sessions[0];
        s.pending_prompt.is_none() && s.status == SessionStatus::Done
    })
    .await;
}

#[tokio::test]
async fn hand_started_sessions_are_unassigned_until_attached() {
    let rig = rig().await;
    let pane = rig
        .mock
        .seed_agent("homelab", "claude", "working", "/srv/homelab");
    let app = rig.app();

    let view = until(&app, "unassigned contact", |v| v.unassigned.len() == 1).await;
    assert_eq!(view.unassigned[0].workspace.as_deref(), Some("homelab"));
    assert_eq!(view.counts.working, 1);

    let a = app
        .create_card("Homelab dashboards", &rig.workdir("a"), "")
        .await
        .unwrap();
    let b = app
        .create_card("Other", &rig.workdir("b"), "")
        .await
        .unwrap();
    app.attach(&a, &pane).unwrap();
    let view = app.view();
    assert!(view.unassigned.is_empty());
    let sessions_of = |v: &BoardView, id: &str| {
        v.cards
            .iter()
            .find(|c| c.card.id == id)
            .unwrap()
            .sessions
            .len()
    };
    assert_eq!(sessions_of(&view, &a), 1);

    // Re-attaching moves the session; it never sits on two cards.
    app.attach(&b, &pane).unwrap();
    let view = app.view();
    assert_eq!(sessions_of(&view, &a), 0);
    assert_eq!(sessions_of(&view, &b), 1);

    // Detaching returns it to the unassigned tray, still running.
    let link = view.cards.iter().find(|c| c.card.id == b).unwrap().sessions[0]
        .link_id
        .clone();
    app.detach(&link).unwrap();
    assert_eq!(app.view().unassigned.len(), 1);
}

#[tokio::test]
async fn closed_sessions_go_offline_and_the_board_survives_a_restart() {
    let rig = rig().await;
    let app = rig.app();
    let card = app
        .create_card("Persist", &rig.workdir("repo"), "notes")
        .await
        .unwrap();
    let result = app
        .dispatch(request(&card, "ds9.kira", "review"))
        .await
        .unwrap();
    until(&app, "done", |v| v.counts.done == 1).await;

    rig.mock.close_pane(&result.pane_id);
    let view = until(&app, "offline", |v| v.counts.offline == 1).await;
    let session = &view.cards[0].sessions[0];
    assert_eq!(session.status, SessionStatus::Offline);
    assert!(!session.live);

    // A second instance reads the same board from disk.
    let again = rig.app();
    let view = until(&again, "reloaded", |v| v.connected && v.cards.len() == 1).await;
    assert_eq!(view.cards[0].card.title, "Persist");
    assert_eq!(view.cards[0].sessions[0].prompt.as_deref(), Some("review"));
}

#[tokio::test]
async fn a_finished_session_keeps_its_output_after_the_pane_closes() {
    let rig = rig().await;
    let app = rig.app();
    let card = app
        .create_card("Record", &rig.workdir("repo"), "")
        .await
        .unwrap();
    let result = app
        .dispatch(request(&card, "daystrom.research", "find out"))
        .await
        .unwrap();

    // The output is saved when the agent finishes, without opening the drawer.
    until(&app, "done", |v| v.counts.done == 1).await;
    let saved = |app: &Starfleet| app.saved_output(&result.link_id).unwrap();
    until(&app, "output saved", |_| {
        saved(&app).is_some_and(|t| t.contains("simulated work complete"))
    })
    .await;

    rig.mock.close_pane(&result.pane_id);
    let view = until(&app, "offline", |v| v.counts.offline == 1).await;
    assert!(view.cards[0].sessions[0].output_at.is_some());
    assert!(saved(&app).unwrap().contains("simulated work complete"));

    // It is part of the board file, so a restart keeps it.
    let again = rig.app();
    until(&again, "reloaded", |v| v.connected).await;
    assert!(saved(&again).unwrap().contains("find out"));
}

#[tokio::test]
async fn failures_are_reported_without_leaving_debris() {
    let rig = rig().await;
    let app = rig.app();

    // Not a directory.
    let err = app
        .create_card("Bad", "/definitely/not/here", "")
        .await
        .unwrap_err();
    assert!(err.to_string().contains("is not a folder"));

    // An agent CLI that cannot start: error surfaces, the new pane is cleaned up.
    let card = app
        .create_card("Card", &rig.workdir("repo"), "")
        .await
        .unwrap();
    let mut bad = request(&card, "enterprise.data", "analyse");
    bad.agent = Some("nonexistent-cli".into());
    let err = app.dispatch(bad).await.unwrap_err();
    assert!(
        format!("{err:#}").contains("could not start `nonexistent-cli`"),
        "{err:#}"
    );
    let view = app.view();
    assert!(view.cards[0].sessions.is_empty());
    assert!(view.unassigned.is_empty());
}

#[tokio::test]
async fn herdr_going_away_is_shown_as_disconnected() {
    let dir = tempfile::tempdir().unwrap();
    let app = Starfleet::new(Options {
        paths: Paths::at(&dir.path().join("starfleet")),
        endpoint: Some(Endpoint::Unix(dir.path().join("nothing-here.sock"))),
        env: Some(HostEnv::Native),
    })
    .unwrap();
    tokio::spawn(app.clone().run_monitor());
    let view = until(&app, "error reported", |v| v.error.is_some()).await;
    assert!(!view.connected);
    assert!(view.error.unwrap().contains("cannot reach Herdr"));

    let checks = app.preflight().await;
    let server = checks.iter().find(|c| c.id == "herdr.server").unwrap();
    assert_eq!(serde_json::to_value(server.status).unwrap(), "fail");
}

/// The Windows path: Herdr reached through a relay process's stdin/stdout.
/// Run here with a native shell standing in for `wsl.exe`.
#[tokio::test]
async fn relay_transport_carries_requests_and_events() {
    let rig = rig().await;
    let app = rig.app_with(Endpoint::Relay {
        env: HostEnv::Native,
        socket_expr: format!("'{}'", rig.socket.display()),
    });
    until(&app, "connected through relay", |v| v.connected).await;
    let card = app
        .create_card("Relay", &rig.workdir("repo"), "")
        .await
        .unwrap();
    app.dispatch(request(&card, "ds9.dax", "explore"))
        .await
        .unwrap();
    until(&app, "done through relay", |v| v.counts.done == 1).await;
}

/// An agent still at a startup dialog (a folder trust prompt) is "not ready",
/// which Herdr reports differently from "blocked". The user must get the same
/// outcome either way: the prompt is held, nothing is lost, and navigation keys
/// (not text) are enough to answer, so nothing here depends on one agent's UI.
#[tokio::test]
async fn an_agent_at_a_startup_dialog_holds_the_prompt() {
    let rig = rig().await;
    let app = rig.app();
    let card = app
        .create_card("Fresh folder", &rig.workdir("untrusted-new"), "")
        .await
        .unwrap();

    let result = app
        .dispatch(request(&card, "ds9.bashir", "find the cause"))
        .await
        .unwrap();
    let warning = result.warning.as_deref().unwrap();
    assert!(warning.contains("waiting on a question"), "{warning}");
    assert!(
        !warning.contains("not delivered"),
        "should not fall back to the generic message: {warning}"
    );
    assert!(rig.mock.prompts_received(&result.pane_id).is_empty());

    let view = until(&app, "held prompt on a blocked session", |v| {
        v.counts.blocked == 1
    })
    .await;
    assert!(view.cards[0].sessions[0].pending_prompt.is_some());

    let refused = app.follow_up(&result.pane_id, "hello").await.unwrap_err();
    assert!(refused.to_string().contains("waiting on a question"));

    // Answer with keys only, as the UI's Enter/Esc/arrow buttons would.
    app.reply_keys(&result.pane_id, None, vec!["down".into(), "enter".into()])
        .await
        .unwrap();
    until(&app, "dialog answered", |v| v.counts.blocked == 0).await;

    app.send_pending(&result.link_id).await.unwrap();
    assert_eq!(rig.mock.prompts_received(&result.pane_id).len(), 1);
    until(&app, "finished after the held prompt", |v| {
        let s = &v.cards[0].sessions[0];
        s.pending_prompt.is_none() && s.status == SessionStatus::Done
    })
    .await;
}

/// An agent that was ready, took the prompt, and then stopped to ask something
/// is "blocked". Follow-ups are refused with the same usable message.
#[tokio::test]
async fn an_agent_that_blocks_mid_task_refuses_follow_ups() {
    let rig = rig().await;
    let app = rig.app();
    let card = app
        .create_card("Mid-task", &rig.workdir("repo"), "")
        .await
        .unwrap();

    let result = app
        .dispatch(request(&card, "enterprise.riker", "build it [block]"))
        .await
        .unwrap();
    assert!(result.warning.is_none());
    assert_eq!(rig.mock.prompts_received(&result.pane_id).len(), 1);

    let view = until(&app, "blocked mid-task", |v| v.counts.blocked == 1).await;
    assert!(view.cards[0].sessions[0].pending_prompt.is_none());

    let refused = app.follow_up(&result.pane_id, "hello").await.unwrap_err();
    assert!(refused.to_string().contains("waiting on a question"));
    assert_eq!(rig.mock.prompts_received(&result.pane_id).len(), 1);

    app.reply_keys(&result.pane_id, None, vec!["enter".into()])
        .await
        .unwrap();
    until(&app, "unblocked", |v| v.counts.blocked == 0).await;
    app.follow_up(&result.pane_id, "carry on").await.unwrap();
    assert_eq!(rig.mock.prompts_received(&result.pane_id).len(), 2);
}
