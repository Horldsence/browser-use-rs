//! Event Bus - Simple event system for browser events
//!
//! Design: Type-safe events with async handlers.
//! No dynamic dispatch overhead - use enums, not trait objects.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Browser events that can be dispatched
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserEvent {
    Started,
    Stopped,
    NavigationStarted { url: String },
    NavigationComplete { url: String },
    TabCreated { target_id: String },
    TabClosed { target_id: String },
    TabSwitched { target_id: String },
    FileDownloaded { path: String },
}

/// Simple event bus using tokio broadcast channel
pub struct EventBus {
    tx: broadcast::Sender<BrowserEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    /// Publish an event
    pub fn publish(&self, event: BrowserEvent) {
        let _ = self.tx.send(event); // Ignore error if no subscribers
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<BrowserEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(BrowserEvent::Started);

        match rx.recv().await {
            Ok(BrowserEvent::Started) => {}
            _ => panic!("Expected Started event"),
        }
    }
}
