//! The application facade: one object the UI shell calls into.

use crate::env::{sh_quote, HostEnv};
use crate::herdr::{Endpoint, HerdrClient, HerdrError, PaneInfo, Snapshot};
use crate::paths::{Paths, Settings};
use crate::preflight::{self, Check};
use crate::stations::Roster;
use crate::store::{now_ms, short_id, SessionLink, Store};
use crate::view::{self, BoardView};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::watch;

/// Workspace token that marks a Herdr workspace as belonging to a card.
const CARD_TOKEN: &str = "starfleet_card";

/// How many lines of output are saved when an agent stops working. Matches
/// what the session drawer shows for a live session.
const OUTPUT_LINES: u32 = 200;

pub struct Options {
    pub paths: Paths,
    /// Override where Herdr is reached; resolved from settings when `None`.
    pub endpoint: Option<Endpoint>,
    /// Override the host environment; detected when `None`.
    pub env: Option<HostEnv>,
}

#[derive(Default)]
struct Live {
    snapshot: Option<Snapshot>,
    error: Option<String>,
}

pub struct Starfleet {
    env: HostEnv,
    client: HerdrClient,
    roster: Roster,
    store: Mutex<Store>,
    live: RwLock<Live>,
    changed: watch::Sender<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DispatchRequest {
    pub card_id: String,
    pub station_id: String,
    pub prompt: String,
    /// Override the station's agent CLI for this dispatch.
    #[serde(default)]
    pub agent: Option<String>,
    /// Override the station's model for this dispatch.
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DispatchResult {
    pub link_id: String,
    pub pane_id: String,
    /// Set when the session started but the prompt could not be delivered.
    pub warning: Option<String>,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Herdr will not take a prompt while the agent is waiting on the user. A
/// freshly started agent sitting at a startup dialog (such as Claude Code's
/// folder trust prompt) is reported as `agent_not_ready`, not `agent_blocked`.
fn awaiting_input(e: &HerdrError) -> bool {
    matches!(e.code(), Some("agent_blocked" | "agent_not_ready"))
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value, field: &str) -> Result<T> {
    let inner = value
        .get(field)
        .ok_or_else(|| anyhow!("Herdr's reply has no `{field}`"))?;
    serde_json::from_value(inner.clone())
        .with_context(|| format!("unexpected `{field}` in Herdr's reply"))
}

impl Starfleet {
    pub fn new(options: Options) -> Result<Arc<Self>> {
        let paths = options.paths;
        let settings = Settings::load(&paths)?;
        let env = options
            .env
            .unwrap_or_else(|| HostEnv::detect(settings.wsl_distro.clone()));
        let endpoint = options
            .endpoint
            .unwrap_or_else(|| Endpoint::resolve(&settings, &env));
        let (changed, _) = watch::channel(0);
        Ok(Arc::new(Self {
            client: HerdrClient::new(endpoint),
            roster: Roster::load_or_seed(&paths)?,
            store: Mutex::new(Store::load(paths.board_file())?),
            live: RwLock::new(Live::default()),
            env,
            changed,
        }))
    }

    /// Fires whenever the board view may have changed.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changed.subscribe()
    }

    fn notify(&self) {
        self.changed.send_modify(|n| *n += 1);
    }

    pub fn roster(&self) -> &Roster {
        &self.roster
    }

    pub fn view(&self) -> BoardView {
        let live = self.live.read().unwrap();
        let store = self.store.lock().unwrap();
        view::build(
            &store.board,
            live.snapshot.as_ref(),
            &self.roster,
            live.error.clone(),
            self.client.endpoint().describe(),
        )
    }

    pub async fn preflight(&self) -> Vec<Check> {
        preflight::run(&self.env, &self.client).await
    }

    // ---- Herdr state -----------------------------------------------------

    /// Fetch a fresh snapshot and fold it into the board.
    pub async fn refresh(&self) -> Result<Snapshot, HerdrError> {
        let result = self
            .client
            .call("session.snapshot", json!({}))
            .await
            .and_then(|r| {
                serde_json::from_value::<Snapshot>(
                    r.get("snapshot").cloned().unwrap_or(Value::Null),
                )
                .map_err(|e| HerdrError::Protocol(format!("unreadable snapshot: {e}")))
            });
        match result {
            Ok(snapshot) => {
                let (_, capture) = self.store.lock().unwrap().reconcile(&snapshot);
                {
                    let mut live = self.live.write().unwrap();
                    live.snapshot = Some(snapshot.clone());
                    live.error = None;
                }
                self.notify();
                self.save_output(capture).await;
                Ok(snapshot)
            }
            Err(e) => {
                let mut live = self.live.write().unwrap();
                live.snapshot = None;
                live.error = Some(e.to_string());
                drop(live);
                self.notify();
                Err(e)
            }
        }
    }

    /// Read the end of each session's output and keep it on its link, so the
    /// card still shows what the agent said after the pane is closed. A failed
    /// read is retried on a later refresh.
    async fn save_output(&self, capture: Vec<(String, String)>) {
        if capture.is_empty() {
            return;
        }
        for (link_id, pane_id) in capture {
            let Ok(text) = self.tail(&pane_id, OUTPUT_LINES).await else {
                continue;
            };
            let _ = self.store.lock().unwrap().record_output(&link_id, &text);
        }
        self.notify();
    }

    /// The output saved for a session when its agent last stopped working.
    pub fn saved_output(&self, link_id: &str) -> Result<Option<String>> {
        let store = self.store.lock().unwrap();
        let link = store
            .board
            .links
            .iter()
            .find(|l| l.id == link_id)
            .ok_or_else(|| anyhow!("that session is no longer on the board"))?;
        Ok(link.output.clone())
    }

    /// Keep the snapshot current for as long as the app runs.
    pub async fn run_monitor(self: Arc<Self>) {
        let mut backoff = Duration::from_secs(1);
        loop {
            let started = tokio::time::Instant::now();
            // Errors are already recorded in `live` by `refresh`.
            let _ = self.watch_once().await;
            if started.elapsed() > Duration::from_secs(30) {
                backoff = Duration::from_secs(1);
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(10));
        }
    }

    async fn watch_once(&self) -> Result<(), HerdrError> {
        let pane_ids = |s: &Snapshot| {
            s.panes
                .iter()
                .map(|p| p.pane_id.clone())
                .collect::<BTreeSet<_>>()
        };
        let mut known = pane_ids(&self.refresh().await?);
        let mut stream = self.client.subscribe(subscriptions(&known)).await?;
        let mut reconcile = tokio::time::interval(Duration::from_secs(10));
        reconcile.tick().await;
        // Throttle: one snapshot at most every 300 ms, however many events arrive.
        let mut due: Option<tokio::time::Instant> = None;
        loop {
            let wait = async {
                match due {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                event = stream.next() => {
                    if let Err(e) = event {
                        self.live.write().unwrap().error = Some(e.to_string());
                        return Err(e);
                    }
                    due.get_or_insert(tokio::time::Instant::now() + Duration::from_millis(300));
                }
                _ = wait => {
                    due = None;
                    let now = pane_ids(&self.refresh().await?);
                    if now != known {
                        // Status events are per pane, so a changed pane set
                        // needs a new subscription.
                        known = now;
                        stream = self.client.subscribe(subscriptions(&known)).await?;
                    }
                }
                // Events are not durable; this catches anything that was missed.
                _ = reconcile.tick() => {
                    due.get_or_insert(tokio::time::Instant::now());
                }
            }
        }
    }

    fn live_pane(&self, pane_id: &str) -> Result<PaneInfo> {
        self.live
            .read()
            .unwrap()
            .snapshot
            .as_ref()
            .and_then(|s| s.panes.iter().find(|p| p.pane_id == pane_id).cloned())
            .ok_or_else(|| anyhow!("that session is no longer running in Herdr"))
    }

    // ---- Cards -----------------------------------------------------------

    /// Turn what the user typed into an absolute directory in the agents'
    /// environment, or explain why it is not usable.
    pub async fn resolve_dir(&self, input: &str) -> Result<String> {
        let input = input.trim();
        if input.is_empty() {
            bail!("enter the folder that sessions for this card should start in");
        }
        let expr = match input.strip_prefix("~/") {
            Some(rest) => format!("\"$HOME\"/{}", sh_quote(rest)),
            None if input == "~" => "\"$HOME\"".to_string(),
            None => sh_quote(input),
        };
        let out = self
            .env
            .sh(&format!("cd -- {expr} 2>/dev/null && pwd"))
            .await?;
        if !out.ok || out.stdout.is_empty() {
            bail!("`{input}` is not a folder in {}", self.env.label());
        }
        Ok(out.stdout)
    }

    pub async fn create_card(&self, title: &str, cwd: &str, notes: &str) -> Result<String> {
        let cwd = self.resolve_dir(cwd).await?;
        let card = self.store.lock().unwrap().create_card(title, &cwd, notes)?;
        self.notify();
        Ok(card.id)
    }

    pub async fn update_card(
        &self,
        id: &str,
        title: Option<String>,
        cwd: Option<String>,
        notes: Option<String>,
        done: Option<bool>,
    ) -> Result<()> {
        let cwd = match cwd {
            Some(c) => Some(self.resolve_dir(&c).await?),
            None => None,
        };
        self.store
            .lock()
            .unwrap()
            .update_card(id, title, cwd, notes, done)?;
        self.notify();
        Ok(())
    }

    pub fn delete_card(&self, id: &str) -> Result<()> {
        self.store.lock().unwrap().delete_card(id)?;
        self.notify();
        Ok(())
    }

    /// Link a live, unassigned (or differently assigned) session to a card.
    pub fn attach(&self, card_id: &str, pane_id: &str) -> Result<()> {
        let pane = self.live_pane(pane_id)?;
        let panes = self
            .live
            .read()
            .unwrap()
            .snapshot
            .as_ref()
            .map(|s| s.panes.clone())
            .unwrap_or_default();
        self.store.lock().unwrap().attach(card_id, &pane, &panes)?;
        self.notify();
        Ok(())
    }

    /// Remove a link. The Herdr session keeps running and becomes unassigned.
    pub fn detach(&self, link_id: &str) -> Result<()> {
        self.store.lock().unwrap().remove_link(link_id)?;
        self.notify();
        Ok(())
    }

    // ---- Dispatch --------------------------------------------------------

    pub async fn dispatch(&self, req: DispatchRequest) -> Result<DispatchResult> {
        let prompt = req.prompt.trim();
        if prompt.is_empty() {
            bail!("write a prompt for the agent first");
        }
        let station = self
            .roster
            .station(&req.station_id)
            .ok_or_else(|| anyhow!("unknown station `{}`", req.station_id))?
            .clone();
        let card = self.store.lock().unwrap().card(&req.card_id)?.clone();
        if card.cwd.is_empty() {
            bail!("this card has no working directory yet");
        }
        let kind = non_empty(req.agent).unwrap_or_else(|| station.agent.clone());
        let mut args = station.args.clone();
        if let Some(model) = non_empty(req.model).or_else(|| non_empty(station.model.clone())) {
            args.push("--model".into());
            args.push(model);
        }

        // One Herdr workspace per card, found again by a token on the
        // workspace; each session gets its own tab in it.
        let snapshot = self.refresh().await?;
        let existing = snapshot
            .workspaces
            .iter()
            .find(|w| w.tokens.get(CARD_TOKEN) == Some(&card.id));
        let pane: PaneInfo = match existing {
            Some(ws) => {
                let created = self
                    .client
                    .call(
                        "tab.create",
                        json!({ "workspace_id": ws.workspace_id, "cwd": card.cwd, "label": station.name, "focus": false }),
                    )
                    .await?;
                parse(&created, "root_pane")?
            }
            None => {
                let created = self
                    .client
                    .call(
                        "workspace.create",
                        json!({ "cwd": card.cwd, "label": card.title, "focus": false }),
                    )
                    .await?;
                let pane: PaneInfo = parse(&created, "root_pane")?;
                // Best effort: without the token the next dispatch simply
                // opens another workspace.
                let _ = self
                    .client
                    .call(
                        "workspace.report_metadata",
                        json!({ "workspace_id": pane.workspace_id, "source": "starfleet", "tokens": { CARD_TOKEN: card.id } }),
                    )
                    .await;
                pane
            }
        };

        let short = station.id.rsplit('.').next().unwrap_or("agent");
        let name = format!("{short}-{}", &short_id()[..4]);
        let started = self
            .client
            .call_with_timeout(
                "agent.start",
                json!({ "name": name, "kind": kind, "pane_id": pane.pane_id, "args": args, "timeout_ms": 60_000 }),
                Duration::from_secs(75),
            )
            .await;
        let agent: PaneInfo = match started
            .map_err(anyhow::Error::from)
            .and_then(|r| parse(&r, "agent"))
        {
            Ok(agent) => agent,
            Err(e) => {
                // Do not leave an empty pane behind.
                let _ = self
                    .client
                    .call("pane.close", json!({ "pane_id": pane.pane_id }))
                    .await;
                return Err(e.context(format!("could not start `{kind}` for {}", station.name)));
            }
        };

        let text = if station.prompt.trim().is_empty() {
            prompt.to_string()
        } else {
            format!("{}\n\n{}", station.prompt.trim(), prompt)
        };
        let delivery = self
            .client
            .call(
                "agent.prompt",
                json!({ "target": agent.pane_id, "text": text }),
            )
            .await;
        let warning = delivery.as_ref().err().map(|e| {
            if awaiting_input(e) {
                "The agent started but is waiting on a question (for example a folder trust prompt), so the prompt was held back. Answer it, then send the prompt.".to_string()
            } else {
                format!("The agent started but the prompt was not delivered: {e}")
            }
        });

        let link = SessionLink {
            id: short_id(),
            card_id: card.id.clone(),
            station_id: Some(station.id.clone()),
            prompt: Some(prompt.to_string()),
            pending_prompt: warning.is_some().then(|| text.clone()),
            created_at: now_ms(),
            terminal_id: Some(agent.terminal_id.clone()).filter(|t| !t.is_empty()),
            pane_id: Some(agent.pane_id.clone()),
            agent_session: agent.agent_session.clone(),
            agent: agent
                .display_agent
                .clone()
                .or_else(|| agent.agent.clone())
                .or(Some(kind)),
            title: Some(station.name.clone()),
            last_status: agent.agent_status,
            last_seen: Some(now_ms()),
            output: None,
            output_at: None,
        };
        let result = DispatchResult {
            link_id: link.id.clone(),
            pane_id: agent.pane_id.clone(),
            warning,
        };
        self.store.lock().unwrap().add_link(link)?;
        let _ = self.refresh().await;
        Ok(result)
    }

    /// Deliver a first prompt that was held back at dispatch.
    pub async fn send_pending(&self, link_id: &str) -> Result<()> {
        let (pane_id, text) = {
            let live = self.live.read().unwrap();
            let store = self.store.lock().unwrap();
            let link = store
                .board
                .links
                .iter()
                .find(|l| l.id == link_id)
                .ok_or_else(|| anyhow!("that session is no longer on the board"))?;
            let pane = live
                .snapshot
                .as_ref()
                .and_then(|s| link.find_live(&s.panes))
                .ok_or_else(|| anyhow!("that session is no longer running in Herdr"))?;
            let text = link
                .pending_prompt
                .clone()
                .ok_or_else(|| anyhow!("nothing is waiting to be sent"))?;
            (pane.pane_id.clone(), text)
        };
        self.follow_up(&pane_id, &text).await?;
        let mut store = self.store.lock().unwrap();
        if let Some(link) = store.board.links.iter_mut().find(|l| l.id == link_id) {
            link.pending_prompt = None;
        }
        store.save()?;
        drop(store);
        self.notify();
        Ok(())
    }

    // ---- Talking to a running session -------------------------------------

    pub async fn follow_up(&self, pane_id: &str, text: &str) -> Result<()> {
        if text.trim().is_empty() {
            bail!("write a message first");
        }
        match self
            .client
            .call("agent.prompt", json!({ "target": pane_id, "text": text }))
            .await
        {
            Ok(_) => Ok(()),
            Err(e) if awaiting_input(&e) => {
                bail!("the agent is waiting on a question; answer that first (reply keys, or jump to the pane)")
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Answer a blocked agent's menu without leaving the app: optional literal
    /// text (for example "1"), then optional named keys (for example "enter").
    pub async fn reply_keys(
        &self,
        pane_id: &str,
        text: Option<String>,
        keys: Vec<String>,
    ) -> Result<()> {
        if let Some(text) = text.filter(|t| !t.is_empty()) {
            self.client
                .call(
                    "pane.send_text",
                    json!({ "pane_id": pane_id, "text": text }),
                )
                .await?;
        }
        if !keys.is_empty() {
            self.client
                .call(
                    "pane.send_keys",
                    json!({ "pane_id": pane_id, "keys": keys }),
                )
                .await?;
        }
        Ok(())
    }

    /// The last `lines` lines of a session's output, as plain text.
    pub async fn tail(&self, pane_id: &str, lines: u32) -> Result<String> {
        let result = self
            .client
            .call(
                "pane.read",
                json!({ "pane_id": pane_id, "source": "recent", "lines": lines, "format": "text", "strip_ansi": true }),
            )
            .await?;
        Ok(result
            .get("read")
            .and_then(|r| r.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string())
    }

    /// Bring a session to the front in whatever Herdr client is attached.
    pub async fn focus(&self, pane_id: &str) -> Result<()> {
        let pane = self.live_pane(pane_id)?;
        let _ = self
            .client
            .call(
                "workspace.focus",
                json!({ "workspace_id": pane.workspace_id }),
            )
            .await;
        let _ = self
            .client
            .call("tab.focus", json!({ "tab_id": pane.tab_id }))
            .await;
        self.client
            .call("pane.focus", json!({ "pane_id": pane_id }))
            .await?;
        Ok(())
    }
}

fn subscriptions(pane_ids: &BTreeSet<String>) -> Vec<Value> {
    const GLOBAL: [&str; 14] = [
        "workspace.created",
        "workspace.updated",
        "workspace.metadata_updated",
        "workspace.renamed",
        "workspace.closed",
        "tab.created",
        "tab.closed",
        "tab.renamed",
        "pane.created",
        "pane.closed",
        "pane.updated",
        "pane.moved",
        "pane.exited",
        "pane.agent_detected",
    ];
    let mut subs: Vec<Value> = GLOBAL.iter().map(|t| json!({ "type": t })).collect();
    subs.extend(
        pane_ids
            .iter()
            .map(|id| json!({ "type": "pane.agent_status_changed", "pane_id": id })),
    );
    subs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn api(code: &str) -> HerdrError {
        HerdrError::Api {
            code: code.into(),
            message: String::new(),
        }
    }

    #[test]
    fn blocked_and_not_ready_agents_hold_the_prompt() {
        assert!(awaiting_input(&api("agent_blocked")));
        assert!(awaiting_input(&api("agent_not_ready")));
        assert!(!awaiting_input(&api("agent_not_found")));
        assert!(!awaiting_input(&HerdrError::Timeout));
    }
}
