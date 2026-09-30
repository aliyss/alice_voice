//! Commands of the machine, read from the completions of fish.
//!
//! Fish knows the commands on the path of the user and carries a short
//! sentence for many of them. One call to `complete -C ""` lists those
//! commands with their names and sentences, so the catalog reads the
//! commands from that call rather than from a walk of the path.

use std::collections::BTreeMap;
use std::process::Command;

use crate::catalog::entry::CommandEntry;
use crate::catalog::error::CatalogError;

/// The program the daemon asks for the commands.
const FISH: &str = "fish";

/// The fish program that lists the completions of an empty command line.
const LIST: &str = r#"complete -C """#;

/// The words fish writes for the kind of an entry, not for what it does.
///
/// Fish marks a name with the kind of thing it is, so a description of
/// `command` says that the name runs a program and says nothing about the
/// program. Such a word is no summary, so the catalog drops it.
const KIND_WORDS: [&str; 6] = [
    "command",
    "command link",
    "function",
    "builtin",
    "directory",
    "variable",
];

/// Read the commands fish offers.
///
/// The daemon runs one fish program and reads its output. A fish that is
/// not installed, or that does not answer, fails with a reason the caller
/// can report.
pub fn fish_entries() -> Result<Vec<CommandEntry>, CatalogError> {
    let output = Command::new(FISH)
        .arg("-c")
        .arg(LIST)
        .output()
        .map_err(|err| CatalogError::Unavailable {
            tool: FISH.to_string(),
            reason: err.to_string(),
        })?;

    if !output.status.success() {
        return Err(CatalogError::Unavailable {
            tool: FISH.to_string(),
            reason: format!("the command exited with {}", output.status),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_fish(&text))
}

/// Read one entry out of every line of the completion list.
///
/// A line holds a name and, after one tab, a description. A line without
/// a tab carries the name alone. The builder keeps one entry per name and
/// sorts the entries by name, so a repeated name does not list twice.
pub fn parse_fish(output: &str) -> Vec<CommandEntry> {
    let mut entries: BTreeMap<String, CommandEntry> = BTreeMap::new();

    for line in output.lines() {
        let mut parts = line.splitn(2, '\t');
        let name = parts.next().unwrap_or("").trim();
        if name.is_empty() {
            continue;
        }
        let summary = clean_summary(parts.next().unwrap_or(""));
        entries
            .entry(name.to_string())
            .or_insert_with(|| CommandEntry::new(name, &summary));
    }

    entries.into_values().collect()
}

/// Drop the word fish writes for the kind of an entry.
///
/// A kind word is not a sentence about the command, so the catalog reads
/// it as no summary. The manual pages fill the summary of such a command.
fn clean_summary(summary: &str) -> String {
    let summary = summary.trim();
    if KIND_WORDS.contains(&summary) {
        return String::new();
    }
    summary.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_with_a_description_reads_as_one_entry() {
        let entries = parse_fish("curl\ttransfer a URL\n");
        assert_eq!(entries, vec![CommandEntry::new("curl", "transfer a URL")]);
    }

    #[test]
    fn a_line_without_a_description_carries_the_name_alone() {
        let entries = parse_fish("alias\nabbr\n");
        assert_eq!(
            entries,
            vec![
                CommandEntry::new("abbr", ""),
                CommandEntry::new("alias", "")
            ]
        );
    }

    #[test]
    fn the_kind_word_of_fish_is_no_summary() {
        assert_eq!(
            parse_fish("abook\tcommand link\n"),
            vec![CommandEntry::new("abook", "")]
        );
        assert_eq!(
            parse_fish("foo\tcommand\n"),
            vec![CommandEntry::new("foo", "")]
        );
    }

    #[test]
    fn a_repeated_name_lists_once() {
        let entries = parse_fish("curl\tfirst\ncurl\tsecond\n");
        assert_eq!(entries, vec![CommandEntry::new("curl", "first")]);
    }

    #[test]
    fn an_empty_line_reads_as_no_entry() {
        assert!(parse_fish("\n   \n").is_empty());
    }
}
