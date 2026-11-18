//! System check: is everything Starfleet depends on installed and running,
//! in the environment where the agents actually live?

use crate::env::{sh_quote, HostEnv};
use crate::herdr::HerdrClient;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Ok,
    /// Optional piece missing or degraded; the app still works.
    Warn,
    Fail,
}

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub id: String,
    pub label: String,
    pub status: CheckStatus,
    pub detail: String,
    pub fix: Option<String>,
}

fn check(
    id: &str,
    label: &str,
    status: CheckStatus,
    detail: impl Into<String>,
    fix: Option<&str>,
) -> Check {
    Check {
        id: id.to_string(),
        label: label.to_string(),
        status,
        detail: detail.into(),
        fix: fix.map(str::to_string),
    }
}

/// Locate a CLI the way a login shell would, since GUI apps do not inherit
/// the PATH that shell profiles set up. Prints the path, then its version.
fn find_tool_script(name: &str) -> String {
    // The name ends up in shell text, so it is quoted once and exported: the
    // inner login shell must see it as data, not expand it before it runs.
    format!(
        "SF_TOOL={}; export SF_TOOL; PATH=\"$PATH:$HOME/.local/bin:$HOME/bin:$HOME/.cargo/bin:/usr/local/bin:/opt/homebrew/bin:\
$HOME/.bun/bin:$HOME/.npm-global/bin:$HOME/.opencode/bin:$HOME/.claude/local:\
/home/linuxbrew/.linuxbrew/bin:$HOME/.linuxbrew/bin\"; \
p=$(command -v \"$SF_TOOL\" 2>/dev/null); \
if [ ! -x \"$p\" ]; then p=$(bash -lic 'command -v \"$SF_TOOL\"' 2>/dev/null | tail -n 1); fi; \
[ -x \"$p\" ] || exit 1; echo \"$p\"; \"$p\" --version 2>/dev/null | head -n 1",
        sh_quote(name)
    )
}

/// GITHUB_TOKEN presence, as a check. Never reads or prints the value.
fn github_token_check(set: bool) -> Check {
    if set {
        check(
            "github.token",
            "GitHub token",
            CheckStatus::Ok,
            "GITHUB_TOKEN is set",
            None,
        )
    } else {
        check(
            "github.token",
            "GitHub token",
            CheckStatus::Warn,
            "GITHUB_TOKEN is not set",
            Some("export GITHUB_TOKEN=… (a classic PAT with repo scope), or run `gh auth login`."),
        )
    }
}

async fn tool_check(env: &HostEnv, name: &str, label: &str, required: bool, fix: &str) -> Check {
    let id = format!("tool.{name}");
    match env.sh(&find_tool_script(name)).await {
        Ok(out) if out.ok => {
            let mut lines = out.stdout.lines();
            let path = lines.next().unwrap_or_default();
            let version = lines.next().unwrap_or_default();
            let detail = if version.is_empty() {
                path.to_string()
            } else {
                format!("{version} ({path})")
            };
            check(&id, label, CheckStatus::Ok, detail, None)
        }
        _ => check(
            &id,
            label,
            if required {
                CheckStatus::Fail
            } else {
                CheckStatus::Warn
            },
            format!("`{name}` was not found in {}", env.label()),
            Some(fix),
        ),
    }
}

const CLAUDE_MEM_PORT_SCRIPT: &str = r#"d="${CLAUDE_MEM_DATA_DIR:-$HOME/.claude-mem}"
[ -d "$d" ] && echo installed || echo missing
if [ -n "$CLAUDE_MEM_WORKER_PORT" ]; then echo "$CLAUDE_MEM_WORKER_PORT"; exit 0; fi
p=$(sed -n 's/.*"CLAUDE_MEM_WORKER_PORT"[^0-9]*\([0-9][0-9]*\).*/\1/p' "$d/settings.json" 2>/dev/null | head -n 1)
if [ -n "$p" ]; then echo "$p"; else echo $((37700 + $(id -u) % 100)); fi"#;

async fn http_ok(port: u16, path: &str) -> bool {
    let attempt = async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .ok()?;
        let request = format!("GET {path} HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n");
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut buf = [0u8; 64];
        let n = stream.read(&mut buf).await.ok()?;
        Some(String::from_utf8_lossy(&buf[..n]).contains(" 200"))
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(3), attempt).await,
        Ok(Some(true))
    )
}

async fn claude_mem_check(env: &HostEnv) -> Check {
    let label = "claude-mem worker";
    let fix = "Optional. Install the claude-mem plugin in Claude Code and start a session so its worker runs.";
    let Ok(out) = env.sh(CLAUDE_MEM_PORT_SCRIPT).await else {
        return check(
            "claude_mem",
            label,
            CheckStatus::Warn,
            "could not probe for claude-mem",
            Some(fix),
        );
    };
    let mut lines = out.stdout.lines();
    let installed = lines.next() == Some("installed");
    let port: Option<u16> = lines.next().and_then(|p| p.trim().parse().ok());
    match (installed, port) {
        (_, Some(port)) if http_ok(port, "/api/health").await => {
            check("claude_mem", label, CheckStatus::Ok, format!("answering on port {port}"), None)
        }
        (true, Some(port)) => check(
            "claude_mem",
            label,
            CheckStatus::Warn,
            format!("installed, but nothing answers on port {port}"),
            Some("Optional. The worker starts with a Claude Code session; start one, or check `CLAUDE_MEM_WORKER_PORT`."),
        ),
        _ => check("claude_mem", label, CheckStatus::Warn, "not installed", Some(fix)),
    }
}

async fn integration_checks(client: &HerdrClient, out: &mut Vec<Check>) {
    let Ok(result) = client.call("integration.list", json!({})).await else {
        return;
    };
    let list = result
        .get("integrations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for target in ["claude", "opencode"] {
        let Some(item) = list
            .iter()
            .find(|i| i.get("target").and_then(Value::as_str) == Some(target))
        else {
            continue;
        };
        if item.get("available").and_then(Value::as_bool) != Some(true) {
            continue; // The CLI itself is missing; the tool check already says so.
        }
        let label = format!("Herdr integration for {target}");
        let id = format!("integration.{target}");
        match item.get("state").and_then(Value::as_str) {
            Some("current") => out.push(check(
                &id,
                &label,
                CheckStatus::Ok,
                "installed and current",
                None,
            )),
            Some("outdated") => out.push(check(
                &id,
                &label,
                CheckStatus::Warn,
                "installed but outdated",
                Some("Reinstall the integration from Herdr so status reporting stays accurate."),
            )),
            _ => out.push(check(
                &id,
                &label,
                CheckStatus::Warn,
                "not installed; Herdr falls back to reading the screen",
                Some("Install the integration from Herdr for reliable status and session resume."),
            )),
        }
    }
}

pub async fn run(env: &HostEnv, client: &HerdrClient) -> Vec<Check> {
    let mut out = Vec::new();

    match env {
        HostEnv::Native => out.push(check(
            "environment",
            "Environment",
            CheckStatus::Ok,
            format!("{} (native)", std::env::consts::OS),
            None,
        )),
        HostEnv::Wsl { .. } => match env.sh("uname -sr").await {
            Ok(o) if o.ok => out.push(check(
                "environment",
                "Environment",
                CheckStatus::Ok,
                format!("{}: {}", env.label(), o.stdout),
                None,
            )),
            other => {
                let why = match other {
                    Ok(o) => o.stderr,
                    Err(e) => e.to_string(),
                };
                out.push(check(
                    "environment",
                    "Environment",
                    CheckStatus::Fail,
                    format!("WSL is not reachable: {why}"),
                    Some("Install WSL with `wsl --install`, or set `wsl_distro` in settings.json."),
                ));
                return out; // Nothing else can be checked without WSL.
            }
        },
    }

    out.push(
        tool_check(
            env,
            "herdr",
            "Herdr",
            true,
            "Install Herdr: curl -fsSL https://herdr.dev/install.sh | sh",
        )
        .await,
    );

    match client.call("ping", json!({})).await {
        Ok(pong) => {
            let version = pong.get("version").and_then(Value::as_str).unwrap_or("unknown version");
            let protocol = pong.get("protocol").and_then(Value::as_u64).unwrap_or(0);
            out.push(check(
                "herdr.server",
                "Herdr server",
                CheckStatus::Ok,
                format!("{version}, protocol {protocol}, at {}", client.endpoint().describe()),
                None,
            ));
            integration_checks(client, &mut out).await;
        }
        Err(e) => out.push(check(
            "herdr.server",
            "Herdr server",
            CheckStatus::Fail,
            e.to_string(),
            Some("Start Herdr (run `herdr` in a terminal). For a named session, set `herdr_session` in settings.json."),
        )),
    }

    out.push(
        tool_check(
            env,
            "claude",
            "Claude Code",
            true,
            "Install Claude Code: https://code.claude.com/docs",
        )
        .await,
    );
    out.push(
        tool_check(
            env,
            "opencode",
            "OpenCode",
            false,
            "Optional, for ChatGPT and Gemini: https://opencode.ai",
        )
        .await,
    );
    out.push(claude_mem_check(env).await);
    out.push(
        tool_check(
            env,
            "gh",
            "GitHub CLI",
            false,
            "Install gh: https://cli.github.com, then `gh auth login` or export GITHUB_TOKEN.",
        )
        .await,
    );
    let token_set = matches!(env.sh("[ -n \"$GITHUB_TOKEN\" ] && echo set || echo unset").await,
        Ok(o) if o.ok && o.stdout == "set");
    out.push(github_token_check(token_set));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HostEnv;
    use crate::herdr::Endpoint;

    #[test]
    fn an_unset_token_is_a_warning_with_a_fix() {
        let c = github_token_check(false);
        assert_eq!(c.id, "github.token");
        assert_eq!(c.status, CheckStatus::Warn);
        assert!(c.fix.unwrap().contains("GITHUB_TOKEN"));
    }

    #[test]
    fn a_set_token_is_ok_and_never_prints_its_value() {
        let c = github_token_check(true);
        assert_eq!(c.status, CheckStatus::Ok);
        assert!(!c.detail.contains("ghp_"));
        assert!(c.detail.contains("set"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_token_probe_reads_the_environment() {
        // A prefix assignment is not expanded on the same command line, so
        // export first — otherwise the probe only passes when the outer
        // environment happens to have GITHUB_TOKEN set.
        let out = HostEnv::Native
            .sh("GITHUB_TOKEN=abc; export GITHUB_TOKEN; [ -n \"$GITHUB_TOKEN\" ] && echo set || echo unset")
            .await
            .unwrap();
        assert_eq!(out.stdout, "set");
    }

    #[tokio::test]
    async fn run_reports_the_gh_and_token_checks() {
        let env = HostEnv::Native;
        let client = HerdrClient::new(Endpoint::Unix("/nonexistent/herdr.sock".into()));
        let checks = run(&env, &client).await;
        assert!(checks.iter().any(|c| c.id == "github.token"));
        // gh itself is only asserted when the host has it installed.
        if env.sh("command -v gh").await.map(|o| o.ok).unwrap_or(false) {
            assert!(checks.iter().any(|c| c.id == "tool.gh"));
        }
    }
}
