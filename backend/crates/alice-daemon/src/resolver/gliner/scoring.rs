//! Scoring of the labels a built in GLiNER model reads.
//!
//! A vanilla GLiNER model is a span finder: it reports the text spans that
//! match a label, with a probability. It is strong at finding a value and
//! thin at naming an action, so this module reads an intent out of two
//! signals: the score of the intent name itself, and the score of the
//! labels of the entities that intent needs.
//!
//! `what is the weather in Berlin` scores the label `get weather` at about
//! seven percent on its own, but the label `city` at about eighty five
//! percent, and only `get weather` offers a city. Reading both signals
//! turns a thin classifier into a usable one.
//!
//! A span finder also scores an action label low when the message names no
//! value for it: `open firefox` scores the phrase `launch firefox` at
//! about twelve percent, and the entity label `applications` at nothing,
//! so the model alone refuses a message the user wrote a phrase for. The
//! model is therefore not the only reader: a message that holds the words
//! of the name of an intent or of one of its phrases points at that intent
//! on its own.
//!
//! A phrase is a label of its own intent, and a label is read anywhere in
//! the message: `close firefox` is the phrase of `close application`, and
//! it also scores the message `open firefox`, because the span `firefox`
//! fits the phrase of the other intent as well. A large catalog therefore
//! reads the surest labels and leaves the phrases to the word reader: the
//! name of an intent and the labels of its entities name one intent, while
//! a phrase names two of them.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};

use alice_core::dto::{EntityKindDto, IntentDto, IntentEntityDto, GLINER_SOFT_LABEL_LIMIT};

use crate::intent::matcher::words_of;
use crate::resolver::gliner::engine::LabelHit;
use crate::resolver::values::reads_as_value;

/// The score of one intent.
#[derive(Clone, Debug, PartialEq)]
pub struct IntentScore {
    /// Position of the intent in the catalog the score came from.
    pub index: usize,
    /// Name of the intent.
    pub name: String,
    /// Score of the intent, between 0 and 1.
    pub score: f32,
    /// The label that scored the intent, when it was not the intent name.
    pub evidence: Option<String>,
}

/// The labels of the entities of one intent, which read its values.
///
/// An open entity adds its name, because the value is a span in the
/// message. A closed entity adds its values as well, because the message
/// carries one of them.
pub fn evidence_labels(intent: &IntentDto) -> Vec<String> {
    let mut labels: Vec<String> = Vec::new();
    for entity in &intent.entities {
        labels.push(entity.name.clone());
        if matches!(entity.kind, EntityKindDto::Closed) {
            labels.extend(entity.values.iter().cloned());
        }
    }
    labels
}

/// The labels that point at one intent when the model chooses it: the
/// name of the intent, the phrases a user may say for it, and the labels
/// of every entity it needs.
///
/// The phrases are the point of the list: a user who says `launch
/// firefox` names the intent `open application` without using a word of
/// its name.
pub fn score_labels(intent: &IntentDto) -> Vec<String> {
    let mut labels: Vec<String> = Vec::with_capacity(intent.examples.len() + 1);
    labels.push(intent.name.clone());
    labels.extend(intent.examples.iter().cloned());
    labels.extend(evidence_labels(intent));
    labels
}

/// The labels that point at one intent without naming another one: the
/// name of the intent and the labels of every entity it needs.
///
/// A label holds the words of one intent, so a message that holds them
/// points at that intent and at no other one. The phrases are left out,
/// because a phrase of one intent shares its value with the phrases of the
/// intents that read the same kind of value.
pub fn sure_labels(intent: &IntentDto) -> Vec<String> {
    let mut labels: Vec<String> = Vec::with_capacity(intent.entities.len() + 1);
    labels.push(intent.name.clone());
    labels.extend(evidence_labels(intent));
    labels
}

/// Every label the model reads for one intent configuration.
///
/// The list is deduplicated without case, because a model reads two
/// spellings of one label as two labels and splits its attention.
///
/// A catalog that fits the budget of a model offers the phrases as well,
/// so a message that holds the words of a phrase points at its intent on
/// its own. A catalog that does not fit offers the sure labels only, and
/// the word reader carries the phrases instead.
pub fn labels_for(intents: &[IntentDto]) -> Vec<String> {
    let full = dedup(intents.iter().flat_map(score_labels));
    if full.len() <= GLINER_SOFT_LABEL_LIMIT {
        return full;
    }
    dedup(intents.iter().flat_map(sure_labels))
}

/// Deduplicate labels without case, in the order they arrive.
fn dedup(labels: impl Iterator<Item = String>) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for label in labels {
        if seen.insert(label.to_lowercase()) {
            kept.push(label);
        }
    }
    kept
}

/// Number of labels a GLiNER model reads for one configuration.
pub fn label_count(intents: &[IntentDto]) -> usize {
    labels_for(intents).len()
}

/// The best hit of every label.
pub fn best_hits(hits: &[LabelHit]) -> BTreeMap<String, LabelHit> {
    let mut best: BTreeMap<String, LabelHit> = BTreeMap::new();
    for hit in hits {
        let key = hit.label.to_lowercase();
        match best.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(hit.clone());
            }
            Entry::Occupied(mut entry) => {
                if hit.score > entry.get().score {
                    entry.insert(hit.clone());
                }
            }
        }
    }
    best
}

/// The words a phrase shares with every other phrase of the language.
///
/// An article and a preposition carry no meaning of their own, so a
/// message that holds one of them names no intent: `what is the weather`
/// shares the `the` of `open the browser` and names nothing.
const FUNCTION_WORDS: [&str; 44] = [
    "a", "an", "the", "of", "for", "to", "in", "on", "at", "by", "with", "from", "into", "and",
    "or", "but", "if", "is", "are", "was", "were", "be", "been", "am", "do", "does", "did", "can",
    "could", "would", "should", "will", "shall", "may", "might", "my", "me", "you", "your", "it",
    "that", "this", "some", "any",
];

/// The words of a phrase that name what it does.
fn content_words(text: &str) -> Vec<String> {
    words_of(text)
        .into_iter()
        .filter(|word| !FUNCTION_WORDS.contains(&word.as_str()))
        .collect()
}

/// The score of the words of one intent that a message holds.
///
/// The name of an intent and the phrases a user wrote for it are the
/// labels of the intent, so a message that holds their words points at the
/// intent, whatever the model read. The score is the share of those words
/// the message holds, from the label that fits it best. A message that
/// holds half of `launch firefox` scores it one half, and a message that
/// holds all of `open obs` scores it whole.
///
/// Only the words that name the action count, so a label that shares only
/// its articles with a message names nothing.
fn lexical_score(intent: &IntentDto, message: &str) -> Option<(String, f32)> {
    let words = words_of(message);
    if words.is_empty() {
        return None;
    }

    let mut best: Option<(String, f32)> = None;
    for label in std::iter::once(&intent.name).chain(intent.examples.iter()) {
        let wanted = content_words(label);
        if wanted.is_empty() {
            continue;
        }
        let held = wanted.iter().filter(|word| words.contains(word)).count();
        let score = held as f32 / wanted.len() as f32;
        let better = match &best {
            None => true,
            Some((_, best_score)) => score > *best_score,
        };
        if better && score > 0.0 {
            best = Some((label.clone(), score));
        }
    }
    best
}

/// Score every intent, best first.
///
/// The score of an intent is the better of what the model read and what
/// the words of the message name, so a thin model does not refuse a
/// message the user wrote a phrase for.
pub fn score_intents(intents: &[IntentDto], hits: &[LabelHit], message: &str) -> Vec<IntentScore> {
    let best = best_hits(hits);
    let mut scores: Vec<IntentScore> = intents
        .iter()
        .enumerate()
        .map(|(index, intent)| {
            let own = best
                .get(&intent.name.to_lowercase())
                .map(|hit| hit.score)
                .unwrap_or(0.0);
            let via = score_labels(intent)
                .into_iter()
                .filter_map(|label| {
                    best.get(&label.to_lowercase())
                        .map(|hit| (label, hit.score))
                })
                .max_by(|left, right| {
                    left.1
                        .partial_cmp(&right.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            let read = match via {
                Some((label, score)) if score > own => IntentScore {
                    index,
                    name: intent.name.clone(),
                    score,
                    evidence: Some(label),
                },
                _ => IntentScore {
                    index,
                    name: intent.name.clone(),
                    score: own,
                    evidence: None,
                },
            };
            match lexical_score(intent, message) {
                Some((label, score)) if score > read.score => IntentScore {
                    index,
                    name: intent.name.clone(),
                    score,
                    // The words of the message named the intent, and the
                    // report says which label they named.
                    evidence: Some(format!("{label} (words)")),
                },
                _ => read,
            }
        })
        .collect();

    scores.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
    });
    scores
}

/// Read the value of every entity of one intent out of the hits.
///
/// An open entity takes the best span its own label matched, when that
/// span reads as a value. A closed entity takes the configured value the
/// message names, either because a label matched it or because the
/// message holds the word itself. A script entity takes a value of its
/// list when the list is short enough to be a label, and the span of its
/// own label otherwise: the caller then reads the entry of the list the
/// span names.
pub fn read_values(
    intent: &IntentDto,
    hits: &[LabelHit],
    message: &str,
    offered: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, String> {
    let best = best_hits(hits);
    let mut values = BTreeMap::new();

    for entity in &intent.entities {
        let offered = offered_values(entity, offered);
        let choices: &[String] = match entity.kind {
            EntityKindDto::Closed => &entity.values,
            _ => offered,
        };

        if !choices.is_empty() {
            let scored = choices
                .iter()
                .filter_map(|value| {
                    best.get(&value.to_lowercase())
                        .map(|hit| (value, hit.score))
                })
                .max_by(|left, right| {
                    left.1
                        .partial_cmp(&right.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            let named = choices.iter().find(|value| mentions_value(message, value));
            if let Some(value) = scored.map(|(value, _)| value).or(named) {
                values.insert(entity.name.clone(), value.clone());
            }
            continue;
        }

        let Some(hit) = best.get(&entity.name.to_lowercase()) else {
            continue;
        };
        if reads_as_value(&hit.text, message) {
            values.insert(entity.name.clone(), hit.text.clone());
        }
    }

    values
}

/// The values of a script entity the model reads as labels.
///
/// A list the service offered for one read is in the label set of that
/// read. A longer list is not, and an entity without a list has none.
fn offered_values<'a>(
    entity: &IntentEntityDto,
    offered: &'a BTreeMap<String, Vec<String>>,
) -> &'a [String] {
    if !matches!(entity.kind, EntityKindDto::Script) {
        return &[];
    }
    offered.get(&entity.name).map_or(&[], Vec::as_slice)
}

/// The labels of one value read.
///
/// The labels of the entities of the intent read a value each, and a
/// script entity whose list is short enough adds the values of that list,
/// so the model chooses one of them the way it chooses a closed value.
pub fn read_labels(intent: &IntentDto, offered: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut labels = evidence_labels(intent);
    for entity in &intent.entities {
        labels.extend(offered_values(entity, offered).iter().cloned());
    }
    labels
}

/// Whether a message names one value as a word of its own.
///
/// A closed entity carries the values the user configured, so a message
/// that holds one of them names it, even when the model gave the label no
/// score of its own.
fn mentions_value(message: &str, value: &str) -> bool {
    let words = words_of(message);
    let wanted = words_of(value);
    if wanted.is_empty() || wanted.len() > words.len() {
        return false;
    }
    words
        .windows(wanted.len())
        .any(|window| window == wanted.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::dto::IntentEntityDto;
    use chrono::Utc;
    use uuid::Uuid;

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

    /// Build the offered values of one script entity for the tests.
    fn offered(values: &[&str]) -> BTreeMap<String, Vec<String>> {
        let mut lists = BTreeMap::new();
        lists.insert(
            "applications".to_string(),
            values.iter().map(|value| value.to_string()).collect(),
        );
        lists
    }

    /// Build one intent for the tests.
    fn intent(name: &str, entities: Vec<IntentEntityDto>) -> IntentDto {
        IntentDto {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: String::new(),
            command: "echo hi".to_string(),
            entities,
            examples: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Build one hit for the tests.
    fn hit(label: &str, text: &str, score: f32) -> LabelHit {
        LabelHit {
            label: label.to_string(),
            text: text.to_string(),
            score,
        }
    }

    /// The catalog the tests score against.
    fn catalog() -> Vec<IntentDto> {
        vec![
            intent(
                "get weather",
                vec![
                    entity("city", EntityKindDto::Open, &[]),
                    entity("when", EntityKindDto::Closed, &["today", "tomorrow"]),
                ],
            ),
            intent(
                "play music",
                vec![entity("artist", EntityKindDto::Open, &[])],
            ),
        ]
    }

    #[test]
    fn the_phrases_of_an_intent_are_labels_of_it() {
        let mut catalog = catalog();
        catalog[1].examples = vec!["launch firefox".to_string(), "open obs".to_string()];

        let labels = score_labels(&catalog[1]);
        assert!(labels.contains(&"launch firefox".to_string()));
        assert!(labels.contains(&"artist".to_string()));
        assert!(labels_for(&catalog).contains(&"open obs".to_string()));
    }

    #[test]
    fn a_phrase_scores_the_intent_it_belongs_to() {
        let mut catalog = catalog();
        catalog[1].examples = vec!["launch firefox".to_string()];
        let hits = vec![hit("launch firefox", "launch firefox", 0.72)];

        let scores = score_intents(&catalog, &hits, "");

        assert_eq!(scores[0].name, "play music");
        assert_eq!(scores[0].evidence.as_deref(), Some("launch firefox"));
    }

    #[test]
    fn the_words_of_a_message_score_the_intent_that_wrote_the_phrase() {
        let mut catalog = catalog();
        catalog[1].examples = vec!["launch firefox".to_string(), "open obs".to_string()];

        // The model read nothing: a span finder scores an action label low
        // when the message names no value for it.
        let scores = score_intents(&catalog, &[], "open firefox");

        assert_eq!(scores[0].name, "play music");
        assert!((scores[0].score - 0.5).abs() < f32::EPSILON);
        assert_eq!(
            scores[0].evidence.as_deref(),
            Some("launch firefox (words)")
        );
    }

    #[test]
    fn the_words_of_a_message_score_the_name_of_an_intent() {
        let scores = score_intents(&catalog(), &[], "the weather tomorrow");

        assert_eq!(scores[0].name, "get weather");
        assert!((scores[0].score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn a_message_without_a_word_of_an_intent_scores_it_nothing() {
        let scores = score_intents(&catalog(), &[], "hello there");

        assert_eq!(scores[0].name, "get weather");
        assert_eq!(scores[0].score, 0.0);
        assert_eq!(scores[0].evidence, None);
    }

    #[test]
    fn a_shared_article_does_not_name_an_intent() {
        let mut catalog = catalog();
        catalog[1].examples = vec!["open the browser".to_string()];

        let scores = score_intents(&catalog, &[], "what is the weather");

        assert_eq!(scores[0].name, "get weather");
        assert!((scores[0].score - 0.5).abs() < f32::EPSILON);
        assert_eq!(scores[1].name, "play music");
        assert_eq!(scores[1].score, 0.0);
    }

    #[test]
    fn the_model_still_wins_when_it_scores_higher_than_the_words() {
        let hits = vec![hit("get weather", "weather", 0.9)];
        let scores = score_intents(&catalog(), &hits, "what is the weather");

        assert_eq!(scores[0].name, "get weather");
        assert_eq!(scores[0].score, 0.9);
        assert_eq!(scores[0].evidence, None);
    }

    #[test]
    fn a_short_list_of_a_script_entity_is_read_as_a_value() {
        let mut catalog = catalog();
        catalog[1].entities = vec![entity("applications", EntityKindDto::Script, &[])];
        let offered = offered(&["firefox", "obs"]);
        let hits = vec![hit("applications", "firefox", 0.8)];

        let values = read_values(&catalog[1], &hits, "open firefox", &offered);

        assert_eq!(values["applications"], "firefox");
    }

    #[test]
    fn a_script_entity_reads_the_span_when_the_list_is_long() {
        let mut catalog = catalog();
        catalog[1].entities = vec![entity("applications", EntityKindDto::Script, &[])];
        let hits = vec![hit("applications", "obs", 0.8)];

        let values = read_values(&catalog[1], &hits, "open obs", &BTreeMap::new());

        assert_eq!(values["applications"], "obs");
    }

    #[test]
    fn labels_hold_every_intent_and_entity() {
        let labels = labels_for(&catalog());

        assert!(labels.contains(&"get weather".to_string()));
        assert!(labels.contains(&"city".to_string()));
        assert!(labels.contains(&"today".to_string()));
        assert!(labels.contains(&"tomorrow".to_string()));
        assert!(labels.contains(&"play music".to_string()));
        assert_eq!(label_count(&catalog()), labels.len());
    }

    #[test]
    fn a_catalog_that_fits_offers_the_phrases() {
        let mut catalog = catalog();
        catalog[1].examples = vec!["launch firefox".to_string()];

        let labels = labels_for(&catalog);

        assert!(labels.len() <= GLINER_SOFT_LABEL_LIMIT);
        assert!(labels.contains(&"launch firefox".to_string()));
    }

    #[test]
    fn a_catalog_past_the_budget_offers_the_sure_labels_only() {
        // A phrase of one intent also fits the message of another intent,
        // so a catalog that reads more labels than a model reads well
        // leaves the phrases to the word reader.
        let catalog: Vec<IntentDto> = (0..GLINER_SOFT_LABEL_LIMIT)
            .map(|index| {
                let mut intent = intent(
                    &format!("intent {index}"),
                    vec![entity("city", EntityKindDto::Open, &[])],
                );
                intent.examples = vec![format!("phrase {index}")];
                intent
            })
            .collect();

        let labels = labels_for(&catalog);

        // The name of every intent and the label of its entity stay, and
        // the phrases go: the catalog reads one label per intent instead
        // of two.
        assert!(labels.contains(&"intent 0".to_string()));
        assert!(labels.contains(&"city".to_string()));
        assert!(!labels.iter().any(|label| label.starts_with("phrase ")));
        assert_eq!(labels.len(), GLINER_SOFT_LABEL_LIMIT + 1);
        assert_eq!(label_count(&catalog), labels.len());
    }

    #[test]
    fn labels_drop_a_repeated_spelling() {
        let intents = vec![
            intent(
                "get weather",
                vec![entity("city", EntityKindDto::Open, &[])],
            ),
            intent("city", Vec::new()),
        ];

        assert_eq!(labels_for(&intents), vec!["get weather", "city"]);
    }

    #[test]
    fn an_entity_label_scores_its_intent() {
        let catalog = catalog();
        let hits = vec![
            hit("get weather", "weather", 0.07),
            hit("city", "Berlin", 0.85),
        ];

        let scores = score_intents(&catalog, &hits, "what is the weather in Berlin");

        assert_eq!(scores[0].name, "get weather");
        assert!((scores[0].score - 0.85).abs() < f32::EPSILON);
        assert_eq!(scores[0].evidence.as_deref(), Some("city"));
    }

    #[test]
    fn the_strongest_evidence_wins() {
        let catalog = catalog();
        let hits = vec![hit("city", "Berlin", 0.85), hit("artist", "jazz", 0.36)];

        let scores = score_intents(&catalog, &hits, "jazz in Berlin");

        assert_eq!(scores[0].name, "get weather");
        assert_eq!(scores[1].name, "play music");
        assert_eq!(scores[1].evidence.as_deref(), Some("artist"));
    }

    #[test]
    fn an_intent_without_evidence_scores_its_own_label() {
        let catalog = vec![intent("tell a joke", Vec::new())];
        let scores = score_intents(&catalog, &[hit("tell a joke", "joke", 0.09)], "hello");

        assert!((scores[0].score - 0.09).abs() < f32::EPSILON);
        assert!(scores[0].evidence.is_none());
    }

    #[test]
    fn a_message_without_a_hit_scores_every_intent_with_zero() {
        let scores = score_intents(&catalog(), &[], "hello there");

        assert_eq!(scores.len(), 2);
        assert!(scores.iter().all(|score| score.score == 0.0));
    }

    #[test]
    fn read_values_reads_an_open_entity_from_its_span() {
        let catalog = catalog();
        let hits = vec![hit("city", "Berlin", 0.85)];
        let values = read_values(
            &catalog[0],
            &hits,
            "what is the weather in Berlin",
            &BTreeMap::new(),
        );

        assert_eq!(values["city"], "Berlin");
    }

    #[test]
    fn read_values_drops_a_span_that_repeats_the_message() {
        let catalog = catalog();
        let message = "is it raining somewhere";
        let hits = vec![hit("city", message, 0.4)];

        assert!(read_values(&catalog[0], &hits, message, &BTreeMap::new()).is_empty());
    }

    #[test]
    fn read_values_reads_a_closed_entity_from_its_label() {
        let catalog = catalog();
        let hits = vec![hit("tomorrow", "tomorrow", 0.4)];
        let values = read_values(
            &catalog[0],
            &hits,
            "the weather in Paris tomorrow",
            &BTreeMap::new(),
        );

        assert_eq!(values["when"], "tomorrow");
    }

    #[test]
    fn read_values_reads_a_closed_entity_the_message_names() {
        let catalog = catalog();
        let values = read_values(
            &catalog[0],
            &[],
            "the weather in Paris tomorrow",
            &BTreeMap::new(),
        );

        assert_eq!(values["when"], "tomorrow");
    }

    #[test]
    fn read_values_ignores_a_value_the_message_does_not_name() {
        let catalog = catalog();
        let values = read_values(&catalog[0], &[], "the weather in Paris", &BTreeMap::new());

        assert!(values.is_empty());
    }

    #[test]
    fn mentions_value_matches_a_word_of_its_own() {
        assert!(mentions_value("the weather tomorrow", "tomorrow"));
        assert!(mentions_value("is tomorrow ok", "tomorrow"));
        assert!(!mentions_value("the weather tomor", "tomorrow"));
        assert!(!mentions_value("the weather", "tomorrow"));
    }
}
