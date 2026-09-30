//! One command of the machine.
//!
//! The catalog hands the model the commands a machine can run, so an
//! entry carries the name to call and the sentence that says what the
//! command does. The name is what runs and the summary is what the model
//! reads to choose.

/// One command the machine can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEntry {
    /// The command name, for example `curl`.
    pub name: String,
    /// One sentence about the command, or empty when the daemon read none.
    pub summary: String,
}

impl CommandEntry {
    /// Create one entry from a name and a summary.
    ///
    /// The builder trims both fields, so a caller may pass the raw text
    /// of a tool without cleaning it first.
    pub fn new(name: &str, summary: &str) -> Self {
        Self {
            name: name.trim().to_string(),
            summary: summary.trim().to_string(),
        }
    }

    /// The line the model reads for this command.
    ///
    /// A command without a summary reads as its name alone, so the list
    /// does not carry an empty separator.
    pub fn line(&self) -> String {
        if self.summary.is_empty() {
            self.name.clone()
        } else {
            format!("{}: {}", self.name, self.summary)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_without_a_summary_is_the_name() {
        assert_eq!(CommandEntry::new("curl", "").line(), "curl");
        assert_eq!(CommandEntry::new("  curl  ", "  ").line(), "curl");
    }

    #[test]
    fn a_line_with_a_summary_names_the_command_and_the_summary() {
        let entry = CommandEntry::new("curl", " transfer a URL ");
        assert_eq!(entry.line(), "curl: transfer a URL");
    }
}
