//! The fixture benchmark of the layered router.
//!
//! A scoring constant is a decision about how sure a match has to be, so a
//! constant moves only when a measurement moves with it. This module reads
//! the fixture set in `backend/config/router-eval.json`, runs the catalog
//! the daemon ships against it with the stages that need no model, and
//! reports what the router did.
//!
//! Three numbers matter, in this order:
//!
//! - **the wrong-intent rate**, because a command that runs for the wrong
//!   intent is worse than a refusal, and worse than an ask.
//! - **the exact rate**, how often the router read the intent the fixture
//!   holds.
//! - **the abstain rate**, how often a message the catalog does not hold
//!   met no intent at all.
//!
//! The benchmark runs with the words of the catalog alone, because a
//! model server is not part of a test run. The same fixture set measures
//! the other stages once a server answers.

use std::collections::BTreeMap;

use serde::Deserialize;

use alice_core::config::RouterConfig;
use alice_core::dto::{EntityKindDto, IntentDto, IntentEntityDto};

use crate::resolver::router::doc::{catalog, IntentDoc};
use crate::resolver::router::fastpath::{self, FastOutcome};
use crate::resolver::router::lexical;
use crate::resolver::router::{decide_by_score, Candidate};

/// The fixture set, read at compile time so a test run needs no file.
const FIXTURES: &str = include_str!("../../../../../config/router-eval.json");

/// One case of the fixture set.
#[derive(Clone, Debug, Deserialize)]
struct Case {
    /// The message the user sent.
    message: String,
    /// The name of the intent the catalog holds for it, or null for none.
    intent: Option<String>,
    /// A sentence about why the case holds what it holds.
    #[serde(rename = "$comment", default)]
    #[allow(dead_code)]
    comment: Option<String>,
}

/// The fixture set.
#[derive(Clone, Debug, Deserialize)]
struct Fixtures {
    /// The values of the list entities of the catalog, by intent name.
    lists: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// The cases.
    cases: Vec<Case>,
}

/// One intent the ranking offered, with its score.
#[derive(Clone, Debug, PartialEq)]
struct Offer {
    /// The name of the intent.
    name: String,
    /// The score between 0 and 1.
    score: f32,
}

/// What one case did.
#[derive(Clone, Debug, PartialEq)]
enum Read {
    /// The router chose this intent at this stage.
    Chosen {
        /// The name of the intent.
        intent: String,
        /// The stage that decided.
        stage: &'static str,
        /// The short list the stage decided from, best first.
        short: Vec<Offer>,
    },
    /// The router refused the message.
    Refused {
        /// The stage that refused.
        stage: &'static str,
        /// The short list the stage refused from, best first.
        short: Vec<Offer>,
    },
}

impl Read {
    /// Whether the read names an intent.
    fn intent(&self) -> Option<&str> {
        match self {
            Self::Chosen { intent, .. } => Some(intent),
            Self::Refused { .. } => None,
        }
    }

    /// The short list the stage read, best first.
    fn short(&self) -> &[Offer] {
        match self {
            Self::Chosen { short, .. } | Self::Refused { short, .. } => short,
        }
    }

    /// The three best offers as one line of a report.
    fn offers(&self) -> String {
        let read: Vec<String> = self
            .short()
            .iter()
            .take(3)
            .map(|offer| format!("{} {:.2}", offer.name, offer.score))
            .collect();
        if read.is_empty() {
            "nothing".to_string()
        } else {
            read.join(", ")
        }
    }
}

/// What the whole fixture set did.
#[derive(Clone, Debug, Default)]
struct Report {
    /// Number of cases.
    cases: usize,
    /// Cases the router read exactly right.
    exact: usize,
    /// Cases the router sent to an intent the fixture does not hold.
    wrong: usize,
    /// Cases the fixture refuses and the router refused.
    refused: usize,
    /// Cases the fixture refuses and the router chose an intent anyway.
    missed: usize,
    /// Cases the fixture holds and the router refused.
    over_refused: usize,
    /// Cases the deterministic pass decided.
    fast_path: usize,
    /// Cases the ranking decided.
    ranked: usize,
    /// The cases the router read wrong, for the report.
    failures: Vec<String>,
}

impl Report {
    /// Read one case into the report.
    fn add(&mut self, case: &Case, read: &Read) {
        self.cases += 1;
        match read {
            Read::Chosen { stage, .. } if *stage == "fast_path" => self.fast_path += 1,
            Read::Chosen { .. } => self.ranked += 1,
            Read::Refused { .. } => {}
        }
        let found = read.intent();
        match (&case.intent, found) {
            (Some(wanted), Some(actual)) if wanted == actual => self.exact += 1,
            (Some(wanted), Some(actual)) => {
                self.wrong += 1;
                self.failures.push(format!(
                    "{} → {actual} (expected {wanted}) · offers {}",
                    case.message,
                    read.offers()
                ));
            }
            (Some(wanted), None) => {
                self.over_refused += 1;
                self.failures.push(format!(
                    "{} → no intent (expected {wanted}) · offers {}",
                    case.message,
                    read.offers()
                ));
            }
            (None, None) => self.refused += 1,
            (None, Some(actual)) => {
                self.missed += 1;
                self.failures.push(format!(
                    "{} → {actual} (expected no intent) · offers {}",
                    case.message,
                    read.offers()
                ));
            }
        }
    }

    /// Write the report the reader reads when a bound fails.
    fn text(&self) -> String {
        let percent = |part: usize| {
            if self.cases == 0 {
                0.0
            } else {
                part as f32 * 100.0 / self.cases as f32
            }
        };
        let mut lines = vec![format!(
            "cases {} · exact {:.0}% · wrong {} ({:.0}%) · refused {} · missed {} · over-refused {} · fast path {} · ranked {}",
            self.cases,
            percent(self.exact),
            self.wrong,
            percent(self.wrong),
            self.refused,
            self.missed,
            self.over_refused,
            self.fast_path,
            self.ranked,
        )];
        for failure in &self.failures {
            lines.push(format!("  {failure}"));
        }
        lines.join("\n")
    }
}

/// Read the fixture set.
fn fixtures() -> Fixtures {
    serde_json::from_str(FIXTURES).expect("the fixture set is valid JSON")
}

/// Build the catalog of the daemon as intents.
pub(crate) fn intents() -> Vec<IntentDto> {
    crate::intent::defaults::example_intents()
        .into_iter()
        .map(|input| IntentDto {
            id: uuid::Uuid::new_v4(),
            name: input.name,
            description: input.description,
            command: input.command,
            entities: input
                .entities
                .into_iter()
                .map(|entity| IntentEntityDto {
                    id: uuid::Uuid::new_v4(),
                    name: entity.name,
                    kind: entity.kind,
                    values: entity.values,
                    script: entity.script,
                    required: entity.required,
                })
                .collect(),
            examples: input.examples,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .collect()
}

/// Read one message with the stages that need no model.
fn read_one(docs: &[IntentDoc], lists: &Lists, message: &str, config: &RouterConfig) -> Read {
    let planned = fastpath::plan(docs, message, config);
    let offered = lists.of(docs, &planned);
    if config.fast_path {
        if let FastOutcome::Match(found) = fastpath::read(docs, message, &offered, config) {
            return Read::Chosen {
                intent: docs[found.index].name.clone(),
                stage: "fast_path",
                short: vec![Offer {
                    name: docs[found.index].name.clone(),
                    score: 1.0,
                }],
            };
        }
    }

    let hits = lexical::search(docs, message, config);
    let short: Vec<Candidate> = hits
        .iter()
        .take(config.top_k)
        .map(|hit| Candidate {
            index: hit.index,
            score: hit.score,
            evidence: "words".to_string(),
        })
        .collect();
    let offers: Vec<Offer> = short
        .iter()
        .map(|candidate| Offer {
            name: docs[candidate.index].name.clone(),
            score: candidate.score,
        })
        .collect();
    match decide_by_score(&short, config) {
        crate::resolver::router::Route::Chosen { index, .. } => Read::Chosen {
            intent: docs[index].name.clone(),
            stage: "retrieve",
            short: offers,
        },
        crate::resolver::router::Route::Refused { .. } => Read::Refused {
            stage: "retrieve",
            short: offers,
        },
    }
}

/// The lists of the fixture set, by the position of an intent.
struct Lists {
    /// The values of every list entity of every intent, by intent name.
    values: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl Lists {
    /// Read the lists of one plan.
    ///
    /// The closed entities of the catalog carry their values, and the
    /// script entities take theirs from the fixture set, because a test
    /// run does not run the scripts of the machine.
    fn of(
        &self,
        docs: &[IntentDoc],
        planned: &[usize],
    ) -> BTreeMap<usize, BTreeMap<String, Vec<String>>> {
        let mut offered: BTreeMap<usize, BTreeMap<String, Vec<String>>> = BTreeMap::new();
        for index in planned {
            let Some(doc) = docs.get(*index) else {
                continue;
            };
            let values = self.values.get(&doc.name).cloned().unwrap_or_default();
            offered.insert(*index, values);
        }
        offered
    }
}

/// Add the closed entities of the catalog to the lists of the fixture set.
fn lists(fixtures: &Fixtures, intents: &[IntentDto]) -> Lists {
    let mut values = fixtures.lists.clone();
    for intent in intents {
        let entry = values.entry(intent.name.clone()).or_default();
        for entity in &intent.entities {
            if matches!(entity.kind, EntityKindDto::Closed) {
                entry.insert(entity.name.clone(), entity.values.clone());
            }
        }
    }
    Lists { values }
}

/// Run the whole fixture set.
fn run(config: &RouterConfig) -> Report {
    let fixtures = fixtures();
    let intents = intents();
    let docs = catalog(&intents);
    let lists = lists(&fixtures, &intents);

    let mut report = Report::default();
    for case in &fixtures.cases {
        let read = read_one(&docs, &lists, &case.message, config);
        report.add(case, &read);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::config::RetrieveEngine;

    #[test]
    fn the_fixture_set_reads() {
        let fixtures = fixtures();
        assert!(fixtures.cases.len() >= 60, "{}", fixtures.cases.len());
        assert!(
            fixtures.cases.iter().any(|case| case.intent.is_none()),
            "the set holds no message the catalog does not answer"
        );
        assert!(
            fixtures.cases.iter().any(|case| case.intent.is_some()),
            "the set holds no message the catalog answers"
        );
    }

    #[test]
    fn the_router_reads_the_fixture_set() {
        let config = RouterConfig::default();
        let report = run(&config);
        println!("{}", report.text());

        // A command that runs for the wrong intent is the failure that
        // costs the most, so it is held to zero before the accuracy.
        assert_eq!(
            report.wrong,
            0,
            "the router sent {} cases to the wrong intent\n{}",
            report.wrong,
            report.text()
        );
        assert_eq!(
            report.missed,
            0,
            "the router chose an intent for {} messages the catalog does not hold\n{}",
            report.missed,
            report.text()
        );
        assert!(
            report.exact * 100 >= report.cases * 85,
            "the router read fewer than 85 percent of the cases exactly\n{}",
            report.text()
        );
        assert!(
            report.over_refused <= 1,
            "the router refused {} cases the catalog holds\n{}",
            report.over_refused,
            report.text()
        );
        assert!(
            report.fast_path > 0,
            "the deterministic pass decided nothing\n{}",
            report.text()
        );
    }

    #[test]
    fn the_words_alone_read_the_catalog() {
        // The fixture set is the contract of the words of the catalog: a
        // reader that needs a model to pass it is a reader the daemon
        // cannot fall back to.
        let config = RouterConfig {
            retrieve: RetrieveEngine::Lexical,
            ..RouterConfig::default()
        };
        let report = run(&config);
        assert_eq!(report.wrong, 0, "{}", report.text());
        assert!(report.exact * 100 >= report.cases * 85, "{}", report.text());
    }

    #[test]
    fn the_deterministic_pass_carries_most_of_the_turns() {
        // The pass costs nothing and never guesses, so most of a real
        // catalog has to reach it. A change that moves the work to the
        // ranking is a change that costs latency on every turn.
        let report = run(&RouterConfig::default());
        assert!(
            report.fast_path * 100 >= report.cases * 60,
            "the deterministic pass read fewer than 60 percent of the cases\n{}",
            report.text()
        );
    }
}
