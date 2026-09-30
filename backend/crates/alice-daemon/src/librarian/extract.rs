//! Prompt and parser that turn one turn into memory facts.
//!
//! The librarian reads one episode with its own model and answers with a
//! list of facts. The answer follows a JSON schema, so the daemon reads a
//! structure rather than prose. A small model often wraps the JSON in a
//! markdown fence, so the parser finds the object before it reads it.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::librarian::facts::{DEFAULT_CONFIDENCE, DEFAULT_IMPORTANCE};
use crate::resolver::answer::AnswerPrompt;

/// The name of the answer schema of the extraction request.
pub const EXTRACT_SCHEMA_NAME: &str = "memory_facts";

/// The system message of the extraction request.
///
/// Either half of a turn may teach something worth keeping: the user may
/// say a fact, and the answer of the assistant may state one the turn
/// turned up, for example the release the machine runs or the editor a
/// setting belongs to. What neither half may teach is talk that does not
/// stay true: small talk, a one-off question, a guess the assistant
/// offered as a possibility, and a credential, which the memory must
/// never hold because it repeats what it holds in a later prompt.
const SYSTEM_MESSAGE: &str = "You are the librarian of a personal assistant. Read one turn between a user and the assistant and write the durable facts it teaches about the user, the machine, or their projects. Either half of the turn may teach a fact: the user may say something worth keeping, and the assistant may state something durable in its answer, for example what it found on the machine or what a setting belongs to. Report only facts that stay true across sessions, for example a name, a preference, a location, a tool, or a project. Never report small talk, the weather, a one-off question, a guess the assistant offered as a possibility, anything the assistant says about itself or its own abilities, and never a password, a token, a key, or another credential. Use one lower_snake_case key per subject, for example `user`, `project_alice`, or `machine`, and reuse a key the memory already holds when the turn is about that concept. Use one lower_snake_case relation per fact, for example `lives_in`, `prefers`, or `uses`, and keep a value short: it names a thing rather than describes it. A fact that corrects an older fact is the same subject and relation with the new value. Rate every fact twice, from zero to one: `confidence` is how sure you are of it, where a fact the user stated outright is sure and a fact you inferred is not, and `importance` is how much it is worth, where a name, a location, or a standing preference is worth more than a small detail. Answer with one JSON object that follows the schema and nothing else.";

/// One fact the model read out of one turn.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtractedFact {
    /// The key of the node the fact belongs to.
    pub subject: String,
    /// The name the memory shows for that node.
    pub title: String,
    /// The relation the fact names.
    pub relation: String,
    /// The value the fact carries.
    pub value: String,
    /// How sure the reader was of the fact, from zero to one.
    pub confidence: f32,
    /// How much the fact is worth, from zero to one.
    pub importance: f32,
}

/// The shape of one answer of the extraction request.
#[derive(Debug, Deserialize)]
struct FactsShape {
    #[serde(default)]
    facts: Vec<FactShape>,
}

/// The shape of one fact of that answer.
#[derive(Debug, Deserialize)]
struct FactShape {
    #[serde(default)]
    subject: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    relation: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    confidence: Option<f32>,
    #[serde(default)]
    importance: Option<f32>,
}

/// Build the extraction prompt for one episode.
///
/// The prompt names the concepts the memory already holds, so the reader
/// reuses a key instead of inventing one: a reader that never sees the
/// memory names the same person `user` one turn and `flurin` the next, and
/// the memory then holds one thing twice. A memory without a concept names
/// nothing and the reader opens the subject itself.
pub fn build_extract_prompt(
    text: &str,
    reply: &str,
    intent: Option<&str>,
    known: &[(String, String)],
) -> AnswerPrompt {
    let intent_line = match intent {
        Some(name) => format!("The assistant matched the intent `{name}`.\n"),
        None => String::new(),
    };
    let known_line = if known.is_empty() {
        String::new()
    } else {
        let list = known
            .iter()
            .map(|(key, title)| format!("`{key}` ({title})"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "The memory already holds these concepts: {list}. Reuse the key of one of them when the turn is about that concept, and write a new key only for a concept the memory does not hold yet.\n"
        )
    };
    let user = format!(
        "{intent_line}{known_line}The user said:\n{text}\n\nThe assistant replied:\n{reply}\n\nWrite the durable facts this turn teaches."
    );
    AnswerPrompt {
        system: SYSTEM_MESSAGE.to_string(),
        user,
        // Every fact of an episode is a report the daemon reads field by
        // field, so the request always carries the schema of a fact.
        answer_schema: Some(facts_schema()),
    }
}

/// Read the facts out of one answer of the model.
///
/// An answer that holds no object, or an object that holds no facts,
/// teaches nothing and returns no fact.
pub fn parse_extraction(content: &str) -> Vec<ExtractedFact> {
    let Some(object) = first_object(content) else {
        return Vec::new();
    };
    let Ok(shape) = serde_json::from_str::<FactsShape>(&object) else {
        return Vec::new();
    };
    shape
        .facts
        .into_iter()
        .filter_map(|fact| {
            let subject = fact.subject.trim().to_lowercase();
            let relation = fact.relation.trim().to_lowercase();
            let value = fact.value.trim().to_string();
            if subject.is_empty() || relation.is_empty() || value.is_empty() {
                return None;
            }
            let title = if fact.title.trim().is_empty() {
                subject.replace('_', " ")
            } else {
                fact.title.trim().to_string()
            };
            Some(ExtractedFact {
                subject,
                title,
                relation,
                value,
                confidence: rating(fact.confidence, DEFAULT_CONFIDENCE),
                importance: rating(fact.importance, DEFAULT_IMPORTANCE),
            })
        })
        .collect()
}

/// Read one rating of the reader, or the default when it names none.
///
/// A model that answers outside the range still teaches a fact, so the
/// rating is read into the range rather than refused.
fn rating(rated: Option<f32>, fallback: f32) -> f32 {
    rated.unwrap_or(fallback).clamp(0.0, 1.0)
}

/// Read the first JSON object out of one answer.
///
/// The scan tracks the braces and the string state, so a brace inside a
/// value never ends the object early.
fn first_object(content: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let start = bytes.iter().position(|byte| *byte == b'{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(content[start..=start + offset].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// The JSON schema one extraction answer follows.
fn facts_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "facts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "subject": {"type": "string"},
                        "title": {"type": "string"},
                        "relation": {"type": "string"},
                        "value": {"type": "string"},
                        "confidence": {"type": "number"},
                        "importance": {"type": "number"}
                    },
                    "required": ["subject", "relation", "value"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["facts"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_answer_reads_its_facts() {
        let content = r#"{"facts":[{"subject":"user","title":"User","relation":"lives_in","value":"Berlin"}]}"#;
        let facts = parse_extraction(content);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].relation, "lives_in");
        assert_eq!(facts[0].value, "Berlin");
        assert_eq!(facts[0].title, "User");
    }

    #[test]
    fn a_fenced_answer_reads_its_facts() {
        let content =
            "Here is the object:\n```json\n{\"facts\":[{\"subject\":\"user\",\"relation\":\"uses\",\"value\":\"Nix\"}]}\n```";
        let facts = parse_extraction(content);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].value, "Nix");
    }

    #[test]
    fn an_answer_without_facts_reads_nothing() {
        assert!(parse_extraction("no object here").is_empty());
        assert!(parse_extraction("{\"facts\":[]}").is_empty());
        assert!(parse_extraction("{\"facts\":[{\"subject\":\"user\"}]}").is_empty());
    }

    #[test]
    fn both_halves_of_the_turn_reach_the_prompt() {
        let prompt = build_extract_prompt(
            "which editor do I use?",
            "You use Neovim, as your config shows.",
            None,
            &[],
        );

        assert!(prompt.user.contains("which editor do I use?"));
        assert!(prompt.user.contains("You use Neovim"));
        // The reader is told that the answer of the assistant may teach a
        // fact as well, because the durable half of a turn is often its
        // answer rather than its question.
        assert!(prompt.system.contains("durable in its answer"));
        // The memory repeats what it holds in a later prompt, so the
        // reader is told that a credential never becomes a fact.
        assert!(prompt.system.contains("never a password"));
    }

    #[test]
    fn the_keys_the_memory_holds_reach_the_prompt() {
        let known = vec![("user".to_string(), "Flurin".to_string())];
        let prompt = build_extract_prompt("I live in Singapore", "Noted.", None, &known);
        assert!(prompt.user.contains("`user` (Flurin)"));
    }

    #[test]
    fn a_rating_of_the_reader_reaches_the_fact() {
        let content = r#"{"facts":[{"subject":"user","relation":"lives_in","value":"Berlin","confidence":0.9,"importance":0.8}]}"#;
        let facts = parse_extraction(content);
        assert_eq!(facts[0].confidence, 0.9);
        assert_eq!(facts[0].importance, 0.8);
    }

    #[test]
    fn a_fact_without_a_rating_reads_as_the_default() {
        let content = r#"{"facts":[{"subject":"user","relation":"uses","value":"Nix"}]}"#;
        let facts = parse_extraction(content);
        assert_eq!(facts[0].confidence, DEFAULT_CONFIDENCE);
        assert_eq!(facts[0].importance, DEFAULT_IMPORTANCE);
    }

    #[test]
    fn a_rating_outside_the_range_reads_into_it() {
        let content = r#"{"facts":[{"subject":"user","relation":"uses","value":"Nix","confidence":3.0,"importance":-1.0}]}"#;
        let facts = parse_extraction(content);
        assert_eq!(facts[0].confidence, 1.0);
        assert_eq!(facts[0].importance, 0.0);
    }

    #[test]
    fn a_memory_without_a_concept_names_no_key() {
        let prompt = build_extract_prompt("hello", "hi", None, &[]);
        assert!(!prompt.user.contains("already holds"));
    }

    #[test]
    fn a_brace_inside_a_value_never_ends_the_object_early() {
        let content =
            r#"{"facts":[{"subject":"user","relation":"uses","value":"a {brace} tool"}]}"#;
        let facts = parse_extraction(content);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].value, "a {brace} tool");
    }
}
