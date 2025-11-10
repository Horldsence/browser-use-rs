//! CDP Client - The Core Communication Layer
//!
//! Design decisions:
//! 1. Single WebSocket per browser connection (no per-session WS overhead)
//! 2. Async message passing - no locks on send/receive path  
//! 3. Request/response matching via ID, events broadcast to subscribers
//! 4. Fail fast - no retries, no queuing. Let the caller decide.

use dashmap::DashMap;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use super::protocol::*;

type WsSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;

#[derive(Error, Debug)]
pub enum CDPError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CDP protocol error: {code} - {message}")]
    Protocol { code: i32, message: String },

    #[error("Request timeout")]
    Timeout,

    #[error("Connection closed")]
    Closed,

    #[error("Invalid response for request {0}")]
    InvalidResponse(RequestId),
}

/// Result type for CDP operations
pub type Result<T> = std::result::Result<T, CDPError>;

/// Event subscriber callback
pub type EventCallback = Arc<dyn Fn(CDPEvent) + Send + Sync>;

/// CDP Client - manages single WebSocket connection to browser
pub struct CDPClient {
    /// Monotonic request ID counter
    next_id: AtomicU64,

    /// Pending requests waiting for responses
    /// Key: request_id, Value: oneshot sender for response
    pending: Arc<DashMap<RequestId, oneshot::Sender<CDPResponse>>>,

    /// Event subscribers
    /// Key: method name (e.g., "Page.loadEventFired"), Value: callbacks
    subscribers: Arc<DashMap<String, Vec<EventCallback>>>,

    /// WebSocket write half (wrapped for concurrent sending)
    ws_sink: Arc<RwLock<WsSink>>,
}
impl CDPClient {
    /// Connect to Chrome DevTools Protocol endpoint
    pub async fn connect(ws_url: &str) -> Result<Arc<Self>> {
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (sink, mut stream) = ws_stream.split();

        let client = Arc::new(Self {
            next_id: AtomicU64::new(1),
            pending: Arc::new(DashMap::new()),
            subscribers: Arc::new(DashMap::new()),
            ws_sink: Arc::new(RwLock::new(sink)),
        });

        // Spawn message receiver task
        let client_clone = client.clone();
        let (_shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = stream.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                if let Err(e) = client_clone.handle_message(&text).await {
                                    tracing::error!("Failed to handle message: {}", e);
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => {
                                tracing::info!("WebSocket closed");
                                break;
                            }
                            Some(Err(e)) => {
                                tracing::error!("WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        tracing::info!("Shutdown signal received");
                        break;
                    }
                }
            }

            // Clear all pending requests
            client_clone.pending.clear();
        });

        // Store shutdown channel (need to make client mutable - fix this with Arc<Mutex<Option<_>>>)
        // For now, rely on Drop

        Ok(client)
    }

    /// Send CDP request and wait for response
    pub async fn send_request(
        &self,
        method: impl Into<String>,
        params: Option<Value>,
        session_id: Option<SessionId>,
    ) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = CDPRequest {
            id,
            method: method.into(),
            params,
            session_id,
        };

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, tx);

        // Serialize and send
        let json = serde_json::to_string(&request)?;
        let mut sink = self.ws_sink.write().await;
        sink.send(Message::Text(json))
            .await
            .map_err(|e| CDPError::WebSocket(e))?;
        drop(sink); // Release lock immediately

        // Wait for response
        let response = rx.await.map_err(|_| CDPError::Closed)?;

        if let Some(error) = response.error {
            return Err(CDPError::Protocol {
                code: error.code,
                message: error.message,
            });
        }

        Ok(response.result.unwrap_or(Value::Null))
    }

    /// Subscribe to CDP events
    pub fn subscribe(&self, method: impl Into<String>, callback: EventCallback) {
        let method = method.into();
        self.subscribers
            .entry(method)
            .or_insert_with(Vec::new)
            .push(callback);
    }

    /// Handle incoming WebSocket message
    async fn handle_message(&self, text: &str) -> Result<()> {
        let msg: CDPMessage = serde_json::from_str(text)?;

        match msg {
            CDPMessage::Response(response) => {
                if let Some((_, tx)) = self.pending.remove(&response.id) {
                    let _ = tx.send(response); // Ignore send errors (receiver dropped)
                } else {
                    tracing::warn!("Received response for unknown request: {}", response.id);
                }
            }
            CDPMessage::Event(event) => {
                if let Some(subscribers) = self.subscribers.get(&event.method) {
                    for callback in subscribers.value() {
                        callback(event.clone());
                    }
                }
            }
        }

        Ok(())
    }

    /// Close connection gracefully
    pub async fn close(self: Arc<Self>) -> Result<()> {
        let mut sink = self.ws_sink.write().await;
        sink.close().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Real tests need a running Chrome instance
    // These are just compilation tests

    #[tokio::test]
    #[ignore]
    async fn test_connect() {
        let client = CDPClient::connect("ws://localhost:9222/devtools/browser")
            .await
            .unwrap();

        let result = client
            .send_request("Browser.getVersion", None, None)
            .await
            .unwrap();

        println!("Browser version: {:?}", result);
    }
}
