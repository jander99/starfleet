//! The board: cards, and the links from cards to Herdr sessions.
//!
//! Starfleet owns this data. Herdr only knows about panes; which pane belongs
//! to which card is decided here, by the user.

use crate::herdr::{AgentSessionInfo, AgentStatus, PaneInfo, Snapshot};
use crate::paths::write_atomic;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..10].to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub title: String,
    /// Directory that sessions for this card start in. On Windows this is a
    /// path inside WSL.
    pub cwd: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub done: bool,
    pub created_at: u64,
    pub updated_at: u64,
    /// Durable things this card points at: a PR, a URL, a note, a pane.
    /// They outlive the panes, which is the point.
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    Pr,
    Url,
    Note,
    Pane,
}

/// A durable pointer attached to a card. `last_status`/`checked_at` are the
/// last poll's answer, kept so a stale PR state is visible as stale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub label: String,
    pub target: String,
    pub last_status: Option<String>,
    pub checked_at: Option<u64>,
}

/// One PR artifact, flattened out of its card, for the poller.
#[derive(Clone, Debug, PartialEq)]
pub struct PrArtifact {
    pub card_id: String,
    pub artifact_id: String,
    pub target: String,
}

/// Why a session ended, stamped by Starfleet when it closes a pane.
/// No stamp means the pane vanished without us: interrupted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndReason {
    Captain,
    IdleTimeout,
    Interrupted,
    #[default]
    #[serde(other)]
    Unknown,
}

/// A heuristic guess at why an agent stopped, from its output tail. Inert
/// data: shown as "probably", never used to drive behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CauseHint {
    RateLimit,
    UsageWindow,
    Outage,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionLink {
    pub id: String,
    pub card_id: String,
    /// Station that dispatched this session; `None` for sessions attached by hand.
    pub station_id: Option<String>,
    pub prompt: Option<String>,
    /// A first prompt that could not be delivered because the agent was blocked.
    pub pending_prompt: Option<String>,
    pub created_at: u64,
    // How to find the pane again. `terminal_id` identifies the terminal while
    // the Herdr server runs; `agent_session` survives a server restart;
    // `pane_id` is only trusted when nothing better was ever known.
    pub terminal_id: Option<String>,
    pub pane_id: Option<String>,
    pub agent_session: Option<AgentSessionInfo>,
    // Last known facts, shown when the session is no longer live.
    pub agent: Option<String>,
    pub title: Option<String>,
    pub last_status: AgentStatus,
    pub last_seen: Option<u64>,
    /// The end of the agent's output, saved each time it stops working, so
    /// the card keeps a record after the pane is gone.
    pub output: Option<String>,
    /// When `output` was last read (or found empty).
    pub output_at: Option<u64>,
    /// The session this one resumed, if any. History only: the newest link
    /// (by `created_at`) decides a card's state, not the chain.
    pub resumed_from: Option<String>,
    /// The directory this session was aimed at, which may differ from the
    /// live pane's cwd when a card spans a meta directory.
    pub cwd: Option<String>,
    /// Why the session ended; `None` while it runs, or when it died unstamped.
    pub end_reason: Option<EndReason>,
    pub ended_at: Option<u64>,
    /// Optional, heuristic; see [`CauseHint`].
    pub cause_hint: Option<CauseHint>,
}

/// Walk a resume chain from `start`, newest first, as ids.
///
/// History only — card state comes from the newest link by `created_at`, not
/// from this walk — but the walk is still guarded: it stops at an unknown id,
/// at an id seen before (a cycle), or after 1000 entries.
pub fn resume_chain(links: &[SessionLink], start: &str) -> Vec<String> {
    let by_id: std::collections::HashMap<&str, &SessionLink> =
        links.iter().map(|l| (l.id.as_str(), l)).collect();
    let mut visited = std::collections::HashSet::new();
    let mut chain = Vec::new();
    let mut current = start;
    while chain.len() < 1000 {
        let Some(link) = by_id.get(current) else {
            break;
        };
        if !visited.insert(link.id.clone()) {
            break;
        }
        chain.push(link.id.clone());
        let Some(next) = link.resumed_from.as_deref() else {
            break;
        };
        current = next;
    }
    chain
}

/// The most saved output kept per session, in bytes. The end is kept.
pub const OUTPUT_MAX_BYTES: usize = 32 * 1024;

/// An agent that is not working has finished a turn (or is waiting on the
/// user), so its output is worth keeping.
fn settled(status: AgentStatus) -> bool {
    matches!(
        status,
        AgentStatus::Done | AgentStatus::Idle | AgentStatus::Blocked
    )
}

impl SessionLink {
    /// The live pane this link refers to, if it still exists.
    pub fn find_live<'a>(&self, panes: &'a [PaneInfo]) -> Option<&'a PaneInfo> {
        if let Some(tid) = self.terminal_id.as_deref().filter(|t| !t.is_empty()) {
            if let Some(p) = panes.iter().find(|p| p.terminal_id == tid) {
                return Some(p);
            }
        }
        if let Some(session) = &self.agent_session {
            if let Some(p) = panes.iter().find(|p| {
                p.agent_session
                    .as_ref()
                    .is_some_and(|s| s.agent == session.agent && s.value == session.value)
            }) {
                return Some(p);
            }
        }
        // Pane ids look positional, so a bare pane id match could point at a
        // different terminal that reused the slot. Only use it as a last resort.
        if self.terminal_id.is_none() && self.agent_session.is_none() {
            if let Some(pid) = &self.pane_id {
                return panes.iter().find(|p| &p.pane_id == pid);
            }
        }
        None
    }

    fn absorb(&mut self, pane: &PaneInfo) -> bool {
        let mut changed = false;
        let mut set = |slot: &mut Option<String>, value: Option<String>| {
            if value.is_some() && *slot != value {
                *slot = value;
                changed = true;
            }
        };
        set(&mut self.pane_id, Some(pane.pane_id.clone()));
        set(
            &mut self.terminal_id,
            Some(pane.terminal_id.clone()).filter(|t| !t.is_empty()),
        );
        set(
            &mut self.agent,
            pane.display_agent.clone().or_else(|| pane.agent.clone()),
        );
        set(&mut self.title, pane_title(pane));
        if pane.agent_session.is_some() && self.agent_session != pane.agent_session {
            self.agent_session = pane.agent_session.clone();
            changed = true;
        }
        if self.last_status != pane.agent_status {
            self.last_status = pane.agent_status;
            changed = true;
        }
        changed
    }
}

pub fn pane_title(pane: &PaneInfo) -> Option<String> {
    pane.title
        .clone()
        .or_else(|| pane.label.clone())
        .or_else(|| pane.name.clone())
        .or_else(|| pane.terminal_title_stripped.clone())
        .filter(|t| !t.trim().is_empty())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Board {
    pub cards: Vec<Card>,
    pub links: Vec<SessionLink>,
}

pub struct Store {
    file: PathBuf,
    pub board: Board,
}

impl Store {
    pub fn load(file: PathBuf) -> Result<Self> {
        let board = match std::fs::read_to_string(&file) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| anyhow!("{} is not a valid board file: {e}", file.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Board::default(),
            Err(e) => bail!("cannot read {}: {e}", file.display()),
        };
        Ok(Self { file, board })
    }

    pub fn save(&self) -> Result<()> {
        write_atomic(&self.file, &serde_json::to_string_pretty(&self.board)?)
    }

    pub fn card(&self, id: &str) -> Result<&Card> {
        self.board
            .cards
            .iter()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow!("that card no longer exists"))
    }

    pub fn create_card(&mut self, title: &str, cwd: &str, notes: &str) -> Result<Card> {
        let title = title.trim();
        if title.is_empty() {
            bail!("a card needs a title");
        }
        let now = now_ms();
        let card = Card {
            id: short_id(),
            title: title.to_string(),
            cwd: cwd.trim().to_string(),
            notes: notes.trim().to_string(),
            done: false,
            created_at: now,
            updated_at: now,
            artifacts: Vec::new(),
        };
        self.board.cards.push(card.clone());
        self.save()?;
        Ok(card)
    }

    pub fn update_card(
        &mut self,
        id: &str,
        title: Option<String>,
        cwd: Option<String>,
        notes: Option<String>,
        done: Option<bool>,
    ) -> Result<()> {
        let card = self
            .board
            .cards
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or_else(|| anyhow!("that card no longer exists"))?;
        if let Some(t) = title {
            if t.trim().is_empty() {
                bail!("a card needs a title");
            }
            card.title = t.trim().to_string();
        }
        if let Some(c) = cwd {
            card.cwd = c.trim().to_string();
        }
        if let Some(n) = notes {
            card.notes = n.trim().to_string();
        }
        if let Some(d) = done {
            card.done = d;
        }
        card.updated_at = now_ms();
        self.save()
    }

    /// Delete a card and its links. The Herdr sessions themselves are untouched.
    pub fn delete_card(&mut self, id: &str) -> Result<()> {
        self.board.cards.retain(|c| c.id != id);
        self.board.links.retain(|l| l.card_id != id);
        self.save()
    }

    pub fn add_link(&mut self, link: SessionLink) -> Result<()> {
        self.board.links.push(link);
        self.save()
    }

    /// Attach a durable artifact to a card. An unknown card is an error.
    pub fn add_artifact(&mut self, card_id: &str, artifact: Artifact) -> Result<()> {
        let card = self
            .board
            .cards
            .iter_mut()
            .find(|c| c.id == card_id)
            .ok_or_else(|| anyhow!("that card no longer exists"))?;
        card.artifacts.push(artifact);
        self.save()
    }

    /// Record a poll's answer for an artifact. A vanished artifact is an
    /// error, not a panic: the card may have been edited since the poll began.
    pub fn update_artifact(
        &mut self,
        card_id: &str,
        artifact_id: &str,
        last_status: Option<&str>,
        checked_at: Option<u64>,
    ) -> Result<()> {
        let card = self
            .board
            .cards
            .iter_mut()
            .find(|c| c.id == card_id)
            .ok_or_else(|| anyhow!("that card no longer exists"))?;
        let artifact = card
            .artifacts
            .iter_mut()
            .find(|a| a.id == artifact_id)
            .ok_or_else(|| anyhow!("that artifact no longer exists"))?;
        artifact.last_status = last_status.map(str::to_string);
        artifact.checked_at = checked_at;
        self.save()
    }

    /// Every `ArtifactKind::Pr` artifact on the board, with its card.
    /// Done cards are skipped: a finished card costs no gh calls.
    pub fn pr_artifacts(&self) -> Vec<PrArtifact> {
        self.board
            .cards
            .iter()
            .filter(|c| !c.done)
            .flat_map(|c| {
                c.artifacts
                    .iter()
                    .filter(|a| a.kind == ArtifactKind::Pr)
                    .map(|a| PrArtifact {
                        card_id: c.id.clone(),
                        artifact_id: a.id.clone(),
                        target: a.target.clone(),
                    })
            })
            .collect()
    }

    pub fn remove_link(&mut self, link_id: &str) -> Result<()> {
        self.board.links.retain(|l| l.id != link_id);
        self.save()
    }

    /// Link a live pane to a card. A pane belongs to one card at a time, so an
    /// existing link for the same pane is moved.
    pub fn attach(&mut self, card_id: &str, pane: &PaneInfo, all_panes: &[PaneInfo]) -> Result<()> {
        self.card(card_id)?;
        self.board.links.retain(|l| {
            l.find_live(all_panes).map(|p| p.pane_id.as_str()) != Some(pane.pane_id.as_str())
        });
        let mut link = SessionLink {
            id: short_id(),
            card_id: card_id.to_string(),
            created_at: now_ms(),
            ..SessionLink::default()
        };
        link.absorb(pane);
        link.last_seen = Some(now_ms());
        self.add_link(link)
    }

    /// Refresh every link from a snapshot. Returns whether anything changed,
    /// and the links whose output should be saved now, with their pane ids.
    pub fn reconcile(&mut self, snapshot: &Snapshot) -> (bool, Vec<(String, String)>) {
        let now = now_ms();
        let mut changed = false;
        let mut capture = Vec::new();
        for link in &mut self.board.links {
            if let Some(pane) = link.find_live(&snapshot.panes) {
                // Save when the agent stops working, and once for a settled
                // session that has never been read (attached by hand, or the
                // app was not running when it finished).
                let was = link.last_status;
                let now_status = pane.agent_status;
                if settled(now_status) && (was != now_status || link.output_at.is_none()) {
                    capture.push((link.id.clone(), pane.pane_id.clone()));
                }
                if link.absorb(pane) {
                    changed = true;
                    link.last_seen = Some(now);
                } else if link
                    .last_seen
                    .is_none_or(|t| now.saturating_sub(t) > 60_000)
                {
                    link.last_seen = Some(now);
                    changed = true;
                }
            }
        }
        if changed {
            // A failed save is not fatal here: the next change retries.
            let _ = self.save();
        }
        (changed, capture)
    }

    /// Record why a session ended. A vanished link is an error, not a panic.
    pub fn end_session(&mut self, link_id: &str, reason: EndReason) -> Result<()> {
        let link = self
            .board
            .links
            .iter_mut()
            .find(|l| l.id == link_id)
            .ok_or_else(|| anyhow!("that session no longer exists"))?;
        link.end_reason = Some(reason);
        link.ended_at = Some(now_ms());
        self.save()
    }

    /// Link ids eligible for idle parking: not ended, not blocked, and last
    /// seen at least `idle_ms` before `now`.
    ///
    /// This is a pure store query over link fields — it does not read a Herdr
    /// snapshot. Liveness is checked by the caller (`app.rs`) at park time.
    pub fn park_candidates(&self, now: u64, idle_ms: u64) -> Vec<String> {
        self.board
            .links
            .iter()
            .filter(|l| l.end_reason.is_none())
            .filter(|l| l.last_status != AgentStatus::Blocked)
            // saturating: a wrapping add would make a far-future last_seen
            // look long overdue and park a session that just ran.
            .filter(|l| {
                l.last_seen
                    .is_some_and(|t| t.saturating_add(idle_ms) <= now)
            })
            .map(|l| l.id.clone())
            .collect()
    }

    /// Keep the end of a session's output on its link. Empty output leaves an
    /// earlier record in place. Returns whether the link still exists.
    pub fn record_output(&mut self, link_id: &str, text: &str) -> Result<bool> {
        let Some(link) = self.board.links.iter_mut().find(|l| l.id == link_id) else {
            return Ok(false);
        };
        let text = text.trim_end();
        if !text.is_empty() {
            link.output = Some(keep_end(text, OUTPUT_MAX_BYTES).to_string());
        }
        link.output_at = Some(now_ms());
        self.save()?;
        Ok(true)
    }
}

/// The last `max` bytes of `text`, cut at a line start where possible.
fn keep_end(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let tail = &text[start..];
    match tail.find('\n') {
        Some(i) if i + 1 < tail.len() => &tail[i + 1..],
        _ => tail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(pane_id: &str, terminal_id: &str) -> PaneInfo {
        PaneInfo {
            pane_id: pane_id.into(),
            terminal_id: terminal_id.into(),
            agent: Some("claude".into()),
            ..PaneInfo::default()
        }
    }

    #[test]
    fn a_board_written_before_the_lifecycle_fields_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("board.json");
        std::fs::write(
            &file,
            r#"{"cards":[{"id":"c1","title":"t","cwd":"/w","notes":"","done":false,"created_at":1,"updated_at":1}],
                "links":[{"id":"l1","card_id":"c1","created_at":2}]}"#,
        )
        .unwrap();
        let store = Store::load(file).unwrap();
        let link = &store.board.links[0];
        assert_eq!(link.resumed_from, None);
        assert_eq!(link.cwd, None);
        assert_eq!(link.end_reason, None);
        assert_eq!(link.ended_at, None);
        assert_eq!(link.cause_hint, None);
    }

    #[test]
    fn a_cause_hint_round_trips_and_an_unknown_value_falls_back() {
        assert_eq!(
            serde_json::from_str::<CauseHint>("\"usage_window\"").unwrap(),
            CauseHint::UsageWindow
        );
        // Deserialized into the field, an unseen variant does not fail the board load.
        let link: SessionLink =
            serde_json::from_str(r#"{"id":"l1","cause_hint":"provider_outage"}"#).unwrap();
        assert_eq!(link.cause_hint, Some(CauseHint::Unknown));
    }

    #[test]
    fn an_end_reason_round_trips_and_unknown_values_fall_back() {
        let cases = [
            ("\"captain\"", EndReason::Captain),
            ("\"idle_timeout\"", EndReason::IdleTimeout),
            ("\"interrupted\"", EndReason::Interrupted),
            ("\"provider_outage\"", EndReason::Unknown),
        ];
        for (json, expected) in cases {
            assert_eq!(serde_json::from_str::<EndReason>(json).unwrap(), expected);
        }
        assert_eq!(
            serde_json::to_string(&EndReason::IdleTimeout).unwrap(),
            "\"idle_timeout\""
        );
    }

    #[test]
    fn an_artifact_survives_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("board.json");
        let mut store = Store::load(file.clone()).unwrap();
        let card = store.create_card("pr work", "/w", "").unwrap();
        store
            .add_artifact(
                &card.id,
                Artifact {
                    id: "a1".into(),
                    kind: ArtifactKind::Pr,
                    label: "PR #25".into(),
                    target: "https://github.com/jander99/starfleet/pull/25".into(),
                    last_status: None,
                    checked_at: None,
                },
            )
            .unwrap();
        store
            .update_artifact(&card.id, "a1", Some("ci_running"), Some(42))
            .unwrap();
        let reloaded = Store::load(file).unwrap();
        let a = &reloaded.card(&card.id).unwrap().artifacts[0];
        assert_eq!(a.last_status.as_deref(), Some("ci_running"));
        assert_eq!(a.checked_at, Some(42));
    }

    #[test]
    fn updating_an_unknown_artifact_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("board.json")).unwrap();
        let card = store.create_card("c", "/w", "").unwrap();
        assert!(store
            .update_artifact(&card.id, "nope", Some("x"), None)
            .is_err());
    }

    #[test]
    fn the_resume_chain_stops_at_a_dangling_id() {
        let links = vec![
            SessionLink {
                id: "a".into(),
                resumed_from: None,
                ..Default::default()
            },
            SessionLink {
                id: "b".into(),
                resumed_from: Some("a".into()),
                ..Default::default()
            },
            SessionLink {
                id: "c".into(),
                resumed_from: Some("missing".into()),
                ..Default::default()
            },
        ];
        assert_eq!(
            resume_chain(&links, "b"),
            vec!["b".to_string(), "a".to_string()]
        );
        assert_eq!(resume_chain(&links, "c"), vec!["c".to_string()]);
        assert_eq!(resume_chain(&links, "nope"), Vec::<String>::new());
    }

    #[test]
    fn a_cyclic_resume_chain_does_not_loop() {
        let links = vec![
            SessionLink {
                id: "a".into(),
                resumed_from: Some("b".into()),
                ..Default::default()
            },
            SessionLink {
                id: "b".into(),
                resumed_from: Some("a".into()),
                ..Default::default()
            },
        ];
        let chain = resume_chain(&links, "a");
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn parking_skips_blocked_and_recent_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("board.json")).unwrap();
        for (id, seen, status) in [
            ("old", 0u64, AgentStatus::Idle),
            // 1_000_000: seen after now - idle_ms, so not yet parkable.
            ("fresh", 1_000_000, AgentStatus::Idle),
            ("waiting", 0, AgentStatus::Blocked),
            ("closed", 0, AgentStatus::Done),
        ] {
            let mut link = SessionLink {
                id: id.into(),
                last_seen: Some(seen),
                last_status: status,
                ..Default::default()
            };
            if id == "closed" {
                link.end_reason = Some(EndReason::Captain);
                link.ended_at = Some(1);
            }
            store.board.links.push(link);
        }
        // now is past old's idle deadline (0 + 3_600_000) but not fresh's (1_000_000 + 3_600_000).
        assert_eq!(
            store.park_candidates(4_000_000, 3_600_000),
            vec!["old".to_string()]
        );
    }

    #[test]
    fn parking_arithmetic_does_not_overflow() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("board.json")).unwrap();
        store.board.links.push(SessionLink {
            id: "far".into(),
            last_seen: Some(u64::MAX - 5),
            last_status: AgentStatus::Idle,
            ..Default::default()
        });
        // A wrapping add would make a far-future last_seen look long overdue.
        assert!(store.park_candidates(1_000, 10).is_empty());
    }

    #[test]
    fn a_park_candidate_at_exactly_the_deadline_is_parked() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("board.json")).unwrap();
        store.board.links.push(SessionLink {
            id: "edge".into(),
            last_seen: Some(1_000),
            last_status: AgentStatus::Idle,
            ..Default::default()
        });
        // The documented rule is `t + idle_ms <= now`: exactly at the deadline counts.
        assert_eq!(
            store.park_candidates(4_000, 3_000),
            vec!["edge".to_string()]
        );
        assert!(store.park_candidates(3_999, 3_000).is_empty());
    }

    #[test]
    fn ending_a_session_stamps_reason_and_time_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("board.json");
        let mut store = Store::load(file.clone()).unwrap();
        store.board.links.push(SessionLink {
            id: "l1".into(),
            ..Default::default()
        });
        store.end_session("l1", EndReason::IdleTimeout).unwrap();
        let reloaded = Store::load(file).unwrap();
        let link = &reloaded.board.links[0];
        assert_eq!(link.end_reason, Some(EndReason::IdleTimeout));
        assert!(link.ended_at.is_some());
        let mut again = reloaded;
        assert!(again.end_session("missing", EndReason::Captain).is_err());
    }

    #[test]
    fn a_link_without_terminal_or_agent_session_falls_back_to_its_pane_id() {
        let link = SessionLink {
            pane_id: Some("w1:p1".into()),
            ..SessionLink::default()
        };
        assert_eq!(link.find_live(&[pane("w1:p1", "term_1")]).unwrap().pane_id, "w1:p1");
        assert!(link.find_live(&[pane("w2:p2", "term_1")]).is_none());
        // With no pane id either, nothing matches.
        let bare = SessionLink::default();
        assert!(bare.find_live(&[pane("w1:p1", "term_1")]).is_none());
    }

    #[test]
    fn a_link_finds_its_pane_by_agent_session_when_the_terminal_moved() {
        let link = SessionLink {
            agent_session: Some(AgentSessionInfo { source: String::new(), agent: "claude".into(), kind: String::new(), value: "s-9".into() }),
            ..SessionLink::default()
        };
        let mut p = pane("w3:p7", "term_new");
        p.agent_session = Some(AgentSessionInfo { source: String::new(), agent: "claude".into(), kind: String::new(), value: "s-9".into() });
        assert_eq!(link.find_live(&[p]).unwrap().pane_id, "w3:p7");
        // An empty terminal id is ignored rather than matched against another
        // pane that happens to have an empty one.
        let link = SessionLink { terminal_id: Some(String::new()), ..link };
        assert!(link.find_live(&[pane("w1:p1", "")]).is_none());
    }

    #[test]
    fn a_reused_pane_id_does_not_steal_a_link() {
        let link = SessionLink {
            terminal_id: Some("term_1".into()),
            pane_id: Some("w1:p1".into()),
            ..SessionLink::default()
        };
        // Same pane id, different terminal: not our session.
        assert!(link.find_live(&[pane("w1:p1", "term_9")]).is_none());
        // The terminal moved to another pane id: still our session.
        assert_eq!(
            link.find_live(&[pane("w2:p3", "term_1")]).unwrap().pane_id,
            "w2:p3"
        );
    }

    fn store_with(link: SessionLink) -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("board.json")).unwrap();
        store.board.links.push(link);
        (dir, store)
    }

    fn snapshot_with(status: AgentStatus) -> Snapshot {
        let mut p = pane("w1:p1", "term_1");
        p.agent_status = status;
        Snapshot {
            panes: vec![p],
            ..Snapshot::default()
        }
    }

    #[test]
    fn output_is_saved_when_the_agent_stops_working() {
        let link = SessionLink {
            id: "l1".into(),
            terminal_id: Some("term_1".into()),
            last_status: AgentStatus::Working,
            output_at: Some(1),
            ..SessionLink::default()
        };
        let (_dir, mut store) = store_with(link);

        // Still working: nothing to save.
        let (_, capture) = store.reconcile(&snapshot_with(AgentStatus::Working));
        assert!(capture.is_empty());

        // Finished: save it.
        let (_, capture) = store.reconcile(&snapshot_with(AgentStatus::Done));
        assert_eq!(capture, vec![("l1".to_string(), "w1:p1".to_string())]);
        assert!(store.record_output("l1", "all done\n\n").unwrap());

        // Still done, already saved: not again.
        let (_, capture) = store.reconcile(&snapshot_with(AgentStatus::Done));
        assert!(capture.is_empty());

        // Looked at (done becomes idle): saved again, since the user may
        // have typed into the pane.
        let (_, capture) = store.reconcile(&snapshot_with(AgentStatus::Idle));
        assert_eq!(capture.len(), 1);

        // An empty read keeps the earlier record.
        store.record_output("l1", "  \n").unwrap();
        assert_eq!(store.board.links[0].output.as_deref(), Some("all done"));
        assert!(!store.record_output("gone", "x").unwrap());
    }

    #[test]
    fn a_settled_session_that_was_never_read_is_saved_once() {
        let link = SessionLink {
            id: "l1".into(),
            terminal_id: Some("term_1".into()),
            last_status: AgentStatus::Done,
            ..SessionLink::default()
        };
        let (_dir, mut store) = store_with(link);
        let (_, capture) = store.reconcile(&snapshot_with(AgentStatus::Done));
        assert_eq!(capture.len(), 1);
        store.record_output("l1", "").unwrap();
        let (_, capture) = store.reconcile(&snapshot_with(AgentStatus::Done));
        assert!(capture.is_empty());
    }

    #[test]
    fn saved_output_keeps_the_end_at_a_line_start() {
        assert_eq!(keep_end("short", 100), "short");
        assert_eq!(keep_end("one\ntwo\nthree", 9), "three");
        // Never splits a multi-byte character.
        assert_eq!(keep_end("ééé", 3), "é");
    }

    #[test]
    fn agent_session_survives_new_terminal_ids() {
        let session = AgentSessionInfo {
            source: "hook".into(),
            agent: "claude".into(),
            kind: "id".into(),
            value: "abc".into(),
        };
        let link = SessionLink {
            terminal_id: Some("term_1".into()),
            agent_session: Some(session.clone()),
            ..SessionLink::default()
        };
        let mut restored = pane("w1:p1", "term_50");
        restored.agent_session = Some(session);
        assert!(link.find_live(&[restored]).is_some());
    }

    #[test]
    fn pr_artifacts_lists_every_pr_with_its_card() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("board.json")).unwrap();
        let a = store.create_card("a", "/w", "").unwrap();
        let b = store.create_card("b", "/w", "").unwrap();
        let art = |id: &str, kind: ArtifactKind, target: &str| Artifact {
            id: id.into(),
            kind,
            label: "x".into(),
            target: target.into(),
            last_status: None,
            checked_at: None,
        };
        let url = "https://github.com/o/r/pull/7";
        store
            .add_artifact(&a.id, art("p1", ArtifactKind::Pr, url))
            .unwrap();
        store
            .add_artifact(
                &a.id,
                art("u1", ArtifactKind::Url, "https://example.test/x"),
            )
            .unwrap();
        store
            .add_artifact(&b.id, art("p2", ArtifactKind::Pr, url))
            .unwrap();
        let prs = store.pr_artifacts();
        assert_eq!(prs.len(), 2);
        assert_eq!(prs.iter().filter(|p| p.target == url).count(), 2);
        assert!(prs
            .iter()
            .any(|p| p.card_id == b.id && p.artifact_id == "p2"));

        // A done card costs no gh calls.
        store
            .update_card(&b.id, None, None, None, Some(true))
            .unwrap();
        assert_eq!(store.pr_artifacts().len(), 1);
    }
}
