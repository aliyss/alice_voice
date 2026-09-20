//! Intent domain of the daemon.
//! An intent names one capability, describes it to the resolver, and runs
//! one shell command when the resolver chooses it.

pub mod command;
pub mod defaults;
pub mod entity;
pub mod error;
pub mod input;
pub mod matcher;
pub mod service;
pub mod values;

pub use command::{render_command, strip_placeholders};
pub use error::SaveIntentError;
pub use service::IntentService;
pub use values::{EntityList, EntityScripts};
