//! What the UI renders: the board joined with the live Herdr snapshot.

use crate::herdr::{AgentStatus, PaneInfo, Snapshot};
use crate::stations::Roster;
use crate::store::{pane_title, Board, Card, SessionLink};
use serde::Serialize;
use std::collections::HashSet;

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
}

#[derive(Clone, Debug, Serialize)]
pub struct CardView {
    #[serde(flatten)]
    pub card: Card,
    pub sessions: Vec<SessionView>,
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
            cwd: None,
            created_at: link.created_at,
            last_seen: link.last_seen,
            output_at,
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
