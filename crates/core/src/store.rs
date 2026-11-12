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
}
