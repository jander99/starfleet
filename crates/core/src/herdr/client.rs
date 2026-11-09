//! Request/response and event subscription over a Herdr endpoint.
//!
//! Each request uses its own short-lived connection. That costs little on a
//! Unix socket and makes no assumption about whether the server keeps a
//! connection open after answering.

use super::transport::{self, Conn, Endpoint};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum HerdrError {
    #[error("cannot reach Herdr: {0}")]
    Unreachable(String),
    #[error("Herdr refused the request ({code}): {message}")]
    Api { code: String, message: String },
    #[error("unexpected reply from Herdr: {0}")]
    Protocol(String),
    #[error("Herdr did not answer in time")]
    Timeout,
}

impl HerdrError {
    pub fn code(&self) -> Option<&str> {
        match self {
            HerdrError::Api { code, .. } => Some(code),
            _ => None,
        }
    }
}

pub struct HerdrClient {
    endpoint: Endpoint,
    next_id: AtomicU64,
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);

impl HerdrClient {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            endpoint,
            next_id: AtomicU64::new(1),
        }
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    fn id(&self) -> String {
        format!("sf_{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    async fn open(&self) -> Result<Conn, HerdrError> {
        match tokio::time::timeout(CONNECT_TIMEOUT, transport::connect(&self.endpoint)).await {
            Err(_) => Err(HerdrError::Unreachable("connection timed out".into())),
            Ok(Err(e)) => Err(HerdrError::Unreachable(format!(
                "{} ({e})",
                self.endpoint.describe()
            ))),
            Ok(Ok(conn)) => Ok(conn),
        }
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, HerdrError> {
        self.call_with_timeout(method, params, Duration::from_secs(15))
            .await
    }

    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, HerdrError> {
        let id = self.id();
        let request = json!({ "id": id, "method": method, "params": params }).to_string();
        let exchange = async {
            let mut conn = self.open().await?;
            conn.send_line(&request)
                .await
                .map_err(|e| HerdrError::Unreachable(e.to_string()))?;
            read_reply(&mut conn, &id).await
        };
        tokio::time::timeout(timeout, exchange)
            .await
            .map_err(|_| HerdrError::Timeout)?
    }

    /// Open a long-lived subscription. Events arrive through the returned stream.
    pub async fn subscribe(&self, subscriptions: Vec<Value>) -> Result<EventStream, HerdrError> {
        let id = self.id();
        let request = json!({
            "id": id,
            "method": "events.subscribe",
            "params": { "subscriptions": subscriptions },
        })
        .to_string();
        let mut conn = self.open().await?;
        conn.send_line(&request)
            .await
            .map_err(|e| HerdrError::Unreachable(e.to_string()))?;
        tokio::time::timeout(Duration::from_secs(15), read_reply(&mut conn, &id))
            .await
            .map_err(|_| HerdrError::Timeout)??;
        Ok(EventStream { conn })
    }
}

async fn read_reply(conn: &mut Conn, id: &str) -> Result<Value, HerdrError> {
    loop {
        let line = match conn.read_line().await {
            Ok(Some(line)) => line,
            Ok(None) => {
                let why = conn.close_reason().await;
                return Err(HerdrError::Unreachable(
                    why.unwrap_or_else(|| "connection closed before a reply".into()),
                ));
            }
            Err(e) => return Err(HerdrError::Unreachable(e.to_string())),
        };
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line)
            .map_err(|e| HerdrError::Protocol(format!("invalid JSON: {e}")))?;
        let reply_id = value.get("id").and_then(Value::as_str);
        // An empty id means Herdr could not parse our request at all.
        if reply_id != Some(id) && !(reply_id == Some("") && value.get("error").is_some()) {
            continue;
        }
        if let Some(err) = value.get("error") {
            return Err(api_error(err));
        }
        return Ok(value.get("result").cloned().unwrap_or(Value::Null));
    }
}

fn api_error(err: &Value) -> HerdrError {
    HerdrError::Api {
        code: err
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        message: err
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

pub struct EventStream {
    conn: Conn,
}

impl EventStream {
    /// The next event. An error ends the subscription (for example
    /// `events_lost` when this client fell behind); the caller resubscribes.
    pub async fn next(&mut self) -> Result<Value, HerdrError> {
        loop {
            match self.conn.read_line().await {
                Ok(Some(line)) if line.trim().is_empty() => continue,
                Ok(Some(line)) => {
                    let value: Value = serde_json::from_str(&line)
                        .map_err(|e| HerdrError::Protocol(format!("invalid JSON: {e}")))?;
                    if let Some(err) = value.get("error") {
                        return Err(api_error(err));
                    }
                    return Ok(value);
                }
                Ok(None) => {
                    let why = self.conn.close_reason().await;
                    return Err(HerdrError::Unreachable(
                        why.unwrap_or_else(|| "event stream closed".into()),
                    ));
                }
                Err(e) => return Err(HerdrError::Unreachable(e.to_string())),
            }
        }
    }
}
