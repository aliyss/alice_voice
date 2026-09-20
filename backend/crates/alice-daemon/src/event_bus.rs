//! Event bus of the daemon.
//!
//! Every stage of a turn publishes one event. The socket stream forwards
//! the events to the frontend, so the interface shows the stages while
//! the turn runs. The stages are the resolved intent, the model tokens,
//! the command, and the output of the command.

use chrono::Utc;
use tokio::sync::broadcast;

use alice_core::dto::{SystemEventDto, SystemEventPayloadDto};

/// The number of events a slow listener may fall behind.
const CHANNEL_CAPACITY: usize = 256;

/// The publisher of the events of one daemon.
#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<SystemEventDto>,
}

impl EventBus {
    /// Create a new event bus and the receiver of its first listener.
    pub fn new() -> (Self, broadcast::Receiver<SystemEventDto>) {
        let (sender, receiver) = broadcast::channel(CHANNEL_CAPACITY);
        (Self { sender }, receiver)
    }

    /// Subscribe to the events.
    pub fn subscribe(&self) -> broadcast::Receiver<SystemEventDto> {
        self.sender.subscribe()
    }

    /// Publish one event to every listener.
    pub fn publish(&self, payload: SystemEventPayloadDto) {
        let event = SystemEventDto {
            at: Utc::now(),
            payload,
        };
        // A turn runs without a listener when the frontend is closed.
        let _ = self.sender.send(event);
    }
}
