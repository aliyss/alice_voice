//! Ranking of the commands against one message.
//!
//! A machine holds far more commands than one prompt can carry, so the
//! daemon ranks the catalog against the message and hands the model only
//! the commands that fit. The rank reads as meaning rather than as
//! spelling, so `send the file to a machine` reaches `scp` although the
//! message shares no word with the command.
//!
//! The vectors come from the reader of the router, so the vectors of the
//! commands are read once and kept. A rank without a reader, or with a
//! catalog the reader did not answer for, returns the commands in name
//! order rather than nothing.

use crate::catalog::entry::CommandEntry;
use crate::resolver::error::ResolveError;
use crate::resolver::router::embed::{cosine, Embedder};

/// The number of commands one request may read at once.
const BATCH: usize = 128;

/// Ranks the commands of the catalog against a message.
pub struct CatalogRanker<'a> {
    /// The reader of the vectors.
    embedder: &'a Embedder,
    /// The largest number of commands the rank returns.
    limit: usize,
}

impl<'a> CatalogRanker<'a> {
    /// Create a ranker with a reader and a limit.
    pub fn new(embedder: &'a Embedder, limit: usize) -> Self {
        Self { embedder, limit }
    }

    /// Read the commands that fit the message best, best first.
    ///
    /// The reader compares the message with the name and the summary of
    /// every command. A reader that does not answer fails, so the caller
    /// can fall back to a list without a rank.
    pub async fn rank(
        &self,
        message: &str,
        entries: &[CommandEntry],
    ) -> Result<Vec<CommandEntry>, ResolveError> {
        if entries.is_empty() || self.limit == 0 {
            return Ok(Vec::new());
        }

        let query = self.embedder.vector(message).await?;
        let scored = self.score(&query, entries).await?;
        Ok(pick(&scored, entries, self.limit))
    }

    /// Read the similarity of the message with every command.
    ///
    /// The commands are read in batches, so one large catalog does not
    /// become one large request to the reader.
    async fn score(
        &self,
        query: &[f32],
        entries: &[CommandEntry],
    ) -> Result<Vec<(f32, usize)>, ResolveError> {
        let mut scored: Vec<(f32, usize)> = Vec::with_capacity(entries.len());
        let mut base = 0;
        for chunk in entries.chunks(BATCH) {
            let texts: Vec<String> = chunk.iter().map(|entry| entry.line()).collect();
            let vectors = self.embedder.vectors(&texts).await?;
            for (offset, vector) in vectors.iter().enumerate() {
                scored.push((cosine(query, vector), base + offset));
            }
            base += chunk.len();
        }
        Ok(scored)
    }
}

/// Read the commands of the best scores, best first.
fn pick(scored: &[(f32, usize)], entries: &[CommandEntry], limit: usize) -> Vec<CommandEntry> {
    let mut ordered = scored.to_vec();
    ordered.sort_by(|left, right| right.0.total_cmp(&left.0));
    ordered.truncate(limit);
    ordered
        .into_iter()
        .filter_map(|(_, index)| entries.get(index).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalog_without_a_command_ranks_nothing() {
        assert!(pick(&[], &[], 5).is_empty());
    }

    #[test]
    fn the_best_command_comes_first() {
        let entries = vec![
            CommandEntry::new("curl", "transfer a URL"),
            CommandEntry::new("scp", "copy a file to a machine"),
            CommandEntry::new("ls", "list directory contents"),
        ];
        let scored = vec![(0.2, 0), (0.9, 1), (0.4, 2)];

        let best = pick(&scored, &entries, 2);
        assert_eq!(best[0].name, "scp");
        assert_eq!(best[1].name, "ls");
    }

    #[test]
    fn the_limit_keeps_only_the_best_commands() {
        let entries = vec![CommandEntry::new("curl", ""), CommandEntry::new("scp", "")];
        let scored = vec![(0.2, 0), (0.9, 1)];

        assert_eq!(pick(&scored, &entries, 1), vec![entries[1].clone()]);
    }
}
