//! Command template of an intent.
//!
//! The command of an intent names the entities of that intent with a
//! placeholder: `{city}`. The daemon reads the value of the entity out of
//! the message and puts it into the command before the command runs. A
//! placeholder the daemon cannot fill stops the run, so a command never
//! runs half written.
//!
//! A value comes from the model, so the renderer escapes it. The escape
//! targets the double quoted case, because that is where a placeholder
//! usually sits, and it is also correct without quotes. A value that
//! could end the command and start another one is refused instead of
//! escaped, because no escape is correct in both cases.
//!
//! An entity is required or optional. A required entity that reaches the
//! renderer without a value stops the run, so a command never runs half
//! written. The caller removes the placeholders of the optional entities
//! it read no value for with `strip_placeholders`, so the command runs as
//! if the user had left that part out.

use std::collections::BTreeMap;

/// The character that opens a placeholder.
const OPEN: char = '{';

/// The character that closes a placeholder.
const CLOSE: char = '}';

/// The characters the shell reads inside double quotes.
const SHELL_SPECIAL: [char; 4] = ['\\', '"', '$', '`'];

/// The characters that end one command and start another.
const COMMAND_CHAIN: [char; 6] = [';', '|', '&', '<', '>', '\n'];

/// What the renderer made of one command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedCommand {
    /// The command with the values of the entities in place.
    pub command: String,
    /// The entities the renderer could not read a value for.
    pub unreadable: Vec<String>,
}

/// Escape one value for the shell, or null when the shell must not read it.
///
/// The renderer cannot know whether the user wrapped the placeholder in
/// quotes. The escape of the four characters the shell reads inside
/// double quotes is correct in both cases, so it covers the quoted and
/// the bare placeholder. A carriage return becomes a space and a line
/// break refuses the value, because a second line would be a second
/// command.
fn escape_value(value: &str) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if COMMAND_CHAIN.contains(&character) {
            return None;
        }
        if character == '\r' {
            escaped.push(' ');
            continue;
        }
        if SHELL_SPECIAL.contains(&character) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    Some(escaped)
}

/// Whether one character is the whitespace between two parts of a command.
fn is_gap(character: char) -> bool {
    character == ' ' || character == '\t'
}

/// Remove every placeholder that names one of the given entities.
///
/// The caller passes the entities it read no value for and that the
/// intent does not need, so the command runs as if the user had left that
/// part out. The placeholder goes, a quote pair that held the placeholder
/// alone goes with it, and the whitespace the placeholder sat in goes too,
/// so `--when {when}` does not become `--when ` or `--when ""`.
pub fn strip_placeholders(command: &str, names: &[String]) -> String {
    if names.is_empty() {
        return command.to_string();
    }

    let mut rendered = String::with_capacity(command.len());
    let mut rest = command;

    while let Some(start) = rest.find(OPEN) {
        let after_open = &rest[start + OPEN.len_utf8()..];
        let Some(end) = after_open.find(CLOSE) else {
            break;
        };
        let group_end = start + OPEN.len_utf8() + end + CLOSE.len_utf8();
        let name = after_open[..end].trim();

        if !names.iter().any(|found| found == name) {
            rendered.push_str(&rest[..group_end]);
            rest = &rest[group_end..];
            continue;
        }

        rendered.push_str(&rest[..start]);
        let mut tail = &rest[group_end..];

        for quote in ['"', '\''] {
            if rendered.ends_with(quote) && tail.starts_with(quote) {
                rendered.pop();
                tail = &tail[quote.len_utf8()..];
                break;
            }
        }

        if rendered.ends_with(is_gap) {
            while rendered.ends_with(is_gap) {
                rendered.pop();
            }
        } else {
            tail = tail.trim_start_matches(is_gap);
        }
        rest = tail;
    }
    rendered.push_str(rest);
    rendered
}

/// Render the command of an intent with the values of its entities.
///
/// A brace group is a placeholder only when it names an entity of the
/// intent. Every other brace stays as the user wrote it, so a command may
/// carry its own braces, as `awk '{print $1}'` does.
pub fn render_command(
    command: &str,
    entities: &[String],
    values: &BTreeMap<String, String>,
) -> RenderedCommand {
    let mut rendered = String::with_capacity(command.len());
    let mut unreadable: Vec<String> = Vec::new();
    let mut rest = command;

    while let Some(start) = rest.find(OPEN) {
        rendered.push_str(&rest[..start]);
        let after_open = &rest[start + OPEN.len_utf8()..];
        let Some(end) = after_open.find(CLOSE) else {
            rendered.push(OPEN);
            rest = after_open;
            continue;
        };

        let name = after_open[..end].trim();
        rest = &after_open[end + CLOSE.len_utf8()..];

        if !entities.iter().any(|entity| entity == name) {
            rendered.push(OPEN);
            rendered.push_str(&after_open[..end]);
            rendered.push(CLOSE);
            continue;
        }

        match values.get(name).and_then(|value| escape_value(value)) {
            Some(value) => rendered.push_str(&value),
            None => {
                if !unreadable.iter().any(|found| found == name) {
                    unreadable.push(name.to_string());
                }
                rendered.push(OPEN);
                rendered.push_str(&after_open[..end]);
                rendered.push(CLOSE);
            }
        }
    }
    rendered.push_str(rest);

    RenderedCommand {
        command: rendered,
        unreadable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the entity list of the tests.
    fn entities() -> Vec<String> {
        vec!["city".to_string(), "when".to_string()]
    }

    /// Build one value map for the tests.
    fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn render_command_puts_the_value_of_an_entity_in_place() {
        let rendered = render_command(
            r#"curl -s "wttr.in/{city}?format=3""#,
            &entities(),
            &values(&[("city", "Berlin")]),
        );

        assert_eq!(rendered.command, r#"curl -s "wttr.in/Berlin?format=3""#);
        assert!(rendered.unreadable.is_empty());
    }

    #[test]
    fn render_command_replaces_every_placeholder() {
        let rendered = render_command(
            "echo {city} {when}",
            &entities(),
            &values(&[("city", "Berlin"), ("when", "today")]),
        );

        assert_eq!(rendered.command, "echo Berlin today");
    }

    #[test]
    fn render_command_reports_an_entity_without_a_value() {
        let rendered = render_command(
            "curl wttr.in/{city}",
            &entities(),
            &values(&[("when", "today")]),
        );

        assert_eq!(rendered.unreadable, vec!["city"]);
    }

    #[test]
    fn render_command_keeps_a_brace_that_names_no_entity() {
        let rendered = render_command("awk '{print $1}'", &entities(), &values(&[]));

        assert_eq!(rendered.command, "awk '{print $1}'");
        assert!(rendered.unreadable.is_empty());
    }

    #[test]
    fn render_command_escapes_a_quote_inside_a_value() {
        let rendered = render_command(
            r#"echo "{city}""#,
            &entities(),
            &values(&[("city", "a\"b")]),
        );

        assert_eq!(rendered.command, r#"echo "a\"b""#);
        assert!(rendered.unreadable.is_empty());
    }

    #[test]
    fn render_command_escapes_a_variable_inside_a_value() {
        let rendered = render_command("echo {city}", &entities(), &values(&[("city", "$HOME")]));

        assert_eq!(rendered.command, r"echo \$HOME");
    }

    #[test]
    fn render_command_refuses_a_value_that_chains_a_command() {
        let rendered = render_command(
            "curl wttr.in/{city}",
            &entities(),
            &values(&[("city", "Berlin; rm -rf ~")]),
        );

        assert_eq!(rendered.unreadable, vec!["city"]);
        assert_eq!(rendered.command, "curl wttr.in/{city}");
    }

    #[test]
    fn render_command_refuses_a_value_with_a_line_break() {
        let rendered = render_command(
            "echo {city}",
            &entities(),
            &values(&[("city", "Berlin\nrm -rf ~")]),
        );

        assert_eq!(rendered.unreadable, vec!["city"]);
    }

    #[test]
    fn render_command_refuses_an_empty_value() {
        let rendered = render_command("echo {city}", &entities(), &values(&[("city", "")]));

        assert_eq!(rendered.unreadable, vec!["city"]);
    }

    #[test]
    fn render_command_leaves_a_command_without_a_placeholder() {
        let rendered = render_command("echo hello", &entities(), &values(&[]));

        assert_eq!(rendered.command, "echo hello");
        assert!(rendered.unreadable.is_empty());
    }

    #[test]
    fn render_command_reports_one_unreadable_entity_once() {
        let rendered = render_command(
            "echo {city} {city}",
            &entities(),
            &values(&[("when", "today")]),
        );

        assert_eq!(rendered.unreadable, vec!["city"]);
    }

    #[test]
    fn strip_placeholders_drops_a_placeholder_and_the_whitespace_before_it() {
        let found = strip_placeholders("echo --when {when}", &["when".to_string()]);
        assert_eq!(found, "echo --when");
    }

    #[test]
    fn strip_placeholders_drops_a_quoted_placeholder_with_its_quotes() {
        let found = strip_placeholders(r#"curl wttr.in/{city}"#, &["city".to_string()]);
        assert_eq!(found, "curl wttr.in/");

        let quoted = strip_placeholders(r#"echo "{city}""#, &["city".to_string()]);
        assert_eq!(quoted, "echo");
    }

    #[test]
    fn strip_placeholders_drops_the_whitespace_after_a_leading_placeholder() {
        let found = strip_placeholders("{city} --when today", &["city".to_string()]);
        assert_eq!(found, "--when today");
    }

    #[test]
    fn strip_placeholders_keeps_a_placeholder_the_entity_does_not_name() {
        let found = strip_placeholders("echo {city} {when}", &["when".to_string()]);
        assert_eq!(found, "echo {city}");
    }

    #[test]
    fn strip_placeholders_leaves_a_brace_that_names_no_entity() {
        let found = strip_placeholders("awk '{print $1}'", &["when".to_string()]);
        assert_eq!(found, "awk '{print $1}'");
    }

    #[test]
    fn strip_placeholders_does_nothing_without_a_name() {
        assert_eq!(strip_placeholders("echo {city}", &[]), "echo {city}");
    }
}
