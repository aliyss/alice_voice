//! Input of an intent write action.
//! This module normalizes and checks the values the caller sends.

use alice_core::dto::EntityKindDto;

use crate::intent::error::{invalid, SaveIntentError};

/// The longest name of an intent.
pub const MAX_NAME_LEN: usize = 60;

/// The longest description of an intent.
pub const MAX_DESCRIPTION_LEN: usize = 240;

/// The longest command of an intent.
pub const MAX_COMMAND_LEN: usize = 2000;

/// The largest number of entities of one intent.
pub const MAX_ENTITY_COUNT: usize = 8;

/// The longest name of an entity.
pub const MAX_ENTITY_NAME_LEN: usize = 40;

/// The largest number of values of one closed entity.
pub const MAX_ENTITY_VALUE_COUNT: usize = 32;

/// The longest script of one script entity.
pub const MAX_ENTITY_SCRIPT_LEN: usize = 2000;

/// The largest number of phrases of one intent.
pub const MAX_EXAMPLE_COUNT: usize = 12;

/// The longest phrase of one intent.
pub const MAX_EXAMPLE_LEN: usize = 80;

/// The longest value of a closed entity.
pub const MAX_ENTITY_VALUE_LEN: usize = 80;

/// One entity that the caller writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityInput {
    /// Name of the entity, for example `city`.
    pub name: String,
    /// Whether the entity is open, closed, or script.
    pub kind: EntityKindDto,
    /// Values of a closed entity. An open entity has none.
    pub values: Vec<String>,
    /// Shell command that provides the values of a script entity.
    pub script: Option<String>,
    /// Whether the intent needs a value for this entity.
    pub required: bool,
}

/// One intent that the caller writes.
#[derive(Clone, Debug)]
pub struct IntentInput {
    /// Unique name of the intent.
    pub name: String,
    /// What the intent does, in one sentence.
    pub description: String,
    /// Shell command the daemon runs for this intent.
    pub command: String,
    /// The entities the intent reads from the message.
    pub entities: Vec<EntityInput>,
    /// Phrases a user may say for this intent.
    pub examples: Vec<String>,
}

/// Remove the whitespace around a text.
fn trim(text: &str) -> String {
    text.trim().to_string()
}

/// Check the length of one text.
fn check_len(text: &str, max: usize, what: &str) -> Result<(), SaveIntentError> {
    if text.chars().count() > max {
        return Err(invalid(format!("{what} is longer than {max} characters.")));
    }
    Ok(())
}

impl EntityInput {
    /// Normalize the entity and check its rules.
    ///
    /// An open entity keeps no values, because the resolver reads them from
    /// the message later. A script entity keeps no values either: the
    /// daemon runs its script for the list it reads from.
    pub fn normalize(self) -> Result<Self, SaveIntentError> {
        let name = trim(&self.name).to_lowercase();
        if name.is_empty() {
            return Err(invalid("An entity has no name."));
        }
        check_len(&name, MAX_ENTITY_NAME_LEN, "An entity name")?;

        let values = match self.kind {
            EntityKindDto::Closed => normalize_values(self.values)?,
            _ => Vec::new(),
        };
        let script = match self.kind {
            EntityKindDto::Script => Some(normalize_script(self.script)?),
            _ => None,
        };

        Ok(Self {
            name,
            kind: self.kind,
            values,
            script,
            required: self.required,
        })
    }
}

/// Normalize the script of a script entity and check its rules.
fn normalize_script(script: Option<String>) -> Result<String, SaveIntentError> {
    let script = script.map_or_else(String::new, |script| trim(&script));
    if script.is_empty() {
        return Err(invalid("A script entity needs a script."));
    }
    check_len(&script, MAX_ENTITY_SCRIPT_LEN, "The script")?;
    Ok(script)
}

/// Normalize the phrases of one intent and check their rules.
fn normalize_examples(examples: Vec<String>) -> Result<Vec<String>, SaveIntentError> {
    let mut kept: Vec<String> = Vec::new();
    for example in examples {
        let example = trim(&example);
        if example.is_empty() || kept.iter().any(|found| found == &example) {
            continue;
        }
        check_len(&example, MAX_EXAMPLE_LEN, "A phrase of an intent")?;
        kept.push(example);
    }
    if kept.len() > MAX_EXAMPLE_COUNT {
        return Err(invalid(format!(
            "An intent takes at most {MAX_EXAMPLE_COUNT} phrases."
        )));
    }
    Ok(kept)
}

/// Normalize the values of a closed entity and check its rules.
fn normalize_values(values: Vec<String>) -> Result<Vec<String>, SaveIntentError> {
    let mut kept: Vec<String> = Vec::new();
    for value in values {
        let value = trim(&value);
        if value.is_empty() || kept.contains(&value) {
            continue;
        }
        check_len(&value, MAX_ENTITY_VALUE_LEN, "An entity value")?;
        kept.push(value);
    }
    if kept.is_empty() {
        return Err(invalid("A closed entity needs at least one value."));
    }
    if kept.len() > MAX_ENTITY_VALUE_COUNT {
        return Err(invalid(format!(
            "A closed entity takes at most {MAX_ENTITY_VALUE_COUNT} values."
        )));
    }
    Ok(kept)
}

impl IntentInput {
    /// Normalize the intent and check its rules.
    ///
    /// The name is the label the resolver chooses from, so it may not be
    /// empty. The command is what the executor runs, so it may not be empty.
    /// An empty description is allowed: the resolver then reads the name only.
    pub fn normalize(self) -> Result<Self, SaveIntentError> {
        let name = trim(&self.name);
        if name.is_empty() {
            return Err(invalid("The intent name is empty."));
        }
        check_len(&name, MAX_NAME_LEN, "The intent name")?;

        let description = trim(&self.description);
        check_len(&description, MAX_DESCRIPTION_LEN, "The description")?;

        let command = trim(&self.command);
        if command.is_empty() {
            return Err(invalid("The command is empty."));
        }
        check_len(&command, MAX_COMMAND_LEN, "The command")?;

        if self.entities.len() > MAX_ENTITY_COUNT {
            return Err(invalid(format!(
                "An intent takes at most {MAX_ENTITY_COUNT} entities."
            )));
        }

        let mut entities = Vec::with_capacity(self.entities.len());
        for entity in self.entities {
            let entity = entity.normalize()?;
            if entities
                .iter()
                .any(|kept: &EntityInput| kept.name == entity.name)
            {
                return Err(invalid(format!("The entity {} is twice.", entity.name)));
            }
            entities.push(entity);
        }

        Ok(Self {
            name,
            description,
            command,
            entities,
            examples: normalize_examples(self.examples)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one valid input for the tests.
    fn input() -> IntentInput {
        IntentInput {
            name: "  get weather  ".to_string(),
            description: " Look up the weather. ".to_string(),
            command: " curl wttr.in ".to_string(),
            entities: vec![EntityInput {
                name: " City ".to_string(),
                kind: EntityKindDto::Open,
                values: vec!["ignored".to_string()],
                script: None,
                required: true,
            }],
            examples: vec!["  what is the weather  ".to_string()],
        }
    }

    #[test]
    fn normalize_trims_the_text_fields() {
        let normalized = input().normalize().expect("the input is valid");
        assert_eq!(normalized.name, "get weather");
        assert_eq!(normalized.description, "Look up the weather.");
        assert_eq!(normalized.command, "curl wttr.in");
    }

    #[test]
    fn normalize_lowercases_the_entity_name() {
        let normalized = input().normalize().expect("the input is valid");
        assert_eq!(normalized.entities[0].name, "city");
    }

    #[test]
    fn normalize_drops_the_values_of_an_open_entity() {
        let normalized = input().normalize().expect("the input is valid");
        assert!(normalized.entities[0].values.is_empty());
    }

    #[test]
    fn normalize_keeps_the_values_of_a_closed_entity() {
        let mut source = input();
        source.entities[0].kind = EntityKindDto::Closed;
        source.entities[0].values = vec![" today ".to_string(), "tomorrow".to_string()];
        let normalized = source.normalize().expect("the input is valid");
        assert_eq!(normalized.entities[0].values, vec!["today", "tomorrow"]);
    }

    #[test]
    fn normalize_rejects_a_closed_entity_without_values() {
        let mut source = input();
        source.entities[0].kind = EntityKindDto::Closed;
        source.entities[0].values = vec!["  ".to_string()];
        assert!(source.normalize().is_err());
    }

    #[test]
    fn normalize_keeps_the_script_of_a_script_entity() {
        let mut source = input();
        source.entities[0].kind = EntityKindDto::Script;
        source.entities[0].script = Some(" ls /bin ".to_string());
        let normalized = source.normalize().expect("the input is valid");

        assert_eq!(normalized.entities[0].script.as_deref(), Some("ls /bin"));
        assert!(normalized.entities[0].values.is_empty());
    }

    #[test]
    fn normalize_rejects_a_script_entity_without_a_script() {
        let mut source = input();
        source.entities[0].kind = EntityKindDto::Script;
        source.entities[0].script = Some("   ".to_string());
        assert!(source.normalize().is_err());
    }

    #[test]
    fn normalize_drops_the_values_of_an_open_entity_that_becomes_a_script() {
        let mut source = input();
        source.entities[0].kind = EntityKindDto::Script;
        source.entities[0].script = Some("ls".to_string());
        source.entities[0].values = vec!["left over".to_string()];
        let normalized = source.normalize().expect("the input is valid");

        assert!(normalized.entities[0].values.is_empty());
    }

    #[test]
    fn normalize_trims_and_drops_a_repeated_phrase() {
        let mut source = input();
        source.examples = vec![
            " launch firefox ".to_string(),
            "launch firefox".to_string(),
            "  ".to_string(),
        ];
        let normalized = source.normalize().expect("the input is valid");

        assert_eq!(normalized.examples, vec!["launch firefox"]);
    }

    #[test]
    fn normalize_rejects_too_many_phrases() {
        let mut source = input();
        source.examples = (0..MAX_EXAMPLE_COUNT + 1)
            .map(|index| format!("phrase {index}"))
            .collect();
        assert!(source.normalize().is_err());
    }

    #[test]
    fn normalize_rejects_an_empty_name() {
        let mut source = input();
        source.name = "   ".to_string();
        assert!(source.normalize().is_err());
    }

    #[test]
    fn normalize_rejects_an_empty_command() {
        let mut source = input();
        source.command = String::new();
        assert!(source.normalize().is_err());
    }

    #[test]
    fn normalize_rejects_a_repeated_entity_name() {
        let mut source = input();
        source.entities.push(source.entities[0].clone());
        assert!(source.normalize().is_err());
    }

    #[test]
    fn normalize_drops_a_repeated_entity_value() {
        let mut source = input();
        source.entities[0].kind = EntityKindDto::Closed;
        source.entities[0].values = vec!["today".to_string(), " today ".to_string()];
        let normalized = source.normalize().expect("the input is valid");
        assert_eq!(normalized.entities[0].values, vec!["today"]);
    }
}
