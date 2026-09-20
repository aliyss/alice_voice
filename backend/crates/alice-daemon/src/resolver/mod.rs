//! Intent resolver of the daemon.
//! The resolver reads the intent of a message from a small local model.
//! The options arrive with every request, so the intent configuration of
//! the user defines what the model may choose.

pub mod client;
pub mod decision;
pub mod device;
pub mod download;
pub mod error;
pub mod gliner;
pub mod local;
pub mod probability;
pub mod prompt;
pub mod router;
pub mod service;
pub mod values;

pub use error::ResolveError;
pub use gliner::{GlinerResolver, GlinerStore};
pub use local::{LocalEngine, LocalStore};
pub use service::{
    ContextTurn, Resolution, ResolveRequest, ResolvedIntent, ResolverService, RouteReport,
};
