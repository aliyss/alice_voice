//! Prompt that reads the entity values of one intent out of one message.
//!
//! The resolver first chooses one intent. This module then reads the
//! values that intent needs: a closed entity takes exactly one of the
//! values the user configured, a script entity takes one value of the
//! list its script answers with when that list is short enough to be a
//! choice, and an open entity takes the words the user said. One request
//! answers every entity of the intent, so the second model call stays one
//! call.

use std::collections::BTreeMap;

use alice_core::dto::{EntityKindDto, IntentDto, IntentEntityDto};
use serde_json::{json, Map, Value};

use crate::intent::values::offered_for;

/// The system message of a values request.
const SYSTEM_MESSAGE: &str = "You read the values of the entities of one command out of a message. A closed entity takes exactly one of the values it lists. An open entity takes only the words of the message that name its value, never the whole message. An entity the message does not name takes an empty string. You answer with one JSON object that holds every entity name and nothing else.";

/// The request that reads the values of the entities of one intent.
#[derive(Clone, Debug, PartialEq)]
pub struct ValuesPrompt {
    /// The system message.
    pub system: String,
    /// The user message with the state and the entities.
    pub user: String,
    /// The JSON schema the answer must follow.
    pub answer_schema: Value,
}

/// The largest text the resolver reads as the value of one entity.
///
/// A value names a thing. A model that runs out of ideas repeats the
/// message, and the daemon must not put a sentence into a command.
const MAX_VALUE_CHARS: usize = 60;

/// Read the entities of an intent that the resolver reads a value for.
///
/// A closed entity without values cannot offer a choice, so the resolver
/// reads it like an open one instead of failing the turn.
fn readable_entities(intent: &IntentDto) -> Vec<&IntentEntityDto> {
    intent.entities.iter().collect()
}

/// The values an entity offers the model as a choice.
///
/// A closed entity carries them and a short list of a script entity is
/// offered the same way. A longer list is not a choice but a span: the
/// model reads the words and the daemon reads the entry they name.
fn choices_of(entity: &IntentEntityDto, offered: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    match entity.kind {
        EntityKindDto::Closed => entity.values.clone(),
        _ => offered_for(entity, offered).to_vec(),
    }
}

/// Build the schema of the answer of one values request.
fn build_schema(intent: &IntentDto, offered: &BTreeMap<String, Vec<String>>) -> Value {
    let mut properties = Map::new();
    let mut required: Vec<Value> = Vec::new();

    for entity in readable_entities(intent) {
        let choices = choices_of(entity, offered);
        let property = if choices.is_empty() {
            json!({ "type": "string" })
        } else {
            json!({
                "type": "string",
                "enum": choices,
            })
        };
        properties.insert(entity.name.clone(), property);
        required.push(json!(entity.name));
    }

    json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
        "additionalProperties": false,
    })
}

/// Describe one entity to the model.
fn describe_entity(entity: &IntentEntityDto, offered: &BTreeMap<String, Vec<String>>) -> String {
    let choices = choices_of(entity, offered);
    if !choices.is_empty() {
        let kind = match entity.kind {
            EntityKindDto::Closed => "closed",
            _ => "list",
        };
        return format!(
            "- {} ({}): exactly one of {}",
            entity.name,
            kind,
            choices.join(", ")
        );
    }
    if matches!(entity.kind, EntityKindDto::Script) {
        return format!(
            "- {} (list): the name of one item of a catalog",
            entity.name
        );
    }
    format!("- {} (open): the words of the state", entity.name)
}

/// Build the prompt that reads the values of the entities of one intent.
///
/// The builder returns null when the intent has no entity, so an intent
/// that needs no value costs no second model call.
pub fn build_values_prompt(
    state: &str,
    intent: &IntentDto,
    offered: &BTreeMap<String, Vec<String>>,
) -> Option<ValuesPrompt> {
    if intent.entities.is_empty() {
        return None;
    }

    let mut lines = vec!["State:".to_string(), state.to_string(), String::new()];
    lines.push(format!("Command: {}", intent.command));
    lines.push(String::new());
    lines.push("Entities:".to_string());
    for entity in readable_entities(intent) {
        lines.push(describe_entity(entity, offered));
    }
    lines.push(String::new());
    lines.push(
        "Answer with one JSON object that holds every entity name. Use an empty string for an entity the state does not name."
            .to_string(),
    );

    Some(ValuesPrompt {
        system: SYSTEM_MESSAGE.to_string(),
        user: lines.join("\n"),
        answer_schema: build_schema(intent, offered),
    })
}

/// Whether a text reads as the value of one entity.
///
/// A value names a thing. A model that runs out of ideas repeats the
/// message, and a sentence is no value either, so both stay out. The built
/// in GLiNER resolver applies the same guard to the spans it reads.
pub fn reads_as_value(value: &str, message: &str) -> bool {
    let value = value.trim();
    !value.is_empty()
        && value.chars().count() <= MAX_VALUE_CHARS
        && normalize(value) != normalize(message)
}

/// Normalize a text to compare a value with the message it came from.
fn normalize(text: &str) -> String {
    text.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_alphanumeric())
        .to_string()
}

/// Read the values of the entities out of the answer of the model.
///
/// A value the model did not answer, or answered with words of its own,
/// stays out of the map. The renderer of the command then reports the
/// entity as unreadable instead of running a command with a guess. The
/// message the answer came from is the last guard: an answer that only
/// repeats it holds no value.
pub fn parse_values(
    content: &str,
    intent: &IntentDto,
    message: &str,
    offered: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    let Some(Value::Object(answer)) = serde_json::from_str::<Value>(content).ok() else {
        return values;
    };

    for entity in readable_entities(intent) {
        let Some(Value::String(value)) = answer.get(&entity.name) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        let choices = choices_of(entity, offered);
        if !choices.is_empty() {
            let allowed = choices
                .iter()
                .find(|allowed| allowed.eq_ignore_ascii_case(value));
            if let Some(allowed) = allowed {
                values.insert(entity.name.clone(), allowed.clone());
            }
            continue;
        }
        if !reads_as_value(value, message) {
            continue;
        }
        values.insert(entity.name.clone(), value.to_string());
    }

    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::dto::{IntentDto, IntentEntityDto};
    use chrono::Utc;
    use uuid::Uuid;

    /// Build one intent for the tests.
    fn intent(entities: Vec<IntentEntityDto>) -> IntentDto {
        IntentDto {
            id: Uuid::new_v4(),
            name: "get weather".to_string(),
            description: "Read the weather of one city.".to_string(),
            command: r#"curl -s "wttr.in/{city}?format=3""#.to_string(),
            entities,
            examples: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Build the offered values of one script entity for the tests.
    fn offered(values: &[&str]) -> BTreeMap<String, Vec<String>> {
        let mut map = BTreeMap::new();
        map.insert(
            "applications".to_string(),
            values.iter().map(|value| value.to_string()).collect(),
        );
        map
    }

    /// Build one entity for the tests.
    fn entity(name: &str, kind: EntityKindDto, values: &[&str]) -> IntentEntityDto {
        IntentEntityDto {
            id: Uuid::new_v4(),
            name: name.to_string(),
            kind,
            values: values.iter().map(|value| value.to_string()).collect(),
            script: matches!(kind, EntityKindDto::Script).then(|| "ls".to_string()),
            required: true,
        }
    }

    #[test]
    fn build_values_prompt_returns_none_without_an_entity() {
        assert!(build_values_prompt("hello", &intent(Vec::new()), &BTreeMap::new()).is_none());
    }

    #[test]
    fn build_values_prompt_limits_a_closed_entity_to_its_values() {
        let prompt = build_values_prompt(
            "user: the weather tomorrow",
            &intent(vec![entity(
                "when",
                EntityKindDto::Closed,
                &["today", "tomorrow"],
            )]),
            &BTreeMap::new(),
        )
        .expect("the intent has an entity");

        let allowed = &prompt.answer_schema["properties"]["when"]["enum"];
        assert_eq!(allowed, &json!(["today", "tomorrow"]));
        assert_eq!(prompt.answer_schema["required"], json!(["when"]));
    }

    #[test]
    fn build_values_prompt_offers_a_short_list_of_a_script_entity() {
        let prompt = build_values_prompt(
            "user: open obs",
            &intent(vec![entity("applications", EntityKindDto::Script, &[])]),
            &offered(&["firefox", "obs"]),
        )
        .expect("the intent has an entity");

        assert_eq!(
            prompt.answer_schema["properties"]["applications"]["enum"],
            json!(["firefox", "obs"])
        );
        assert!(prompt
            .user
            .contains("applications (list): exactly one of firefox, obs"));
    }

    #[test]
    fn build_values_prompt_reads_a_long_list_as_a_name() {
        let prompt = build_values_prompt(
            "user: open obs",
            &intent(vec![entity("applications", EntityKindDto::Script, &[])]),
            &BTreeMap::new(),
        )
        .expect("the intent has an entity");

        assert_eq!(
            prompt.answer_schema["properties"]["applications"],
            json!({ "type": "string" })
        );
        assert!(prompt.user.contains("applications (list)"));
    }

    #[test]
    fn parse_values_keeps_the_spelling_of_the_list() {
        let catalog = intent(vec![entity("applications", EntityKindDto::Script, &[])]);

        let values = parse_values(
            r#"{"applications":"OBS"}"#,
            &catalog,
            "open obs",
            &offered(&["firefox", "obs"]),
        );

        assert_eq!(values["applications"], "obs");
    }

    #[test]
    fn parse_values_drops_a_value_the_list_does_not_hold() {
        let catalog = intent(vec![entity("applications", EntityKindDto::Script, &[])]);

        let values = parse_values(
            r#"{"applications":"vscodium"}"#,
            &catalog,
            "open vscodium",
            &offered(&["firefox", "obs"]),
        );

        assert!(values.is_empty());
    }

    #[test]
    fn build_values_prompt_reads_an_open_entity_as_text() {
        let prompt = build_values_prompt(
            "user: the weather in Berlin",
            &intent(vec![entity("city", EntityKindDto::Open, &[])]),
            &BTreeMap::new(),
        )
        .expect("the intent has an entity");

        assert_eq!(
            prompt.answer_schema["properties"]["city"],
            json!({ "type": "string" })
        );
        assert!(prompt.user.contains("- city (open)"));
        assert!(prompt.user.contains("the weather in Berlin"));
    }

    #[test]
    fn parse_values_reads_the_answer() {
        let catalog = intent(vec![
            entity("city", EntityKindDto::Open, &[]),
            entity("when", EntityKindDto::Closed, &["today", "tomorrow"]),
        ]);

        let values = parse_values(
            r#"{"city":"Berlin","when":"tomorrow"}"#,
            &catalog,
            "the weather in Berlin tomorrow",
            &BTreeMap::new(),
        );

        assert_eq!(values["city"], "Berlin");
        assert_eq!(values["when"], "tomorrow");
    }

    #[test]
    fn parse_values_matches_a_closed_value_without_case() {
        let catalog = intent(vec![entity(
            "when",
            EntityKindDto::Closed,
            &["today", "tomorrow"],
        )]);

        let values = parse_values(
            r#"{"when":"Tomorrow"}"#,
            &catalog,
            "the weather",
            &BTreeMap::new(),
        );

        assert_eq!(values["when"], "tomorrow");
    }

    #[test]
    fn parse_values_drops_a_value_a_closed_entity_does_not_offer() {
        let catalog = intent(vec![entity(
            "when",
            EntityKindDto::Closed,
            &["today", "tomorrow"],
        )]);

        assert!(parse_values(
            r#"{"when":"next week"}"#,
            &catalog,
            "the weather",
            &BTreeMap::new()
        )
        .is_empty());
    }

    #[test]
    fn parse_values_drops_an_empty_value() {
        let catalog = intent(vec![entity("city", EntityKindDto::Open, &[])]);

        assert!(parse_values(
            r#"{"city":"   "}"#,
            &catalog,
            "the weather in Berlin",
            &BTreeMap::new()
        )
        .is_empty());
    }

    #[test]
    fn parse_values_drops_an_open_value_that_repeats_the_message() {
        let catalog = intent(vec![entity("city", EntityKindDto::Open, &[])]);
        let message = "is it raining somewhere";

        let answer = format!(r#"{{"city":"{message}"}}"#);

        assert!(parse_values(&answer, &catalog, message, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn parse_values_drops_an_open_value_that_is_a_sentence() {
        let catalog = intent(vec![entity("city", EntityKindDto::Open, &[])]);
        let answer = format!(r#"{{"city":"{}"}}"#, "a".repeat(MAX_VALUE_CHARS + 1));

        assert!(parse_values(&answer, &catalog, "the weather", &BTreeMap::new()).is_empty());
    }

    #[test]
    fn parse_values_reads_an_answer_that_is_not_json_as_nothing() {
        let catalog = intent(vec![entity("city", EntityKindDto::Open, &[])]);

        assert!(parse_values(
            "Berlin",
            &catalog,
            "the weather in Berlin",
            &BTreeMap::new()
        )
        .is_empty());
    }

    #[test]
    fn reads_as_value_names_a_thing_and_nothing_else() {
        assert!(reads_as_value("Berlin", "the weather in Berlin"));
        assert!(!reads_as_value("  ", "the weather in Berlin"));
        assert!(!reads_as_value(
            "the weather in Berlin",
            "the weather in Berlin"
        ));
        assert!(!reads_as_value(
            &"a".repeat(MAX_VALUE_CHARS + 1),
            "the weather"
        ));
    }
}
