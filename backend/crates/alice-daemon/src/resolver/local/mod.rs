//! The built in models of the router.
//!
//! The router reads vectors and, when the decision stage reranks, reads how
//! well a message fits a candidate. Both reads can come from the model
//! server the daemon already talks to, and both can run in the daemon on a
//! model of this module.
//!
//! A built in model is a file on disk, so unlike a server it never fails
//! on a port. The daemon downloads the model through the settings page,
//! reports whether the file is there, and runs the graph through ONNX
//! Runtime on the processor or on a CUDA capable graphics card.

pub mod catalog;
pub mod encoder;
pub mod error;
pub mod store;

pub use encoder::LocalEngine;
pub use error::LocalError;
pub use store::LocalStore;
