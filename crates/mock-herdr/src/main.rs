//! Run a fake Herdr server for UI demos:
//!
//!     cargo run -p mock-herdr -- /tmp/mock-herdr.sock
//!     HERDR_SOCKET_PATH=/tmp/mock-herdr.sock npm run tauri dev

use mock_herdr::MockHerdr;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let socket = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/mock-herdr.sock".to_string()),
    );
    let mock = MockHerdr::serve(&socket, 6000).await?;
    mock.seed_agent(
        "homelab",
        "claude",
        "working",
        "/home/me/meta/operations/homelab",
    );
    mock.seed_agent("scratch", "opencode", "blocked", "/home/me/scratch");
    println!("mock Herdr listening on {}", socket.display());
    std::future::pending::<()>().await;
    Ok(())
}
