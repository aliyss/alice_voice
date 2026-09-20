//! Execution domain of the daemon.
//! The executor runs the shell command of a resolved intent.

pub mod runner;

pub use runner::{CommandOutcome, CommandRunner};
