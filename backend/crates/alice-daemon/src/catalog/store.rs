//! The commands of the machine, kept for a time.
//!
//! The catalog comes from two tools and costs one process each, so the
//! daemon reads it once and keeps it. The store reads the catalog again
//! when the kept one is older than its time to live, which is how a
//! command the user installed later still appears.
//!
//! The read runs on the blocking pool, because both tools are blocking
//! processes and must not stall the socket stream.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::catalog::entry::CommandEntry;
use crate::catalog::error::CatalogError;
use crate::catalog::fish::fish_entries;
use crate::catalog::man::man_summaries;

/// The time the daemon keeps one catalog before it reads it again.
const DEFAULT_TTL: Duration = Duration::from_secs(900);

/// The commands of the machine and the time the daemon read them.
#[derive(Clone, Debug)]
pub struct CatalogStore {
    /// The time one catalog stays fresh.
    ttl: Duration,
    /// The largest number of commands the store keeps, or zero for all.
    max_entries: usize,
    /// The catalog the daemon read, or none before the first read.
    state: Arc<Mutex<Option<Cached>>>,
}

/// One catalog with the time the daemon read it.
#[derive(Clone, Debug)]
struct Cached {
    /// The time the daemon read the catalog.
    at: Instant,
    /// The commands of the machine.
    entries: Arc<Vec<CommandEntry>>,
}

impl CatalogStore {
    /// Create a store with the default time to live.
    pub fn new() -> Self {
        Self::with_limit(DEFAULT_TTL, 0)
    }

    /// Create a store with a time to live and a largest size.
    ///
    /// A size of zero keeps every command of the machine. A smaller
    /// catalog is cheaper to read and faster to rank, so the size is a
    /// setting of the caller rather than a constant of the store.
    pub fn with_limit(ttl: Duration, max_entries: usize) -> Self {
        Self {
            ttl,
            max_entries,
            state: Arc::new(Mutex::new(None)),
        }
    }

    /// Read the commands of the machine.
    ///
    /// The store answers from the kept catalog when it is still fresh and
    /// builds a new one when it is not. The build runs on the blocking
    /// pool, so a slow tool does not stall the caller.
    pub async fn read(&self) -> Result<Arc<Vec<CommandEntry>>, CatalogError> {
        if let Some(entries) = self.fresh() {
            return Ok(entries);
        }

        let entries = tokio::task::spawn_blocking(build_catalog)
            .await
            .map_err(|err| CatalogError::Unavailable {
                tool: "catalog".to_string(),
                reason: err.to_string(),
            })??;

        let entries = cap_entries(entries, self.max_entries);
        let entries = Arc::new(entries);
        self.keep(Arc::clone(&entries));
        Ok(entries)
    }

    /// The kept catalog, when it is still fresh.
    fn fresh(&self) -> Option<Arc<Vec<CommandEntry>>> {
        let state = self.lock();
        let cached = state.as_ref()?;
        if cached.at.elapsed() < self.ttl {
            Some(Arc::clone(&cached.entries))
        } else {
            None
        }
    }

    /// Keep one catalog as the new one.
    fn keep(&self, entries: Arc<Vec<CommandEntry>>) {
        let mut state = self.lock();
        *state = Some(Cached {
            at: Instant::now(),
            entries,
        });
    }

    /// Lock the state and read it through a poisoned lock.
    fn lock(&self) -> MutexGuard<'_, Option<Cached>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for CatalogStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Read the commands of the machine from fish and the manual pages.
///
/// The fish list names the commands, and the manual pages give the
/// sentence of every command they document. A machine without a manual
/// index still yields the fish list, because a command without a sentence
/// is still a command the model may choose.
pub fn build_catalog() -> Result<Vec<CommandEntry>, CatalogError> {
    let mut entries = fish_entries()?;
    let summaries = man_summaries().unwrap_or_default();
    apply_summaries(&mut entries, &summaries);
    Ok(entries)
}

/// Keep at most `max` commands, the documented ones first.
///
/// A command with a sentence about what it does is a command the model
/// can use well, so a smaller catalog drops the names the manual pages
/// and the completions describe with nothing before it drops a real tool.
/// A size of zero, or a catalog that already fits, keeps every command.
fn cap_entries(mut entries: Vec<CommandEntry>, max: usize) -> Vec<CommandEntry> {
    if max == 0 || entries.len() <= max {
        return entries;
    }
    entries.sort_by(|left, right| {
        left.summary
            .is_empty()
            .cmp(&right.summary.is_empty())
            .then_with(|| left.name.cmp(&right.name))
    });
    entries.truncate(max);
    entries
}

/// Replace the summary of every command the manual pages document.
fn apply_summaries(entries: &mut [CommandEntry], summaries: &BTreeMap<String, String>) {
    for entry in entries {
        if let Some(summary) = summaries.get(&entry.name) {
            entry.summary = summary.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a store that holds two commands.
    fn seeded() -> CatalogStore {
        let store = CatalogStore::with_limit(Duration::from_secs(60), 0);
        store.keep(Arc::new(vec![
            CommandEntry::new("curl", "transfer a URL"),
            CommandEntry::new("ls", "list directory contents"),
        ]));
        store
    }

    #[test]
    fn a_fresh_catalog_is_answered_from_the_store() {
        let store = seeded();
        let entries = store.fresh().expect("the catalog is fresh");
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn a_catalog_older_than_the_time_to_live_is_not_fresh() {
        let store = CatalogStore::with_limit(Duration::ZERO, 0);
        store.keep(Arc::new(vec![CommandEntry::new("curl", "")]));
        assert!(store.fresh().is_none());
    }

    #[test]
    fn a_size_keeps_the_documented_commands_first() {
        let entries = vec![
            CommandEntry::new("aaa", ""),
            CommandEntry::new("zzz", "a documented tool"),
            CommandEntry::new("bbb", ""),
        ];

        let kept = cap_entries(entries, 1);

        assert_eq!(kept, vec![CommandEntry::new("zzz", "a documented tool")]);
    }

    #[test]
    fn a_size_of_zero_keeps_every_command() {
        let entries = vec![CommandEntry::new("curl", ""), CommandEntry::new("ls", "")];
        assert_eq!(cap_entries(entries.clone(), 0), entries);
    }

    #[test]
    fn a_catalog_inside_its_size_stays_as_it_is() {
        let entries = vec![CommandEntry::new("curl", "")];
        assert_eq!(cap_entries(entries.clone(), 5), entries);
    }

    #[test]
    fn the_manual_pages_replace_the_fish_summary() {
        let mut entries = vec![
            CommandEntry::new("curl", ""),
            CommandEntry::new("ls", "command"),
        ];
        let summaries = BTreeMap::from([("ls".to_string(), "list directory contents".to_string())]);

        apply_summaries(&mut entries, &summaries);

        assert_eq!(entries[0], CommandEntry::new("curl", ""));
        assert_eq!(
            entries[1],
            CommandEntry::new("ls", "list directory contents")
        );
    }
}
