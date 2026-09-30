//! Catalog of the commands the machine can run.
//!
//! When no intent matches a message, the daemon may propose a shell
//! script that answers it. The model writes that script against the
//! commands the machine really holds, so the daemon reads those commands
//! from the completions of fish and from the manual pages and hands the
//! model a short, ranked list.
//!
//! The full list is long, so the daemon keeps it for a time and ranks it
//! against every message. See `store` for the cache and `rank` for the
//! ranking.

pub mod entry;
pub mod error;
pub mod fish;
pub mod man;
pub mod rank;
pub mod store;

pub use entry::CommandEntry;
pub use rank::CatalogRanker;
pub use store::CatalogStore;
