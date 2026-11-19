//! What the UI renders: the board joined with the live Herdr snapshot.

use crate::herdr::{AgentStatus, PaneInfo, Snapshot};
use crate::project::{project_kind, ProjectKind};
use crate::stations::Roster;
use crate::store::{pane_title, Board, Card, EndReason, SessionLink};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Waiting on the user (a permission prompt or a question).
    Blocked,
    /// Finished, and nobody has looked at the pane yet.
    Done,
    Working,
    Idle,
    Unknown,
    /// The pane no longer exists in Herdr.
    Offline,
}

impl From<AgentStatus> for SessionStatus {
    fn from(s: AgentStatus) -> Self {
        match s {
            AgentStatus::Blocked => SessionStatus::Blocked,
            AgentStatus::Done => SessionStatus::Done,
            AgentStatus::Working => SessionStatus::Working,
            AgentStatus::Idle => SessionStatus::Idle,
            AgentStatus::Unknown => SessionStatus::Unknown,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SessionView {
    pub link_id: String,
    pub status: SessionStatus,
    /// Status when last seen; only differs from `status` when offline.
    pub last_status: AgentStatus,
    pub live: bool,
    pub pane_id: Option<String>,
    pub agent: Option<String>,
    pub title: Option<String>,
    pub station_id: Option<String>,
    pub station_name: Option<String>,
    pub prompt: Option<String>,
    pub pending_prompt: Option<String>,
    pub workspace: Option<String>,
    pub cwd: Option<String>,
    pub created_at: u64,
    pub last_seen: Option<u64>,
    /// When output was last saved for this session; `None` if there is none.
    /// The text itself is fetched separately, to keep the view small.
    pub output_at: Option<u64>,
    /// Why the session ended, if Starfleet stamped it. History: the card's
    /// state comes from the newest session.
    pub end_reason: Option<EndReason>,
    /// The session this one resumed, if any. Mirror of `SessionLink`.
    pub resumed_from: Option<String>,
}

impl SessionView {
    pub fn state(&self) -> SessionState {
        SessionState {
            id: self.link_id.clone(),
            created_at: self.created_at,
            live: self.live,
            status: self.status,
            end_reason: self.end_reason,
        }
    }
}

/// The few facts card state needs from a session, so the rule is testable
/// without a snapshot or a roster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionState {
    pub id: String,
    pub created_at: u64,
    pub live: bool,
    pub status: SessionStatus,
    pub end_reason: Option<EndReason>,
}

/// A card's state, computed from its sessions and never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardState {
    /// No sessions at all.
    Empty,
    /// A live session is waiting on the user.
    Blocked,
    Working,
    Idle,
    Done,
    /// Closed by the Captain or by idle timeout.
    Paused,
    /// Gone without a stamp.
    Interrupted,
    Offline,
}

/// The newest session decides, except that blocked aggregates: a permission
/// prompt on an older session is never hidden by a newer working one.
/// Newest is max by `(created_at, live, id)` — the id tie-break makes it
/// order-independent, and a live session beats a closed one when two
/// dispatches share a millisecond (or the clock stepped back).
pub fn card_state(sessions: &[SessionState]) -> CardState {
    let Some(newest) = sessions
        .iter()
        .max_by_key(|s| (s.created_at, s.live, s.id.as_str()))
    else {
        return CardState::Empty;
    };
    if sessions
        .iter()
        .any(|s| s.live && s.status == SessionStatus::Blocked)
    {
        return CardState::Blocked;
    }
    if newest.live {
        return match newest.status {
            SessionStatus::Blocked => CardState::Blocked,
            SessionStatus::Working => CardState::Working,
            SessionStatus::Done => CardState::Done,
            SessionStatus::Idle | SessionStatus::Unknown => CardState::Idle,
            SessionStatus::Offline => CardState::Offline,
        };
    }
    match newest.end_reason {
        Some(EndReason::Captain | EndReason::IdleTimeout) => CardState::Paused,
        None | Some(EndReason::Interrupted | EndReason::Unknown) => CardState::Interrupted,
    }
}

/// Sort key for the attention board: lower is more urgent. Blocked first,
/// then stale/interrupted, then needs a decision, then working.
pub fn attention_rank(state: CardState) -> u8 {
    match state {
        CardState::Blocked => 0,
        CardState::Interrupted => 1,
        CardState::Done => 2,
        CardState::Paused => 3,
        CardState::Working => 4,
        CardState::Idle => 5,
        CardState::Offline => 6,
        CardState::Empty => 7,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CardView {
    #[serde(flatten)]
    pub card: Card,
    pub sessions: Vec<SessionView>,
    /// Derived from `sessions`; never stored.
    pub state: CardState,
    pub kind: ProjectKind,
}

/// A live agent that is not linked to any card.
#[derive(Clone, Debug, Serialize)]
pub struct ContactView {
    pub pane_id: String,
    pub status: SessionStatus,
    pub agent: Option<String>,
    pub title: Option<String>,
    pub workspace: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Counts {
    pub blocked: usize,
    pub done: usize,
    pub working: usize,
    pub idle: usize,
    pub offline: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct BoardView {
    pub connected: bool,
    pub error: Option<String>,
    pub herdr_version: Option<String>,
    pub endpoint: String,
    pub cards: Vec<CardView>,
    pub unassigned: Vec<ContactView>,
    /// Counts over every live agent plus offline linked sessions on open cards.
    pub counts: Counts,
}

fn agent_label(pane: &PaneInfo) -> Option<String> {
    pane.display_agent.clone().or_else(|| pane.agent.clone())
}

fn session_view(link: &SessionLink, snapshot: Option<&Snapshot>, roster: &Roster) -> SessionView {
    let live = snapshot.and_then(|s| link.find_live(&s.panes).map(|p| (s, p)));
    let station_name = link
        .station_id
        .as_deref()
        .and_then(|id| roster.station(id))
        .map(|s| s.name.clone());
    let output_at = link.output.as_ref().and(link.output_at);
    match live {
        Some((snap, pane)) => SessionView {
            link_id: link.id.clone(),
            status: pane.agent_status.into(),
            last_status: pane.agent_status,
            live: true,
            pane_id: Some(pane.pane_id.clone()),
            agent: agent_label(pane).or_else(|| link.agent.clone()),
            title: pane_title(pane).or_else(|| link.title.clone()),
            station_id: link.station_id.clone(),
            station_name,
            prompt: link.prompt.clone(),
            pending_prompt: link.pending_prompt.clone(),
            workspace: snap.workspace(&pane.workspace_id).map(|w| w.label.clone()),
            cwd: pane.foreground_cwd.clone().or_else(|| pane.cwd.clone()),
            created_at: link.created_at,
            last_seen: link.last_seen,
            output_at,
            end_reason: link.end_reason,
            resumed_from: link.resumed_from.clone(),
        },
        None => SessionView {
            link_id: link.id.clone(),
            status: SessionStatus::Offline,
            last_status: link.last_status,
            live: false,
            pane_id: None,
            agent: link.agent.clone(),
            title: link.title.clone(),
            station_id: link.station_id.clone(),
            station_name,
            prompt: link.prompt.clone(),
            pending_prompt: link.pending_prompt.clone(),
            workspace: None,
            // Offline: no pane, so fall back to the directory the session
            // was aimed at.
            cwd: link.cwd.clone(),
            created_at: link.created_at,
            last_seen: link.last_seen,
            output_at,
            end_reason: link.end_reason,
            resumed_from: link.resumed_from.clone(),
        },
    }
}

fn count(counts: &mut Counts, status: SessionStatus) {
    match status {
        SessionStatus::Blocked => counts.blocked += 1,
        SessionStatus::Done => counts.done += 1,
        SessionStatus::Working => counts.working += 1,
        SessionStatus::Idle | SessionStatus::Unknown => counts.idle += 1,
        SessionStatus::Offline => counts.offline += 1,
    }
}

pub fn build(
    board: &Board,
    snapshot: Option<&Snapshot>,
    roster: &Roster,
    error: Option<String>,
    endpoint: String,
) -> BoardView {
    let mut counts = Counts::default();
    let mut linked_panes: HashSet<String> = HashSet::new();
    // `project_kind` hits the filesystem (read_dir on the cwd and its parent),
    // so classify each distinct cwd once per view build, not per card.
    let mut kinds: HashMap<String, ProjectKind> = HashMap::new();

    let mut cards: Vec<CardView> = board
        .cards
        .iter()
        .map(|card| {
            let mut sessions: Vec<SessionView> = board
                .links
                .iter()
                .filter(|l| l.card_id == card.id)
                .map(|l| session_view(l, snapshot, roster))
                .collect();
            sessions.sort_by_key(|s| std::cmp::Reverse(s.created_at));
            for s in &sessions {
                if let Some(p) = &s.pane_id {
                    linked_panes.insert(p.clone());
                }
                if s.live || !card.done {
                    count(&mut counts, s.status);
                }
            }
            CardView {
                card: card.clone(),
                state: card_state(&sessions.iter().map(SessionView::state).collect::<Vec<_>>()),
                kind: *kinds
                    .entry(card.cwd.clone())
                    .or_insert_with(|| project_kind(std::path::Path::new(&card.cwd))),
                sessions,
            }
        })
        .collect();
    // Open cards first, most recently touched first.
    cards.sort_by_key(|c| (c.card.done, std::cmp::Reverse(c.card.updated_at)));

    let unassigned: Vec<ContactView> = snapshot
        .map(|snap| {
            snap.agent_panes()
                .into_iter()
                .filter(|p| !linked_panes.contains(&p.pane_id))
                .map(|p| ContactView {
                    status: p.agent_status.into(),
                    agent: agent_label(&p),
                    title: pane_title(&p),
                    workspace: snap.workspace(&p.workspace_id).map(|w| w.label.clone()),
                    cwd: p.foreground_cwd.clone().or_else(|| p.cwd.clone()),
                    pane_id: p.pane_id,
                })
                .collect()
        })
        .unwrap_or_default();
    for c in &unassigned {
        count(&mut counts, c.status);
    }

    BoardView {
        connected: snapshot.is_some(),
        error,
        herdr_version: snapshot
            .map(|s| s.version.clone())
            .filter(|v| !v.is_empty()),
        endpoint,
        cards,
        unassigned,
        counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ss(
        id: &str,
        at: u64,
        live: bool,
        status: SessionStatus,
        reason: Option<EndReason>,
    ) -> SessionState {
        SessionState {
            id: id.into(),
            created_at: at,
            live,
            status,
            end_reason: reason,
        }
    }

    #[test]
    fn the_newest_session_decides_the_card_state() {
        let older = ss(
            "l2",
            10,
            false,
            SessionStatus::Offline,
            Some(EndReason::Interrupted),
        );
        let newer = ss("l1", 20, true, SessionStatus::Working, None);
        assert_eq!(card_state(&[older, newer]), CardState::Working);
    }

    #[test]
    fn equal_timestamps_are_broken_by_id() {
        let a = ss("a", 10, true, SessionStatus::Idle, None);
        let b = ss("b", 10, true, SessionStatus::Working, None);
        assert_eq!(card_state(&[a.clone(), b.clone()]), CardState::Working);
        assert_eq!(card_state(&[b, a]), CardState::Working); // order-independent
    }

    #[test]
    fn a_live_session_wins_a_created_at_tie() {
        // Same millisecond dispatch (or a clock that stepped back): the resumed,
        // live session must not be hidden by the older closed one, whatever ids sort.
        let closed = ss("z", 10, false, SessionStatus::Offline, None);
        let resumed = ss("a", 10, true, SessionStatus::Working, None);
        assert_eq!(card_state(&[closed, resumed]), CardState::Working);
    }

    #[test]
    fn a_blocked_session_hidden_behind_a_newer_one_still_surfaces() {
        let waiting = ss("l1", 10, true, SessionStatus::Blocked, None);
        let newer = ss("l2", 20, true, SessionStatus::Working, None);
        assert_eq!(card_state(&[waiting, newer]), CardState::Blocked);
    }

    #[test]
    fn a_closed_session_is_paused_or_interrupted_by_its_stamp() {
        let paused = ss(
            "l",
            1,
            false,
            SessionStatus::Offline,
            Some(EndReason::Captain),
        );
        let died = ss("l", 1, false, SessionStatus::Offline, None);
        assert_eq!(card_state(&[paused]), CardState::Paused);
        assert_eq!(card_state(&[died]), CardState::Interrupted);
        assert_eq!(card_state(&[]), CardState::Empty);
    }

    #[test]
    fn every_agent_status_maps_to_a_session_status() {
        use SessionStatus as S;
        for (from, want) in [
            (AgentStatus::Blocked, S::Blocked),
            (AgentStatus::Done, S::Done),
            (AgentStatus::Working, S::Working),
            (AgentStatus::Idle, S::Idle),
            (AgentStatus::Unknown, S::Unknown),
        ] {
            assert_eq!(S::from(from), want);
        }
    }

    #[test]
    fn the_attention_order_puts_blocked_first_then_interrupted() {
        assert!(attention_rank(CardState::Blocked) < attention_rank(CardState::Interrupted));
        assert!(attention_rank(CardState::Interrupted) < attention_rank(CardState::Done));
        assert!(attention_rank(CardState::Done) < attention_rank(CardState::Working));
    }
}
