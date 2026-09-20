//! Resolver service of the daemon.
//!
//! The service turns the intent configuration and one message into a
//! decision, asks the model for the option that fits, and reads the
//! chosen intent back. It reads the server address and the model name
//! from the settings, so the settings page controls the resolver.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use uuid::Uuid;

use alice_core::config::{
    CoreConfig, DecideEngine, ExtractEngine, LocalDevice, ResolverBackend, ResolverEngine,
    RouterConfig, RouterStage,
};
use alice_core::dto::{
    ChatRoleDto, EntityKindDto, IntentDto, MessageCandidateDto, MessageEntityDto,
    MessageRouteStepDto,
};

use crate::intent::matcher::{best_match, best_mention, mentions_of, words_of};
use crate::intent::{EntityList, EntityScripts, IntentService};
use crate::resolver::client::{AnswerRequest, DecisionRequest, LlamaClient};
use crate::resolver::decision::{Decision, DecisionOption};
use crate::resolver::error::ResolveError;
use crate::resolver::gliner::GlinerResolver;
use crate::resolver::local::LocalEngine;
use crate::resolver::prompt::build_prompt;
use crate::resolver::router::{self, Route, RouteInput, RouteModel, RouteTrace, Router};
use crate::resolver::values::{build_values_prompt, parse_values};
use crate::settings::SettingsService;

/// The option that states that no intent fits the message.
pub const NO_INTENT_OPTION: &str = "__none__";

/// The name of the answer schema of a values request.
const VALUES_SCHEMA_NAME: &str = "entity_values";

/// The name of the stage that reads the values of the intent that won.
const STEP_EXTRACT: &str = "extract";

/// The reader of the values the deterministic pass proved.
const READ_FAST_PATH: &str = "fast_path";

/// The reader that matched a value to an entry of a list.
const READ_LIST: &str = "list";

/// The reader that matched a value to an entry of a list by its vectors.
const READ_EMBEDDING: &str = "embedding";

/// The reader that found the span of a label.
const READ_SPANS: &str = "spans";

/// The reader that asked a language model for a value.
const READ_MODEL: &str = "model";

/// The reader that left the value to the lists alone.
const READ_LISTS: &str = "lists";

/// One earlier turn the resolver reads as context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextTurn {
    /// The speaker of the turn.
    pub role: ChatRoleDto,
    /// The text of the turn.
    pub text: String,
}

/// One message the resolver reads.
#[derive(Clone, Debug)]
pub struct ResolveRequest {
    /// The text the user sent.
    pub text: String,
    /// The earlier turns of the conversation, oldest first.
    pub history: Vec<ContextTurn>,
}

/// The intent the resolver chose.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedIntent {
    /// The identifier of the intent.
    pub id: Uuid,
    /// The name of the intent.
    pub name: String,
    /// The command the daemon runs for the intent.
    pub command: String,
    /// The names of the entities of the intent, in name order.
    pub entities: Vec<String>,
    /// The names of the entities the intent needs a value for, in name
    /// order. The rest are optional: the command runs without them.
    pub required: Vec<String>,
    /// The value the resolver read for each entity, in name order.
    pub values: BTreeMap<String, String>,
    /// How each of those values was read, in name order. An entity the
    /// daemon read no value for appears in no entry.
    pub origins: BTreeMap<String, EntityRead>,
    /// The probability of the choice, or null when the model sent none.
    pub confidence: Option<f32>,
    /// The engine that chose the intent.
    pub intent_engine: ResolverEngine,
    /// The model that engine ran, or null when the caller cannot name it.
    pub intent_model: Option<String>,
    /// The engine that read the values, or null when the intent needs none.
    pub value_engine: Option<ResolverEngine>,
    /// The model that engine ran, or null when the intent needs none.
    pub value_model: Option<String>,
    /// How the layered router read the turn, or empty for the other
    /// engines. A turn the deterministic pass proved, a turn the ranking
    /// decided and a turn a model chose are told apart this way.
    pub route: RouteReport,
}

/// How the layered router read one turn.
///
/// The router reads a message in stages and stops at the first stage that
/// can answer, so a turn reports the stage that decided and the short list
/// it decided from. The steps name every stage the turn passed with the
/// reader it ran, which is how a user reads why a message met an intent or
/// met none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RouteReport {
    /// The stage that decided, or null when the backend is not the router.
    pub stage: Option<String>,
    /// The short list the stage decided from, best first.
    pub candidates: Vec<MessageCandidateDto>,
    /// True when the turn met an intent.
    pub matched: bool,
    /// Why the router refused, or null when it chose.
    pub reason: Option<String>,
    /// Every stage the turn passed, in the order they ran.
    pub steps: Vec<MessageRouteStepDto>,
}

impl RouteReport {
    /// Read a report out of one routing, with the names of the catalog.
    ///
    /// The router names an intent by its position in the catalog of the
    /// turn, and the metadata of a stored turn names it the way the user
    /// configured it, so the positions are read back into names here. The
    /// trace carries the stages of the turn, so a report names the whole
    /// route and not the stage that answered it alone.
    pub fn read(route: &Route, trace: &RouteTrace, intents: &[IntentDto]) -> Self {
        let (stage, candidates, reason) = match route {
            Route::Chosen {
                stage, candidates, ..
            } => (Some(*stage), candidates, None),
            Route::Refused {
                stage,
                candidates,
                reason,
                ..
            } => (Some(*stage), candidates, Some(reason.clone())),
        };
        Self {
            stage: stage.map(|stage| stage.as_str().to_string()),
            candidates: named(candidates, intents),
            matched: matches!(route, Route::Chosen { .. }),
            reason,
            steps: trace
                .steps
                .iter()
                .map(|step| MessageRouteStepDto {
                    stage: step.stage.to_string(),
                    outcome: step.outcome.as_str().to_string(),
                    reader: step.reader.clone(),
                    model: step.model.clone(),
                    detail: step.detail.clone(),
                    candidates: named(&step.candidates, intents),
                    duration_ms: step.duration.as_millis() as u64,
                })
                .collect(),
        }
    }

    /// Record the stage that read the values of the intent that won.
    ///
    /// The extraction stage runs in the service rather than in the router,
    /// so the service writes its step into the report the router built.
    pub fn record_extraction(
        &mut self,
        reader: &str,
        model: Option<String>,
        detail: String,
        duration_ms: u64,
    ) {
        self.steps.push(MessageRouteStepDto {
            stage: STEP_EXTRACT.to_string(),
            outcome: "matched".to_string(),
            reader: reader.to_string(),
            model,
            detail: Some(detail),
            candidates: Vec::new(),
            duration_ms,
        });
    }
}

/// Read the names of one short list out of the catalog of the turn.
fn named(candidates: &[router::Candidate], intents: &[IntentDto]) -> Vec<MessageCandidateDto> {
    candidates
        .iter()
        .map(|candidate| MessageCandidateDto {
            name: intents
                .get(candidate.index)
                .map_or_else(|| candidate.index.to_string(), |intent| intent.name.clone()),
            score: candidate.score,
            evidence: candidate.evidence.clone(),
        })
        .collect()
}

impl ResolvedIntent {
    /// The entities the daemon needs a value for and read none.
    ///
    /// The daemon asks the user for these instead of running the command
    /// of the intent, because a command that misses a value does the
    /// wrong thing.
    pub fn missing_required(&self) -> Vec<String> {
        self.required
            .iter()
            .filter(|name| !self.values.contains_key(*name))
            .cloned()
            .collect()
    }

    /// The optional entities the daemon read no value for.
    ///
    /// The command of the intent runs without them: the daemon removes
    /// their placeholders before it renders the command.
    pub fn missing_optional(&self) -> Vec<String> {
        self.entities
            .iter()
            .filter(|name| {
                !self.required.iter().any(|needed| needed == *name)
                    && !self.values.contains_key(*name)
            })
            .cloned()
            .collect()
    }

    /// Read the entities of the intent with the values of the turn.
    ///
    /// Every entity of the intent appears, so the transcript shows the
    /// entity the daemon could not fill as an empty value rather than
    /// leaving it out. A value carries the reader that read it, so a turn
    /// says where every value came from and not only that it has one.
    pub fn entity_values(&self) -> Vec<MessageEntityDto> {
        self.entities
            .iter()
            .map(|name| {
                let read = self.origins.get(name);
                MessageEntityDto {
                    name: name.clone(),
                    value: self.values.get(name).cloned().unwrap_or_default(),
                    source: read.map(|read| read.source.to_string()),
                    engine: read
                        .and_then(|read| read.engine)
                        .map(|engine| engine.as_str().to_string()),
                    model: read.and_then(|read| read.model.clone()),
                    read: read.and_then(|read| read.read.clone()),
                    score: read.and_then(|read| read.score),
                }
            })
            .collect()
    }
}

/// How the daemon read the value of one entity.
///
/// A value is read by one reader of the pipeline, and which reader it was
/// is what tells a value the daemon proved from a value a model guessed,
/// so the metadata of a stored turn carries the read next to the value.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityRead {
    /// The reader: `fast_path`, `list`, `embedding`, `spans`, `model`, or
    /// `lists`.
    pub source: &'static str,
    /// The engine that ran the reader, or null when the daemon read it.
    pub engine: Option<ResolverEngine>,
    /// The model the reader ran, or null when it ran none.
    pub model: Option<String>,
    /// The value the reader read before the daemon matched it to an entry
    /// of a list, or null.
    pub read: Option<String>,
    /// The similarity the match read, between 0 and 1, or null.
    pub score: Option<f32>,
}

impl EntityRead {
    /// The reader the daemon runs itself, which needs no engine.
    fn daemon(source: &'static str) -> Self {
        Self {
            source,
            engine: None,
            model: None,
            read: None,
            score: None,
        }
    }

    /// The reader one engine ran.
    fn of_engine(engine: ResolverEngine, model: Option<String>) -> Self {
        let source = match engine {
            ResolverEngine::Gliner => READ_SPANS,
            ResolverEngine::Llama => READ_MODEL,
            // The engine that reads the lists runs in the daemon itself.
            ResolverEngine::Router => READ_LISTS,
        };
        Self {
            source,
            engine: Some(engine),
            model,
            read: None,
            score: None,
        }
    }

    /// The same reader, with the value it read from reported.
    fn reading(mut self, read: Option<String>) -> Self {
        if self.read.is_none() {
            self.read = read;
        }
        self
    }
}

/// The reader of every value one engine read.
fn origins_of(
    engine: ResolverEngine,
    model: Option<String>,
    values: &BTreeMap<String, String>,
) -> BTreeMap<String, EntityRead> {
    values
        .keys()
        .map(|name| (name.clone(), EntityRead::of_engine(engine, model.clone())))
        .collect()
}

/// Record every value one reader read.
///
/// A reader reports itself once and this function reads which entities it
/// really filled: a value the map held before the reader ran and holds the
/// same after is not its work. The value that stood there before is what
/// the reader read, so a reader that matched a name to an entry of a list
/// reports the name it started from.
fn record_origins(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
    reader: &EntityRead,
    origins: &mut BTreeMap<String, EntityRead>,
) {
    for (name, value) in after {
        if before.get(name) == Some(value) {
            continue;
        }
        origins.insert(
            name.clone(),
            reader.clone().reading(before.get(name).cloned()),
        );
    }
}

/// One sentence about the values one extraction stage read.
fn values_summary(intent: &IntentDto, origins: &BTreeMap<String, EntityRead>) -> String {
    let reads: Vec<String> = intent
        .entities
        .iter()
        .filter_map(|entity| {
            origins
                .get(&entity.name)
                .map(|origin| format!("{} by {}", entity.name, origin.source))
        })
        .collect();
    if reads.is_empty() {
        return format!("read none of the {} values", intent.entities.len());
    }
    format!(
        "read {} of {} values: {}",
        reads.len(),
        intent.entities.len(),
        reads.join(", ")
    )
}

/// The result of one resolution.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolution {
    /// The resolver chose an intent.
    Matched(ResolvedIntent),
    /// The resolver read the message and chose no intent.
    ///
    /// The engine and the model are the ones that read the message, so the
    /// transcript can still report how the daemon read the turn.
    Unmatched {
        /// The engine that read the message.
        engine: ResolverEngine,
        /// The model that engine ran, or null when the caller cannot name it.
        model: Option<String>,
        /// How the layered router read the turn, or empty for the other
        /// engines.
        route: RouteReport,
    },
}

/// Address and model name of the llama.cpp server.
#[derive(Clone, Debug)]
struct LlamaSettings {
    base_url: String,
    model: String,
}

/// Model, device, and threshold of the built in GLiNER resolver.
#[derive(Clone, Debug)]
struct GlinerSettings {
    model: String,
    device: LocalDevice,
    threshold: f32,
}

/// The values of one turn while a reader fills them.
///
/// The extraction stage reads the values and the reader that read each one
/// together, so the two maps travel as one value rather than as separate
/// arguments.
struct TurnValues<'a> {
    /// The value of every entity the daemon read for the turn.
    values: &'a mut BTreeMap<String, String>,
    /// How each of those values was read.
    origins: &'a mut BTreeMap<String, EntityRead>,
    /// The settings of the router of this turn.
    config: &'a RouterConfig,
    /// The model server the stage may ask.
    model: &'a RouteModel,
}

/// The readers that may read the values of one turn.
///
/// The extraction stage of the router chooses between them per turn, so
/// the two are passed together rather than one by one.
#[derive(Clone, Copy, Debug)]
struct Readers<'a> {
    /// The server that reads a value out of a message.
    llama: &'a LlamaSettings,
    /// The built in model that finds the span of a label.
    gliner: &'a GlinerSettings,
}

/// Service that resolves the intent of one message.
///
/// Which engine reads the message follows the settings page: a llama.cpp
/// server, the built in GLiNER model, or both. The default reads the
/// intent with the language model and the entity values with GLiNER,
/// because a small language model names an action well and GLiNER finds
/// the exact span of a label well.
#[derive(Clone, Debug)]
pub struct ResolverService {
    client: LlamaClient,
    gliner: GlinerResolver,
    intents: IntentService,
    settings: SettingsService,
    config: Arc<CoreConfig>,
    /// The live lists of the script entities.
    scripts: EntityScripts,
    /// The layered router, its built in models, and the vectors it read.
    router: Router,
}

impl ResolverService {
    /// Create a new resolver service.
    pub fn new(
        client: LlamaClient,
        gliner: GlinerResolver,
        local: LocalEngine,
        intents: IntentService,
        settings: SettingsService,
        config: Arc<CoreConfig>,
        scripts: EntityScripts,
    ) -> Self {
        let router = Router::new(client.clone(), local.clone(), Arc::clone(&config));
        Self {
            client,
            gliner,
            intents,
            settings,
            config,
            scripts,
            router,
        }
    }

    /// Resolve the intent of one message and stream the answer to `on_delta`.
    ///
    /// The service reads the stored settings first, so a change on the
    /// settings page applies to the next message without a restart. A
    /// settings read that fails falls back to the configured defaults.
    pub async fn resolve<F>(
        &self,
        request: &ResolveRequest,
        mut on_delta: F,
    ) -> Result<Resolution, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        // 1. Read the intent catalog.
        let intents = self.intents.list_intents().await?;
        if intents.is_empty() {
            return Err(ResolveError::NoIntents);
        }

        // 2. Read which engine reads the message, and how it is set up.
        let backend = self.backend().await;
        let llama = self.llama_settings().await;
        let gliner = self.gliner_settings().await;
        tracing::debug!(backend = backend.as_str(), "the resolver reads the message");

        match backend {
            // A llama.cpp server reads the intent and the values.
            ResolverBackend::Llama => {
                let Resolution::Matched(mut matched) = self
                    .decide_intent(&intents, request, &llama, &mut on_delta)
                    .await?
                else {
                    return Ok(Resolution::Unmatched {
                        engine: ResolverEngine::Llama,
                        model: Some(llama.model.clone()),
                        route: RouteReport::default(),
                    });
                };
                if let Some(intent) = intents.iter().find(|intent| intent.id == matched.id) {
                    let lists = self.scripts.lists(intent).await;
                    let offered = self.scripts.offered_values(&lists);
                    matched.values = self
                        .read_llama_values(intent, request, &llama, &offered, &mut on_delta)
                        .await;
                    let mut origins = origins_of(
                        ResolverEngine::Llama,
                        Some(llama.model.clone()),
                        &matched.values,
                    );
                    let before = matched.values.clone();
                    match_script_values(intent, &mut matched.values, &lists, &request.text);
                    record_origins(
                        &before,
                        &matched.values,
                        &EntityRead::daemon(READ_LIST),
                        &mut origins,
                    );
                    matched.origins = origins;
                }
                matched.value_engine = value_engine_of(ResolverEngine::Llama, &matched.entities);
                matched.value_model = read_model(matched.value_engine, &llama.model, &llama.model);
                Ok(Resolution::Matched(matched))
            }

            // The built in GLiNER model reads the intent and the values.
            ResolverBackend::Gliner => {
                let resolution = self
                    .gliner
                    .resolve(
                        &intents,
                        &request.text,
                        &gliner.model,
                        gliner.device,
                        gliner.threshold,
                        &self.scripts,
                    )
                    .await
                    .map_err(|err| ResolveError::GlinerNotReady {
                        reason: err.to_string(),
                    })?;
                Ok(match resolution {
                    Resolution::Matched(mut matched) => {
                        // The built in reader matches the list of every
                        // script entity against the value it read.
                        if let Some(intent) = intents.iter().find(|intent| intent.id == matched.id)
                        {
                            let lists = self.scripts.lists(intent).await;
                            let mut origins = origins_of(
                                ResolverEngine::Gliner,
                                Some(gliner.model.clone()),
                                &matched.values,
                            );
                            let before = matched.values.clone();
                            match_script_values(intent, &mut matched.values, &lists, &request.text);
                            record_origins(
                                &before,
                                &matched.values,
                                &EntityRead::daemon(READ_LIST),
                                &mut origins,
                            );
                            matched.origins = origins;
                        }
                        matched.intent_model = Some(gliner.model.clone());
                        matched.value_model =
                            read_model(matched.value_engine, &gliner.model, &gliner.model);
                        Resolution::Matched(matched)
                    }
                    // The built in model already reports itself.
                    unmatched @ Resolution::Unmatched { .. } => unmatched,
                })
            }

            // The llama.cpp server names the intent, GLiNER reads the
            // values of the entities that intent needs.
            ResolverBackend::Hybrid => {
                let Resolution::Matched(mut matched) = self
                    .decide_intent(&intents, request, &llama, &mut on_delta)
                    .await?
                else {
                    return Ok(Resolution::Unmatched {
                        engine: ResolverEngine::Llama,
                        model: Some(llama.model.clone()),
                        route: RouteReport::default(),
                    });
                };
                if let Some(intent) = intents.iter().find(|intent| intent.id == matched.id) {
                    let lists = self.scripts.lists(intent).await;
                    let offered = self.scripts.offered_values(&lists);
                    let (mut values, engine) = self
                        .read_hybrid_values(
                            intent,
                            request,
                            &llama,
                            &gliner,
                            &offered,
                            &mut on_delta,
                        )
                        .await;
                    let mut origins = origins_of(
                        engine,
                        read_model(Some(engine), &gliner.model, &llama.model),
                        &values,
                    );
                    let before = values.clone();
                    match_script_values(intent, &mut values, &lists, &request.text);
                    record_origins(
                        &before,
                        &values,
                        &EntityRead::daemon(READ_LIST),
                        &mut origins,
                    );
                    matched.values = values;
                    matched.origins = origins;
                    matched.value_engine = value_engine_of(engine, &matched.entities);
                    matched.value_model =
                        read_model(matched.value_engine, &gliner.model, &llama.model);
                }
                Ok(Resolution::Matched(matched))
            }

            // The layered router reads the message in stages: a proven
            // message first, then a ranking of the whole catalog, then a
            // decision, then the values of the intent that won.
            ResolverBackend::Router => {
                self.route_message(&intents, request, &llama, &gliner, &mut on_delta)
                    .await
            }
        }
    }

    /// Read one message with the layered router.
    ///
    /// The stages run in order and each stage falls back to the cheaper
    /// reader of its own stage when the reader it wants cannot answer, so
    /// a server that is down costs accuracy and not the turn.
    async fn route_message<F>(
        &self,
        intents: &[IntentDto],
        request: &ResolveRequest,
        llama: &LlamaSettings,
        gliner: &GlinerSettings,
        on_delta: &mut F,
    ) -> Result<Resolution, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        let config = self.router_settings().await;
        let docs = router::doc::catalog(intents);
        let model = RouteModel {
            base_url: llama.base_url.clone(),
            model: llama.model.clone(),
        };
        // The stage that decided names the model that read the turn. A
        // stage that proves a message from the message alone, and a stage
        // that reads the scores of the ranking, reads no model at all, so
        // the report names none of them.
        let model_of_stage = |stage: RouterStage| {
            Self::read_model_of(stage, config.decide, &model, &config.rerank_model)
        };

        // The deterministic pass reads the lists of the documents whose
        // action the message holds, and of no others, so a turn never runs
        // every script of a large catalog.
        let plan = Router::plan(&docs, &request.text, &config);
        let lists = self.planned_lists(intents, &plan).await;
        let mut trace = RouteTrace::default();
        let route = self
            .router
            .route(
                &RouteInput {
                    docs: &docs,
                    text: &request.text,
                    lists: &lists,
                    config: &config,
                    model_settings: &model,
                },
                &mut trace,
                on_delta,
            )
            .await?;
        let mut report = RouteReport::read(&route, &trace, intents);

        let (index, proven) = match &route {
            Route::Chosen { index, values, .. } => (*index, values.clone()),
            Route::Refused { stage, reason, .. } => {
                tracing::debug!(reason = %reason, "the router read no intent");
                return Ok(Resolution::Unmatched {
                    engine: ResolverEngine::Router,
                    model: model_of_stage(*stage),
                    route: report,
                });
            }
        };
        let Some(intent) = intents.get(index) else {
            return Ok(Resolution::Unmatched {
                engine: ResolverEngine::Router,
                model: model_of_stage(RouterStage::None),
                route: report,
            });
        };
        let (stage, confidence) = match &route {
            Route::Chosen {
                stage, confidence, ..
            } => (*stage, *confidence),
            Route::Refused { .. } => (RouterStage::None, None),
        };
        let intent_model = model_of_stage(stage);

        // The values of the intent that won, read by the extraction stage.
        let extracts_at = Instant::now();
        let lists = self.scripts.lists(intent).await;
        let offered = self.scripts.offered_values(&lists);
        let readers = Readers { llama, gliner };
        let (mut values, engine) = self
            .read_router_values(
                intent,
                request,
                &readers,
                &offered,
                config.extract,
                on_delta,
            )
            .await;
        let entities: Vec<String> = intent
            .entities
            .iter()
            .map(|entity| entity.name.clone())
            .collect();
        let value_engine = value_engine_of(engine, &entities);
        let value_model = read_model(value_engine, &gliner.model, &llama.model);
        let mut origins = origins_of(engine, value_model.clone(), &values);

        // A value the deterministic pass proved names an entry of a list
        // the daemon holds, so it wins over a value another reader guessed
        // and it fills an entity that reader left empty.
        for (name, value) in proven {
            origins.insert(
                name.clone(),
                EntityRead {
                    source: READ_FAST_PATH,
                    engine: Some(ResolverEngine::Router),
                    model: None,
                    read: None,
                    score: None,
                },
            );
            values.insert(name, value);
        }
        self.match_list_values(
            intent,
            &request.text,
            &lists,
            &mut TurnValues {
                values: &mut values,
                origins: &mut origins,
                config: &config,
                model: &model,
            },
        )
        .await;

        // The extraction stage runs here rather than in the router, so it
        // writes its own step into the report of the turn.
        report.record_extraction(
            EntityRead::of_engine(engine, None).source,
            value_model.clone(),
            values_summary(intent, &origins),
            extracts_at.elapsed().as_millis() as u64,
        );
        Ok(Resolution::Matched(ResolvedIntent {
            id: intent.id,
            name: intent.name.clone(),
            command: intent.command.clone(),
            entities,
            required: required_entities(intent),
            values,
            origins,
            confidence,
            intent_engine: ResolverEngine::Router,
            intent_model,
            value_engine,
            value_model,
            route: report,
        }))
    }

    /// The model that read one router turn, by the stage that decided it.
    ///
    /// A stage that proves a message from the message alone, and a stage that
    /// reads the scores of the ranking, reads no model at all, so the turn
    /// names none. A reranking stage reads the built in reranker, and a stage
    /// that asks a language model reads the server.
    fn read_model_of(
        stage: RouterStage,
        decide: DecideEngine,
        model: &RouteModel,
        rerank_model: &str,
    ) -> Option<String> {
        match (stage, decide) {
            (RouterStage::Rerank, DecideEngine::Rerank) => Some(rerank_model.to_string()),
            (RouterStage::Rerank, DecideEngine::Generative) => Some(model.model.clone()),
            _ => None,
        }
    }

    /// Read the value of every list entity out of the list it offers.
    ///
    /// The rules of the matcher run first and a vector scan follows for
    /// what the rules could not read, so a name the message spells out is
    /// read by the rule and a name the message only means is read by the
    /// vector. A rule set that reads the rules alone, or a server that
    /// does not answer, leaves the turn with what the rules read.
    async fn match_list_values(
        &self,
        intent: &IntentDto,
        message: &str,
        lists: &BTreeMap<String, EntityList>,
        turn: &mut TurnValues<'_>,
    ) {
        if turn.config.list_match.uses_rules() {
            let before = turn.values.clone();
            match_script_values(intent, turn.values, lists, message);
            record_origins(
                &before,
                turn.values,
                &EntityRead::daemon(READ_LIST),
                turn.origins,
            );
        }
        if !turn.config.list_match.uses_embeddings() {
            return;
        }

        let embedder = self.router.embedder(turn.config, turn.model);
        let mentions = mentions_of(message, &action_words(intent));
        for entity in &intent.entities {
            if !matches!(entity.kind, EntityKindDto::Script) {
                continue;
            }
            if turn.values.contains_key(&entity.name) {
                continue;
            }
            let Some(list) = lists.get(&entity.name) else {
                continue;
            };
            match router::embed::best_value(
                &embedder,
                &mentions,
                &list.values,
                turn.config.list_floor,
            )
            .await
            {
                Ok(Some((found, score))) => {
                    tracing::debug!(
                        entity = %entity.name,
                        value = %found,
                        score,
                        "the vectors of the list read a value"
                    );
                    turn.origins.insert(
                        entity.name.clone(),
                        EntityRead {
                            source: READ_EMBEDDING,
                            engine: None,
                            model: None,
                            read: None,
                            score: Some(score),
                        },
                    );
                    turn.values.insert(entity.name.clone(), found);
                }
                Ok(None) => {}
                Err(err) => {
                    // A server that does not answer cannot answer for the
                    // next entity either, so the turn keeps what the
                    // rules read and moves on.
                    tracing::warn!(
                        error = %err,
                        "the embedding server did not answer, the rules of the matcher stand"
                    );
                    break;
                }
            }
        }
    }

    /// Read the router configuration of this turn.
    async fn router_settings(&self) -> RouterConfig {
        self.settings.get_router().await.unwrap_or_else(|err| {
            tracing::warn!(error = %err, "settings read failed, using the configured router");
            self.config.resolver.router.clone()
        })
    }

    /// Read the values of every list entity of the planned documents.
    ///
    /// A closed entity carries its values in the configuration and a
    /// script entity takes them from a live list. Both are read here, so
    /// the deterministic pass can prove a message against either of them.
    async fn planned_lists(
        &self,
        intents: &[IntentDto],
        plan: &[usize],
    ) -> BTreeMap<usize, BTreeMap<String, Vec<String>>> {
        let mut planned: BTreeMap<usize, BTreeMap<String, Vec<String>>> = BTreeMap::new();
        for index in plan {
            let Some(intent) = intents.get(*index) else {
                continue;
            };
            let mut values: BTreeMap<String, Vec<String>> = intent
                .entities
                .iter()
                .filter(|entity| matches!(entity.kind, EntityKindDto::Closed))
                .map(|entity| (entity.name.clone(), entity.values.clone()))
                .collect();
            for (name, list) in self.scripts.lists(intent).await {
                values.insert(name, list.values);
            }
            planned.insert(*index, values);
        }
        planned
    }

    /// Read the values of one intent with the extraction stage.
    ///
    /// - `lists` reads the lists alone, so a turn needs no model at all.
    /// - `spans` lets the built in model find the span of every label and
    ///   lets the language model read what the built in model could not.
    /// - `generative` reads every value with the language model.
    async fn read_router_values<F>(
        &self,
        intent: &IntentDto,
        request: &ResolveRequest,
        readers: &Readers<'_>,
        offered: &BTreeMap<String, Vec<String>>,
        engine: ExtractEngine,
        on_delta: &mut F,
    ) -> (BTreeMap<String, String>, ResolverEngine)
    where
        F: FnMut(&str) + Send,
    {
        let (llama, gliner) = (readers.llama, readers.gliner);
        if intent.entities.is_empty() {
            return (BTreeMap::new(), ResolverEngine::Router);
        }
        match engine {
            // The lists alone. An open entity is left without a value and
            // the turn asks the user for it.
            ExtractEngine::Lists => (BTreeMap::new(), ResolverEngine::Router),
            ExtractEngine::Spans => {
                match self
                    .gliner
                    .read_values(intent, &request.text, &gliner.model, gliner.device, offered)
                    .await
                {
                    Ok(values) if !values.is_empty() => (values, ResolverEngine::Gliner),
                    Ok(_) => {
                        tracing::debug!(
                            intent = %intent.name,
                            "the spans read no value, the language model reads them"
                        );
                        let values = self
                            .read_llama_values(intent, request, llama, offered, on_delta)
                            .await;
                        (values, ResolverEngine::Llama)
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            intent = %intent.name,
                            "the spans read no value, the language model reads them"
                        );
                        let values = self
                            .read_llama_values(intent, request, llama, offered, on_delta)
                            .await;
                        (values, ResolverEngine::Llama)
                    }
                }
            }
            ExtractEngine::Generative => {
                let values = self
                    .read_llama_values(intent, request, llama, offered, on_delta)
                    .await;
                (values, ResolverEngine::Llama)
            }
        }
    }

    /// The client of the llama.cpp server.
    ///
    /// The settings page asks it for the models the server offers, so the
    /// user picks a name the server really answers to.
    pub fn client(&self) -> &LlamaClient {
        &self.client
    }

    /// Read which engine reads the message.
    async fn backend(&self) -> ResolverBackend {
        match self.settings.get_resolver_backend().await {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(error = %err, "settings read failed, using the configured backend");
                self.config.resolver.backend
            }
        }
    }

    /// Read the address and the model name of the llama.cpp server.
    async fn llama_settings(&self) -> LlamaSettings {
        let base_url = match self.settings.get_resolver_base_url().await {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(error = %err, "settings read failed, using the configured base url");
                self.config.resolver.base_url.clone()
            }
        };
        let model = match self.settings.get_resolver_model().await {
            Ok(value) => value,
            Err(err) => {
                tracing::warn!(error = %err, "settings read failed, using the configured model");
                self.config.resolver.model.clone()
            }
        };
        LlamaSettings { base_url, model }
    }

    /// Read the model, the device, and the threshold of GLiNER.
    async fn gliner_settings(&self) -> GlinerSettings {
        let config = &self.config.resolver.gliner;
        GlinerSettings {
            model: self
                .settings
                .get_gliner_model()
                .await
                .unwrap_or_else(|err| {
                    tracing::warn!(error = %err, "settings read failed, using the configured GLiNER model");
                    config.model.clone()
                }),
            device: self
                .settings
                .get_gliner_device()
                .await
                .unwrap_or(config.device),
            threshold: self
                .settings
                .get_gliner_threshold()
                .await
                .unwrap_or(config.threshold),
        }
    }

    /// Ask the llama.cpp server which intent the message asks for.
    async fn decide_intent<F>(
        &self,
        intents: &[IntentDto],
        request: &ResolveRequest,
        llama: &LlamaSettings,
        on_delta: &mut F,
    ) -> Result<Resolution, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        let decision = build_decision(request, intents);
        let prompt = build_prompt(&decision)?;
        let outcome = self
            .client
            .decide(
                &DecisionRequest {
                    base_url: llama.base_url.clone(),
                    model: llama.model.clone(),
                    prompt,
                    max_tokens: self.config.resolver.max_tokens,
                    thinking: self.config.resolver.thinking,
                    timeout: Duration::from_secs(self.config.resolver.timeout_secs),
                },
                on_delta,
            )
            .await?;
        Ok(match_intent(
            &outcome.option_id,
            outcome.confidence,
            intents,
            &llama.model,
        ))
    }

    /// Read the values of one intent with the built in GLiNER model.
    ///
    /// GLiNER reads only the labels of the chosen intent, and the language
    /// model reads the values only when GLiNER reads none at all. A turn
    /// must not lose a value to an engine that cannot see it. The function
    /// reports which engine read the values, so the turn can say so.
    async fn read_hybrid_values<F>(
        &self,
        intent: &IntentDto,
        request: &ResolveRequest,
        llama: &LlamaSettings,
        gliner: &GlinerSettings,
        offered: &BTreeMap<String, Vec<String>>,
        on_delta: &mut F,
    ) -> (BTreeMap<String, String>, ResolverEngine)
    where
        F: FnMut(&str) + Send,
    {
        if intent.entities.is_empty() {
            return (BTreeMap::new(), ResolverEngine::Gliner);
        }

        match self
            .gliner
            .read_values(intent, &request.text, &gliner.model, gliner.device, offered)
            .await
        {
            Ok(values) if !values.is_empty() => {
                tracing::debug!(intent = %intent.name, entities = values.len(), "GLiNER read the values");
                (values, ResolverEngine::Gliner)
            }
            Ok(_) => {
                tracing::debug!(
                    intent = %intent.name,
                    "GLiNER read no value, the language model reads them"
                );
                let values = self
                    .read_llama_values(intent, request, llama, offered, on_delta)
                    .await;
                (values, ResolverEngine::Llama)
            }
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    intent = %intent.name,
                    "GLiNER read no values, the language model reads them"
                );
                let values = self
                    .read_llama_values(intent, request, llama, offered, on_delta)
                    .await;
                (values, ResolverEngine::Llama)
            }
        }
    }

    /// Read the values of every entity of one intent.
    ///
    /// A request that fails leaves the values empty. The renderer of the
    /// command then reports the entities it could not fill, so a resolver
    /// failure never becomes a command with a guessed value.
    async fn read_llama_values<F>(
        &self,
        intent: &IntentDto,
        request: &ResolveRequest,
        llama: &LlamaSettings,
        offered: &BTreeMap<String, Vec<String>>,
        on_delta: &mut F,
    ) -> BTreeMap<String, String>
    where
        F: FnMut(&str) + Send,
    {
        let state = render_state(request);
        let base_url = &llama.base_url;
        let model = &llama.model;
        let message = &request.text;
        let Some(prompt) = build_values_prompt(&state, intent, offered) else {
            return BTreeMap::new();
        };

        let answer = self
            .client
            .read_answer(
                &AnswerRequest {
                    base_url: base_url.to_string(),
                    model: model.to_string(),
                    system: prompt.system,
                    user: prompt.user,
                    answer_schema: prompt.answer_schema,
                    schema_name: VALUES_SCHEMA_NAME.to_string(),
                    max_tokens: self.config.resolver.max_tokens,
                    thinking: self.config.resolver.thinking,
                    timeout: Duration::from_secs(self.config.resolver.timeout_secs),
                },
                on_delta,
            )
            .await;

        match answer {
            Ok(content) => parse_values(&content, intent, message, offered),
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    intent = %intent.name,
                    "the resolver read no entity values"
                );
                BTreeMap::new()
            }
        }
    }
}

/// Read the value of every script entity out of the list it offers.
///
/// The engine names the words of a value and this function names the
/// entry of the list those words name. The entry is the value the command
/// runs with, so a command never runs with a name the list does not hold.
///
/// An engine that read nothing for an entity leaves the words of the
/// message: the user said the name of one entry, and the name is all the
/// message had to say about it. The daemon reads those words against the
/// list as well, so a list whose entries are the paths a machine runs
/// still answers to the name a user says.
///
/// A value that matches no entry leaves the entity without a value, and
/// the turn reports that it could not read it, or asks the user for it.
fn match_script_values(
    intent: &IntentDto,
    values: &mut BTreeMap<String, String>,
    lists: &BTreeMap<String, EntityList>,
    message: &str,
) {
    let mentions = mentions_of(message, &action_words(intent));
    for entity in &intent.entities {
        if !matches!(entity.kind, EntityKindDto::Script) {
            continue;
        }
        let Some(list) = lists.get(&entity.name) else {
            continue;
        };
        let read = values.get(&entity.name);
        if let Some(read) = read {
            match best_match(read, &list.values) {
                Some(found) => {
                    tracing::debug!(
                        entity = %entity.name,
                        read = %read,
                        value = %found.value,
                        rule = found.kind.as_str(),
                        "the daemon read a value of the list"
                    );
                    values.insert(entity.name.clone(), found.value);
                    continue;
                }
                None => {
                    tracing::debug!(
                        entity = %entity.name,
                        read = %read,
                        "the value the engine read names no entry, the words of the message read it"
                    );
                    values.remove(&entity.name);
                }
            }
        }
        match best_mention(&mentions, &list.values) {
            Some(found) => {
                tracing::debug!(
                    entity = %entity.name,
                    value = %found.value,
                    rule = found.kind.as_str(),
                    "the words of the message read a value of the list"
                );
                values.insert(entity.name.clone(), found.value);
            }
            None => {
                tracing::warn!(
                    entity = %entity.name,
                    values = list.values.len(),
                    "the message named no value of the list"
                );
            }
        }
    }
}

/// The words of one intent that name the action.
///
/// The name of an intent names the action, so those words never name one
/// entry of a list of its entities: `open firefox` names the entry
/// `firefox`, and `open` is the action. The phrases are left in, because a
/// phrase names the value the user says: `launch firefox` names the entry
/// `firefox` through the phrase the user wrote.
fn action_words(intent: &IntentDto) -> Vec<String> {
    words_of(&intent.name)
}

/// The names of the entities of one intent that need a value.
fn required_entities(intent: &IntentDto) -> Vec<String> {
    intent
        .entities
        .iter()
        .filter(|entity| entity.required)
        .map(|entity| entity.name.clone())
        .collect()
}

/// The engine that read the values of one intent.
///
/// An intent that needs no value has no value engine, so the transcript
/// does not report a read that never happened.
fn value_engine_of(engine: ResolverEngine, entities: &[String]) -> Option<ResolverEngine> {
    (!entities.is_empty()).then_some(engine)
}

/// Read the model that ran for one value engine, or null when none ran.
///
/// The hybrid reader falls back to the language model when the built in
/// model cannot read, so the caller passes the model of each engine and
/// this function names the one that really read the values.
fn read_model(engine: Option<ResolverEngine>, gliner: &str, llama: &str) -> Option<String> {
    engine.map(|engine| match engine {
        ResolverEngine::Gliner => gliner.to_string(),
        ResolverEngine::Llama => llama.to_string(),
        // The engine that reads the lists runs in the daemon itself, so
        // it reports the model of the reader that would have read them.
        ResolverEngine::Router => gliner.to_string(),
    })
}

/// Build the decision of one message out of the intent catalog.
///
/// The catalog becomes the option list, so the resolver chooses among the
/// intents the user configured. The last option states that no intent
/// fits, which keeps a weak model from picking an intent at any cost.
fn build_decision(request: &ResolveRequest, intents: &[IntentDto]) -> Decision {
    let mut options: Vec<DecisionOption> = intents
        .iter()
        .map(|intent| DecisionOption {
            id: intent.id.to_string(),
            label: intent.name.clone(),
            detail: describe_intent(intent),
        })
        .collect();

    options.push(DecisionOption {
        id: NO_INTENT_OPTION.to_string(),
        label: "none of these".to_string(),
        detail: "The message does not ask for any option above.".to_string(),
    });

    Decision {
        state: render_state(request),
        question: "Which option does the message ask for?".to_string(),
        options,
    }
}

/// Describe one intent to the model.
fn describe_intent(intent: &IntentDto) -> String {
    let mut parts = Vec::new();
    if !intent.description.is_empty() {
        parts.push(intent.description.clone());
    }
    let entities = describe_entities(intent);
    if !entities.is_empty() {
        parts.push(format!("Entities: {entities}"));
    }
    if !intent.examples.is_empty() {
        parts.push(format!("Also: {}", intent.examples.join(", ")));
    }
    parts.join(" ")
}

/// Describe the entities of one intent to the model.
///
/// A closed entity lists its values, so the model reads the choices it
/// has. A script entity takes a value of a live list the daemon holds in
/// memory, so the option says that the entity takes a name of a list. An
/// open entity names itself only, because the model reads its value out
/// of the message later. An optional entity says so, because the message
/// does not have to name it.
fn describe_entities(intent: &IntentDto) -> String {
    intent
        .entities
        .iter()
        .map(|entity| {
            let kind = match entity.kind {
                EntityKindDto::Open => "open",
                EntityKindDto::Closed => "closed",
                EntityKindDto::Script => "a name of a list",
            };
            let values = match entity.kind {
                EntityKindDto::Closed => format!(": {}", entity.values.join(", ")),
                _ => String::new(),
            };
            let needed = if entity.required { "" } else { ", optional" };
            format!("{} ({kind}{values}{needed})", entity.name)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render the state the model reads.
fn render_state(request: &ResolveRequest) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(request.history.len() + 1);
    for turn in &request.history {
        lines.push(format!("{}: {}", turn.role.as_str(), turn.text));
    }
    lines.push(format!("user: {}", request.text));
    lines.join("\n")
}

/// Read the chosen option back into an intent.
/// The chosen option is always the answer of the llama.cpp server, so the
/// matched intent reports that engine and the model the server ran. The
/// caller sets the value engine once it knows which engine read the values.
fn match_intent(
    option_id: &str,
    confidence: Option<f32>,
    intents: &[IntentDto],
    model: &str,
) -> Resolution {
    let unmatched = Resolution::Unmatched {
        engine: ResolverEngine::Llama,
        model: Some(model.to_string()),
        route: RouteReport::default(),
    };
    if option_id == NO_INTENT_OPTION {
        return unmatched;
    }
    let found = intents
        .iter()
        .find(|intent| intent.id.to_string() == option_id);
    match found {
        Some(intent) => Resolution::Matched(ResolvedIntent {
            id: intent.id,
            name: intent.name.clone(),
            command: intent.command.clone(),
            entities: intent
                .entities
                .iter()
                .map(|entity| entity.name.clone())
                .collect(),
            required: required_entities(intent),
            values: BTreeMap::new(),
            origins: BTreeMap::new(),
            confidence,
            intent_engine: ResolverEngine::Llama,
            intent_model: Some(model.to_string()),
            value_engine: None,
            value_model: None,
            route: RouteReport::default(),
        }),
        None => unmatched,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::dto::IntentEntityDto;
    use chrono::Utc;

    use crate::resolver::router::Candidate;

    /// Build one intent for the tests.
    fn intent(name: &str, entities: Vec<IntentEntityDto>) -> IntentDto {
        IntentDto {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: "Look something up.".to_string(),
            command: "echo hi".to_string(),
            entities,
            examples: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Build one request for the tests.
    fn request() -> ResolveRequest {
        ResolveRequest {
            text: "what is the weather in Berlin".to_string(),
            history: vec![ContextTurn {
                role: ChatRoleDto::User,
                text: "hello".to_string(),
            }],
        }
    }

    /// Build one script intent with one entity for the tests.
    fn script_intent() -> IntentDto {
        let mut found = intent(
            "open application",
            vec![IntentEntityDto {
                id: Uuid::new_v4(),
                name: "applications".to_string(),
                kind: EntityKindDto::Script,
                values: Vec::new(),
                script: Some("ls /bin".to_string()),
                required: true,
            }],
        );
        found.examples = vec!["launch firefox".to_string()];
        found
    }

    /// Build one list of applications for the tests.
    fn application_list() -> BTreeMap<String, EntityList> {
        let mut lists = BTreeMap::new();
        lists.insert(
            "applications".to_string(),
            EntityList {
                values: vec![
                    "firefox".to_string(),
                    "/nix/store/ggm28k0vkw1jg801hwvchi0vfizj0c0c-firefox-116.0.3/bin/firefox"
                        .to_string(),
                    "blueman-manager".to_string(),
                ],
                error: None,
            },
        );
        lists
    }

    #[test]
    fn match_script_values_reads_the_words_of_the_message() {
        let mut values = BTreeMap::new();
        match_script_values(
            &script_intent(),
            &mut values,
            &application_list(),
            "open firefox",
        );

        assert_eq!(values["applications"], "firefox");
    }

    #[test]
    fn match_script_values_reads_the_base_name_of_a_path() {
        let mut values = BTreeMap::new();
        values.insert("applications".to_string(), "firefox".to_string());
        let mut lists = application_list();
        lists
            .get_mut("applications")
            .expect("the list is there")
            .values = vec![
            "/nix/store/ggm28k0vkw1jg801hwvchi0vfizj0c0c-firefox-116.0.3/bin/firefox".to_string(),
        ];
        match_script_values(&script_intent(), &mut values, &lists, "open firefox");

        assert_eq!(
            values["applications"],
            "/nix/store/ggm28k0vkw1jg801hwvchi0vfizj0c0c-firefox-116.0.3/bin/firefox"
        );
    }

    #[test]
    fn match_script_values_leaves_a_name_that_no_entry_holds() {
        let mut values = BTreeMap::new();
        match_script_values(
            &script_intent(),
            &mut values,
            &application_list(),
            "open obs",
        );

        assert!(values.is_empty());
    }

    #[test]
    fn match_script_values_never_reads_the_action_of_the_intent() {
        let mut values = BTreeMap::new();
        let mut lists = application_list();
        lists
            .get_mut("applications")
            .expect("the list is there")
            .values = vec!["open".to_string(), "firefox".to_string()];
        match_script_values(&script_intent(), &mut values, &lists, "open firefox");

        assert_eq!(values["applications"], "firefox");
    }

    #[test]
    fn match_script_values_reads_the_value_of_a_phrase() {
        let mut values = BTreeMap::new();
        match_script_values(
            &script_intent(),
            &mut values,
            &application_list(),
            "launch firefox",
        );

        assert_eq!(values["applications"], "firefox");
    }

    #[test]
    fn build_decision_keeps_the_no_intent_option_last() {
        let catalog = vec![intent("get weather", Vec::new())];
        let decision = build_decision(&request(), &catalog);

        assert_eq!(decision.options.len(), 2);
        assert_eq!(decision.options[0].label, "get weather");
        assert_eq!(decision.options[1].id, NO_INTENT_OPTION);
    }

    #[test]
    fn build_decision_lists_the_values_of_a_closed_entity() {
        let catalog = vec![intent(
            "get weather",
            vec![
                IntentEntityDto {
                    id: Uuid::new_v4(),
                    name: "when".to_string(),
                    kind: EntityKindDto::Closed,
                    values: vec!["today".to_string(), "tomorrow".to_string()],
                    script: None,
                    required: true,
                },
                IntentEntityDto {
                    id: Uuid::new_v4(),
                    name: "city".to_string(),
                    kind: EntityKindDto::Open,
                    values: Vec::new(),
                    script: None,
                    required: true,
                },
            ],
        )];
        let decision = build_decision(&request(), &catalog);
        let detail = &decision.options[0].detail;

        assert!(detail.contains("when (closed: today, tomorrow)"));
        assert!(detail.contains("city (open)"));
    }

    #[test]
    fn build_decision_renders_the_history_before_the_message() {
        let decision = build_decision(&request(), &[intent("get weather", Vec::new())]);
        let state = &decision.state;

        assert!(state.starts_with("user: hello"));
        assert!(state.ends_with("user: what is the weather in Berlin"));
    }

    #[test]
    fn match_intent_reads_the_chosen_intent_back() {
        let catalog = vec![intent("get weather", Vec::new())];
        let id = catalog[0].id.to_string();

        match match_intent(&id, Some(0.8), &catalog, "qwen3.5-4b") {
            Resolution::Matched(found) => {
                assert_eq!(found.name, "get weather");
                assert_eq!(found.command, "echo hi");
                assert_eq!(found.confidence, Some(0.8));
                assert_eq!(found.intent_engine, ResolverEngine::Llama);
                assert_eq!(found.intent_model, Some("qwen3.5-4b".to_string()));
                assert_eq!(found.value_engine, None);
                assert_eq!(found.value_model, None);
            }
            Resolution::Unmatched { .. } => panic!("the intent is in the catalog"),
        }
    }

    #[test]
    fn match_intent_reads_the_no_intent_option_as_unmatched() {
        let catalog = vec![intent("get weather", Vec::new())];
        assert_eq!(
            match_intent(NO_INTENT_OPTION, Some(0.9), &catalog, "qwen3.5-4b"),
            Resolution::Unmatched {
                engine: ResolverEngine::Llama,
                model: Some("qwen3.5-4b".to_string()),
                route: RouteReport::default(),
            }
        );
    }

    #[test]
    fn match_intent_ignores_an_unknown_option() {
        let catalog = vec![intent("get weather", Vec::new())];
        assert_eq!(
            match_intent("unknown", None, &catalog, "qwen3.5-4b"),
            Resolution::Unmatched {
                engine: ResolverEngine::Llama,
                model: Some("qwen3.5-4b".to_string()),
                route: RouteReport::default(),
            }
        );
    }

    #[test]
    fn a_route_report_names_the_stage_and_the_short_list() {
        let catalog = vec![
            intent("get weather", Vec::new()),
            intent("tell time", Vec::new()),
        ];
        let route = Route::Chosen {
            index: 1,
            stage: alice_core::config::RouterStage::Retrieve,
            confidence: Some(0.7),
            candidates: vec![
                Candidate {
                    index: 1,
                    score: 0.7,
                    evidence: "words".to_string(),
                },
                Candidate {
                    index: 0,
                    score: 0.2,
                    evidence: "embedding".to_string(),
                },
            ],
            reason: None,
            values: BTreeMap::new(),
        };

        let report = RouteReport::read(&route, &RouteTrace::default(), &catalog);

        assert_eq!(report.stage.as_deref(), Some("retrieve"));
        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.candidates[0].name, "tell time");
        assert_eq!(report.candidates[1].evidence, "embedding");
        assert!(report.matched);
        assert!(report.steps.is_empty());
    }

    #[test]
    fn a_route_report_carries_every_stage_of_the_turn() {
        let catalog = vec![
            intent("get weather", Vec::new()),
            intent("tell time", Vec::new()),
        ];
        let route = Route::Chosen {
            index: 1,
            stage: alice_core::config::RouterStage::Retrieve,
            confidence: Some(0.7),
            candidates: vec![Candidate {
                index: 1,
                score: 0.7,
                evidence: "words".to_string(),
            }],
            reason: None,
            values: BTreeMap::new(),
        };
        let mut trace = RouteTrace::default();
        trace.record(router::RouteStep {
            stage: "retrieve",
            outcome: router::StepOutcome::Passed,
            reader: "lexical".to_string(),
            model: None,
            detail: Some("ranked 2 intents and kept the best 1".to_string()),
            candidates: vec![Candidate {
                index: 1,
                score: 0.7,
                evidence: "words".to_string(),
            }],
            duration: Duration::from_millis(3),
        });

        let mut report = RouteReport::read(&route, &trace, &catalog);
        report.record_extraction(
            "spans",
            Some("gliner_small-v2.1".to_string()),
            "read 0 of 0 values".to_string(),
            12,
        );

        assert_eq!(report.steps.len(), 2);
        assert_eq!(report.steps[0].stage, "retrieve");
        assert_eq!(report.steps[0].outcome, "passed");
        assert_eq!(report.steps[0].reader, "lexical");
        assert_eq!(report.steps[0].duration_ms, 3);
        assert_eq!(report.steps[1].stage, "extract");
        assert_eq!(report.steps[1].reader, "spans");
        assert_eq!(report.steps[1].duration_ms, 12);
    }

    #[test]
    fn a_value_reports_the_reader_that_read_it() {
        let mut intent = intent("open application", Vec::new());
        intent.entities = vec![alice_core::dto::IntentEntityDto {
            id: Uuid::new_v4(),
            name: "applications".to_string(),
            kind: EntityKindDto::Script,
            values: Vec::new(),
            script: Some("ls".to_string()),
            required: true,
        }];
        let mut values = BTreeMap::new();
        values.insert("applications".to_string(), "firefox".to_string());
        let mut origins = BTreeMap::new();
        origins.insert(
            "applications".to_string(),
            EntityRead {
                source: READ_LIST,
                engine: None,
                model: None,
                read: Some("firefox browser".to_string()),
                score: None,
            },
        );
        let mut matched =
            match match_intent(&intent.id.to_string(), Some(0.6), &[intent.clone()], "m") {
                Resolution::Matched(matched) => matched,
                other => panic!("{other:?}"),
            };
        matched.values = values;
        matched.origins = origins;

        let entities = matched.entity_values();

        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "applications");
        assert_eq!(entities[0].value, "firefox");
        assert_eq!(entities[0].source.as_deref(), Some("list"));
        assert_eq!(entities[0].read.as_deref(), Some("firefox browser"));
        assert!(entities[0].engine.is_none());
    }

    #[test]
    fn recording_origins_reads_which_entities_a_reader_filled() {
        let mut before = BTreeMap::new();
        before.insert("city".to_string(), "Berlin".to_string());
        before.insert("when".to_string(), "tomorrow".to_string());
        let mut after = before.clone();
        after.insert("city".to_string(), "Berlin".to_string());
        after.insert("when".to_string(), "today".to_string());

        let mut origins = BTreeMap::new();
        record_origins(
            &before,
            &after,
            &EntityRead::daemon(READ_LIST),
            &mut origins,
        );

        assert!(
            !origins.contains_key("city"),
            "a value the reader kept is not its work"
        );
        assert_eq!(origins["when"].source, READ_LIST);
        assert_eq!(origins["when"].read.as_deref(), Some("tomorrow"));
    }

    #[test]
    fn a_values_summary_names_every_reader_of_the_turn() {
        let mut intent = intent("get weather", Vec::new());
        intent.entities = vec![
            alice_core::dto::IntentEntityDto {
                id: Uuid::new_v4(),
                name: "city".to_string(),
                kind: EntityKindDto::Open,
                values: Vec::new(),
                script: None,
                required: true,
            },
            alice_core::dto::IntentEntityDto {
                id: Uuid::new_v4(),
                name: "when".to_string(),
                kind: EntityKindDto::Closed,
                values: vec!["today".to_string()],
                script: None,
                required: false,
            },
        ];
        let mut origins = BTreeMap::new();
        origins.insert("city".to_string(), EntityRead::daemon(READ_SPANS));

        assert_eq!(
            values_summary(&intent, &origins),
            "read 1 of 2 values: city by spans"
        );
        assert_eq!(
            values_summary(&intent, &BTreeMap::new()),
            "read none of the 2 values"
        );
    }

    #[test]
    fn a_refused_route_reports_the_stage_that_refused() {
        let catalog = vec![intent("get weather", Vec::new())];
        let route = Route::Refused {
            stage: alice_core::config::RouterStage::None,
            candidates: Vec::new(),
            reason: "nothing fit".to_string(),
        };

        let report = RouteReport::read(&route, &RouteTrace::default(), &catalog);

        assert_eq!(report.stage.as_deref(), Some("none"));
        assert!(report.candidates.is_empty());
        assert!(!report.matched);
        assert_eq!(report.reason.as_deref(), Some("nothing fit"));
    }
}
