//! Librarian domain of the daemon.
//!
//! The librarian keeps the long term memory of the daemon: one node per
//! concept and one dated edge per relation, written from the turns the
//! user really sent.

pub mod entity;
pub mod episode;
pub mod extract;
pub mod facts;
pub mod gate;
pub mod service;
pub mod store;

#[cfg(test)]
mod eval;

pub use service::LibrarianService;
pub use store::LibrarianStore;
