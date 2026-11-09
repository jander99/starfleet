//! The environment the agents live in.
//!
//! On Linux and macOS that is the machine itself. On Windows the agents and
//! Herdr live inside WSL, so every probe and every Herdr connection is routed
//! through `wsl.exe`.

use anyhow::{anyhow, Result};
use serde::Serialize;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostEnv {
    Native,
    Wsl { distro: Option<String> },
}

#[derive(Debug)]
pub struct ShOutput {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

impl HostEnv {
    pub fn detect(wsl_distro: Option<String>) -> Self {
        if cfg!(windows) {
            HostEnv::Wsl { distro: wsl_distro }
        } else {
            HostEnv::Native
        }
    }

    pub fn label(&self) -> String {
        match self {
            HostEnv::Native => std::env::consts::OS.to_string(),
            HostEnv::Wsl { distro: Some(d) } => format!("windows + wsl ({d})"),
            HostEnv::Wsl { distro: None } => "windows + wsl (default distribution)".to_string(),
        }
    }

    /// A command that runs `script` with a POSIX shell in this environment.
    pub fn command(&self, script: &str) -> Command {
        let mut cmd = match self {
            HostEnv::Native => {
                let mut c = Command::new("sh");
                c.arg("-c").arg(script);
                c
            }
            HostEnv::Wsl { distro } => {
                let mut c = Command::new("wsl.exe");
                if let Some(d) = distro {
                    c.arg("-d").arg(d);
                }
                c.arg("-e").arg("sh").arg("-c").arg(script);
                c
            }
        };
        cmd.kill_on_drop(true);
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: do not flash a console for every probe.
            cmd.creation_flags(0x0800_0000);
        }
        cmd
    }

    pub async fn sh(&self, script: &str) -> Result<ShOutput> {
        let mut cmd = self.command(script);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let out = tokio::time::timeout(Duration::from_secs(15), cmd.output())
            .await
            .map_err(|_| anyhow!("command timed out"))?
            .map_err(|e| match self {
                HostEnv::Wsl { .. } => anyhow!("could not run wsl.exe: {e}"),
                HostEnv::Native => anyhow!("could not run sh: {e}"),
            })?;
        Ok(ShOutput {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).trim().to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        })
    }
}

/// Quote a string for a POSIX shell.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_survives_single_quotes() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn native_shell_runs() {
        let out = HostEnv::Native
            .sh(&format!("printf %s {}", sh_quote("a 'b' $c")))
            .await
            .unwrap();
        assert!(out.ok);
        assert_eq!(out.stdout, "a 'b' $c");
    }
}
