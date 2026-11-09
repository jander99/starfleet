//! The parts of Herdr's `session.snapshot` that Starfleet reads.
//!
//! Field names follow the Herdr API schema (protocol 22). Everything optional
//! is defaulted so that newer or older servers still parse.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Done,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionInfo {
    #[serde(default)]
    pub source: String,
    pub agent: String,
    /// "id" or "path".
    #[serde(default)]
    pub kind: String,
    pub value: String,
}

/// A pane. `AgentInfo` in the schema carries the same fields plus `name`, so
/// one struct serves both lists.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PaneInfo {
    pub pane_id: String,
    pub terminal_id: String,
    pub workspace_id: String,
    pub tab_id: String,
    pub focused: bool,
    pub agent_status: AgentStatus,
    pub agent: Option<String>,
    pub display_agent: Option<String>,
    pub name: Option<String>,
    pub label: Option<String>,
    pub title: Option<String>,
    pub terminal_title_stripped: Option<String>,
    pub cwd: Option<String>,
    pub foreground_cwd: Option<String>,
    pub agent_session: Option<AgentSessionInfo>,
    pub tokens: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceInfo {
    pub workspace_id: String,
    pub number: u64,
    pub label: String,
    pub focused: bool,
    pub agent_status: AgentStatus,
    pub tokens: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TabInfo {
    pub tab_id: String,
    pub workspace_id: String,
    pub number: u64,
    pub label: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub version: String,
    pub protocol: u32,
    pub workspaces: Vec<WorkspaceInfo>,
    pub tabs: Vec<TabInfo>,
    pub panes: Vec<PaneInfo>,
    pub agents: Vec<PaneInfo>,
}

impl Snapshot {
    pub fn workspace(&self, id: &str) -> Option<&WorkspaceInfo> {
        self.workspaces.iter().find(|w| w.workspace_id == id)
    }

    /// Every pane that hosts an agent, with agent-only fields (`name`) merged in.
    pub fn agent_panes(&self) -> Vec<PaneInfo> {
        let mut out: Vec<PaneInfo> = self.agents.clone();
        for pane in &self.panes {
            if pane.agent.is_some() && !out.iter().any(|a| a.pane_id == pane.pane_id) {
                out.push(pane.clone());
            }
        }
        out
    }
}
