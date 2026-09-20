//! Conversation domain of the daemon.
//! A conversation owns its message history and its generated title.

pub mod entity;
pub mod service;
pub mod title;

pub use service::{ConversationService, NewMessage};
