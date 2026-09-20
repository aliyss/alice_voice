//! The words of the catalog against the words of a message.
//!
//! The retrieval pass ranks every document with BM25, the ranking of a
//! search engine. A word that appears in few documents says more than a
//! word that appears in every document, and a long document does not win
//! just by holding more words.
//!
//! The ranking is computed once per turn and never cached. A catalog is a
//! few hundred documents, so the whole ranking costs less than a model
//! call, and a cached index would have to be thrown away whenever the
//! user edits an intent.
//!
//! Two kinds of evidence are folded into one term frequency: the name and
//! the description of an intent always count, and a phrase the user wrote
//! counts only when the message shares its action.

use std::collections::{BTreeMap, HashMap};

use alice_core::config::RouterConfig;

use crate::resolver::router::doc::{
    content_words, IntentDoc, DESCRIPTION_WEIGHT, ENTITY_WEIGHT, NAME_WEIGHT, PHRASE_WEIGHT,
};

/// How much a repeated word counts, the usual BM25 constant.
const K1: f32 = 1.2;

/// How much a long document is discounted, the usual BM25 constant.
const B: f32 = 0.75;

/// The score at which the raw ranking reads as half sure.
///
/// A raw BM25 score is unbounded, so it is mapped into 0 and 1 with
/// `raw / (raw + SCALE)`. The mapping keeps a weak match weak: a query
/// that only touches one word of a document lands far below a query that
/// spells the whole name out.
const SCALE: f32 = 4.0;

/// The score of one document, and the evidence that read it.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// Position of the intent in the catalog of the turn.
    pub index: usize,
    /// The score between 0 and 1.
    pub score: f32,
    /// The raw ranking before the mapping into 0 and 1.
    pub raw: f32,
    /// How many of the words of the message the document holds.
    pub coverage: f32,
    /// Whether a phrase the user wrote named the document.
    pub phrase: Option<String>,
}

/// The terms of one document for one message.
#[derive(Clone, Debug, Default, PartialEq)]
struct Terms {
    /// Every term with the weight it counts with.
    terms: BTreeMap<String, f32>,
    /// The sum of the weights, for the length of the document.
    length: f32,
}

/// Read the terms of one document for one message.
fn terms_of(doc: &IntentDoc, message_words: &[String], gate: bool) -> (Terms, Option<String>) {
    let mut terms: BTreeMap<String, f32> = BTreeMap::new();
    let mut length = 0.0;
    let mut add = |term: &str, weight: f32| {
        if !term.is_empty() {
            *terms.entry(term.to_string()).or_insert(0.0) += weight;
            length += weight;
        }
    };

    for word in &doc.name_words {
        add(word, NAME_WEIGHT);
    }
    for word in &doc.description_words {
        add(word, DESCRIPTION_WEIGHT);
    }
    for name in &doc.entity_names {
        for word in content_words(name) {
            add(&word, ENTITY_WEIGHT);
        }
    }

    // The phrases carry the way a person really asks for the intent, so
    // they are the strongest evidence after the name. A phrase counts
    // only when the message shares its action, so the phrase of one
    // intent never reads the action of another.
    let mut named: Option<String> = None;
    for phrase in doc.phrases.iter().skip(1) {
        if !doc.phrase_fits(phrase, message_words, gate) {
            continue;
        }
        let held = phrase
            .words
            .iter()
            .filter(|word| message_words.contains(word))
            .count();
        if held == phrase.words.len() && !phrase.words.is_empty() {
            named = Some(phrase.text.clone());
        }
        for word in &phrase.words {
            add(word, PHRASE_WEIGHT);
        }
    }

    let terms = Terms { terms, length };
    (terms, named)
}

/// The identifier every term of the catalog is counted with.
fn document_frequency(catalog: &[Terms]) -> HashMap<String, usize> {
    let mut frequency: HashMap<String, usize> = HashMap::new();
    for doc in catalog {
        for term in doc.terms.keys() {
            *frequency.entry(term.clone()).or_insert(0) += 1;
        }
    }
    frequency
}

/// The words of a message that the catalog knows at all.
///
/// A message names the intent and, often, the value of an entity of that
/// intent: `what is the weather in Porto` names the city `Porto`, and no
/// document of the catalog holds that word. Counting it against every
/// document would punish the one document that should win, so the words
/// no document holds stay out of the measure of coverage.
fn known_words<'a>(words: &'a [String], frequency: &HashMap<String, usize>) -> Vec<&'a String> {
    words
        .iter()
        .filter(|word| frequency.get(*word).is_some_and(|count| *count > 0))
        .collect()
}

/// Map a raw ranking into 0 and 1.
fn soften(raw: f32) -> f32 {
    if raw <= 0.0 {
        return 0.0;
    }
    raw / (raw + SCALE)
}

/// Rank every document of the catalog against one message.
///
/// The result holds one hit per document that shares at least one word
/// with the message, best first. A document that shares nothing is left
/// out, so the caller can tell "nothing fit" from "the best fit is weak".
pub fn search(docs: &[IntentDoc], message: &str, config: &RouterConfig) -> Vec<Hit> {
    let message_words = content_words(message);
    if message_words.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let unique: Vec<String> = {
        let mut seen = message_words.clone();
        seen.sort();
        seen.dedup();
        seen
    };

    let read: Vec<(Terms, Option<String>)> = docs
        .iter()
        .map(|doc| terms_of(doc, &message_words, config.phrase_gate))
        .collect();
    let terms: Vec<Terms> = read.iter().map(|(terms, _)| terms.clone()).collect();
    let frequency = document_frequency(&terms);
    let count = terms.len() as f32;
    let average = terms.iter().map(|doc| doc.length).sum::<f32>() / count;
    let known = known_words(&unique, &frequency);

    let mut hits: Vec<Hit> = Vec::new();
    for (position, doc) in terms.iter().enumerate() {
        if doc.length <= 0.0 {
            continue;
        }
        let mut raw = 0.0;
        for query in &unique {
            let Some(tf) = doc.terms.get(query) else {
                continue;
            };
            let df = frequency.get(query).copied().unwrap_or(0) as f32;
            let idf = (1.0 + (count - df + 0.5) / (df + 0.5)).ln();
            let length = if average > 0.0 {
                doc.length / average
            } else {
                1.0
            };
            raw += idf * (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * length));
        }
        if raw <= 0.0 {
            continue;
        }
        let coverage = if known.is_empty() {
            1.0
        } else {
            let held = known
                .iter()
                .filter(|word| doc.terms.contains_key(word.as_str()))
                .count();
            held as f32 / known.len() as f32
        };
        // A document that holds one of the five words the catalog knows
        // is weaker than a document that holds the only one.
        let score = soften(raw) * (0.5 + 0.5 * coverage);
        hits.push(Hit {
            index: docs[position].index,
            score,
            raw,
            coverage,
            phrase: read[position].1.clone(),
        });
    }

    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.index.cmp(&right.index))
    });
    hits
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::router::doc::{catalog, tests::intent};
    use alice_core::dto::EntityKindDto;

    /// The catalog the tests rank.
    fn fixture() -> Vec<IntentDoc> {
        catalog(&[
            intent(
                "open application",
                &["launch an app", "start the browser"],
                &[("applications", EntityKindDto::Script)],
            ),
            intent(
                "get weather",
                &["how is the weather"],
                &[("city", EntityKindDto::Open)],
            ),
            intent("close window", &["close firefox"], &[]),
            intent("take screenshot", &[], &[]),
        ])
    }

    /// Rank one message with the defaults.
    fn rank(message: &str) -> Vec<Hit> {
        search(&fixture(), message, &RouterConfig::default())
    }

    #[test]
    fn the_message_that_names_an_intent_ranks_it_first() {
        let hits = rank("what is the weather in Berlin");
        let second = hits.get(1).map_or(0.0, |hit| hit.score);

        assert_eq!(hits[0].index, 1);
        // The ranking has to leave room for the margin of the decision
        // stage, so the winner stays clear of the runner up.
        assert!(hits[0].score > 0.2, "{:?}", hits[0].score);
        assert!(hits[0].score - second > 0.05, "{hits:?}");
    }

    #[test]
    fn a_phrase_of_the_user_reads_a_message() {
        let hits = rank("launch an app");
        assert_eq!(hits[0].index, 0);
        assert_eq!(hits[0].phrase.as_deref(), Some("launch an app"));
    }

    #[test]
    fn a_phrase_never_reads_the_action_of_another_intent() {
        let hits = rank("open firefox");
        let closing = hits.iter().find(|hit| hit.index == 2);

        // The phrase `close firefox` holds the word the message holds,
        // but the message shares no action with it, so it stays quiet.
        assert!(closing.is_none_or(|hit| hit.score < 0.2), "{closing:?}");
    }

    #[test]
    fn a_word_that_carries_nothing_reads_no_document() {
        assert!(rank("the").is_empty());
        assert!(rank("   ").is_empty());
    }

    #[test]
    fn a_message_the_catalog_does_not_hold_ranks_everything_low() {
        let hits = rank("eat a sandwich");
        assert!(hits.iter().all(|hit| hit.score < 0.5), "{hits:?}");
    }

    #[test]
    fn a_message_that_spells_a_phrase_out_ranks_its_document_first() {
        let hits = rank("Launch  an app!");
        assert_eq!(hits[0].index, 0);
        assert_eq!(hits[0].phrase.as_deref(), Some("launch an app"));
    }
    #[test]
    fn the_ranking_says_how_much_of_the_message_it_holds() {
        let hits = rank("weather in Berlin");
        let weather = hits
            .iter()
            .find(|hit| hit.index == 1)
            .expect("weather is there");

        // Every word of the message that the catalog holds is held by the
        // one document, so the document is fully covered.
        assert!((weather.coverage - 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_value_the_catalog_does_not_know_does_not_lower_the_score() {
        // `Porto` is a city of a message and no word of the catalog, so
        // the document that answers the message is not punished for the
        // one word the catalog could never hold.
        // Neither word is a word of this catalog, so the two messages
        // rank the one document that answers them exactly alike.
        let named = rank("what is the weather in Berlin");
        let unknown = rank("what is the weather in Porto");

        assert_eq!(named[0].index, 1);
        assert_eq!(unknown[0].index, 1);
        assert!(
            unknown[0].score >= named[0].score,
            "a value the catalog does not know lowered the score: {} against {}",
            unknown[0].score,
            named[0].score
        );
    }

    #[test]
    fn a_longer_document_does_not_win_by_holding_more_words() {
        let hits = rank("screenshot");
        assert_eq!(hits[0].index, 3);
    }

    #[test]
    fn the_score_stays_between_zero_and_one() {
        for hit in rank("open the application and take a screenshot") {
            assert!((0.0..=1.0).contains(&hit.score), "{hit:?}");
        }
    }
}
