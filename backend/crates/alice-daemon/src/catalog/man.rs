//! One sentence about a command, read from the manual pages.
//!
//! The completions of fish name the commands and mark the kind of each
//! one, so a name without a sentence still needs a sentence. The manual
//! pages hold that sentence in their name line, so the catalog reads
//! every name line once with `man -k .` and joins it to the fish list.

use std::collections::BTreeMap;
use std::process::Command;

use crate::catalog::error::CatalogError;

/// The program the daemon asks for the manual pages.
const MAN: &str = "man";

/// The flag that lists every name line in the manual index.
const LIST: &str = "-k";

/// The query that matches every name line.
const QUERY: &str = ".";

/// Read one sentence about every documented command.
///
/// The daemon runs one `man` program and reads its output. A machine
/// without a manual index fails with a reason the caller can report, and
/// the catalog then keeps the sentences of the fish list.
pub fn man_summaries() -> Result<BTreeMap<String, String>, CatalogError> {
    let output = Command::new(MAN)
        .arg(LIST)
        .arg(QUERY)
        .output()
        .map_err(|err| CatalogError::Unavailable {
            tool: MAN.to_string(),
            reason: err.to_string(),
        })?;

    if !output.status.success() {
        return Err(CatalogError::Unavailable {
            tool: MAN.to_string(),
            reason: format!("the command exited with {}", output.status),
        });
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_man(&text))
}

/// Read the name and the sentence out of every name line.
///
/// A name line reads `name (section) - sentence`. One name appears in
/// more than one section, so the builder keeps the sentence of the
/// section a reader reaches first: the commands of the user, then the
/// commands of the administrator, then the rest.
pub fn parse_man(output: &str) -> BTreeMap<String, String> {
    let mut best: BTreeMap<String, (u8, String)> = BTreeMap::new();

    for line in output.lines() {
        let Some((left, summary)) = line.split_once(" - ") else {
            continue;
        };
        let Some((name, section)) = split_name(left) else {
            continue;
        };
        let summary = summary.trim();
        if summary.is_empty() {
            continue;
        }

        let rank = section_rank(section);
        let held = best.get(name);
        if held.is_none_or(|(held_rank, _)| rank < *held_rank) {
            best.insert(name.to_string(), (rank, summary.to_string()));
        }
    }

    best.into_iter()
        .map(|(name, (_, summary))| (name, summary))
        .collect()
}

/// Split the left half of a name line into the name and the section.
fn split_name(left: &str) -> Option<(&str, &str)> {
    let open = left.find('(')?;
    let close = left.find(')')?;
    if close <= open {
        return None;
    }
    let name = left[..open].trim();
    let section = left[open + 1..close].trim();
    if name.is_empty() || section.is_empty() {
        return None;
    }
    Some((name, section))
}

/// How early a reader reaches one section of the manual.
///
/// A lower rank wins, so the section of the user commands beats the one
/// of the administrator commands and both beat the rest.
fn section_rank(section: &str) -> u8 {
    if section.starts_with('1') {
        0
    } else if section.starts_with('8') {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_line_reads_as_one_summary() {
        let summaries = parse_man("ls (1)               - list directory contents\n");
        assert_eq!(
            summaries.get("ls"),
            Some(&"list directory contents".to_string())
        );
    }

    #[test]
    fn the_section_of_the_user_wins_over_another_section() {
        let summaries = parse_man("open (3) - open a file\nopen (1) - start a program\n");
        assert_eq!(summaries.get("open"), Some(&"start a program".to_string()));
    }

    #[test]
    fn the_first_section_of_a_name_wins_over_the_next_one() {
        let summaries = parse_man("open (3) - first\nopen (3) - second\n");
        assert_eq!(summaries.get("open"), Some(&"first".to_string()));
    }

    #[test]
    fn a_line_without_a_sentence_reads_as_no_summary() {
        assert!(parse_man("not a name line\n").is_empty());
        assert!(parse_man("ls (1) - \n").is_empty());
    }

    #[test]
    fn a_rank_reads_the_section_of_the_user_first() {
        assert!(section_rank("1") < section_rank("8"));
        assert!(section_rank("8") < section_rank("3"));
    }
}
