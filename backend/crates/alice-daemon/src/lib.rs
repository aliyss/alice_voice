//! Library entry of the daemon.
//! This crate exposes the server and services for the binary and the OpenAPI generator.

pub(crate) mod chat;
pub(crate) mod conversation;
pub(crate) mod db;
pub(crate) mod event_bus;
pub(crate) mod execution;
pub(crate) mod handling;
pub(crate) mod intent;
pub mod lifecycle;
pub(crate) mod queue;
pub(crate) mod resolver;
pub mod server;
pub(crate) mod settings;
