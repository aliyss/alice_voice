//! Typed configuration of the backend.
//! This module owns the server address, database url, and limits.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Root configuration of the backend.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CoreConfig {
    /// Server configuration.
    pub server: ServerConfig,
    /// Database configuration.
    pub database: DatabaseConfig,
    /// Queue configuration.
    pub queue: QueueConfig,
    /// Intent resolver configuration.
    pub resolver: ResolverConfig,
    /// Intent execution configuration.
    pub execution: ExecutionConfig,
    /// Configuration of the scripts that provide entity values.
    pub entity_script: EntityScriptConfig,
}

/// Server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Listen address, example: 127.0.0.1:8787
    pub listen_addr: String,
    /// Allowed CORS origin for the frontend, example: http://localhost:5173
    pub cors_origin: String,
}

/// Database configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL. Use sqlite://alice.db?mode=rwc or postgres://user:pass@host/db
    pub url: String,
    /// Maximum connections.
    pub max_connections: u32,
}

/// Queue configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    /// Whether the queue stores messages.
    pub enabled: bool,
    /// Page size of a list endpoint when the request sends no limit.
    pub page_limit_default: u64,
    /// Largest page size a list endpoint accepts.
    pub page_limit_max: u64,
    /// Maximum length of a chat message.
    pub max_message_len: usize,
}

/// The engine that reads the intent of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverBackend {
    /// A llama.cpp server reads the intent and the entity values.
    Llama,
    /// The built in GLiNER model reads the intent and the entity values.
    Gliner,
    /// The llama.cpp server reads the intent, GLiNER reads the values.
    ///
    /// A small language model is good at reading what a message asks
    /// for, and GLiNER is good at finding the exact span a label names.
    Hybrid,
    /// The layered router reads the message in stages.
    ///
    /// No single model reads the whole catalog. A deterministic pass
    /// answers the messages it can prove, a retrieval pass narrows the
    /// catalog to a short list, a decision pass chooses one of the short
    /// list or refuses, and an extraction pass reads the values of the
    /// intent that won. Each stage is configurable on its own, so a
    /// catalog of any size stays readable and every stage can fall back
    /// to a cheaper reader.
    Router,
}

impl ResolverBackend {
    /// The stored and reported form of the backend.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Llama => "llama",
            Self::Gliner => "gliner",
            Self::Hybrid => "hybrid",
            Self::Router => "router",
        }
    }

    /// Parse the stored form of the backend. An unknown value reads as the default.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "llama" => Self::Llama,
            "gliner" => Self::Gliner,
            "router" => Self::Router,
            _ => Self::Hybrid,
        }
    }

    /// Whether the backend reads the intent from a llama.cpp server.
    pub fn uses_llama(&self) -> bool {
        matches!(self, Self::Llama | Self::Hybrid)
    }

    /// Whether the backend reads entity values with GLiNER.
    pub fn uses_gliner(&self) -> bool {
        matches!(self, Self::Gliner | Self::Hybrid)
    }

    /// Every backend, in the order the settings page shows them.
    pub const ALL: [Self; 4] = [Self::Router, Self::Hybrid, Self::Llama, Self::Gliner];
}

/// The evidence the router reads the catalog with.
///
/// The first stage of the router ranks every intent against the message
/// and keeps a short list. The engine decides what the ranking reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrieveEngine {
    /// Words only. No model and no server are needed, so this engine
    /// always answers.
    Lexical,
    /// Embeddings only.
    Dense,
    /// Words and embeddings together, weighted.
    Hybrid,
}

impl RetrieveEngine {
    /// The stored and reported form of the engine.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Dense => "dense",
            Self::Hybrid => "hybrid",
        }
    }

    /// Parse the stored form of the engine. An unknown value reads as lexical.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "dense" => Self::Dense,
            "hybrid" => Self::Hybrid,
            _ => Self::Lexical,
        }
    }

    /// Whether the engine reads embeddings, and therefore needs a server.
    pub fn uses_embeddings(&self) -> bool {
        matches!(self, Self::Dense | Self::Hybrid)
    }
}

/// How the router chooses one of the short list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecideEngine {
    /// The scores of the first stage alone, held against a floor and a
    /// margin. No model is needed, so this engine always answers.
    Score,
    /// A built in model reads the message against every candidate of the
    /// short list and reports how well each pair fits.
    ///
    /// A reader that sees the message and the intent in one sequence is
    /// surer than a reader that embeds the two apart, so its score is a
    /// calibration the floor and the margin can hold. The model runs in
    /// the daemon and needs no server.
    Rerank,
    /// A language model reads the short list and chooses one of it, or
    /// none of it. Only the short list reaches the model, so the prompt
    /// stays small however large the catalog is.
    Generative,
}

impl DecideEngine {
    /// The stored and reported form of the engine.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Score => "score",
            Self::Rerank => "rerank",
            Self::Generative => "generative",
        }
    }

    /// Parse the stored form of the engine. An unknown value reads as score.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "rerank" => Self::Rerank,
            "generative" => Self::Generative,
            _ => Self::Score,
        }
    }

    /// Whether the engine needs a model server.
    pub fn uses_model(&self) -> bool {
        matches!(self, Self::Generative)
    }

    /// Whether the engine reads a built in model that has to be on disk.
    pub fn uses_local_model(&self) -> bool {
        matches!(self, Self::Rerank)
    }
}

/// Where the retrieval stage reads the vectors of the catalog.
///
/// Both places answer the same question, and both are a switch of their
/// own: a machine that already runs a model server needs no download, and
/// a machine that runs the daemon alone needs no second process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbedSource {
    /// The model server the daemon already talks to answers the
    /// `/embeddings` path.
    Server,
    /// A built in ONNX model runs in the daemon and needs no server.
    Local,
}

impl EmbedSource {
    /// The stored and reported form of the source.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Local => "local",
        }
    }

    /// Parse the stored form of the source. An unknown value reads as server.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "local" => Self::Local,
            _ => Self::Server,
        }
    }

    /// Whether the source needs a model server.
    pub fn uses_server(&self) -> bool {
        matches!(self, Self::Server)
    }
}

/// How the router reads the values of the entities of the chosen intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtractEngine {
    /// Lists only. A closed entity and a script entity are read against
    /// their list, and an open entity is left without a value, so the
    /// turn asks for it. No model is needed.
    Lists,
    /// The built in GLiNER model finds the span of every label of the
    /// chosen intent.
    Spans,
    /// A language model reads the values of the chosen intent.
    Generative,
}

impl ExtractEngine {
    /// The stored and reported form of the engine.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lists => "lists",
            Self::Spans => "spans",
            Self::Generative => "generative",
        }
    }

    /// Parse the stored form of the engine. An unknown value reads as lists.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "spans" => Self::Spans,
            "generative" => Self::Generative,
            _ => Self::Lists,
        }
    }
}

/// How a mention is read against the values of an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListMatch {
    /// The rules of the matcher: the same spelling, the same words, a
    /// prefix, a part, every word of the value, and a small distance.
    Lexical,
    /// Embeddings only. The values of the list and the mention are
    /// compared as vectors.
    Dense,
    /// The rules first and embeddings for what the rules could not read.
    Both,
}

impl ListMatch {
    /// The stored and reported form of the rule set.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Lexical => "lexical",
            Self::Dense => "dense",
            Self::Both => "both",
        }
    }

    /// Parse the stored form of the rule set. An unknown value reads as lexical.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "dense" => Self::Dense,
            "both" => Self::Both,
            _ => Self::Lexical,
        }
    }

    /// Whether the rule set reads vectors, and therefore needs a server.
    pub fn uses_embeddings(&self) -> bool {
        matches!(self, Self::Dense | Self::Both)
    }

    /// Whether the rule set reads the rules of the matcher.
    pub fn uses_rules(&self) -> bool {
        matches!(self, Self::Lexical | Self::Both)
    }
}

/// One engine that read a part of a handled turn.
///
/// The backend setting says which engines may read a message. This type
/// says which one really read it, so a turn reports the engine it used
/// rather than the engine the settings allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolverEngine {
    /// A llama.cpp server read that part of the turn.
    Llama,
    /// The built in GLiNER model read that part of the turn.
    Gliner,
    /// The layered router read that part of the turn.
    ///
    /// The router reports the stage that decided next to the engine, so a
    /// turn read by the words of the catalog and a turn read by the
    /// embeddings of the catalog are told apart.
    Router,
}

impl ResolverEngine {
    /// The stored and reported form of the engine.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Llama => "llama",
            Self::Gliner => "gliner",
            Self::Router => "router",
        }
    }
}

/// The stage of the layered router that answered a turn.
///
/// The router reads a message in stages and stops at the first stage that
/// can answer, so the metadata of a turn names the stage that decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterStage {
    /// The deterministic pass proved the answer from the message alone.
    FastPath,
    /// The retrieval pass ranked the catalog and the scores decided.
    Retrieve,
    /// The retrieval pass narrowed the catalog and a model chose.
    Rerank,
    /// No stage could answer, so the turn refused.
    None,
}

impl RouterStage {
    /// The stored and reported form of the stage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FastPath => "fast_path",
            Self::Retrieve => "retrieve",
            Self::Rerank => "rerank",
            Self::None => "none",
        }
    }
}

/// The device a built in ONNX model runs on.
///
/// The daemon runs every built in model through ONNX Runtime, so the
/// built in GLiNER reader and the local models of the router share one
/// preference. The runtime reports which devices a build and a machine
/// offer, and the settings page shows only those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LocalDevice {
    /// Pick the best device the build and the machine offer.
    Auto,
    /// Run on the processor.
    Cpu,
    /// Run on a CUDA capable graphics card.
    Cuda,
}

impl LocalDevice {
    /// The stored and reported form of the device.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
        }
    }

    /// Parse the stored form of the device. An unknown value reads as auto.
    pub fn from_stored(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "cpu" => Self::Cpu,
            "cuda" => Self::Cuda,
            _ => Self::Auto,
        }
    }
}

/// Resolver configuration.
/// The resolver reads the intent of a message and the values of its entities.
#[derive(Debug, Clone, Deserialize)]
pub struct ResolverConfig {
    /// The engine that reads the intent and the values.
    pub backend: ResolverBackend,
    /// Base URL of the OpenAI compatible llama.cpp server.
    pub base_url: String,
    /// Model name the server answers to.
    pub model: String,
    /// Time the resolver waits for one decision, in seconds.
    pub timeout_secs: u64,
    /// Largest answer the resolver reads from the model.
    pub max_tokens: u32,
    /// Number of earlier turns the resolver reads as context.
    pub context_turns: u64,
    /// Whether the model may reason before it answers.
    ///
    /// The decision is small, so a reasoning trace costs seconds and adds
    /// little. Turn it on to read the trace of a reasoning model on the
    /// socket stream while the resolver works.
    pub thinking: bool,
    /// Configuration of the built in GLiNER models.
    pub gliner: GlinerConfig,
    /// Configuration of the layered router.
    pub router: RouterConfig,
}

/// Configuration of the layered router.
///
/// The router reads a message in stages. Every stage is switched on its
/// own, so a catalog of two intents and a catalog of two hundred are both
/// served, and every stage has a reader that needs no model at all.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RouterConfig {
    /// Whether the deterministic pass reads the message first.
    ///
    /// The pass answers a message it can prove: the message spells an
    /// intent out, or the message names one value of one list and nothing
    /// else can own it. It needs no model and no server.
    pub fast_path: bool,
    /// The evidence the retrieval pass ranks the catalog with.
    pub retrieve: RetrieveEngine,
    /// How the decision pass chooses one of the short list.
    pub decide: DecideEngine,
    /// How the values of the entities of the chosen intent are read.
    pub extract: ExtractEngine,
    /// Size of the short list the retrieval pass keeps.
    pub top_k: usize,
    /// Smallest score the decision pass accepts, between 0 and 1.
    ///
    /// Below the floor the turn refuses instead of running a command, so
    /// a message the catalog does not hold stays a refusal. Every engine
    /// reads the floor against the score of the ranking, and an engine
    /// that reranks or asks a model reads it before the model does, so a
    /// message the catalog does not hold costs no read at all.
    pub floor: f32,
    /// Smallest distance between the best and the second best, between 0
    /// and 1. Two intents that fit a message equally well are read as no
    /// match, because running the wrong command is worse than refusing.
    pub margin: f32,
    /// Weight of the words of the catalog.
    pub lexical_weight: f32,
    /// Weight of the embeddings of the catalog.
    pub dense_weight: f32,
    /// Model name the embedding server answers to.
    pub embed_model: String,
    /// Directory that holds the downloaded models of the router.
    pub models_dir: PathBuf,
    /// Where the retrieval stage and the list matcher read the vectors.
    pub embed_source: EmbedSource,
    /// Identifier of the built in embedding model.
    ///
    /// The daemon reads this model only when the source above is local.
    pub embed_local_model: String,
    /// Identifier of the built in reranker.
    ///
    /// The daemon reads this model only when the decision stage reranks.
    pub rerank_model: String,
    /// The device a built in model of the router runs on.
    pub local_device: LocalDevice,
    /// Whether a phrase of an intent counts only when the message shares
    /// the action of the phrase.
    ///
    /// A phrase such as `close firefox` holds the word `firefox`, and
    /// without the gate the phrase alone reads `open firefox` as a close.
    pub phrase_gate: bool,
    /// How a mention is read against the values of an entity.
    pub list_match: ListMatch,
    /// Smallest cosine similarity an embedding match of a value needs.
    pub list_floor: f32,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            fast_path: true,
            retrieve: RetrieveEngine::Lexical,
            decide: DecideEngine::Generative,
            extract: ExtractEngine::Spans,
            top_k: 8,
            floor: 0.45,
            margin: 0.10,
            lexical_weight: 1.0,
            dense_weight: 1.0,
            embed_model: "bge-small-en-v1.5".to_string(),
            models_dir: PathBuf::from("models/router"),
            embed_source: EmbedSource::Server,
            embed_local_model: "bge-small-en-v1.5".to_string(),
            rerank_model: "ms-marco-MiniLM-L-6-v2".to_string(),
            local_device: LocalDevice::Auto,
            phrase_gate: true,
            list_match: ListMatch::Lexical,
            list_floor: 0.80,
        }
    }
}

impl RouterConfig {
    /// Whether a stage of the router reads the model server.
    ///
    /// The retrieval stage reads vectors, the list matcher reads vectors,
    /// and the decision stage reads a model. Each of the three reads the
    /// server only when its own settings point at it, so a router that
    /// runs on built in models alone reports no server it needs.
    pub fn needs_server(&self) -> bool {
        let vectors_from_server = self.embed_source.uses_server()
            && (self.retrieve.uses_embeddings() || self.list_match.uses_embeddings());
        vectors_from_server || self.decide.uses_model()
    }

    /// Whether a stage of the router reads a built in model.
    ///
    /// A built in model is a file on disk, so the settings page shows
    /// whether that file is there before a turn depends on it.
    pub fn uses_local_models(&self) -> bool {
        let vectors_from_disk = matches!(self.embed_source, EmbedSource::Local)
            && (self.retrieve.uses_embeddings() || self.list_match.uses_embeddings());
        vectors_from_disk || self.decide.uses_local_model()
    }
}

/// Configuration of the built in GLiNER models.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GlinerConfig {
    /// Identifier of the model, for example `gliner_small-v2.1`.
    pub model: String,
    /// The device the model runs on.
    pub device: LocalDevice,
    /// Smallest probability a label needs to count, between 0 and 1.
    pub threshold: f32,
    /// Directory that holds the downloaded models.
    pub models_dir: PathBuf,
    /// Number of threads one inference may use.
    pub threads: usize,
}

/// Configuration of the scripts that provide the values of an entity.
///
/// A script entity is a live list: the daemon runs the script, keeps the
/// values in memory, and reads the value of a message out of that list.
/// The list belongs to the turn and not to the database, so a script that
/// lists the applications of a machine stays a script and not a table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntityScriptConfig {
    /// Time one script may run, in seconds.
    ///
    /// A turn waits for the list, so this is much shorter than the timeout
    /// of an intent command.
    pub timeout_secs: u64,
    /// How long the daemon keeps the values of one script in memory.
    ///
    /// Zero runs the script on every turn that needs those values.
    pub cache_secs: u64,
    /// Largest number of values the daemon keeps from one script.
    pub max_values: usize,
    /// Largest number of values the daemon offers a model as labels.
    ///
    /// A vanilla GLiNER model degrades past about thirty labels and its
    /// window is small, so a longer list is read by matching instead: the
    /// model finds the words of the value and the daemon reads the entry
    /// of the list those words name.
    pub label_budget: usize,
}

impl Default for EntityScriptConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 3,
            cache_secs: 60,
            max_values: 200,
            label_budget: 24,
        }
    }
}

/// Execution configuration.
/// The executor runs the shell command of a resolved intent.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    /// Whether the daemon runs the command of a resolved intent.
    pub enabled: bool,
    /// Time one command may run, in seconds.
    pub timeout_secs: u64,
    /// Largest output the daemon keeps from one command.
    pub max_output_bytes: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8787".to_string(),
            cors_origin: "http://localhost:5173".to_string(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://alice.db?mode=rwc".to_string(),
            max_connections: 5,
        }
    }
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            page_limit_default: 50,
            page_limit_max: 200,
            max_message_len: 4000,
        }
    }
}

impl Default for ResolverConfig {
    fn default() -> Self {
        Self {
            backend: ResolverBackend::Hybrid,
            base_url: "http://127.0.0.1:8012/v1".to_string(),
            model: "qwen3.5-4b".to_string(),
            timeout_secs: 30,
            max_tokens: 512,
            context_turns: 6,
            thinking: false,
            gliner: GlinerConfig::default(),
            router: RouterConfig::default(),
        }
    }
}

impl Default for GlinerConfig {
    fn default() -> Self {
        Self {
            model: "gliner_small-v2.1".to_string(),
            device: LocalDevice::Auto,
            threshold: 0.3,
            models_dir: PathBuf::from("models/gliner"),
            threads: 4,
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_secs: 20,
            max_output_bytes: 16_384,
        }
    }
}

/// Read one environment variable into a trimmed string.
fn read_text(name: &str) -> Option<String> {
    let value = std::env::var(name).ok()?;
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Read one environment variable into a number.
fn read_number<T: std::str::FromStr>(name: &str) -> Option<T> {
    read_text(name)?.parse().ok()
}

/// Read one environment variable into a boolean.
fn read_flag(name: &str) -> Option<bool> {
    let value = read_text(name)?.to_lowercase();
    match value.as_str() {
        "1" | "true" | "on" | "yes" => Some(true),
        "0" | "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

impl CoreConfig {
    /// Load config from environment with defaults.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Some(addr) = read_text("ALICE_LISTEN_ADDR") {
            cfg.server.listen_addr = addr;
        }
        if let Some(origin) = read_text("ALICE_CORS_ORIGIN") {
            cfg.server.cors_origin = origin;
        }
        if let Some(url) = read_text("DATABASE_URL") {
            cfg.database.url = url;
        }
        if let Some(value) = read_number("ALICE_MAX_CONNECTIONS") {
            cfg.database.max_connections = value;
        }
        if let Some(value) = read_flag("ALICE_QUEUE_ENABLED") {
            cfg.queue.enabled = value;
        }
        if let Some(url) = read_text("ALICE_RESOLVER_BASE_URL") {
            cfg.resolver.base_url = url;
        }
        if let Some(model) = read_text("ALICE_RESOLVER_MODEL") {
            cfg.resolver.model = model;
        }
        if let Some(backend) = read_text("ALICE_RESOLVER_BACKEND") {
            cfg.resolver.backend = ResolverBackend::from_stored(&backend);
        }
        if let Some(model) = read_text("ALICE_GLINER_MODEL") {
            cfg.resolver.gliner.model = model;
        }
        if let Some(device) = read_text("ALICE_GLINER_DEVICE") {
            cfg.resolver.gliner.device = LocalDevice::from_stored(&device);
        }
        if let Some(value) = read_number("ALICE_GLINER_THRESHOLD") {
            cfg.resolver.gliner.threshold = value;
        }
        if let Some(value) = read_number("ALICE_GLINER_THREADS") {
            cfg.resolver.gliner.threads = value;
        }
        if let Some(dir) = read_text("ALICE_GLINER_MODELS_DIR") {
            cfg.resolver.gliner.models_dir = PathBuf::from(dir);
        }
        if let Some(value) = read_number("ALICE_RESOLVER_TIMEOUT_SECS") {
            cfg.resolver.timeout_secs = value;
        }
        if let Some(value) = read_number("ALICE_RESOLVER_MAX_TOKENS") {
            cfg.resolver.max_tokens = value;
        }
        if let Some(value) = read_number("ALICE_RESOLVER_CONTEXT_TURNS") {
            cfg.resolver.context_turns = value;
        }
        if let Some(value) = read_flag("ALICE_RESOLVER_THINKING") {
            cfg.resolver.thinking = value;
        }
        if let Some(value) = read_flag("ALICE_EXECUTION_ENABLED") {
            cfg.execution.enabled = value;
        }
        if let Some(value) = read_number("ALICE_EXECUTION_TIMEOUT_SECS") {
            cfg.execution.timeout_secs = value;
        }
        if let Some(value) = read_number("ALICE_EXECUTION_MAX_OUTPUT_BYTES") {
            cfg.execution.max_output_bytes = value;
        }
        if let Some(value) = read_number("ALICE_ENTITY_SCRIPT_TIMEOUT_SECS") {
            cfg.entity_script.timeout_secs = value;
        }
        if let Some(value) = read_number("ALICE_ENTITY_SCRIPT_CACHE_SECS") {
            cfg.entity_script.cache_secs = value;
        }
        if let Some(value) = read_number("ALICE_ENTITY_SCRIPT_MAX_VALUES") {
            cfg.entity_script.max_values = value;
        }
        if let Some(value) = read_flag("ALICE_ROUTER_FAST_PATH") {
            cfg.resolver.router.fast_path = value;
        }
        if let Some(value) = read_text("ALICE_ROUTER_RETRIEVE") {
            cfg.resolver.router.retrieve = RetrieveEngine::from_stored(&value);
        }
        if let Some(value) = read_text("ALICE_ROUTER_DECIDE") {
            cfg.resolver.router.decide = DecideEngine::from_stored(&value);
        }
        if let Some(value) = read_text("ALICE_ROUTER_EXTRACT") {
            cfg.resolver.router.extract = ExtractEngine::from_stored(&value);
        }
        if let Some(value) = read_number("ALICE_ROUTER_TOP_K") {
            cfg.resolver.router.top_k = value;
        }
        if let Some(value) = read_number("ALICE_ROUTER_FLOOR") {
            cfg.resolver.router.floor = value;
        }
        if let Some(value) = read_number("ALICE_ROUTER_MARGIN") {
            cfg.resolver.router.margin = value;
        }
        if let Some(value) = read_number("ALICE_ROUTER_LEXICAL_WEIGHT") {
            cfg.resolver.router.lexical_weight = value;
        }
        if let Some(value) = read_number("ALICE_ROUTER_DENSE_WEIGHT") {
            cfg.resolver.router.dense_weight = value;
        }
        if let Some(value) = read_text("ALICE_ROUTER_EMBED_MODEL") {
            cfg.resolver.router.embed_model = value;
        }
        if let Some(dir) = read_text("ALICE_ROUTER_MODELS_DIR") {
            cfg.resolver.router.models_dir = PathBuf::from(dir);
        }
        if let Some(value) = read_text("ALICE_ROUTER_EMBED_SOURCE") {
            cfg.resolver.router.embed_source = EmbedSource::from_stored(&value);
        }
        if let Some(value) = read_text("ALICE_ROUTER_EMBED_LOCAL_MODEL") {
            cfg.resolver.router.embed_local_model = value;
        }
        if let Some(value) = read_text("ALICE_ROUTER_RERANK_MODEL") {
            cfg.resolver.router.rerank_model = value;
        }
        if let Some(value) = read_text("ALICE_ROUTER_LOCAL_DEVICE") {
            cfg.resolver.router.local_device = LocalDevice::from_stored(&value);
        }
        if let Some(value) = read_flag("ALICE_ROUTER_PHRASE_GATE") {
            cfg.resolver.router.phrase_gate = value;
        }
        if let Some(value) = read_text("ALICE_ROUTER_LIST_MATCH") {
            cfg.resolver.router.list_match = ListMatch::from_stored(&value);
        }
        if let Some(value) = read_number("ALICE_ROUTER_LIST_FLOOR") {
            cfg.resolver.router.list_floor = value;
        }
        cfg
    }

    /// Validate the config and return `CoreError` when it is invalid.
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.server.listen_addr.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "listen_addr is empty".to_string(),
            });
        }
        if self.database.url.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "database.url is empty".to_string(),
            });
        }
        if self.queue.max_message_len == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "max_message_len is zero".to_string(),
            });
        }
        if self.resolver.base_url.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.base_url is empty".to_string(),
            });
        }
        if self.resolver.model.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.model is empty".to_string(),
            });
        }
        if self.resolver.gliner.model.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.gliner.model is empty".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&self.resolver.gliner.threshold) {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.gliner.threshold is outside 0 and 1".to_string(),
            });
        }
        if self.resolver.gliner.threads == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.gliner.threads is zero".to_string(),
            });
        }
        if self.resolver.timeout_secs == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.timeout_secs is zero".to_string(),
            });
        }
        if self.execution.timeout_secs == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "execution.timeout_secs is zero".to_string(),
            });
        }
        if self.execution.max_output_bytes == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "execution.max_output_bytes is zero".to_string(),
            });
        }
        if self.entity_script.max_values == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "entity_script.max_values is zero".to_string(),
            });
        }
        let router = &self.resolver.router;
        if router.top_k == 0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.top_k is zero".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&router.floor) {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.floor is outside 0 and 1".to_string(),
            });
        }
        if !(0.0..=1.0).contains(&router.margin) {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.margin is outside 0 and 1".to_string(),
            });
        }
        if router.lexical_weight < 0.0 || router.dense_weight < 0.0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router weights are negative".to_string(),
            });
        }
        if router.lexical_weight == 0.0 && router.dense_weight == 0.0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router weights are both zero".to_string(),
            });
        }
        if router.retrieve.uses_embeddings() && router.dense_weight == 0.0 {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.dense_weight is zero while the retrieval reads embeddings"
                    .to_string(),
            });
        }
        if !(0.0..=1.0).contains(&router.list_floor) {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.list_floor is outside 0 and 1".to_string(),
            });
        }
        if router.embed_source.uses_server()
            && (router.retrieve.uses_embeddings() || router.list_match.uses_embeddings())
            && router.embed_model.trim().is_empty()
        {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.embed_model is empty".to_string(),
            });
        }
        if router.embed_local_model.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.embed_local_model is empty".to_string(),
            });
        }
        if router.rerank_model.trim().is_empty() {
            return Err(crate::error::CoreError::ConfigInvalid {
                reason: "resolver.router.rerank_model is empty".to_string(),
            });
        }
        Ok(())
    }

    /// Return the database file path when the URL is a sqlite file.
    pub fn sqlite_path(&self) -> Option<PathBuf> {
        let url = self.database.url.strip_prefix("sqlite://")?;
        let path = url.split('?').next()?;
        Some(PathBuf::from(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_point_at_the_local_llama_server() {
        let cfg = CoreConfig::default();
        assert_eq!(cfg.resolver.base_url, "http://127.0.0.1:8012/v1");
        assert_eq!(cfg.resolver.model, "qwen3.5-4b");
        assert!(!cfg.resolver.thinking);
        assert!(cfg.execution.enabled);
    }

    #[test]
    fn defaults_use_both_resolvers() {
        let cfg = CoreConfig::default();
        assert_eq!(cfg.resolver.backend, ResolverBackend::Hybrid);
        assert!(cfg.resolver.backend.uses_llama());
        assert!(cfg.resolver.backend.uses_gliner());
        assert_eq!(cfg.resolver.gliner.device, LocalDevice::Auto);
        assert_eq!(cfg.resolver.gliner.model, "gliner_small-v2.1");
    }

    #[test]
    fn backend_reads_the_stored_form() {
        assert_eq!(
            ResolverBackend::from_stored("gliner"),
            ResolverBackend::Gliner
        );
        assert_eq!(
            ResolverBackend::from_stored("llama"),
            ResolverBackend::Llama
        );
        assert_eq!(
            ResolverBackend::from_stored("something else"),
            ResolverBackend::Hybrid
        );
        assert!(!ResolverBackend::Gliner.uses_llama());
    }

    #[test]
    fn validate_rejects_a_threshold_outside_the_range() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.gliner.threshold = 1.5;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_a_blank_resolver_model() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.model = "  ".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_accepts_the_defaults() {
        assert!(CoreConfig::default().validate().is_ok());
    }

    #[test]
    fn defaults_wait_briefly_for_an_entity_script() {
        let cfg = CoreConfig::default();
        assert_eq!(cfg.entity_script.timeout_secs, 3);
        assert_eq!(cfg.entity_script.cache_secs, 60);
        assert_eq!(cfg.entity_script.label_budget, 24);
    }

    #[test]
    fn validate_rejects_an_entity_script_without_room() {
        let mut cfg = CoreConfig::default();
        cfg.entity_script.max_values = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn the_router_reads_the_short_list_with_a_model_it_already_has() {
        let cfg = CoreConfig::default();
        let router = &cfg.resolver.router;
        assert!(router.fast_path);
        assert_eq!(router.retrieve, RetrieveEngine::Lexical);
        assert_eq!(router.decide, DecideEngine::Generative);
        assert_eq!(router.extract, ExtractEngine::Spans);
        assert!(router.phrase_gate);
        assert_eq!(router.top_k, 8);
    }

    #[test]
    fn the_router_reads_the_stored_form_of_every_stage() {
        assert_eq!(
            ResolverBackend::from_stored("router"),
            ResolverBackend::Router
        );
        assert_eq!(ResolverBackend::Router.as_str(), "router");
        assert_eq!(RetrieveEngine::from_stored("DENSE"), RetrieveEngine::Dense);
        assert_eq!(DecideEngine::from_stored("nonsense"), DecideEngine::Score);
        assert_eq!(
            ExtractEngine::from_stored("generative"),
            ExtractEngine::Generative
        );
        assert_eq!(ListMatch::from_stored(" both "), ListMatch::Both);
        assert_eq!(RouterStage::FastPath.as_str(), "fast_path");
    }

    #[test]
    fn an_engine_of_a_stage_says_what_it_needs() {
        assert!(!RetrieveEngine::Lexical.uses_embeddings());
        assert!(RetrieveEngine::Dense.uses_embeddings());
        assert!(RetrieveEngine::Hybrid.uses_embeddings());
        assert!(!DecideEngine::Score.uses_model());
        assert!(DecideEngine::Generative.uses_model());
    }

    #[test]
    fn validate_rejects_a_floor_outside_the_range() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.router.floor = 1.5;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_a_short_list_without_room() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.router.top_k = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_two_weights_of_zero() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.router.lexical_weight = 0.0;
        cfg.resolver.router.dense_weight = 0.0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_embeddings_without_a_model() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.router.retrieve = RetrieveEngine::Dense;
        cfg.resolver.router.embed_model = "  ".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn the_router_defaults_read_the_vectors_from_the_server() {
        let cfg = CoreConfig::default();
        let router = &cfg.resolver.router;
        assert_eq!(router.embed_source, EmbedSource::Server);
        assert_eq!(router.embed_local_model, "bge-small-en-v1.5");
        assert_eq!(router.rerank_model, "ms-marco-MiniLM-L-6-v2");
        assert_eq!(router.local_device, LocalDevice::Auto);
    }

    #[test]
    fn a_stage_says_which_place_it_needs() {
        let mut router = RouterConfig {
            retrieve: RetrieveEngine::Dense,
            decide: DecideEngine::Score,
            list_match: ListMatch::Lexical,
            ..RouterConfig::default()
        };
        assert!(router.needs_server());
        assert!(!router.uses_local_models());

        router.embed_source = EmbedSource::Local;
        assert!(!router.needs_server());
        assert!(router.uses_local_models());

        router.decide = DecideEngine::Rerank;
        assert!(!router.needs_server());

        router.decide = DecideEngine::Generative;
        assert!(router.needs_server());
    }

    #[test]
    fn the_rerank_engine_has_to_be_told_which_model_to_run() {
        assert!(DecideEngine::Rerank.uses_local_model());
        assert!(!DecideEngine::Score.uses_local_model());
        assert_eq!(DecideEngine::from_stored(" rerank "), DecideEngine::Rerank);
        assert_eq!(DecideEngine::Rerank.as_str(), "rerank");
    }

    #[test]
    fn validate_rejects_a_blank_local_model() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.router.embed_local_model = "  ".to_string();
        assert!(cfg.validate().is_err());

        let mut cfg = CoreConfig::default();
        cfg.resolver.router.rerank_model = "  ".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn the_embed_source_reads_the_stored_form() {
        assert_eq!(EmbedSource::from_stored(" local "), EmbedSource::Local);
        assert_eq!(EmbedSource::from_stored("nonsense"), EmbedSource::Server);
        assert!(EmbedSource::Server.uses_server());
        assert!(!EmbedSource::Local.uses_server());
    }

    #[test]
    fn validate_accepts_the_router_defaults() {
        let mut cfg = CoreConfig::default();
        cfg.resolver.backend = ResolverBackend::Router;
        assert!(cfg.validate().is_ok());
    }
}
