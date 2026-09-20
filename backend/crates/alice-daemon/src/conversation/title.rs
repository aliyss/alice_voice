//! Title generation of a conversation.
//! This module builds the title from the first message of the conversation.

/// The longest title the daemon generates.
const MAX_TITLE_LEN: usize = 60;

/// The title of a conversation that has no text to read.
pub const UNTITLED_CONVERSATION: &str = "New conversation";

/// Build a conversation title from the first message.
///
/// The title uses the first line of the message. Runs of whitespace become
/// one space. A long line is cut after `MAX_TITLE_LEN` characters and ends
/// with an ellipsis. A blank message returns `UNTITLED_CONVERSATION`.
pub fn build_title(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or("");
    let collapsed = first_line.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return UNTITLED_CONVERSATION.to_string();
    }
    cut_to_length(&collapsed)
}

/// Cut a text to `MAX_TITLE_LEN` characters and mark the cut.
fn cut_to_length(text: &str) -> String {
    if text.chars().count() <= MAX_TITLE_LEN {
        return text.to_string();
    }
    let kept: String = text.chars().take(MAX_TITLE_LEN).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_title_uses_the_first_line_only() {
        let title = build_title("Turn on the light\nand close the door");
        assert_eq!(title, "Turn on the light");
    }

    #[test]
    fn build_title_collapses_runs_of_whitespace() {
        let title = build_title("  set   a   timer  ");
        assert_eq!(title, "set a timer");
    }

    #[test]
    fn build_title_cuts_a_long_line() {
        let text = "a".repeat(MAX_TITLE_LEN + 10);
        let title = build_title(&text);
        assert_eq!(title.chars().count(), MAX_TITLE_LEN + 1);
        assert!(title.ends_with('…'));
    }

    #[test]
    fn build_title_returns_the_fallback_for_blank_text() {
        assert_eq!(build_title("   \n\n  "), UNTITLED_CONVERSATION);
        assert_eq!(build_title(""), UNTITLED_CONVERSATION);
    }
}
