//! The documents the retrieval pass ranks.
//!
//! One intent becomes one document. The document holds what a user may
//! say for that intent: the name, the sentence that describes it, the
//! phrases the user wrote, and the names of its entities. The retrieval
//! pass ranks every document against the message, so the name of an
//! intent stops being the only thing a message can match.
//!
//! Two rules keep the evidence honest:
//!
//! - A phrase counts only when the message shares the action of the
//!   phrase. Without the rule the phrase `close firefox` of one intent
//!   holds the word `firefox`, and the message `open firefox` would match
//!   the intent that closes things.
//! - A word that carries no meaning on its own is dropped before a
//!   document or a message is read, so `what is the weather` does not
//!   match every intent that holds a `the`.

use uuid::Uuid;

use alice_core::dto::IntentDto;

use crate::intent::matcher::words_of;

/// Words a document and a message both drop before they are read.
///
/// The list holds the words that only glue a sentence together. A word
/// of an intent name is never on the list, because a user who writes
/// `close window` means the word `close`.
const STOPWORDS: &[&str] = &[
    "a", "about", "all", "an", "and", "any", "are", "as", "at", "be", "been", "but", "by", "can",
    "could", "did", "do", "does", "for", "from", "had", "has", "have", "he", "her", "here", "him",
    "his", "how", "i", "if", "in", "into", "is", "it", "its", "just", "let", "me", "more", "most",
    "my", "no", "not", "of", "or", "our", "please", "she", "should", "so", "some", "than", "that",
    "the", "their", "them", "then", "there", "these", "they", "this", "those", "to", "us", "was",
    "we", "were", "what", "when", "where", "which", "who", "why", "will", "with", "would", "you",
    "your",
];

// Three words are left out of the list on purpose, because a command uses
// them: `up` and `down` say which way the volume goes, and `on` and `off`
// say which way a switch goes. A message of `turn the volume up` and a
// message of `turn the volume down` differ in those words alone.

/// How often the name of an intent counts in its document.
///
/// The name is what the user configured, so it is the surest evidence a
/// document holds.
pub const NAME_WEIGHT: f32 = 3.0;

/// How often a phrase of an intent counts in its document.
pub const PHRASE_WEIGHT: f32 = 2.0;

/// How often the sentence of an intent counts in its document.
pub const DESCRIPTION_WEIGHT: f32 = 1.0;

/// How often the name of an entity counts in its document.
pub const ENTITY_WEIGHT: f32 = 1.0;

/// Whether a word carries meaning on its own.
pub fn is_content_word(word: &str) -> bool {
    !STOPWORDS.contains(&word)
}

/// The words of a text that carry meaning on their own.
pub fn content_words(text: &str) -> Vec<String> {
    words_of(text)
        .into_iter()
        .filter(|word| is_content_word(word))
        .collect()
}

/// One phrase a user may say for an intent.
#[derive(Clone, Debug, PartialEq)]
pub struct Phrase {
    /// The phrase as the user wrote it.
    pub text: String,
    /// The words of the phrase that carry meaning on their own.
    pub words: Vec<String>,
    /// The phrase without case and punctuation, for an exact comparison.
    pub normalized: String,
    /// The word of the phrase that names the action, or none.
    ///
    /// A phrase counts as evidence only when the message holds this
    /// word, so `close firefox` never answers `open firefox`.
    pub action: Option<String>,
}

impl Phrase {
    /// Read one phrase.
    pub fn read(text: &str) -> Self {
        let words = content_words(text);
        Self {
            text: text.trim().to_string(),
            action: words.first().cloned(),
            words,
            normalized: normalize(text),
        }
    }

    /// Whether the phrase may read a message.
    pub fn fits(&self, message_words: &[String]) -> bool {
        match &self.action {
            Some(action) => message_words.contains(action),
            None => true,
        }
    }
}

/// One intent as the retrieval pass reads it.
#[derive(Clone, Debug)]
pub struct IntentDoc {
    /// Position of the intent in the catalog of the turn.
    pub index: usize,
    /// Identifier of the intent.
    pub id: Uuid,
    /// Name of the intent.
    pub name: String,
    /// The words of the name that carry meaning.
    pub name_words: Vec<String>,
    /// The first word of the name that carries meaning, or none.
    ///
    /// The word names the action of the intent, so the deterministic pass
    /// reads the lists of the intents whose action the message holds.
    pub action: Option<String>,
    /// The words of the sentence that describes the intent.
    pub description_words: Vec<String>,
    /// The phrases the user wrote, in the order of the intent.
    pub phrases: Vec<Phrase>,
    /// The names of the entities of the intent.
    pub entity_names: Vec<String>,
    /// The names of the entities that carry a list of values.
    pub list_entities: Vec<String>,
}

impl IntentDoc {
    /// Read one intent as a document.
    pub fn read(intent: &IntentDto, index: usize) -> Self {
        let name_words = content_words(&intent.name);
        let examples: Vec<Phrase> = intent
            .examples
            .iter()
            .filter(|example| !example.trim().is_empty())
            .map(|example| Phrase::read(example))
            .collect();
        let mut phrases = vec![Phrase::read(&intent.name)];
        phrases.extend(examples);
        Self {
            index,
            id: intent.id,
            action: name_words.first().cloned(),
            name: intent.name.clone(),
            name_words,
            description_words: content_words(&intent.description),
            phrases,
            entity_names: intent
                .entities
                .iter()
                .map(|entity| entity.name.clone())
                .collect(),
            list_entities: intent
                .entities
                .iter()
                .filter(|entity| entity.kind.has_values())
                .map(|entity| entity.name.clone())
                .collect(),
        }
    }

    /// Whether a phrase of this document counts for a message.
    ///
    /// The names of the document always count. A phrase the user wrote
    /// counts only when the message shares its action, unless the gate is
    /// switched off.
    pub fn phrase_fits(&self, phrase: &Phrase, message_words: &[String], gate: bool) -> bool {
        if !gate {
            return true;
        }
        if phrase.text == self.name {
            return true;
        }
        phrase.fits(message_words)
    }

    /// The text the embedding pass reads for this document.
    ///
    /// The phrases of the user stand in the text, because a phrase a
    /// person wrote reads a message of a person far better than the name
    /// of the intent does.
    pub fn embed_text(&self) -> String {
        let mut parts: Vec<String> = Vec::with_capacity(self.phrases.len() + 2);
        parts.push(self.name.clone());
        if !self.description_words.is_empty() {
            parts.push(self.description_words.join(" "));
        }
        for phrase in self.phrases.iter().skip(1) {
            parts.push(phrase.text.clone());
        }
        if !self.entity_names.is_empty() {
            parts.push(self.entity_names.join(", "));
        }
        parts.join(". ")
    }
}

/// Read the catalog of one turn as documents.
pub fn catalog(intents: &[IntentDto]) -> Vec<IntentDoc> {
    intents
        .iter()
        .enumerate()
        .map(|(index, intent)| IntentDoc::read(intent, index))
        .collect()
}

/// Put a text into the form an exact comparison reads.
pub fn normalize(text: &str) -> String {
    text.chars()
        .filter(|character| character.is_alphanumeric() || character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use alice_core::dto::EntityKindDto;
    use chrono::Utc;

    /// Build one intent for the tests.
    pub fn intent(name: &str, examples: &[&str], entities: &[(&str, EntityKindDto)]) -> IntentDto {
        IntentDto {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: format!("Do {name}."),
            command: "echo hi".to_string(),
            entities: entities
                .iter()
                .map(|(name, kind)| alice_core::dto::IntentEntityDto {
                    id: Uuid::new_v4(),
                    name: (*name).to_string(),
                    kind: *kind,
                    values: Vec::new(),
                    script: None,
                    required: true,
                })
                .collect(),
            examples: examples
                .iter()
                .map(|example| (*example).to_string())
                .collect(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn a_message_drops_the_words_that_carry_nothing() {
        assert_eq!(content_words("What is the weather"), vec!["weather"]);
        assert_eq!(content_words("open firefox"), vec!["open", "firefox"]);
    }

    #[test]
    fn a_word_that_says_which_way_a_command_goes_stays() {
        assert_eq!(
            content_words("turn the volume up"),
            vec!["turn", "volume", "up"]
        );
        assert_eq!(
            content_words("turn the wifi on"),
            vec!["turn", "wifi", "on"]
        );
    }

    #[test]
    fn a_document_holds_its_name_and_its_phrases() {
        let catalog = catalog(&[intent(
            "open application",
            &["launch firefox"],
            &[("applications", EntityKindDto::Script)],
        )]);

        assert_eq!(catalog[0].name_words, vec!["open", "application"]);
        assert_eq!(catalog[0].action.as_deref(), Some("open"));
        assert_eq!(catalog[0].phrases.len(), 2);
        assert_eq!(catalog[0].list_entities, vec!["applications"]);
        assert!(catalog[0].embed_text().contains("launch firefox"));
    }

    #[test]
    fn a_phrase_counts_only_when_the_message_shares_its_action() {
        let catalog = catalog(&[intent("open application", &["close firefox"], &[])]);
        let doc = &catalog[0];
        let phrase = &doc.phrases[1];

        assert!(doc.phrase_fits(phrase, &content_words("close firefox"), true));
        assert!(!doc.phrase_fits(phrase, &content_words("open firefox"), true));
        // The name of the document always counts.
        assert!(doc.phrase_fits(&doc.phrases[0], &content_words("open firefox"), true));
        // Without the gate every phrase counts.
        assert!(doc.phrase_fits(phrase, &content_words("open firefox"), false));
    }

    #[test]
    fn the_normalized_form_of_a_text_drops_case_and_punctuation() {
        assert_eq!(normalize("  Open  Firefox! "), "open firefox");
        // A message and a phrase are compared in that form, so a mark of
        // punctuation has to read the same on both sides.
        assert_eq!(normalize("github.com"), normalize("GitHub,com"));
        assert_ne!(normalize("github.com"), normalize("github com"));
    }

    #[test]
    fn a_phrase_without_content_words_always_fits() {
        let phrase = Phrase::read("the");
        assert!(phrase.action.is_none());
        assert!(phrase.fits(&[]));
    }

    #[test]
    fn only_an_entity_that_carries_a_list_is_read_from_one() {
        let catalog = catalog(&[intent(
            "get weather",
            &[],
            &[
                ("city", EntityKindDto::Open),
                ("when", EntityKindDto::Closed),
            ],
        )]);

        assert_eq!(catalog[0].entity_names, vec!["city", "when"]);
        assert_eq!(catalog[0].list_entities, vec!["when"]);
    }
}
