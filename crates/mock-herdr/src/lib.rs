//! A fake Herdr server.
//!
//! It speaks the subset of the Herdr socket API that Starfleet uses, written
//! from the published schema (protocol 22). It exists so the app can be
//! tested and demoed without Herdr; it is not evidence of how the real server
//! behaves in cases the schema does not spell out.
//!
//! Simulated agents: a prompt makes the agent `working`, then `done` after
//! `work_ms`. A prompt containing `[block]` ends `blocked` instead. An agent
//! started in a directory whose path contains `untrusted` starts `blocked`,
//! like a CLI showing a trust dialog. Sending keys to a blocked agent
//! unblocks it.

use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
struct Pane {
    pane_id: String,
    terminal_id: String,
    workspace_id: String,
    tab_id: String,
    cwd: String,
    agent: Option<String>,
    name: Option<String>,
    status: &'static str,
    transcript: Vec<String>,
}

#[derive(Default)]
struct State {
    counter: u64,
    workspaces: Vec<(String, String, Map<String, Value>)>, // id, label, tokens
    tabs: Vec<(String, String, String)>,                   // id, workspace, label
    panes: Vec<Pane>,
}

/// An event as broadcast to subscribers: dotted kind, pane it concerns, wire payload.
type Event = (String, Option<String>, Value);

#[derive(Clone)]
pub struct MockHerdr {
    state: Arc<Mutex<State>>,
    events: broadcast::Sender<Event>,
    work_ms: u64,
}

impl State {
    fn next(&mut self) -> u64 {
        self.counter += 1;
        self.counter
    }

    fn pane_json(&self, p: &Pane) -> Value {
        json!({
            "pane_id": p.pane_id, "terminal_id": p.terminal_id, "workspace_id": p.workspace_id,
            "tab_id": p.tab_id, "focused": false, "agent_status": if p.agent.is_some() { p.status } else { "unknown" },
            "revision": p.transcript.len(), "agent": p.agent, "name": p.name, "cwd": p.cwd,
            "agent_session": p.agent.as_ref().map(|a| json!({
                "source": "mock", "agent": a, "kind": "id", "value": format!("session-{}", p.terminal_id),
            })),
        })
    }

    fn workspace_json(&self, id: &str) -> Value {
        let (_, label, tokens) = self
            .workspaces
            .iter()
            .find(|w| w.0 == id)
            .expect("workspace");
        let panes: Vec<&Pane> = self.panes.iter().filter(|p| p.workspace_id == id).collect();
        json!({
            "workspace_id": id, "number": 1, "label": label, "focused": false,
            "pane_count": panes.len(), "tab_count": self.tabs.iter().filter(|t| t.1 == id).count(),
            "active_tab_id": self.tabs.iter().find(|t| t.1 == id).map(|t| t.0.clone()).unwrap_or_default(),
            "agent_status": "unknown", "tokens": tokens,
        })
    }

    fn tab_json(&self, id: &str) -> Value {
        let (_, ws, label) = self.tabs.iter().find(|t| t.0 == id).expect("tab");
        json!({ "tab_id": id, "workspace_id": ws, "number": 1, "label": label, "focused": false,
                "pane_count": 1, "agent_status": "unknown" })
    }

    fn add_tab(&mut self, workspace_id: &str, label: &str, cwd: &str) -> Pane {
        let n = self.next();
        let tab_id = format!("{workspace_id}:t{n}");
        self.tabs
            .push((tab_id.clone(), workspace_id.to_string(), label.to_string()));
        let pane = Pane {
            pane_id: format!("{workspace_id}:p{n}"),
            terminal_id: format!("term_{n}"),
            workspace_id: workspace_id.to_string(),
            tab_id,
            cwd: cwd.to_string(),
            agent: None,
            name: None,
            status: "unknown",
            transcript: vec![],
        };
        self.panes.push(pane.clone());
        pane
    }

    fn add_workspace(&mut self, label: &str, cwd: &str) -> Pane {
        let id = format!("w{}", self.next());
        self.workspaces
            .push((id.clone(), label.to_string(), Map::new()));
        self.add_tab(&id, "main", cwd)
    }
}

fn err(code: &str, message: &str) -> Value {
    json!({ "code": code, "message": message })
}

fn str_param<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(Value::as_str).unwrap_or_default()
}

impl MockHerdr {
    /// Bind `socket` and serve until the returned handle and all clones are dropped
    /// and the runtime shuts down.
    pub async fn serve(socket: &Path, work_ms: u64) -> Result<Self> {
        let _ = std::fs::remove_file(socket);
        if let Some(dir) = socket.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let listener = UnixListener::bind(socket)?;
        let (events, _) = broadcast::channel(256);
        let mock = MockHerdr {
            state: Arc::default(),
            events,
            work_ms,
        };
        let server = mock.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let server = server.clone();
                tokio::spawn(async move {
                    let _ = server.connection(stream).await;
                });
            }
        });
        Ok(mock)
    }

    /// Add an agent that Starfleet did not start, as if launched by hand.
    pub fn seed_agent(
        &self,
        workspace_label: &str,
        agent: &str,
        status: &'static str,
        cwd: &str,
    ) -> String {
        let mut s = self.state.lock().unwrap();
        let pane = s.add_workspace(workspace_label, cwd);
        let p = s
            .panes
            .iter_mut()
            .find(|p| p.pane_id == pane.pane_id)
            .unwrap();
        p.agent = Some(agent.to_string());
        p.status = status;
        p.transcript
            .push(format!("{agent} session started by hand in {cwd}"));
        pane.pane_id
    }

    /// Close a pane, as if the user closed it in Herdr.
    pub fn close_pane(&self, pane_id: &str) {
        let workspace_id = {
            let mut s = self.state.lock().unwrap();
            let ws = s
                .panes
                .iter()
                .find(|p| p.pane_id == pane_id)
                .map(|p| p.workspace_id.clone());
            s.panes.retain(|p| p.pane_id != pane_id);
            ws
        };
        if let Some(workspace_id) = workspace_id {
            self.emit("pane.closed", None, json!({
                "event": "pane_closed",
                "data": { "type": "pane_closed", "pane_id": pane_id, "workspace_id": workspace_id },
            }));
        }
    }

    pub fn prompts_received(&self, pane_id: &str) -> Vec<String> {
        let s = self.state.lock().unwrap();
        s.panes
            .iter()
            .find(|p| p.pane_id == pane_id)
            .map(|p| {
                p.transcript
                    .iter()
                    .filter(|l| l.starts_with("> "))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn emit(&self, kind: &str, pane_id: Option<&str>, payload: Value) {
        let _ = self
            .events
            .send((kind.to_string(), pane_id.map(str::to_string), payload));
    }

    fn set_status(&self, pane_id: &str, status: &'static str, line: Option<String>) {
        let workspace_id = {
            let mut s = self.state.lock().unwrap();
            let Some(p) = s.panes.iter_mut().find(|p| p.pane_id == pane_id) else {
                return;
            };
            p.status = status;
            if let Some(line) = line {
                p.transcript.push(line);
            }
            p.workspace_id.clone()
        };
        self.emit("pane.agent_status_changed", Some(pane_id), json!({
            "event": "pane.agent_status_changed",
            "data": { "pane_id": pane_id, "workspace_id": workspace_id, "agent_status": status },
        }));
    }

    fn finish_later(&self, pane_id: String, outcome: &'static str) {
        let mock = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(mock.work_ms)).await;
            let line = match outcome {
                "blocked" => "Do you want to proceed?  1. Yes  2. No",
                _ => "Finished. Summary: simulated work complete.",
            };
            mock.set_status(&pane_id, outcome, Some(line.to_string()));
        });
    }

    async fn connection(&self, stream: tokio::net::UnixStream) -> Result<()> {
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        while let Some(line) = lines.next_line().await? {
            let Ok(request) = serde_json::from_str::<Value>(&line) else {
                let reply = json!({ "id": "", "error": err("invalid_request", "not JSON") });
                write.write_all(format!("{reply}\n").as_bytes()).await?;
                continue;
            };
            let id = request.get("id").cloned().unwrap_or(json!(""));
            let method = str_param(&request, "method").to_string();
            let params = request.get("params").cloned().unwrap_or(json!({}));

            if method == "events.subscribe" {
                // Subscribe before acknowledging so no event is missed.
                let mut rx = self.events.subscribe();
                let wanted: Vec<(String, Option<String>)> = params["subscriptions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|s| {
                        (
                            str_param(s, "type").to_string(),
                            s.get("pane_id").and_then(Value::as_str).map(str::to_string),
                        )
                    })
                    .collect();
                let known: Vec<String> = self
                    .state
                    .lock()
                    .unwrap()
                    .panes
                    .iter()
                    .map(|p| p.pane_id.clone())
                    .collect();
                if let Some(missing) = wanted
                    .iter()
                    .filter_map(|w| w.1.as_ref())
                    .find(|p| !known.contains(p))
                {
                    let reply = json!({ "id": id, "error": err("pane_not_found", missing) });
                    write.write_all(format!("{reply}\n").as_bytes()).await?;
                    continue;
                }
                let ack = json!({ "id": id, "result": { "type": "subscription_started" } });
                write.write_all(format!("{ack}\n").as_bytes()).await?;
                loop {
                    let (kind, pane, payload) = rx.recv().await?;
                    let matches = wanted
                        .iter()
                        .any(|(t, p)| *t == kind && (p.is_none() || *p == pane));
                    if matches {
                        write.write_all(format!("{payload}\n").as_bytes()).await?;
                    }
                }
            }

            let reply = match self.handle(&method, &params) {
                Ok(result) => json!({ "id": id, "result": result }),
                Err(error) => json!({ "id": id, "error": error }),
            };
            write.write_all(format!("{reply}\n").as_bytes()).await?;
        }
        Ok(())
    }

    fn handle(&self, method: &str, params: &Value) -> Result<Value, Value> {
        let ok = || Ok(json!({ "type": "ok" }));
        match method {
            "ping" => Ok(json!({ "type": "pong", "version": "0.9.5-mock", "protocol": 22 })),
            "session.snapshot" => {
                let s = self.state.lock().unwrap();
                let panes: Vec<Value> = s.panes.iter().map(|p| s.pane_json(p)).collect();
                let agents: Vec<Value> = s
                    .panes
                    .iter()
                    .filter(|p| p.agent.is_some())
                    .map(|p| s.pane_json(p))
                    .collect();
                Ok(json!({ "type": "session_snapshot", "snapshot": {
                    "version": "0.9.5-mock", "protocol": 22,
                    "workspaces": s.workspaces.iter().map(|w| s.workspace_json(&w.0)).collect::<Vec<_>>(),
                    "tabs": s.tabs.iter().map(|t| s.tab_json(&t.0)).collect::<Vec<_>>(),
                    "panes": panes, "layouts": [], "agents": agents,
                }}))
            }
            "integration.list" => Ok(json!({ "type": "integration_list", "integrations": [
                { "target": "claude", "label": "Claude Code", "command": "claude", "available": true, "state": "current" },
                { "target": "opencode", "label": "OpenCode", "command": "opencode", "available": true, "state": "not_installed" },
            ]})),
            "workspace.create" => {
                let (ws, tab, pane) = {
                    let mut s = self.state.lock().unwrap();
                    let pane =
                        s.add_workspace(str_param(params, "label"), str_param(params, "cwd"));
                    (
                        s.workspace_json(&pane.workspace_id),
                        s.tab_json(&pane.tab_id),
                        s.pane_json(&pane),
                    )
                };
                self.emit("pane.created", None, json!({ "event": "pane_created", "data": { "type": "pane_created", "pane": pane } }));
                Ok(
                    json!({ "type": "workspace_created", "workspace": ws, "tab": tab, "root_pane": pane }),
                )
            }
            "tab.create" => {
                let (tab, pane) = {
                    let mut s = self.state.lock().unwrap();
                    let ws = str_param(params, "workspace_id");
                    if !s.workspaces.iter().any(|w| w.0 == ws) {
                        return Err(err("workspace_not_found", ws));
                    }
                    let pane = s.add_tab(ws, str_param(params, "label"), str_param(params, "cwd"));
                    (s.tab_json(&pane.tab_id), s.pane_json(&pane))
                };
                self.emit("pane.created", None, json!({ "event": "pane_created", "data": { "type": "pane_created", "pane": pane } }));
                Ok(json!({ "type": "tab_created", "tab": tab, "root_pane": pane }))
            }
            "workspace.report_metadata" => {
                let mut s = self.state.lock().unwrap();
                let ws = str_param(params, "workspace_id");
                let Some(entry) = s.workspaces.iter_mut().find(|w| w.0 == ws) else {
                    return Err(err("workspace_not_found", ws));
                };
                if let Some(tokens) = params.get("tokens").and_then(Value::as_object) {
                    entry.2.extend(tokens.clone());
                }
                ok()
            }
            "agent.start" => {
                let kind = str_param(params, "kind");
                if !["claude", "opencode", "codex"].contains(&kind) {
                    return Err(err(
                        "agent_start_failed",
                        &format!("`{kind}` is not installed"),
                    ));
                }
                let pane_id = str_param(params, "pane_id");
                let args: Vec<String> = params["args"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                let info = {
                    let mut s = self.state.lock().unwrap();
                    let Some(p) = s.panes.iter_mut().find(|p| p.pane_id == pane_id) else {
                        return Err(err("pane_not_found", pane_id));
                    };
                    p.agent = Some(kind.to_string());
                    p.name = Some(str_param(params, "name").to_string());
                    p.status = if p.cwd.contains("untrusted") {
                        "blocked"
                    } else {
                        "idle"
                    };
                    p.transcript.push(format!("$ {kind} {}", args.join(" ")));
                    if p.status == "blocked" {
                        p.transcript
                            .push("Do you trust the files in this folder?  1. Yes  2. No".into());
                    }
                    let p = p.clone();
                    s.pane_json(&p)
                };
                let mut argv = vec![kind.to_string()];
                argv.extend(args);
                Ok(json!({ "type": "agent_started", "agent": info, "argv": argv }))
            }
            "agent.prompt" => {
                let target = str_param(params, "target");
                let text = str_param(params, "text");
                let info = {
                    let s = self.state.lock().unwrap();
                    let Some(p) = s
                        .panes
                        .iter()
                        .find(|p| p.pane_id == target && p.agent.is_some())
                    else {
                        return Err(err("agent_not_found", target));
                    };
                    if p.status == "blocked" {
                        return Err(err("agent_blocked", "agent is blocked"));
                    }
                    s.pane_json(p)
                };
                self.set_status(target, "working", Some(format!("> {text}")));
                self.finish_later(
                    target.to_string(),
                    if text.contains("[block]") {
                        "blocked"
                    } else {
                        "done"
                    },
                );
                Ok(json!({ "type": "agent_prompted", "agent": info }))
            }
            "pane.send_text" | "pane.send_keys" => {
                let pane_id = str_param(params, "pane_id");
                let blocked = {
                    let s = self.state.lock().unwrap();
                    match s.panes.iter().find(|p| p.pane_id == pane_id) {
                        Some(p) => p.status == "blocked",
                        None => return Err(err("pane_not_found", pane_id)),
                    }
                };
                if blocked {
                    self.set_status(pane_id, "idle", Some("(answered)".into()));
                }
                ok()
            }
            "pane.read" => {
                let s = self.state.lock().unwrap();
                let pane_id = str_param(params, "pane_id");
                let Some(p) = s.panes.iter().find(|p| p.pane_id == pane_id) else {
                    return Err(err("pane_not_found", pane_id));
                };
                Ok(json!({ "type": "pane_read", "read": {
                    "pane_id": p.pane_id, "workspace_id": p.workspace_id, "tab_id": p.tab_id,
                    "source": "recent", "format": "text", "text": p.transcript.join("\n"),
                    "revision": p.transcript.len(), "truncated": false,
                }}))
            }
            "pane.close" => {
                self.close_pane(str_param(params, "pane_id"));
                ok()
            }
            "workspace.focus" | "tab.focus" | "pane.focus" => ok(),
            other => Err(err("unknown_method", other)),
        }
    }
}
