//! The layered intent router.
//!
//! A flat model over the whole catalog meets a wall. A span reader treats
//! every intent as a label that competes with every other label, so the
//! catalog has to stay small, a large label set costs accuracy, and the
//! phrases a user wrote fight each other. A language model asked to
//! choose among the whole catalog needs one option per intent, so the
//! catalog has a hard ceiling there too.
//!
//! The router reads a message in stages and stops at the first stage that
//! can answer. Each stage is switched on its own, and each stage has a
//! reader that needs no model at all, so the daemon keeps working when a
//! server is down and a catalog of any size stays readable.
//!
//! ```text
//! message
//!   │
//!   ├─ 1. deterministic pass   prove it from the message alone       no model
//!   │
//!   ├─ 2. retrieval pass       rank every intent, keep the top K     words, vectors
//!   │
//!   ├─ 3. decision pass        choose one of the top K, or none       scores, model
//!   │
//!   └─ 4. extraction pass      read the values of the chosen intent   lists, spans, model
//! ```
//!
//! The stages live in this module: the pass that proves a message in
//! [`fastpath`], the ranking in [`lexical`] and [`embed`], the catalog
//! documents in [`doc`], and the decision in this file.

pub mod doc;
pub mod embed;
#[cfg(test)]
pub(crate) mod eval;
pub mod fastpath;
pub mod lexical;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use alice_core::config::{CoreConfig, DecideEngine, EmbedSource, RouterConfig, RouterStage};

use crate::resolver::client::{DecisionRequest, LlamaClient};
use crate::resolver::decision::{Decision, DecisionOption};
use crate::resolver::error::ResolveError;
use crate::resolver::local::LocalEngine;
use crate::resolver::prompt::build_prompt;
use crate::resolver::router::doc::IntentDoc;
use crate::resolver::router::embed::{as_score, cosine, EmbedCache, Embedder};
use crate::resolver::router::fastpath::FastOutcome;

/// The option that states that no intent of the short list fits.
pub const NO_INTENT_OPTION: &str = "__none__";

/// The path of a score that came from the words of the catalog.
const EVIDENCE_WORDS: &str = "words";

/// The path of a score the deterministic pass proved from the message.
const EVIDENCE_FAST_PATH: &str = "fast_path";

/// The name of the stage that proves a message from the message alone.
const STEP_FAST_PATH: &str = "fast_path";

/// The name of the stage that ranks the catalog.
const STEP_RETRIEVE: &str = "retrieve";

/// The name of the stage that chooses one of the short list.
const STEP_DECIDE: &str = "decide";

/// The reader of the deterministic pass, which needs no model.
const READER_RULES: &str = "rules";

/// The reader that ranks the catalog by its words alone.
const READER_WORDS: &str = "words";

/// The reader that decides from the scores of the ranking.
const READER_SCORES: &str = "scores";

/// The reader that decides with the built in reranker.
const READER_RERANKER: &str = "reranker";

/// The reader that decides with the language model.
const READER_MODEL: &str = "model";

/// The path of a score that came from the vectors of the catalog.
const EVIDENCE_VECTOR: &str = "embedding";

/// The path of a score both evidence paths read.
const EVIDENCE_BOTH: &str = "words+embedding";

/// The path of a score the built in reranker read.
const EVIDENCE_RERANKER: &str = "reranker";

/// Address and model name of the server the decision stage asks.
#[derive(Clone, Debug)]
pub struct RouteModel {
    /// Base URL of the model server.
    pub base_url: String,
    /// Model name the server answers to.
    pub model: String,
}

/// One intent the retrieval pass offered for a message.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// Position of the intent in the catalog of the turn.
    pub index: usize,
    /// The score between 0 and 1.
    pub score: f32,
    /// How the score was read.
    pub evidence: String,
}

/// How one stage of the router ended.
///
/// A stage that ran and answered, a stage that read the message and
/// refused it, and a stage that only handed the turn on are told apart, so
/// the route of a turn reads as what really happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    /// The stage answered the turn.
    Matched,
    /// The stage read the message and refused it.
    Refused,
    /// The stage found nothing and handed the turn on.
    Passed,
    /// The reader the stage wanted did not answer, so it ran its fallback.
    FellBack,
    /// The stage did not run for this turn.
    Skipped,
}

impl StepOutcome {
    /// The reported form of the outcome.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Matched => "matched",
            Self::Refused => "refused",
            Self::Passed => "passed",
            Self::FellBack => "fell_back",
            Self::Skipped => "skipped",
        }
    }
}

/// One stage of the router in one turn.
///
/// A stage reports the reader it ran, what that reader read, and how long
/// it took, so a stored turn says where its time went and which reader
/// really answered it rather than which reader the settings name.
#[derive(Clone, Debug)]
pub struct RouteStep {
    /// The stage: `fast_path`, `retrieve`, or `decide`.
    pub stage: &'static str,
    /// How the stage ended.
    pub outcome: StepOutcome,
    /// The reader the stage ran.
    pub reader: String,
    /// The model the reader ran, or null when it ran none.
    pub model: Option<String>,
    /// One sentence about what the stage read, or null.
    pub detail: Option<String>,
    /// The candidates the stage read, best first.
    pub candidates: Vec<Candidate>,
    /// How long the stage took.
    pub duration: Duration,
}

/// The stages one turn passed, in the order they ran.
///
/// The router writes the route as it reads, and the caller keeps it with
/// the turn, so the metadata of a stored message names every stage the
/// message met instead of the last one alone.
#[derive(Clone, Debug, Default)]
pub struct RouteTrace {
    /// One entry per stage the turn reached.
    pub steps: Vec<RouteStep>,
}

impl RouteTrace {
    /// Record one stage of the turn.
    pub fn record(&mut self, step: RouteStep) {
        self.steps.push(step);
    }
}

/// What the router read out of one message.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// One intent of the catalog answers the message.
    Chosen {
        /// Position of the intent in the catalog of the turn.
        index: usize,
        /// The stage that decided the turn.
        stage: RouterStage,
        /// How sure the stage is, between 0 and 1.
        confidence: Option<f32>,
        /// The short list the stage decided from, best first.
        candidates: Vec<Candidate>,
        /// One sentence about what the message said, or none.
        reason: Option<String>,
        /// The values the deterministic pass read, if it decided.
        values: BTreeMap<String, String>,
    },
    /// No intent of the catalog fits the message.
    Refused {
        /// The stage that refused.
        stage: RouterStage,
        /// The short list the stage refused from, best first.
        candidates: Vec<Candidate>,
        /// Why the stage refused.
        reason: String,
    },
}
/// The input of one routing.
pub struct RouteInput<'a> {
    /// The catalog of the turn as documents.
    pub docs: &'a [IntentDoc],
    /// The text the user sent.
    pub text: &'a str,
    /// The values of the list entities of the planned documents.
    pub lists: &'a BTreeMap<usize, BTreeMap<String, Vec<String>>>,
    /// The stages of the router for this turn.
    pub config: &'a RouterConfig,
    /// The server the decision stage asks.
    pub model_settings: &'a RouteModel,
}

/// The router that reads one message in stages.
#[derive(Clone, Debug)]
pub struct Router {
    /// The client that asks the model server.
    client: LlamaClient,
    /// The engine that runs a built in model.
    local: LocalEngine,
    /// The configuration of the daemon.
    config: Arc<CoreConfig>,
    /// The vectors the daemon already read.
    cache: Arc<EmbedCache>,
}

impl Router {
    /// Create a new router.
    pub fn new(client: LlamaClient, local: LocalEngine, config: Arc<CoreConfig>) -> Self {
        Self {
            client,
            local,
            config,
            cache: Arc::new(EmbedCache::new()),
        }
    }

    /// A reader of vectors for one turn, from the place the settings name.
    ///
    /// The reader shares the cache of this router. The values of a list do
    /// not change between two turns and the documents of a catalog change
    /// only when the user edits an intent, so the vectors a turn already
    /// read are read again by the next turn for nothing.
    pub fn embedder(&self, settings: &RouterConfig, model: &RouteModel) -> Embedder {
        let cache = Arc::clone(&self.cache);
        match settings.embed_source {
            EmbedSource::Server => Embedder::new(
                self.client.clone(),
                &model.base_url,
                &settings.embed_model,
                Duration::from_secs(self.config.resolver.timeout_secs),
                cache,
            ),
            EmbedSource::Local => Embedder::local(
                self.local.clone(),
                &settings.embed_local_model,
                settings.local_device,
                cache,
            ),
        }
    }

    /// The documents whose lists the deterministic pass wants to read.
    pub fn plan(docs: &[IntentDoc], text: &str, config: &RouterConfig) -> Vec<usize> {
        if !config.fast_path {
            return Vec::new();
        }
        fastpath::plan(docs, text, config)
    }

    /// Read one message.
    ///
    /// The stages run in order and the first one that can answer ends the
    /// turn. A stage that cannot run its reader falls back to the cheaper
    /// reader of the same stage, so a message is never lost because the
    /// model server is down.
    ///
    /// The caller passes the trace of the turn, and every stage writes
    /// what it read into it, so the metadata of a stored message names the
    /// whole route rather than the stage that happened to answer.
    pub async fn route<F>(
        &self,
        input: &RouteInput<'_>,
        trace: &mut RouteTrace,
        on_delta: &mut F,
    ) -> Result<Route, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        // 1. The deterministic pass.
        if input.config.fast_path {
            let started = Instant::now();
            if let FastOutcome::Match(found) =
                fastpath::read(input.docs, input.text, input.lists, input.config)
            {
                tracing::debug!(
                    intent = %input.docs[found.index].name,
                    reason = %found.reason,
                    "the deterministic pass read the message"
                );
                trace.record(RouteStep {
                    stage: STEP_FAST_PATH,
                    outcome: StepOutcome::Matched,
                    reader: READER_RULES.to_string(),
                    model: None,
                    detail: Some(found.reason.clone()),
                    candidates: vec![Candidate {
                        index: found.index,
                        score: 1.0,
                        evidence: EVIDENCE_FAST_PATH.to_string(),
                    }],
                    duration: started.elapsed(),
                });
                return Ok(Route::Chosen {
                    index: found.index,
                    stage: RouterStage::FastPath,
                    confidence: Some(1.0),
                    candidates: vec![Candidate {
                        index: found.index,
                        score: 1.0,
                        evidence: EVIDENCE_FAST_PATH.to_string(),
                    }],
                    reason: Some(found.reason),
                    values: found.values,
                });
            }
            trace.record(RouteStep {
                stage: STEP_FAST_PATH,
                outcome: StepOutcome::Passed,
                reader: READER_RULES.to_string(),
                model: None,
                detail: Some("the catalog proved nothing from the message alone".to_string()),
                candidates: Vec::new(),
                duration: started.elapsed(),
            });
        } else {
            trace.record(RouteStep {
                stage: STEP_FAST_PATH,
                outcome: StepOutcome::Skipped,
                reader: READER_RULES.to_string(),
                model: None,
                detail: Some("the pass is off".to_string()),
                candidates: Vec::new(),
                duration: Duration::ZERO,
            });
        }

        // 2. The retrieval pass.
        let started = Instant::now();
        let (scored, no_vectors) = self.retrieve(input).await;
        if scored.is_empty() {
            trace.record(RouteStep {
                stage: STEP_RETRIEVE,
                outcome: StepOutcome::Refused,
                reader: READER_WORDS.to_string(),
                model: None,
                detail: Some("no intent shares a word or a vector with the message".to_string()),
                candidates: Vec::new(),
                duration: started.elapsed(),
            });
            return Ok(Route::Refused {
                stage: RouterStage::None,
                candidates: Vec::new(),
                reason: "no intent shares a word or a vector with the message".to_string(),
            });
        }
        let short: Vec<Candidate> = scored.iter().take(input.config.top_k).cloned().collect();
        trace.record(retrieval_step(
            input,
            scored.len(),
            &short,
            no_vectors,
            started.elapsed(),
        ));

        // 3. The decision pass.
        let started = Instant::now();
        match input.config.decide {
            DecideEngine::Generative => match self.ask_model(input, &short, on_delta).await {
                Ok(Some(route)) => {
                    trace.record(decision_step(
                        &route,
                        READER_MODEL,
                        Some(input.model_settings.model.clone()),
                        choice_note(&route, input.docs),
                        false,
                        started.elapsed(),
                    ));
                    Ok(route)
                }
                Ok(None) => {
                    let route = Route::Refused {
                        stage: RouterStage::Rerank,
                        candidates: short,
                        reason: "the model chose no intent of the short list".to_string(),
                    };
                    trace.record(decision_step(
                        &route,
                        READER_MODEL,
                        Some(input.model_settings.model.clone()),
                        choice_note(&route, input.docs),
                        false,
                        started.elapsed(),
                    ));
                    Ok(route)
                }
                Err(err) => {
                    // A server that does not answer must not cost the turn
                    // its intent: the scores of the ranking can still
                    // decide, so the stage falls back to them.
                    tracing::warn!(
                        error = %err,
                        "the model did not answer, the scores of the ranking decide"
                    );
                    let route = decide_by_score(&short, input.config);
                    trace.record(decision_step(
                        &route,
                        READER_SCORES,
                        None,
                        fallback_note(
                            "the model did not answer",
                            &err.to_string(),
                            &route,
                            input.docs,
                        ),
                        true,
                        started.elapsed(),
                    ));
                    Ok(route)
                }
            },
            DecideEngine::Rerank => match self.rerank(input, &short).await {
                Ok(route) => {
                    trace.record(decision_step(
                        &route,
                        READER_RERANKER,
                        Some(input.config.rerank_model.clone()),
                        choice_note(&route, input.docs),
                        false,
                        started.elapsed(),
                    ));
                    Ok(route)
                }
                Err(err) => {
                    // A built in model that is not on disk, or a device
                    // that cannot run it, must not cost the turn its
                    // intent either, so the scores of the ranking decide.
                    tracing::warn!(
                        error = %err,
                        model = %input.config.rerank_model,
                        "the built in reranker did not answer, the scores of the ranking decide"
                    );
                    let route = decide_by_score(&short, input.config);
                    trace.record(decision_step(
                        &route,
                        READER_SCORES,
                        None,
                        fallback_note(
                            "the built in reranker did not answer",
                            &err.to_string(),
                            &route,
                            input.docs,
                        ),
                        true,
                        started.elapsed(),
                    ));
                    Ok(route)
                }
            },
            DecideEngine::Score => {
                let route = decide_by_score(&short, input.config);
                trace.record(decision_step(
                    &route,
                    READER_SCORES,
                    None,
                    choice_note(&route, input.docs),
                    false,
                    started.elapsed(),
                ));
                Ok(route)
            }
        }
    }

    /// Choose one of the short list with the built in reranker.
    ///
    /// Two readers answer two questions, and each one is read where it is
    /// sure.
    ///
    /// - The floor and the margin hold the ranking. A message the catalog
    ///   does not really answer is refused before a model reads it, so the
    ///   floor keeps the meaning it has for the other engines and a
    ///   hopeless message costs no read at all.
    /// - The reranker chooses among the intents the ranking allowed. The
    ///   model reads the message and one candidate in one sequence, so it
    ///   orders two plausible intents better than two separate vectors
    ///   can, and its score is sharp enough for that and not for a floor.
    ///
    /// A refusal reports the ranking it refused from, so the numbers in
    /// the reason and the numbers in the list are the same numbers.
    async fn rerank(
        &self,
        input: &RouteInput<'_>,
        short: &[Candidate],
    ) -> Result<Route, ResolveError> {
        if let Route::Refused {
            candidates, reason, ..
        } = decide_by_score(short, input.config)
        {
            return Ok(Route::Refused {
                stage: RouterStage::Rerank,
                candidates,
                reason,
            });
        }

        let engine = self.local.clone();
        let model = input.config.rerank_model.clone();
        let device = input.config.local_device;
        let message = input.text.to_string();
        let texts: Vec<String> = short
            .iter()
            .map(|candidate| input.docs[candidate.index].embed_text())
            .collect();

        let scores =
            tokio::task::spawn_blocking(move || engine.rerank(&model, device, &message, &texts))
                .await
                .map_err(|err| ResolveError::Embeddings {
                    reason: err.to_string(),
                })?
                .map_err(|err| ResolveError::Embeddings {
                    reason: err.to_string(),
                })?;
        if scores.len() != short.len() {
            return Err(ResolveError::Embeddings {
                reason: format!(
                    "the reranker scored {} of {} candidates",
                    scores.len(),
                    short.len()
                ),
            });
        }

        let mut ranked: Vec<Candidate> = short
            .iter()
            .zip(scores)
            .map(|(candidate, score)| Candidate {
                index: candidate.index,
                score,
                evidence: EVIDENCE_RERANKER.to_string(),
            })
            .collect();
        ranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.index.cmp(&right.index))
        });
        let Some(best) = ranked.first() else {
            return Ok(Route::Refused {
                stage: RouterStage::Rerank,
                candidates: ranked,
                reason: "the short list is empty".to_string(),
            });
        };
        tracing::debug!(
            intent = %input.docs[best.index].name,
            score = best.score,
            "the built in reranker chose an intent"
        );
        Ok(Route::Chosen {
            index: best.index,
            stage: RouterStage::Rerank,
            confidence: Some(best.score),
            candidates: ranked,
            reason: None,
            values: BTreeMap::new(),
        })
    }

    /// Rank every document of the catalog against the message.
    ///
    /// The words and the vectors are read apart and folded together with
    /// the weights of the settings. A vector read that fails leaves the
    /// words alone, so an embedding server that is not there costs the
    /// turn nothing.
    ///
    /// The function reports why the vectors read nothing, or null when
    /// they read the catalog, so the route of the turn names the reader
    /// that really ranked it.
    async fn retrieve(&self, input: &RouteInput<'_>) -> (Vec<Candidate>, Option<String>) {
        let words = lexical::search(input.docs, input.text, input.config);
        let mut by_words: BTreeMap<usize, f32> = BTreeMap::new();
        for hit in &words {
            by_words.insert(hit.index, hit.score);
        }

        let mut by_vectors: BTreeMap<usize, f32> = BTreeMap::new();
        let mut no_vectors: Option<String> = None;
        if input.config.retrieve.uses_embeddings() {
            match self.vectors(input).await {
                Ok(read) => by_vectors = read,
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "the embedding server did not answer, the words alone rank the catalog"
                    );
                    no_vectors = Some(err.to_string());
                }
            }
        }

        let mut candidates: BTreeMap<usize, Candidate> = BTreeMap::new();
        for doc in input.docs {
            let words_score = by_words.get(&doc.index).copied();
            let vector_score = by_vectors.get(&doc.index).copied();
            let (score, evidence) = combine(words_score, vector_score, input.config);
            if score <= 0.0 {
                continue;
            }
            candidates.insert(
                doc.index,
                Candidate {
                    index: doc.index,
                    score,
                    evidence: evidence.to_string(),
                },
            );
        }

        let mut ranked: Vec<Candidate> = candidates.into_values().collect();
        ranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.index.cmp(&right.index))
        });
        (ranked, no_vectors)
    }

    /// Read the vector of the message and of every document.
    async fn vectors(&self, input: &RouteInput<'_>) -> Result<BTreeMap<usize, f32>, ResolveError> {
        let embedder = self.embedder(input.config, input.model_settings);
        let query = embedder.vector(input.text).await?;
        let texts: Vec<String> = input.docs.iter().map(IntentDoc::embed_text).collect();
        let documents = embedder.vectors(&texts).await?;

        let mut scores: BTreeMap<usize, f32> = BTreeMap::new();
        for (doc, vector) in input.docs.iter().zip(documents) {
            let score = as_score(cosine(&query, &vector));
            if score > 0.0 {
                scores.insert(doc.index, score);
            }
        }
        Ok(scores)
    }

    /// Ask the model which intent of the short list answers the message.
    ///
    /// Only the short list reaches the model, so the prompt stays small
    /// however large the catalog is. The model may answer that no option
    /// fits, which is what keeps a weak catalog from running a command.
    async fn ask_model<F>(
        &self,
        input: &RouteInput<'_>,
        short: &[Candidate],
        on_delta: &mut F,
    ) -> Result<Option<Route>, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        let mut options: Vec<DecisionOption> = Vec::with_capacity(short.len() + 1);
        for candidate in short {
            let doc = &input.docs[candidate.index];
            options.push(DecisionOption {
                id: doc.id.to_string(),
                label: doc.name.clone(),
                detail: format!(
                    "score {:.2}; phrases: {}",
                    candidate.score,
                    doc.phrases
                        .iter()
                        .skip(1)
                        .map(|phrase| phrase.text.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        options.push(DecisionOption {
            id: NO_INTENT_OPTION.to_string(),
            label: "none of these".to_string(),
            detail: "The message asks for no option above.".to_string(),
        });

        let decision = Decision {
            state: input.text.to_string(),
            question: "Which option does the message ask for?".to_string(),
            options,
        };
        let prompt = build_prompt(&decision)?;
        let outcome = self
            .client
            .decide(
                &DecisionRequest {
                    base_url: input.model_settings.base_url.clone(),
                    model: input.model_settings.model.clone(),
                    prompt,
                    max_tokens: self.config.resolver.max_tokens,
                    thinking: self.config.resolver.thinking,
                    timeout: Duration::from_secs(self.config.resolver.timeout_secs),
                },
                on_delta,
            )
            .await?;

        if outcome.option_id == NO_INTENT_OPTION {
            return Ok(None);
        }
        let found = short
            .iter()
            .find(|candidate| input.docs[candidate.index].id.to_string() == outcome.option_id);
        let Some(found) = found else {
            return Ok(None);
        };
        tracing::debug!(
            intent = %input.docs[found.index].name,
            score = found.score,
            "the model chose an intent of the short list"
        );
        Ok(Some(Route::Chosen {
            index: found.index,
            stage: RouterStage::Rerank,
            confidence: outcome.confidence.or(Some(found.score)),
            candidates: short.to_vec(),
            reason: None,
            values: BTreeMap::new(),
        }))
    }
}

/// Record the stage that ranked the catalog.
///
/// The stage ran by definition of reaching this function, and the reader
/// it names is the one that really ranked the catalog: an embedding read
/// that did not answer leaves the words alone, so the stage reports that
/// reader and says why the other one stood aside.
fn retrieval_step(
    input: &RouteInput<'_>,
    read: usize,
    short: &[Candidate],
    no_vectors: Option<String>,
    duration: Duration,
) -> RouteStep {
    let reader = match &no_vectors {
        Some(_) => READER_WORDS,
        None => input.config.retrieve.as_str(),
    };
    let detail = match &no_vectors {
        Some(reason) => format!(
            "ranked {read} intents and kept {}; the embeddings did not answer: {reason}",
            short.len()
        ),
        None => format!("ranked {read} intents and kept the best {}", short.len()),
    };
    RouteStep {
        stage: STEP_RETRIEVE,
        outcome: if no_vectors.is_some() {
            StepOutcome::FellBack
        } else {
            StepOutcome::Passed
        },
        reader: reader.to_string(),
        model: None,
        detail: Some(detail),
        candidates: short.to_vec(),
        duration,
    }
}

/// Record the stage that chose one of the short list.
///
/// The chosen reader and the route it reached say how the stage ended, so
/// a stage that answered and a stage that refused are told apart, and a
/// stage that ran its fallback says which reader failed it.
fn decision_step(
    route: &Route,
    reader: &str,
    model: Option<String>,
    detail: String,
    fell_back: bool,
    duration: Duration,
) -> RouteStep {
    let outcome = match (route, fell_back) {
        (_, true) => StepOutcome::FellBack,
        (Route::Chosen { .. }, false) => StepOutcome::Matched,
        (Route::Refused { .. }, false) => StepOutcome::Refused,
    };
    RouteStep {
        stage: STEP_DECIDE,
        outcome,
        reader: reader.to_string(),
        model,
        detail: Some(detail),
        candidates: candidates_of(route).to_vec(),
        duration,
    }
}

/// One sentence about the choice a decision reached.
fn choice_note(route: &Route, docs: &[IntentDoc]) -> String {
    match route {
        Route::Chosen { index, .. } => docs.get(*index).map_or_else(
            || format!("chose the intent at position {index}"),
            |doc| format!("chose `{}`", doc.name),
        ),
        Route::Refused { reason, .. } => reason.clone(),
    }
}

/// One sentence about a decision that read its fallback.
fn fallback_note(what: &str, reason: &str, route: &Route, docs: &[IntentDoc]) -> String {
    format!(
        "{what} ({reason}), so the scores of the ranking decided: {}",
        choice_note(route, docs)
    )
}

/// The short list one route carries.
fn candidates_of(route: &Route) -> &[Candidate] {
    match route {
        Route::Chosen { candidates, .. } | Route::Refused { candidates, .. } => candidates,
    }
}

/// Fold the words and the vectors of one document into one score.
fn combine(words: Option<f32>, vectors: Option<f32>, config: &RouterConfig) -> (f32, &'static str) {
    match (words, vectors) {
        (Some(words), Some(vectors)) => {
            let weight = config.lexical_weight + config.dense_weight;
            if weight <= 0.0 {
                return (0.0, EVIDENCE_BOTH);
            }
            let score = (config.lexical_weight * words + config.dense_weight * vectors) / weight;
            (score, EVIDENCE_BOTH)
        }
        (Some(words), None) => (words, EVIDENCE_WORDS),
        (None, Some(vectors)) => (vectors, EVIDENCE_VECTOR),
        (None, None) => (0.0, EVIDENCE_WORDS),
    }
}

/// Decide a turn from the scores of the ranking alone.
///
/// Two rules hold the decision:
///
/// - The best score has to reach the floor. Below it the catalog did not
///   really answer the message.
/// - The best score has to stay clear of the second best by the margin.
///   Two intents that fit the message equally well mean the message is
///   ambiguous, and running the wrong command is worse than refusing.
pub fn decide_by_score(short: &[Candidate], config: &RouterConfig) -> Route {
    let Some(best) = short.first() else {
        return Route::Refused {
            stage: RouterStage::None,
            candidates: Vec::new(),
            reason: "no intent shares a word with the message".to_string(),
        };
    };
    if best.score < config.floor {
        return Route::Refused {
            stage: RouterStage::Retrieve,
            candidates: short.to_vec(),
            reason: format!(
                "the best match scores {:.2}, below the floor of {:.2}",
                best.score, config.floor
            ),
        };
    }
    if let Some(second) = short.get(1) {
        if best.score - second.score < config.margin {
            return Route::Refused {
                stage: RouterStage::Retrieve,
                candidates: short.to_vec(),
                reason: format!(
                    "the best two matches score {:.2} and {:.2}, closer than the margin of {:.2}",
                    best.score, second.score, config.margin
                ),
            };
        }
    }
    Route::Chosen {
        index: best.index,
        stage: RouterStage::Retrieve,
        confidence: Some(best.score),
        candidates: short.to_vec(),
        reason: None,
        values: BTreeMap::new(),
    }
}

/// Read the ranking of one list of hits, for the tests of the stages.
#[cfg(test)]
pub fn ranked(hits: &[crate::resolver::router::lexical::Hit]) -> Vec<Candidate> {
    hits.iter()
        .map(|hit| Candidate {
            index: hit.index,
            score: hit.score,
            evidence: EVIDENCE_WORDS.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::router::doc::{catalog, tests::intent};
    use alice_core::dto::EntityKindDto;

    /// Build one candidate for the tests.
    fn candidate(index: usize, score: f32) -> Candidate {
        Candidate {
            index,
            score,
            evidence: EVIDENCE_WORDS.to_string(),
        }
    }

    /// The catalog the tests rank.
    fn fixture() -> Vec<IntentDoc> {
        catalog(&[
            intent(
                "open application",
                &["launch an app"],
                &[("applications", EntityKindDto::Script)],
            ),
            intent("get weather", &[], &[("city", EntityKindDto::Open)]),
            intent("close window", &["close firefox"], &[]),
        ])
    }

    #[test]
    fn a_clear_winner_is_chosen() {
        let short = vec![candidate(1, 0.8), candidate(0, 0.2)];
        match decide_by_score(&short, &RouterConfig::default()) {
            Route::Chosen { index, stage, .. } => {
                assert_eq!(index, 1);
                assert_eq!(stage, RouterStage::Retrieve);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_weak_best_match_is_refused() {
        let short = vec![candidate(1, 0.20)];
        match decide_by_score(&short, &RouterConfig::default()) {
            Route::Refused { stage, reason, .. } => {
                assert_eq!(stage, RouterStage::Retrieve);
                assert!(reason.contains("below the floor"), "{reason}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn two_matches_that_tie_are_refused() {
        let short = vec![candidate(1, 0.80), candidate(0, 0.75)];
        match decide_by_score(&short, &RouterConfig::default()) {
            Route::Refused { reason, .. } => assert!(reason.contains("margin"), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_empty_short_list_is_refused_without_a_stage() {
        match decide_by_score(&[], &RouterConfig::default()) {
            Route::Refused { stage, .. } => assert_eq!(stage, RouterStage::None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_weights_fold_the_words_and_the_vectors_together() {
        let config = RouterConfig {
            lexical_weight: 3.0,
            dense_weight: 1.0,
            ..RouterConfig::default()
        };

        let (score, evidence) = combine(Some(1.0), Some(0.0), &config);

        assert!((score - 0.75).abs() < 1e-6);
        assert_eq!(evidence, EVIDENCE_BOTH);
    }

    #[test]
    fn one_kind_of_evidence_alone_carries_the_score() {
        let config = RouterConfig::default();
        assert_eq!(combine(Some(0.6), None, &config), (0.6, EVIDENCE_WORDS));
        assert_eq!(combine(None, Some(0.6), &config), (0.6, EVIDENCE_VECTOR));
        assert_eq!(combine(None, None, &config).0, 0.0);
    }

    #[test]
    fn the_plan_of_the_router_is_empty_when_the_pass_is_off() {
        let docs = fixture();
        let config = RouterConfig {
            fast_path: false,
            ..RouterConfig::default()
        };

        assert!(Router::plan(&docs, "open firefox", &config).is_empty());
        assert_eq!(
            Router::plan(&docs, "open firefox", &RouterConfig::default()),
            vec![0]
        );
    }

    #[test]
    fn the_ranking_of_a_message_is_read_back_as_candidates() {
        let config = RouterConfig::default();
        let hits = lexical::search(&fixture(), "what is the weather", &config);
        let ranked = ranked(&hits);

        assert_eq!(ranked[0].index, 1);
        assert_eq!(ranked[0].evidence, EVIDENCE_WORDS);
    }
}
