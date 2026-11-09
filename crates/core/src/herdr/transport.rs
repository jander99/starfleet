//! How bytes reach the Herdr socket.
//!
//! Natively this is a Unix socket. From Windows the socket lives inside WSL,
//! where a Windows process cannot open it, so a small relay process started
//! through `wsl.exe` bridges the socket to its own stdin and stdout.

use crate::env::{sh_quote, HostEnv};
use crate::paths::{home_dir, Settings};
use std::io;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{
    AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, Lines,
};
use tokio::process::Child;

#[derive(Clone, Debug)]
pub enum Endpoint {
    /// A Unix socket on this machine.
    Unix(PathBuf),
    /// A relay process inside `env`. `socket_expr` is a shell expression that
    /// evaluates to the socket path there.
    Relay { env: HostEnv, socket_expr: String },
}

const DEFAULT_SOCKET_EXPR: &str = r#""${HERDR_SOCKET_PATH:-$HOME/.config/herdr/herdr.sock}""#;

/// Bridges a Unix socket to stdin/stdout. No single quotes, so it can be
/// passed as one shell-quoted argument.
const PY_RELAY: &str = r#"
import os, socket, sys, threading
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
def up():
    while True:
        d = os.read(0, 65536)
        if not d:
            break
        s.sendall(d)
    try:
        s.shutdown(socket.SHUT_WR)
    except OSError:
        pass
threading.Thread(target=up, daemon=True).start()
while True:
    d = s.recv(65536)
    if not d:
        break
    os.write(1, d)
"#;

impl Endpoint {
    /// Pick the endpoint from settings and environment, following Herdr's own
    /// order: explicit socket, then named session, then the default socket.
    pub fn resolve(settings: &Settings, env: &HostEnv) -> Endpoint {
        let session = settings
            .herdr_session
            .clone()
            .or_else(|| std::env::var("HERDR_SESSION").ok())
            .filter(|s| {
                !s.is_empty()
                    && s.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            });
        match env {
            HostEnv::Native => {
                let explicit = settings
                    .herdr_socket
                    .clone()
                    .or_else(|| std::env::var("HERDR_SOCKET_PATH").ok())
                    .filter(|s| !s.is_empty());
                if let Some(p) = explicit {
                    return Endpoint::Unix(PathBuf::from(p));
                }
                let base = home_dir().unwrap_or_default().join(".config").join("herdr");
                match session {
                    Some(name) => {
                        Endpoint::Unix(base.join("sessions").join(name).join("herdr.sock"))
                    }
                    None => Endpoint::Unix(base.join("herdr.sock")),
                }
            }
            HostEnv::Wsl { .. } => {
                let socket_expr = match (&settings.herdr_socket, session) {
                    (Some(p), _) if !p.is_empty() => sh_quote(p),
                    (_, Some(name)) => {
                        format!(r#""$HOME/.config/herdr/sessions/{name}/herdr.sock""#)
                    }
                    _ => DEFAULT_SOCKET_EXPR.to_string(),
                };
                Endpoint::Relay {
                    env: env.clone(),
                    socket_expr,
                }
            }
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Endpoint::Unix(p) => p.display().to_string(),
            Endpoint::Relay { env, socket_expr } => format!("{} via {}", socket_expr, env.label()),
        }
    }
}

pub struct Conn {
    lines: Lines<BufReader<Box<dyn AsyncRead + Send + Unpin>>>,
    writer: Box<dyn AsyncWrite + Send + Unpin>,
    child: Option<Child>,
}

impl Conn {
    pub async fn send_line(&mut self, line: &str) -> io::Result<()> {
        self.writer.write_all(line.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await
    }

    /// Next line, or `None` once the peer has closed.
    pub async fn read_line(&mut self) -> io::Result<Option<String>> {
        self.lines.next_line().await
    }

    /// After an unexpected close: whatever the relay process said on stderr.
    pub async fn close_reason(&mut self) -> Option<String> {
        let mut stderr = self.child.as_mut()?.stderr.take()?;
        let mut text = String::new();
        let read = stderr.read_to_string(&mut text);
        let _ = tokio::time::timeout(std::time::Duration::from_millis(500), read).await;
        let text = text.trim().to_string();
        (!text.is_empty()).then_some(text)
    }
}

pub async fn connect(endpoint: &Endpoint) -> io::Result<Conn> {
    match endpoint {
        Endpoint::Unix(path) => connect_unix(path).await,
        Endpoint::Relay { env, socket_expr } => {
            let script = format!(
                "S={socket_expr}; \
                 if command -v socat >/dev/null 2>&1; then exec socat - \"UNIX-CONNECT:$S\"; \
                 elif command -v python3 >/dev/null 2>&1; then exec python3 -c {py} \"$S\"; \
                 elif command -v nc >/dev/null 2>&1; then exec nc -U \"$S\"; \
                 else echo 'no relay tool found: install socat, python3 or netcat' >&2; exit 127; fi",
                py = sh_quote(PY_RELAY),
            );
            let mut child = env
                .command(&script)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            let stdin = child.stdin.take().expect("piped stdin");
            let stdout = child.stdout.take().expect("piped stdout");
            let reader: Box<dyn AsyncRead + Send + Unpin> = Box::new(stdout);
            Ok(Conn {
                lines: BufReader::new(reader).lines(),
                writer: Box::new(stdin),
                child: Some(child),
            })
        }
    }
}

#[cfg(unix)]
async fn connect_unix(path: &std::path::Path) -> io::Result<Conn> {
    let stream = tokio::net::UnixStream::connect(path).await?;
    let (r, w) = stream.into_split();
    let reader: Box<dyn AsyncRead + Send + Unpin> = Box::new(r);
    Ok(Conn {
        lines: BufReader::new(reader).lines(),
        writer: Box::new(w),
        child: None,
    })
}

#[cfg(not(unix))]
async fn connect_unix(_path: &std::path::Path) -> io::Result<Conn> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "direct Unix sockets are not available on this platform",
    ))
}
