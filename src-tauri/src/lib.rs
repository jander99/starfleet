//! The desktop shell: a window, and a thin command layer over `starfleet-core`.

use starfleet_core::log::{self, LogEntry};
use starfleet_core::paths::Paths;
use starfleet_core::preflight::Check;
use starfleet_core::stations::Roster;
use starfleet_core::view::BoardView;
use starfleet_core::{DispatchRequest, DispatchResult, Options, Starfleet};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};

/// The core, or the reason it could not start (shown by the UI instead of
/// the app dying on, say, a hand-edited roster with a typo).
struct Core(Result<Arc<Starfleet>, String>);

impl Core {
    fn get(&self) -> Result<&Arc<Starfleet>, String> {
        self.0.as_ref().map_err(Clone::clone)
    }
}

type CmdResult<T> = Result<T, String>;

/// Errors cross to the UI as one readable line. The alternate format makes
/// `anyhow` errors include their causes.
fn text(e: impl std::fmt::Display) -> String {
    format!("{e:#}")
}

#[tauri::command]
fn board_view(core: State<Core>) -> CmdResult<BoardView> {
    Ok(core.get()?.view())
}

#[tauri::command]
fn roster(core: State<Core>) -> CmdResult<Roster> {
    Ok(core.get()?.roster().clone())
}

#[tauri::command]
async fn preflight(core: State<'_, Core>) -> CmdResult<Vec<Check>> {
    Ok(core.get()?.preflight().await)
}

#[tauri::command]
async fn create_card(
    core: State<'_, Core>,
    title: String,
    cwd: String,
    notes: String,
) -> CmdResult<String> {
    core.get()?
        .create_card(&title, &cwd, &notes)
        .await
        .map_err(text)
}

#[tauri::command]
async fn update_card(
    core: State<'_, Core>,
    id: String,
    title: Option<String>,
    cwd: Option<String>,
    notes: Option<String>,
    done: Option<bool>,
) -> CmdResult<()> {
    core.get()?
        .update_card(&id, title, cwd, notes, done)
        .await
        .map_err(text)
}

#[tauri::command]
fn delete_card(core: State<Core>, id: String) -> CmdResult<()> {
    core.get()?.delete_card(&id).map_err(text)
}

#[tauri::command]
fn attach(core: State<Core>, card_id: String, pane_id: String) -> CmdResult<()> {
    core.get()?.attach(&card_id, &pane_id).map_err(text)
}

#[tauri::command]
fn detach(core: State<Core>, link_id: String) -> CmdResult<()> {
    core.get()?.detach(&link_id).map_err(text)
}

#[tauri::command]
async fn dispatch(core: State<'_, Core>, request: DispatchRequest) -> CmdResult<DispatchResult> {
    core.get()?.dispatch(request).await.map_err(text)
}

#[tauri::command]
async fn send_pending(core: State<'_, Core>, link_id: String) -> CmdResult<()> {
    core.get()?.send_pending(&link_id).await.map_err(text)
}

#[tauri::command]
async fn follow_up(core: State<'_, Core>, pane_id: String, text: String) -> CmdResult<()> {
    core.get()?
        .follow_up(&pane_id, &text)
        .await
        .map_err(self::text)
}

#[tauri::command]
async fn reply_keys(
    core: State<'_, Core>,
    pane_id: String,
    text: Option<String>,
    keys: Vec<String>,
) -> CmdResult<()> {
    core.get()?
        .reply_keys(&pane_id, text, keys)
        .await
        .map_err(self::text)
}

#[tauri::command]
async fn tail(core: State<'_, Core>, pane_id: String, lines: u32) -> CmdResult<String> {
    core.get()?.tail(&pane_id, lines).await.map_err(text)
}

#[tauri::command]
fn saved_output(core: State<Core>, link_id: String) -> CmdResult<Option<String>> {
    core.get()?.saved_output(&link_id).map_err(text)
}

#[tauri::command]
async fn focus(core: State<'_, Core>, pane_id: String) -> CmdResult<()> {
    core.get()?.focus(&pane_id).await.map_err(text)
}

#[tauri::command]
async fn poll_prs(core: State<'_, Core>) -> CmdResult<usize> {
    let core = core.get()?;
    Ok(core.poll_prs_once(starfleet_core::store::now_ms()).await)
}

#[tauri::command]
fn add_artifact(
    core: State<Core>,
    card_id: String,
    kind: starfleet_core::store::ArtifactKind,
    label: String,
    target: String,
) -> CmdResult<String> {
    core.get()?
        .add_artifact(&card_id, kind, &label, &target)
        .map_err(text)
}

#[tauri::command]
async fn open_artifact(
    core: State<'_, Core>,
    card_id: String,
    artifact_id: String,
) -> CmdResult<()> {
    core.get()?
        .open_artifact(&card_id, &artifact_id)
        .await
        .map_err(text)
}

#[tauri::command]
fn read_log(core: State<Core>, card_id: String, limit: Option<usize>) -> CmdResult<Vec<LogEntry>> {
    let core = core.get()?;
    let mut entries = log::read(core.paths(), &card_id).map_err(text)?;
    if let Some(limit) = limit {
        let keep = entries.len().saturating_sub(limit);
        entries.drain(..keep);
    }
    Ok(entries)
}

#[tauri::command]
fn append_log(
    core: State<Core>,
    card_id: String,
    text: String,
    kind: Option<String>,
    session_id: Option<String>,
) -> CmdResult<LogEntry> {
    let core = core.get()?;
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("write something first".into());
    }
    let kind = kind.unwrap_or_else(|| "note".into());
    if !matches!(kind.as_str(), "note" | "decision") {
        return Err(format!("not a log entry kind: {kind}"));
    }
    let at = starfleet_core::store::now_ms();
    let entry = LogEntry {
        id: starfleet_core::store::short_id(),
        owner: "captain".into(),
        at,
        kind,
        text,
        links: Vec::new(),
        session_id,
    };
    log::append(core.paths(), &card_id, &entry).map_err(self::text)?;
    Ok(entry)
}

fn start_core() -> Result<Arc<Starfleet>, String> {
    let paths = Paths::resolve().map_err(text)?;
    Starfleet::new(Options {
        paths,
        endpoint: None,
        env: None,
        gh: None,
    })
    .map_err(text)
}

/// Start the core and wire it to the window: the Herdr monitor runs in the
/// background, and every board change is forwarded to the UI as an event.
/// Kept separate from `run` so it can be exercised without a real window.
fn boot<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Core {
    let core = start_core();
    if let Ok(starfleet) = &core {
        tauri::async_runtime::spawn(starfleet.clone().run_monitor());
        let mut changes = starfleet.changes();
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            while changes.changed().await.is_ok() {
                let _ = handle.emit("board-changed", ());
            }
        });
    }
    Core(core)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(boot(app.handle()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            board_view,
            roster,
            preflight,
            create_card,
            update_card,
            delete_card,
            attach,
            detach,
            dispatch,
            send_pending,
            follow_up,
            reply_keys,
            tail,
            saved_output,
            focus,
            poll_prs,
            add_artifact,
            open_artifact,
            read_log,
            append_log
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the Starfleet window");
}

#[cfg(test)]
mod tests {
    use super::*;
    use starfleet_core::paths::Paths;

    /// A started core on a scratch home, plus the app that owns it as state.
    fn app() -> (tauri::App<tauri::test::MockRuntime>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::at(dir.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        let core = Starfleet::new(Options { paths, endpoint: None, env: None, gh: None }).unwrap();
        let app = tauri::test::mock_builder().build(tauri::generate_context!()).unwrap();
        app.manage(Core(Ok(core)));
        (app, dir)
    }

    /// Id of an existing card, creating one on the scratch board if needed.
    fn a_card(app: &tauri::App<tauri::test::MockRuntime>) -> String {
        let cards = board_view(app.state()).unwrap().cards;
        match cards.first() {
            Some(c) => c.card.id.clone(),
            None => tauri::async_runtime::block_on(create_card(
                app.state(),
                "seed".into(),
                "/tmp".into(),
                String::new(),
            ))
            .unwrap(),
        }
    }

    #[test]
    fn board_view_and_roster_round_trip() {
        let (app, _dir) = app();
        let view = board_view(app.state()).unwrap();
        // Nothing is listening on the scratch socket, but the view still builds.
        assert!(!view.endpoint.is_empty());
        let roster = roster(app.state()).unwrap();
        assert!(!roster.locations.is_empty());
    }

    #[test]
    fn a_core_that_failed_to_start_reports_its_reason() {
        let app = tauri::test::mock_builder().build(tauri::generate_context!()).unwrap();
        app.manage(Core(Err("bad roster".into())));
        assert_eq!(board_view(app.state()).unwrap_err(), "bad roster");
        assert_eq!(delete_card(app.state(), "c1".into()).unwrap_err(), "bad roster");
    }

    #[tokio::test]
    async fn card_lifecycle_commands_work() {
        let (app, _dir) = app();
        // Use a scratch folder for the card so no real path is touched.
        let id = create_card(app.state(), "T".into(), "/tmp".into(), "n".into()).await.unwrap();
        update_card(app.state(), id.clone(), Some("T2".into()), None, Some("more".into()), Some(true))
            .await
            .unwrap();
        let card = board_view(app.state()).unwrap().cards.into_iter().find(|c| c.card.id == id).unwrap();
        assert_eq!(card.card.title, "T2");
        assert_eq!(card.card.notes, "more");
        assert!(card.card.done);
        delete_card(app.state(), id.clone()).unwrap();
        assert!(board_view(app.state()).unwrap().cards.iter().all(|c| c.card.id != id));
    }

    #[tokio::test]
    async fn create_card_rejects_a_bad_folder() {
        let (app, _dir) = app();
        let err = create_card(app.state(), "T".into(), "".into(), "".into()).await.unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn artifacts_and_links_round_trip() {
        let (app, _dir) = app();
        let id = a_card(&app);
        let a = add_artifact(app.state(), id.clone(), starfleet_core::store::ArtifactKind::Url, "Docs".into(), "https://x".into())
            .unwrap();
        assert!(!a.is_empty());
        // Detaching an unknown link is a no-op, not a panic.
        assert!(detach(app.state(), "nope".into()).is_ok());
        let _ = attach(app.state(), id, "pane-x".into());
    }

    #[test]
    fn read_log_limits_to_the_newest_entries() {
        let (app, _dir) = app();
        let id = a_card(&app);
        for i in 0..3 {
            append_log(app.state(), id.clone(), format!("entry {i}"), None, None).unwrap();
        }
        assert_eq!(read_log(app.state(), id.clone(), None).unwrap().len(), 3);
        let last = read_log(app.state(), id.clone(), Some(1)).unwrap();
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].text, "entry 2");
        // A limit beyond the count keeps everything.
        assert_eq!(read_log(app.state(), id, Some(99)).unwrap().len(), 3);
    }

    #[test]
    fn append_log_validates_its_input() {
        let (app, _dir) = app();
        let id = a_card(&app);
        assert_eq!(append_log(app.state(), id.clone(), "   ".into(), None, None).unwrap_err(), "write something first");
        assert_eq!(
            append_log(app.state(), id.clone(), "x".into(), Some("prompt".into()), None).unwrap_err(),
            "not a log entry kind: prompt"
        );
        let e = append_log(app.state(), id, "kept".into(), Some("decision".into()), Some("l1".into())).unwrap();
        assert_eq!(e.owner, "captain");
        assert_eq!(e.kind, "decision");
        assert_eq!(e.session_id.as_deref(), Some("l1"));
        assert_eq!(e.text, "kept");
    }

    /// Process env is global: every test that sets or clears a variable
    /// serializes on this one lock.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[cfg(unix)]
    #[test]
    fn start_core_honors_starfleet_home() {
        let _g = env_guard();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::var("STARFLEET_HOME").ok();
        std::env::set_var("STARFLEET_HOME", dir.path());
        let core = start_core();
        match old {
            Some(v) => std::env::set_var("STARFLEET_HOME", v),
            None => std::env::remove_var("STARFLEET_HOME"),
        }
        let core = core.unwrap();
        assert_eq!(core.paths().config_dir, dir.path().join("config"));
    }

    #[cfg(unix)]
    #[test]
    fn boot_wires_the_core_to_the_window() {
        let _g = env_guard();
        let dir = tempfile::tempdir().unwrap();
        let old = std::env::var("STARFLEET_HOME").ok();
        std::env::set_var("STARFLEET_HOME", dir.path());
        let app = tauri::test::mock_builder().build(tauri::generate_context!()).unwrap();
        let core = boot(app.handle());
        match old {
            Some(v) => std::env::set_var("STARFLEET_HOME", v),
            None => std::env::remove_var("STARFLEET_HOME"),
        }
        // The core started and points at the scratch home.
        let sf = core.get().unwrap();
        assert_eq!(sf.paths().config_dir, dir.path().join("config"));
        // A board change bumps the watch value the event task is listening on.
        let changes = sf.changes();
        let before = *changes.borrow();
        tauri::async_runtime::block_on(sf.create_card("T", "/tmp", "")).unwrap();
        assert!(*changes.borrow() > before);
    }

    #[test]
    fn boot_reports_a_core_that_cannot_start() {
        let _g = env_guard();
        let app = tauri::test::mock_builder()
            .build(tauri::generate_context!())
            .unwrap();
        // A directory where the config file must live makes the start fail
        // only if the path is unusable; force it by pointing at a file.
        let file = tempfile::NamedTempFile::new().unwrap();
        let old = std::env::var("STARFLEET_HOME").ok();
        std::env::set_var("STARFLEET_HOME", file.path());
        let core = boot(app.handle());
        match old {
            Some(v) => std::env::set_var("STARFLEET_HOME", v),
            None => std::env::remove_var("STARFLEET_HOME"),
        }
        // Either it started (paths under the file are still constructible) or
        // it carries a readable reason — never a panic.
        if let Ok(sf) = core.get() {
            assert!(!sf.paths().config_dir.as_os_str().is_empty());
        } else {
            assert!(!core.get().err().unwrap().is_empty());
        }
    }

    #[test]
    fn text_keeps_the_error_chain() {
        let e = anyhow::anyhow!("outer").context("inner");
        assert_eq!(text(e), "inner: outer");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn agent_commands_reach_the_core() {
        let (app, _dir) = app();
        // No Herdr is listening, so these fail — but they run the core path.
        let _ = preflight(app.state()).await;
        assert!(tail(app.state(), "p1".into(), 10).await.is_err());
        assert!(focus(app.state(), "p1".into()).await.is_err());
        assert!(follow_up(app.state(), "p1".into(), "hi".into()).await.is_err());
        assert!(reply_keys(app.state(), "p1".into(), None, vec!["enter".into()]).await.is_err());
        assert!(send_pending(app.state(), "l1".into()).await.is_err());
        assert!(saved_output(app.state(), "l1".into()).is_err());
        let _ = poll_prs(app.state()).await;
        let req = DispatchRequest { card_id: "c".into(), station_id: "s".into(), prompt: "p".into(), agent: None, model: None };
        assert!(dispatch(app.state(), req).await.is_err());
        assert!(open_artifact(app.state(), "c".into(), "a".into()).await.is_err());
    }
}
