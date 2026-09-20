//! The deterministic pass of the router.
//!
//! Before any ranking runs, the router tries to prove the answer from the
//! message alone. Two shapes of message can be proved:
//!
//! - The message spells an intent out. `take a screenshot` is the name of
//!   one intent and the phrase of no other, so there is nothing to weigh.
//! - The message names one value of one list and reads as the action of
//!   one intent. `open firefox` is the action `open` plus the entry
//!   `firefox`, and only the intent that owns the list holding `firefox`
//!   reads that way. If two intents read the same way, the message is
//!   ambiguous and the pass gives up, because running the wrong command
//!   is worse than refusing or asking.
//!
//! The pass needs no model and no server. It also never guesses: a
//! message it cannot prove is left to the retrieval stage, which weighs
//! every intent of the catalog against it.

use std::collections::{BTreeMap, BTreeSet};

use alice_core::config::RouterConfig;

use crate::resolver::router::doc::{content_words, normalize, IntentDoc};

/// The result of the deterministic pass.
#[derive(Clone, Debug, PartialEq)]
pub enum FastOutcome {
    /// The pass proved one intent and, when the message names a list
    /// value, the value of one of its entities.
    Match(FastMatch),
    /// The pass proved nothing, so the retrieval stage reads the message.
    None,
}

/// One intent the deterministic pass proved.
#[derive(Clone, Debug, PartialEq)]
pub struct FastMatch {
    /// Position of the intent in the catalog of the turn.
    pub index: usize,
    /// One sentence about what the message said, for the metadata.
    pub reason: String,
    /// The value of every entity the message named.
    pub values: BTreeMap<String, String>,
}

/// The words of one document that may stand next to a value.
///
/// The name of the intent names what it does, and a phrase the user wrote
/// names it the way that user speaks. A message is read as the action of
/// an intent when what stays of it after the value is removed is made of
/// those words alone.
fn action_words(doc: &IntentDoc, message_words: &[String], gate: bool) -> BTreeSet<String> {
    let mut words: BTreeSet<String> = doc.name_words.iter().cloned().collect();
    for phrase in doc.phrases.iter().skip(1) {
        if !doc.phrase_fits(phrase, message_words, gate) {
            continue;
        }
        words.extend(phrase.words.iter().cloned());
    }
    words
}

/// Whether a phrase of one document names the action of a message.
fn reads_as_action(doc: &IntentDoc, message_words: &[String], gate: bool) -> bool {
    if doc
        .action
        .as_ref()
        .is_some_and(|action| message_words.contains(action))
    {
        return true;
    }
    doc.phrases
        .iter()
        .skip(1)
        .any(|phrase| doc.phrase_fits(phrase, message_words, gate))
}

/// The documents whose lists the deterministic pass wants to read.
///
/// The pass reads the lists of the documents whose action the message
/// holds, and of no others. Reading every list of a large catalog would
/// run every script of every intent for each turn, so the plan keeps the
/// work bounded by what the message could possibly ask for.
pub fn plan(docs: &[IntentDoc], message: &str, config: &RouterConfig) -> Vec<usize> {
    let message_words = content_words(message);
    if message_words.is_empty() {
        return Vec::new();
    }
    docs.iter()
        .filter(|doc| !doc.list_entities.is_empty())
        .filter(|doc| reads_as_action(doc, &message_words, config.phrase_gate))
        .map(|doc| doc.index)
        .collect()
}

/// Read a message with the deterministic rules.
///
/// `lists` holds the values of the list entities of the documents in the
/// plan, by document and by entity name.
pub fn read(
    docs: &[IntentDoc],
    message: &str,
    lists: &BTreeMap<usize, BTreeMap<String, Vec<String>>>,
    config: &RouterConfig,
) -> FastOutcome {
    let message_words = content_words(message);
    if message_words.is_empty() {
        return FastOutcome::None;
    }
    let message_normalized = normalize(message);

    if let Some(found) = spelled_out(docs, &message_normalized) {
        return found;
    }
    named_value(docs, &message_words, lists, config)
}

/// Read a message that spells one intent out.
fn spelled_out(docs: &[IntentDoc], message: &str) -> Option<FastOutcome> {
    let mut found: Option<usize> = None;
    let mut count = 0usize;
    for doc in docs {
        if !doc
            .phrases
            .iter()
            .any(|phrase| phrase.normalized == message)
        {
            continue;
        }
        count += 1;
        found = Some(doc.index);
    }
    if count != 1 {
        return None;
    }
    let index = found?;
    Some(FastOutcome::Match(FastMatch {
        index,
        reason: "the message spells the intent out".to_string(),
        values: BTreeMap::new(),
    }))
}

/// Read a message that names one value of one list.
fn named_value(
    docs: &[IntentDoc],
    message_words: &[String],
    lists: &BTreeMap<usize, BTreeMap<String, Vec<String>>>,
    config: &RouterConfig,
) -> FastOutcome {
    let mut read: Vec<FastMatch> = Vec::new();
    for doc in docs {
        if doc.list_entities.is_empty() {
            continue;
        }
        if !reads_as_action(doc, message_words, config.phrase_gate) {
            continue;
        }
        let Some(entities) = lists.get(&doc.index) else {
            continue;
        };
        let allowed = action_words(doc, message_words, config.phrase_gate);
        for entity in &doc.list_entities {
            let Some(values) = entities.get(entity) else {
                continue;
            };
            for value in values {
                let value_words = content_words(value);
                if value_words.is_empty() {
                    continue;
                }
                let Some(remainder) = without(message_words, &value_words) else {
                    continue;
                };
                if !remainder.iter().all(|word| allowed.contains(word)) {
                    continue;
                }
                let mut values_for = BTreeMap::new();
                values_for.insert(entity.clone(), value.clone());
                read.push(FastMatch {
                    index: doc.index,
                    reason: format!("the message names the entry `{value}` of `{entity}`"),
                    values: values_for,
                });
                break;
            }
        }
    }

    // Two intents that read the same way leave the message ambiguous, so
    // the pass refuses to choose and the retrieval stage decides.
    let distinct: BTreeSet<usize> = read.iter().map(|found| found.index).collect();
    if distinct.len() != 1 {
        return FastOutcome::None;
    }
    match read.into_iter().next() {
        Some(found) => FastOutcome::Match(found),
        None => FastOutcome::None,
    }
}

/// Remove one occurrence of every word of a value from a message.
///
/// Returns `None` when the message does not hold the value, so a value a
/// message merely resembles never reads as the value.
fn without(message_words: &[String], value_words: &[String]) -> Option<Vec<String>> {
    let mut left = message_words.to_vec();
    for word in value_words {
        let at = left.iter().position(|held| held == word)?;
        left.remove(at);
    }
    Some(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::router::doc::{catalog, tests::intent};
    use alice_core::dto::EntityKindDto;

    /// The catalog the tests read.
    fn fixture() -> Vec<IntentDoc> {
        catalog(&[
            intent(
                "open application",
                &["launch an app"],
                &[("applications", EntityKindDto::Script)],
            ),
            intent("open folder", &[], &[("folder", EntityKindDto::Script)]),
            intent("close window", &["close firefox"], &[]),
            intent(
                "get weather",
                &["how is the weather"],
                &[("city", EntityKindDto::Open)],
            ),
            intent(
                "switch workspace",
                &["go to workspace"],
                &[("workspace", EntityKindDto::Closed)],
            ),
        ])
    }

    /// The lists the tests offer.
    fn lists() -> BTreeMap<usize, BTreeMap<String, Vec<String>>> {
        let mut lists: BTreeMap<usize, BTreeMap<String, Vec<String>>> = BTreeMap::new();
        lists.insert(
            0,
            [(
                "applications".to_string(),
                vec!["firefox".to_string(), "obs".to_string()],
            )]
            .into_iter()
            .collect(),
        );
        lists.insert(
            1,
            [(
                "folder".to_string(),
                vec!["Downloads".to_string(), "Documents".to_string()],
            )]
            .into_iter()
            .collect(),
        );
        lists.insert(
            4,
            [(
                "workspace".to_string(),
                vec!["1".to_string(), "2".to_string(), "3".to_string()],
            )]
            .into_iter()
            .collect(),
        );
        lists
    }

    /// Read one message with the defaults.
    fn read_one(message: &str) -> FastOutcome {
        read(&fixture(), message, &lists(), &RouterConfig::default())
    }

    #[test]
    fn a_message_that_spells_an_intent_out_is_proved() {
        match read_one("take a screenshot") {
            // The message spells no intent of this fixture out.
            FastOutcome::None => {}
            other => panic!("{other:?}"),
        }
        match read_one("get weather") {
            FastOutcome::Match(found) => {
                assert_eq!(found.index, 3);
                assert!(found.values.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_message_that_names_one_value_of_one_list_is_proved() {
        match read_one("open firefox") {
            FastOutcome::Match(found) => {
                assert_eq!(found.index, 0);
                assert_eq!(found.values["applications"], "firefox");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_value_of_another_list_reads_its_own_intent() {
        match read_one("open Downloads") {
            FastOutcome::Match(found) => {
                assert_eq!(found.index, 1);
                assert_eq!(found.values["folder"], "Downloads");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_value_of_a_closed_entity_is_proved() {
        match read_one("switch workspace 3") {
            FastOutcome::Match(found) => {
                assert_eq!(found.index, 4);
                assert_eq!(found.values["workspace"], "3");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_value_nothing_holds_is_not_proved() {
        assert_eq!(read_one("open obsidian"), FastOutcome::None);
    }

    #[test]
    fn a_message_of_another_action_is_not_proved() {
        // `firefox` belongs to the list of the opening intent only, so
        // the closing intent never reads the message.
        assert_eq!(read_one("close obsidian"), FastOutcome::None);
    }

    #[test]
    fn a_message_that_two_intents_could_own_is_left_to_the_ranking() {
        let mut docs = fixture();
        docs[1].list_entities = vec!["folder".to_string()];
        let mut lists = lists();
        lists
            .get_mut(&1)
            .expect("the folder list is there")
            .insert("folder".to_string(), vec!["firefox".to_string()]);
        docs[1]
            .phrases
            .push(crate::resolver::router::doc::Phrase::read("open folder"));

        assert_eq!(
            read(&docs, "open firefox", &lists, &RouterConfig::default()),
            FastOutcome::None
        );
    }

    #[test]
    fn the_plan_reads_the_lists_the_message_could_ask_for() {
        let docs = fixture();
        let planned = plan(&docs, "open firefox", &RouterConfig::default());
        let downloads = plan(&docs, "open Downloads", &RouterConfig::default());

        assert_eq!(planned, vec![0, 1]);
        assert_eq!(downloads, vec![0, 1]);
        assert!(plan(&docs, "what is the weather", &RouterConfig::default()).is_empty());
    }

    #[test]
    fn a_message_of_no_content_word_is_never_proved() {
        assert_eq!(read_one("the"), FastOutcome::None);
    }

    #[test]
    fn removing_a_value_leaves_the_rest_of_the_message() {
        assert_eq!(
            without(
                &content_words("open firefox now"),
                &content_words("firefox")
            ),
            Some(vec!["open".to_string(), "now".to_string()])
        );
        assert_eq!(
            without(&content_words("open obs"), &content_words("firefox")),
            None
        );
    }
}
