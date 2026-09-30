//! DTOs of the REST and socket contract.
//! The backend owns these types. The frontend copies them into `frontend-web/src/types/dto.ts`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Daemon state that mirrors the backend state machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum DaemonStateDto {
    Idle,
    Listening,
    Transcribing,
    Resolving,
    Executing,
    Error,
}

/// Reason why the daemon left the Listening state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub enum StopReasonDto {
    SilenceTimeout,
    UserStopped,
    WakeDetected,
    Error,
}

/// Reply of `GET /api/v1/status`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusDto {
    /// Current daemon state.
    pub state: DaemonStateDto,
    /// Time the daemon entered the state, in ISO 8601.
    pub since: DateTime<Utc>,
    /// Daemon version.
    pub version: String,
}

/// One event of the socket stream.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SystemEventDto {
    /// Time the daemon emitted the event, in ISO 8601.
    pub at: DateTime<Utc>,
    /// Payload of the event, tagged by the backend variant name.
    pub payload: SystemEventPayloadDto,
}

/// Payload of a `SystemEventDto`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum SystemEventPayloadDto {
    WakeDetected {
        timestamp: DateTime<Utc>,
    },
    ListeningStarted,
    ListeningStopped {
        reason: StopReasonDto,
    },
    TranscriptInterim {
        text: String,
    },
    TranscriptFinal {
        text: String,
    },
    TranscriptCorrected {
        original: String,
        corrected: String,
    },
    /// Token the resolver read from the model while it decides.
    IntentThinking {
        delta: String,
    },
    /// The stage of the resolver that runs right now.
    ///
    /// The resolver reports each stage as it starts, so a surface shows
    /// the work of a turn step by step instead of one label that never
    /// changes. A stage that reads a model says so in its own event, so
    /// this event names the step and not the reader behind it.
    IntentStage {
        /// The stage: `fast_path`, `retrieve`, `decide`, `extract`, or
        /// `answer` for the model that answers a message no intent matched.
        stage: String,
        /// One sentence about what the stage is doing, in the present tense.
        detail: String,
    },
    IntentResolved {
        intent: String,
        confidence: Option<f32>,
        /// The engine that chose the intent.
        engine: String,
        /// The model that engine ran, or null when the daemon cannot name it.
        #[serde(default)]
        model: Option<String>,
    },
    /// One value the resolver read for an entity of the chosen intent.
    IntentValuesRead {
        /// The engine that read the values.
        engine: String,
        /// The model that engine ran, or null when the daemon cannot name it.
        #[serde(default)]
        model: Option<String>,
        /// The entities with the values the engine read.
        entities: Vec<MessageEntityDto>,
    },
    IntentNotFound {
        text: String,
    },
    ExecutionStarted {
        intent: String,
        command: String,
    },
    /// One line the command of an intent wrote to its output.
    ExecutionOutput {
        stream: OutputStreamDto,
        line: String,
    },
    ExecutionCompleted {
        intent: String,
        exit_code: i32,
        duration_ms: u64,
        output: String,
    },
    ExecutionFailed {
        intent: String,
        error: String,
    },
    /// Queue event emitted when a chat message is stored.
    MessageQueued {
        id: Uuid,
    },
    /// Queue event emitted when the daemon replies.
    MessageReplied {
        id: Uuid,
    },
    /// The model wrote a shell script for a message no intent matched.
    ///
    /// The daemon runs no script of its own accord, so the event carries
    /// the script and its rating for the user to approve or deny.
    ScriptProposed {
        id: Uuid,
        summary: String,
        script: String,
        destructiveness: u8,
    },
    /// The user approved one script, so the daemon may run it.
    ScriptApproved {
        id: Uuid,
    },
    /// The user denied one script, so the daemon runs it never.
    ScriptDenied {
        id: Uuid,
    },
    /// The memory learned facts from one stored turn.
    ///
    /// The memory reads a turn after the daemon answered it, so this event
    /// reaches the surface later than the reply it belongs to. It names the
    /// message of the turn, so the transcript marks the message the memory
    /// learned from rather than the turn that happens to be on screen.
    MemorySaved {
        message_id: Uuid,
        facts: Vec<MemorySavedFactDto>,
    },
}

/// The stream a command wrote one line to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(rename_all = "lowercase")]
pub enum OutputStreamDto {
    /// Standard output.
    Stdout,
    /// Standard error.
    Stderr,
}

/// Speaker of one chat message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(rename_all = "lowercase")]
pub enum ChatRoleDto {
    User,
    Assistant,
}

impl ChatRoleDto {
    /// The stored form of the role.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    /// Parse the stored form of the role. An unknown value reads as user.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "assistant" => Self::Assistant,
            _ => Self::User,
        }
    }
}

/// One stored turn of a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ChatMessageDto {
    /// Stable message identifier.
    pub id: Uuid,
    /// Conversation the message belongs to, or null when the queue is off.
    pub conversation_id: Option<Uuid>,
    /// Speaker.
    pub role: ChatRoleDto,
    /// Message text.
    pub text: String,
    /// Time the daemon stored the message, in ISO 8601.
    pub created_at: DateTime<Utc>,
    /// Resolved intent identifier, or null when none.
    pub intent_id: Option<String>,
    /// Name of the resolved intent, or null when none.
    pub intent_name: Option<String>,
    /// Resolver confidence between 0 and 1, or null.
    pub confidence: Option<f32>,
    /// How the daemon read the turn, or null when it read nothing.
    pub meta: Option<MessageMetaDto>,
    ///
    /// What the turn taught the long term memory, or null when the memory
    /// learned nothing from it. The memory reads a turn after the daemon
    /// answered it, so the field is filled when a reader reads the message
    /// back rather than when the turn is stored.
    pub memory: Option<MessageMemoryDto>,
}

/// What one turn taught the long term memory.
///
/// The memory learns from the turns that met no intent, and it writes them
/// in the background. This is what a reader sees of that work: the facts
/// the turn taught, each with the concept it belongs to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MessageMemoryDto {
    /// The facts the turn taught, in the order the memory wrote them.
    pub facts: Vec<MemorySavedFactDto>,
}

/// One fact that one turn taught the long term memory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemorySavedFactDto {
    /// The name of the concept the fact belongs to.
    pub concept: String,
    /// The relation the fact names.
    pub relation: String,
    /// The value the fact carries.
    pub value: String,
}

/// How the daemon read one handled turn.
///
/// The resolver owns two steps: it chooses the intent, and it reads the
/// values of the entities of that intent. Either step may run on the
/// llama.cpp server or on the built in GLiNER model, so a turn reports
/// the engine of each step. The rest of the metadata is what the daemon
/// then ran, so the transcript can explain a stored turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MessageMetaDto {
    /// The engine that chose the intent, or null when none chose one.
    pub intent_engine: Option<String>,
    /// The model that engine ran, or null when the daemon cannot name it.
    pub intent_model: Option<String>,
    /// The engine that read the entity values, or null when none did.
    pub value_engine: Option<String>,
    /// The model that engine ran, or null when the daemon cannot name it.
    pub value_model: Option<String>,
    /// The stage of the layered router that decided the turn, or null
    /// when the backend of the turn is not the router.
    ///
    /// A turn the deterministic pass proved reads `fast_path`, a turn the
    /// retrieval scores decided reads `retrieve`, and a turn a model
    /// chose from the short list reads `rerank`.
    pub stage: Option<String>,
    /// The intents the retrieval pass kept for this turn, best first.
    ///
    /// The short list is the evidence of the decision, so a user who
    /// reads `no intent matched` still reads what the catalog offered.
    #[serde(default)]
    pub candidates: Vec<MessageCandidateDto>,
    /// The entities of the intent with the values the daemon read.
    #[serde(default)]
    pub entities: Vec<MessageEntityDto>,
    /// How the layered router read the turn, stage by stage, or null when
    /// the backend of the turn is not the router.
    ///
    /// The stage above names the stage that decided, and this names every
    /// stage the turn passed with the reader it ran and the short list it
    /// read, so a stored turn can be read back the way the daemon read it.
    pub route: Option<MessageRouteDto>,
    /// The command after the daemon put the values in, or null.
    pub command: Option<String>,
    /// The exit code of the command, or null when none ran.
    pub exit_code: Option<i32>,
    /// How long the command ran, in milliseconds, or null when none ran.
    pub duration_ms: Option<u64>,
    /// The identifier of the script the model wrote for this turn, or
    /// null when the model answered in words.
    ///
    /// The store keeps the script, so a user can still decide about a
    /// turn that a restart or a closed socket left behind.
    #[serde(default)]
    pub script_id: Option<String>,
    /// How long the daemon needed to read the turn, in milliseconds, or
    /// null when it did not measure the read.
    ///
    /// The read runs from the queued message to the command, so a slow
    /// turn is told apart from a slow command: the duration above is the
    /// command alone and this is the resolver.
    #[serde(default)]
    pub resolve_ms: Option<u64>,
    /// The concepts the daemon read from the long term memory for this
    /// turn, as plain text, or null when the memory carried none.
    ///
    /// The memory belongs to the branch that no intent matches, so only a
    /// turn without an intent reports what the daemon read of it: the
    /// facts the answer of the turn leaned on.
    #[serde(default)]
    pub memory_seed: Option<String>,
}

/// How the layered router read one turn, stage by stage.
///
/// The stages of the router run in order and the first one that can answer
/// ends the turn, so the stages a turn passed are its route. The route
/// carries the reader every stage ran and the short list it read, which is
/// what a reader of the transcript needs to see why a message met an
/// intent, met none, or was answered by a stage it did not expect.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MessageRouteDto {
    /// The stage that decided the turn, or null when the router refused.
    pub stage: Option<String>,
    /// True when the turn met an intent.
    pub matched: bool,
    /// Why the router refused, or null when it chose.
    pub reason: Option<String>,
    /// The stages the turn passed, in the order they ran.
    #[serde(default)]
    pub steps: Vec<MessageRouteStepDto>,
}

/// One stage of the layered router in one turn.
///
/// Every stage reports what it read and how it ended, so a turn that a
/// model answered and a turn the words answered are told apart, and a
/// stage that fell back to its cheaper reader says so.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MessageRouteStepDto {
    /// The stage: `fast_path`, `retrieve`, `decide`, or `extract`.
    pub stage: String,
    /// How the stage ended: `matched`, `refused`, `passed`, `fell_back`,
    /// or `skipped`.
    pub outcome: String,
    /// The reader the stage ran, for example `rules`, `words`, `reranker`,
    /// or `model`.
    pub reader: String,
    /// The model the reader ran, or null when it ran none.
    pub model: Option<String>,
    /// One sentence about what the stage read, or null.
    pub detail: Option<String>,
    /// The candidates the stage read, best first.
    #[serde(default)]
    pub candidates: Vec<MessageCandidateDto>,
    /// How long the stage took, in milliseconds.
    pub duration_ms: u64,
}

/// One entity of an intent with the value the resolver read for it.
///
/// A value is read by one reader of the pipeline, and which one read it is
/// what tells a value the daemon proved from a value a model guessed, so
/// the read is reported next to the value it produced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MessageEntityDto {
    /// Name of the entity, for example `city`.
    pub name: String,
    /// The value the resolver read, empty when it read none.
    pub value: String,
    /// How the daemon read the value, or null when it read none:
    /// `fast_path`, `list`, `embedding`, `spans`, or `model`.
    #[serde(default)]
    pub source: Option<String>,
    /// The engine that ran the reader, or null when the daemon read the
    /// value itself.
    #[serde(default)]
    pub engine: Option<String>,
    /// The model the reader ran, or null when it ran none.
    #[serde(default)]
    pub model: Option<String>,
    /// What the reader read before the value was matched to an entry of a
    /// list, or null when the value is the entry itself.
    #[serde(default)]
    pub read: Option<String>,
    /// The similarity the match read, between 0 and 1, or null when the
    /// reader reported no score.
    #[serde(default)]
    pub score: Option<f32>,
}

/// One intent the retrieval pass offered for a turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MessageCandidateDto {
    /// Name of the intent.
    pub name: String,
    /// The score of the intent for this turn, between 0 and 1.
    pub score: f32,
    /// Which evidence read the score, for example `words` or `embedding`.
    pub evidence: String,
}

/// One conversation of the daemon. The conversation owns its message history.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ConversationDto {
    /// Stable conversation identifier.
    pub id: Uuid,
    /// Title the daemon generated from the first message.
    pub title: String,
    /// Time the daemon created the conversation, in ISO 8601.
    pub created_at: DateTime<Utc>,
    /// Time the daemon stored the last message, in ISO 8601.
    pub updated_at: DateTime<Utc>,
}

/// Reply of `GET /api/v1/conversations`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ConversationListDto {
    /// Conversations, most recently updated first.
    pub items: Vec<ConversationDto>,
}

/// Reply of `GET /api/v1/conversations/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ConversationDetailDto {
    /// The conversation.
    pub conversation: ConversationDto,
    /// The full message history, oldest first.
    pub messages: Vec<ChatMessageDto>,
}

/// Query of `GET /api/v1/conversations`.
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(clippy::needless_maybe_sized)]
pub struct ConversationQuery {
    /// Maximum items to return.
    pub limit: Option<u64>,
}

impl ConversationQuery {
    /// The page size, kept inside the given range.
    pub fn limit_within(&self, default: u64, max: u64) -> u64 {
        self.limit.unwrap_or(default).clamp(1, max)
    }
}

/// Body of `POST /api/v1/chat`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ChatRequestDto {
    /// Text the user typed.
    pub text: String,
    /// Conversation to append to. Null starts a new conversation.
    pub conversation_id: Option<Uuid>,
    /// Whether the daemon reads the earlier turns of the conversation as
    /// the context of this message. Null reads them.
    ///
    /// A message sent with this false is read on its own, so the router
    /// holds it against the catalog alone and a model answers without the
    /// turns before it. A follow-up whose meaning depends on the turn
    /// before it needs the context, so the default reads it.
    #[serde(default)]
    pub context: Option<bool>,
}

/// Reply of `POST /api/v1/chat`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ChatReplyDto {
    /// True when the daemon stored the turn, false when the queue is off.
    pub stored: bool,
    /// The conversation of the turn, or null when the queue is off.
    pub conversation: Option<ConversationDto>,
    /// Stored user message.
    pub user: ChatMessageDto,
    /// Daemon reply.
    pub reply: ChatMessageDto,
}

/// Body of `POST /api/v1/resolver/preview`.
///
/// A preview reads one message the way a turn would and runs nothing, so
/// the settings page can show the route of a sentence before the user
/// sends it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ResolverPreviewRequestDto {
    /// The message to read.
    pub text: String,
}

/// Reply of `POST /api/v1/resolver/preview`.
///
/// The preview carries the same metadata a stored turn carries, so a
/// sentence the user tries is read with the same report as a message the
/// daemon really handled, minus the run. The intent it met, the reader
/// that read every value, and the route the message took are the report a
/// user tunes the pipeline by.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ResolverPreviewDto {
    /// The message the daemon read.
    pub text: String,
    /// True when the resolver met an intent.
    pub matched: bool,
    /// The name of the intent the resolver met, or null when it met none.
    pub intent: Option<String>,
    /// The probability of the choice, or null when the resolver sent none.
    pub confidence: Option<f32>,
    /// The reply the daemon would store, or empty when the turn would run
    /// the command of its intent.
    pub reply: String,
    /// How the daemon read the turn, or null when it read nothing at all.
    pub meta: Option<MessageMetaDto>,
}

/// Health reply of `GET /api/health`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HealthDto {
    /// Service status.
    pub status: String,
    /// Daemon version.
    pub version: String,
}

/// One shell script that waits for the decision of the user.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ScriptDto {
    /// Stable identifier of the script.
    pub id: Uuid,
    /// Conversation the turn belongs to, or null when the queue is off.
    pub conversation_id: Option<Uuid>,
    /// The message the model wrote the script for.
    pub request_text: String,
    /// One sentence about what the script does.
    pub summary: String,
    /// The shell script.
    pub script: String,
    /// How rough the script is on the machine, between 0 and 100.
    pub destructiveness: u8,
    /// The decision of the user: pending, approved, denied, or ran.
    pub status: String,
    /// Time the daemon stored the script, in ISO 8601.
    pub created_at: DateTime<Utc>,
}

/// Reply of `GET /api/v1/settings` and `PUT /api/v1/settings`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SettingsDto {
    /// Whether the queue stores messages.
    pub queue_enabled: bool,
    /// The engine that reads the intent of a message.
    pub resolver_backend: String,
    /// Base URL of the intent resolver.
    pub resolver_base_url: String,
    /// Model name the intent resolver asks the server for.
    pub resolver_model: String,
    /// Identifier of the built in GLiNER model.
    pub gliner_model: String,
    /// The device a GLiNER model runs on.
    pub gliner_device: String,
    /// Smallest probability a GLiNER label needs to count.
    pub gliner_threshold: f32,
    /// Whether the deterministic pass of the router reads the message first.
    pub router_fast_path: bool,
    /// The evidence the retrieval pass ranks the catalog with.
    pub router_retrieve: String,
    /// How the decision pass chooses one of the short list.
    pub router_decide: String,
    /// How the values of the entities of the chosen intent are read.
    pub router_extract: String,
    /// Size of the short list the retrieval pass keeps.
    pub router_top_k: usize,
    /// Smallest score the decision pass accepts.
    pub router_floor: f32,
    /// Smallest distance between the best and the second best.
    pub router_margin: f32,
    /// Weight of the words of the catalog.
    pub router_lexical_weight: f32,
    /// Weight of the embeddings of the catalog.
    pub router_dense_weight: f32,
    /// Model name the embedding server answers to.
    pub router_embed_model: String,
    /// Directory that holds the downloaded models of the router.
    pub router_models_dir: String,
    /// Where the retrieval stage and the list matcher read the vectors.
    pub router_embed_source: String,
    /// Identifier of the built in embedding model.
    pub router_embed_local_model: String,
    /// Identifier of the built in reranker.
    pub router_rerank_model: String,
    /// Identifier of the built in decision model.
    pub router_laya_model: String,
    /// The device a built in model of the router runs on.
    pub router_local_device: String,
    /// Whether a phrase counts only when the message shares its action.
    pub router_phrase_gate: bool,
    /// How a mention is read against the values of an entity.
    pub router_list_match: String,
    /// Smallest cosine similarity an embedding match of a value needs.
    pub router_list_floor: f32,
    /// Whether the language model answers a message no intent matched.
    pub router_fallback_llm: bool,
    /// Whether the language model may write a shell script for a message
    /// no intent matched. The daemon runs no script without approval.
    pub router_script_fallback: bool,
    /// Whether the language model reads an open value the built in reader
    /// found none of.
    pub router_open_values_llm: bool,
    /// Response quality between 0 and 100. 0 is fastest, 100 is best.
    /// Speed is 100 - quality. The value lerps between the minimum
    /// successful settings and the maximum quality settings.
    pub response_quality: u8,
    /// Response speed between 0 and 100. Always 100 - quality.
    pub response_speed: u8,
    /// The sentences the settings page tries against the resolver.
    ///
    /// The sentences are the tests of a user and not a setting of the
    /// pipeline, so they are stored with the settings the page shows and
    /// never reach a turn.
    pub preview_sentences: Vec<String>,
    /// Whether the librarian keeps a long term memory.
    pub librarian_enabled: bool,
    /// Base URL of the model server the librarian reads.
    pub librarian_base_url: String,
    /// Model name the librarian server answers to.
    pub librarian_model: String,
}

/// Body of `PUT /api/v1/settings`.
/// A null field keeps the stored value.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SettingsUpdateDto {
    /// Whether the queue stores messages.
    pub queue_enabled: Option<bool>,
    /// The engine that reads the intent of a message.
    pub resolver_backend: Option<String>,
    /// Base URL of the intent resolver.
    pub resolver_base_url: Option<String>,
    /// Model name the intent resolver asks the server for.
    pub resolver_model: Option<String>,
    /// Identifier of the built in GLiNER model.
    pub gliner_model: Option<String>,
    /// The device a GLiNER model runs on.
    pub gliner_device: Option<String>,
    /// Smallest probability a GLiNER label needs to count.
    pub gliner_threshold: Option<f32>,
    /// Whether the deterministic pass of the router reads the message first.
    pub router_fast_path: Option<bool>,
    /// The evidence the retrieval pass ranks the catalog with.
    pub router_retrieve: Option<String>,
    /// How the decision pass chooses one of the short list.
    pub router_decide: Option<String>,
    /// How the values of the entities of the chosen intent are read.
    pub router_extract: Option<String>,
    /// Size of the short list the retrieval pass keeps.
    pub router_top_k: Option<usize>,
    /// Smallest score the decision pass accepts.
    pub router_floor: Option<f32>,
    /// Smallest distance between the best and the second best.
    pub router_margin: Option<f32>,
    /// Weight of the words of the catalog.
    pub router_lexical_weight: Option<f32>,
    /// Weight of the embeddings of the catalog.
    pub router_dense_weight: Option<f32>,
    /// Model name the embedding server answers to.
    pub router_embed_model: Option<String>,
    /// Directory that holds the downloaded models of the router.
    pub router_models_dir: Option<String>,
    /// Where the retrieval stage and the list matcher read the vectors.
    pub router_embed_source: Option<String>,
    /// Identifier of the built in embedding model.
    pub router_embed_local_model: Option<String>,
    /// Identifier of the built in reranker.
    pub router_rerank_model: Option<String>,
    /// Identifier of the built in decision model.
    pub router_laya_model: Option<String>,
    /// The device a built in model of the router runs on.
    pub router_local_device: Option<String>,
    /// Whether a phrase counts only when the message shares its action.
    pub router_phrase_gate: Option<bool>,
    /// How a mention is read against the values of an entity.
    pub router_list_match: Option<String>,
    /// Smallest cosine similarity an embedding match of a value needs.
    pub router_list_floor: Option<f32>,
    /// Whether the language model answers a message no intent matched.
    pub router_fallback_llm: Option<bool>,
    /// Whether the language model may write a shell script for a message
    /// no intent matched. The daemon runs no script without approval.
    pub router_script_fallback: Option<bool>,
    /// Whether the language model reads an open value the built in reader
    /// found none of.
    pub router_open_values_llm: Option<bool>,
    /// Response quality between 0 and 100. 0 is fastest, 100 is best.
    pub response_quality: Option<u8>,
    /// Response speed between 0 and 100. Always 100 - quality.
    pub response_speed: Option<u8>,
    /// The sentences the settings page tries against the resolver.
    ///
    /// The list is written whole, because the page adds and removes one
    /// sentence at a time and always knows the list it wants to keep.
    pub preview_sentences: Option<Vec<String>>,
    /// Whether the librarian keeps a long term memory.
    pub librarian_enabled: Option<bool>,
    /// Base URL of the model server the librarian reads.
    pub librarian_base_url: Option<String>,
    /// Model name the librarian server answers to.
    pub librarian_model: Option<String>,
}

impl SettingsUpdateDto {
    /// Whether the write changes a stage of the layered router.
    ///
    /// The stages are stored as one set, so a write that touches one of
    /// them reads the stored set first and writes the whole set back.
    pub fn changes_the_router(&self) -> bool {
        self.router_fast_path.is_some()
            || self.router_retrieve.is_some()
            || self.router_decide.is_some()
            || self.router_extract.is_some()
            || self.router_top_k.is_some()
            || self.router_floor.is_some()
            || self.router_margin.is_some()
            || self.router_lexical_weight.is_some()
            || self.router_dense_weight.is_some()
            || self.router_embed_model.is_some()
            || self.router_models_dir.is_some()
            || self.router_embed_source.is_some()
            || self.router_embed_local_model.is_some()
            || self.router_rerank_model.is_some()
            || self.router_laya_model.is_some()
            || self.router_local_device.is_some()
            || self.router_phrase_gate.is_some()
            || self.router_list_match.is_some()
            || self.router_list_floor.is_some()
            || self.router_fallback_llm.is_some()
            || self.router_script_fallback.is_some()
            || self.router_open_values_llm.is_some()
    }

    /// Whether the write changes the librarian.
    pub fn changes_the_librarian(&self) -> bool {
        self.librarian_enabled.is_some()
            || self.librarian_base_url.is_some()
            || self.librarian_model.is_some()
    }
}

/// The state of the librarian as the settings page sees it.
///
/// The librarian is a store beside the conversation store, so the page
/// reports what the store holds and whether the model that writes it
/// answers. A model that is down leaves the seeding of a turn alone: the
/// store still answers, it just stops learning.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LibrarianStatusDto {
    /// Whether the daemon keeps a memory at all.
    pub enabled: bool,
    /// The address of the model server the librarian reads.
    pub base_url: String,
    /// The model the librarian reads.
    pub model: String,
    /// Whether the model server answers.
    pub reachable: bool,
    /// Why the server does not answer, or null.
    pub detail: Option<String>,
    /// Number of stored memory nodes.
    pub nodes: u64,
    /// Number of stored memory edges, closed edges included.
    pub edges: u64,
    /// Number of stored edges that are true now.
    pub current_edges: u64,
    /// Number of stored edges a later turn taught again.
    pub confirmed_edges: u64,
    /// Number of stored episodes the worker has not read yet.
    pub pending_episodes: u64,
    /// Number of episodes the worker read.
    pub ingested_episodes: u64,
}

/// One dated relation of one memory node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryFactDto {
    /// Stable identifier of the fact.
    pub id: Uuid,
    /// The relation the fact names, for example `lives_in`.
    pub relation: String,
    /// The value the fact carries.
    pub value: String,
    /// Whether the fact is true now.
    pub current: bool,
    /// How sure the reader was of the fact, from zero to one.
    pub confidence: f32,
    /// How much the fact is worth, from zero to one.
    pub importance: f32,
    /// Number of later turns that taught the same fact again.
    pub confirmations: i32,
    /// Time the memory last saw the fact, in ISO 8601.
    pub last_confirmed_at: DateTime<Utc>,
    /// Time the fact became true, in ISO 8601.
    pub valid_at: DateTime<Utc>,
    /// Time the fact stopped being true, or null while it is current.
    pub invalid_at: Option<DateTime<Utc>>,
}

/// One memory node with its facts.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryNodeDto {
    /// Stable identifier of the node.
    pub id: Uuid,
    /// The key that names the node, for example `user`.
    pub key: String,
    /// The name the memory shows.
    pub title: String,
    /// The body of the concept in plain text.
    pub body: String,
    /// The relations of the node, current first.
    pub facts: Vec<MemoryFactDto>,
    /// Time the daemon stored the node, in ISO 8601.
    pub created_at: DateTime<Utc>,
    /// Time the daemon last changed the node, in ISO 8601.
    pub updated_at: DateTime<Utc>,
}

/// Body of the memory query.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryQueryRequestDto {
    /// The words to search the memory for. Empty returns the recent nodes.
    pub text: String,
    /// Largest number of nodes to return.
    pub limit: Option<u64>,
}

/// Reply of a memory query.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryQueryDto {
    /// The nodes the query matched, best first.
    pub nodes: Vec<MemoryNodeDto>,
}

/// One relation a write gives to a node.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryFactWriteDto {
    /// The relation the fact names.
    pub relation: String,
    /// The value the fact carries.
    pub value: String,
}

/// Body of a memory write.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryWriteRequestDto {
    /// The key that names the node. An existing key is enriched.
    pub key: String,
    /// The name the memory shows.
    pub title: String,
    /// The body of the concept.
    pub body: String,
    /// The relations to write. A relation with a new value closes the
    /// older value rather than deleting it.
    pub facts: Vec<MemoryFactWriteDto>,
}

/// The report of the memory linter.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LibrarianLintDto {
    /// Nodes two keys name, so the memory holds one thing twice. Every
    /// node of such a pair is reported, because either may take the other.
    pub duplicates: Vec<MemoryNodeDto>,
    /// Nodes that no relation reaches, so nothing links to them.
    pub orphans: Vec<MemoryNodeDto>,
    /// Relations a newer relation replaced. They stay readable.
    pub closed: Vec<MemoryFactDto>,
}

/// Body of a memory merge.
///
/// The concept the request names keeps its key and its facts, and the
/// concept in the path moves into it: its facts move as they stand, its
/// key becomes a name of the target, and the empty concept goes.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct MemoryMergeRequestDto {
    /// Identifier of the concept that absorbs the other.
    pub into: Uuid,
}

/// Number of intent labels a vanilla GLiNER model reads comfortably.
pub const GLINER_SOFT_LABEL_LIMIT: usize = 20;

/// Number of intent labels a vanilla GLiNER model reads before it degrades.
pub const GLINER_HARD_LABEL_LIMIT: usize = 30;

/// Largest number of sentences the settings page may store.
///
/// The list is a scratch pad of the user and not a corpus, so it stops
/// well before a run of every sentence costs more than a coffee break.
pub const PREVIEW_SENTENCE_LIMIT: usize = 50;

/// Largest length of one stored sentence.
pub const PREVIEW_SENTENCE_MAX: usize = 200;

/// Number of intents the llama.cpp resolver can offer.
///
/// The prompt names one letter per option, so the catalog stops at Z.
pub const LLAMA_INTENT_LIMIT: usize = 26;

/// Build the warning for a label count, or `None` when the count is fine.
///
/// A vanilla GLiNER model concatenates the message and every label into
/// one sequence, so a large label set eats the context window and mixes
/// the positions of the labels up.
///
/// Returns the warning and whether the count is past the hard limit.
pub fn gliner_label_warning(labels: usize) -> Option<(String, bool)> {
    if labels > GLINER_HARD_LABEL_LIMIT {
        return Some((
            format!(
                "{labels} labels are more than the {GLINER_HARD_LABEL_LIMIT} a GLiNER model reads well. \
                 The labels share one context window with the message, so accuracy drops. \
                 Remove intents or entities, or leave the intent to llama.cpp and let GLiNER read the values."
            ),
            true,
        ));
    }
    if labels > GLINER_SOFT_LABEL_LIMIT {
        return Some((
            format!(
                "{labels} labels are more than the {GLINER_SOFT_LABEL_LIMIT} a GLiNER model reads comfortably. \
                 Consider fewer intents, or leave the intent to llama.cpp and let GLiNER read the values."
            ),
            false,
        ));
    }
    None
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[test]
    fn a_small_label_set_warns_about_nothing() {
        assert!(gliner_label_warning(0).is_none());
        assert!(gliner_label_warning(GLINER_SOFT_LABEL_LIMIT).is_none());
    }

    #[test]
    fn a_label_set_past_the_soft_limit_warns() {
        let (message, hard) = gliner_label_warning(GLINER_SOFT_LABEL_LIMIT + 1)
            .expect("the count is past the soft limit");
        assert!(!hard);
        assert!(message.contains("comfortably"));
    }

    #[test]
    fn a_label_set_past_the_hard_limit_warns_loudly() {
        let (message, hard) = gliner_label_warning(GLINER_HARD_LABEL_LIMIT + 1)
            .expect("the count is past the hard limit");
        assert!(hard);
        assert!(message.contains("accuracy drops"));
    }
}

/// One dependency a setting needs.
///
/// A setting belongs to a place that stores it and to a service that
/// makes it work. The settings page keeps a control usable only when
/// every place it needs answers, so the user never saves a value that
/// cannot take effect.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DependencyDto {
    /// Short key of the dependency, for example `database`.
    pub key: String,
    /// Name the settings page shows.
    pub label: String,
    /// Whether the dependency answers.
    pub reachable: bool,
    /// Why it does not answer, or what it reports about itself.
    pub detail: Option<String>,
}

/// The llama.cpp server as the settings page sees it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LlamaDependencyDto {
    /// Whether the server answers.
    pub reachable: bool,
    /// Why it does not answer, or null.
    pub detail: Option<String>,
    /// The address the daemon asked.
    pub base_url: String,
    /// The model the daemon stored.
    pub model: String,
    /// The models the server offers. Empty when the server is down.
    pub models: Vec<String>,
}

/// The state of the dependencies of the settings surface.
///
/// The reply needs no database, so the settings page can tell a database
/// that is down from a daemon that is down.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DependenciesDto {
    /// The values the daemon starts from.
    /// The settings page shows them while the database does not answer.
    pub configured: SettingsDto,
    /// The database that stores every setting.
    pub database: DependencyDto,
    /// The llama.cpp server that reads intents and values.
    pub llama: LlamaDependencyDto,
    /// The built in GLiNER resolver.
    pub gliner: DependencyDto,
}

/// Reply of `GET /api/v1/resolver`.
/// This tells the settings page which engines are ready to work.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ResolverStatusDto {
    /// The engine that reads the intent of a message.
    pub backend: String,
    /// Configuration of the llama.cpp resolver.
    pub llama: LlamaStatusDto,
    /// State of the built in GLiNER models.
    pub gliner: GlinerStoreDto,
    /// How many labels the intent configuration adds up to.
    pub budget: LabelBudgetDto,
    /// The state of the layered router.
    pub router: RouterStatusDto,
}

/// The state of the layered router.
///
/// The router has a stage per setting, and each stage names the reader it
/// would use right now. The settings page shows that state, so a stage
/// that cannot run its reader says so before a turn depends on it.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct RouterStatusDto {
    /// Whether the deterministic pass reads the message first.
    pub fast_path: bool,
    /// The evidence the retrieval pass ranks the catalog with.
    pub retrieve: String,
    /// How the decision pass chooses one of the short list.
    pub decide: String,
    /// How the values of the entities of the chosen intent are read.
    pub extract: String,
    /// Size of the short list the retrieval pass keeps.
    pub top_k: usize,
    /// Smallest score the decision pass accepts.
    pub floor: f32,
    /// Smallest distance between the best and the second best.
    pub margin: f32,
    /// Weight of the words of the catalog.
    pub lexical_weight: f32,
    /// Weight of the embeddings of the catalog.
    pub dense_weight: f32,
    /// Model name the embedding server answers to.
    pub embed_model: String,
    /// Where the retrieval stage and the list matcher read the vectors.
    pub embed_source: String,
    /// Identifier of the built in embedding model.
    pub embed_local_model: String,
    /// Identifier of the built in reranker.
    pub rerank_model: String,
    /// Identifier of the built in decision model.
    pub laya_model: String,
    /// The device a built in model of the router runs on.
    pub local_device: String,
    /// The device a built in model of the router would run on now.
    pub active_local_device: String,
    /// State of the built in models of the router.
    pub local: LocalStoreDto,
    /// Whether a phrase counts only when the message shares its action.
    pub phrase_gate: bool,
    /// How a mention is read against the values of an entity.
    pub list_match: String,
    /// Smallest cosine similarity an embedding match of a value needs.
    pub list_floor: f32,
    /// Whether the model server answers the router with embeddings.
    pub embeddings_reachable: bool,
    /// Why the model server answers with no embeddings, or null.
    pub embeddings_detail: Option<String>,
}

/// Configuration of the llama.cpp resolver.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LlamaStatusDto {
    /// Base URL of the llama.cpp server.
    pub base_url: String,
    /// Model name the server answers to.
    pub model: String,
    /// Number of intents the resolver can offer.
    pub intent_limit: usize,
}

/// State of the built in GLiNER models.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct GlinerStoreDto {
    /// Identifier of the selected model.
    pub model: String,
    /// The device the selected model should run on.
    pub device: String,
    /// The device the selected model would run on now.
    pub active_device: String,
    /// Whether the selected model is installed.
    pub installed: bool,
    /// Smallest probability a label needs to count.
    pub threshold: f32,
    /// Directory that holds the downloaded models.
    pub models_dir: String,
    /// Devices this build and this machine can run on, best first.
    pub devices: Vec<String>,
    /// Whether this build carries CUDA support.
    pub cuda_build: bool,
    /// Every model the daemon can download.
    pub models: Vec<GlinerModelDto>,
    /// The download that runs right now, or null.
    pub download: Option<GlinerDownloadDto>,
}

/// One GLiNER model the daemon can download.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct GlinerModelDto {
    /// Identifier of the model, for example `gliner_small-v2.1`.
    pub id: String,
    /// Name the settings page shows.
    pub name: String,
    /// One sentence about the model.
    pub note: String,
    /// Size of the download in bytes.
    pub size_bytes: u64,
    /// Whether the model is on disk.
    pub installed: bool,
}

/// The GLiNER download that runs right now.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct GlinerDownloadDto {
    /// Identifier of the model.
    pub model: String,
    /// Bytes written so far.
    pub received_bytes: u64,
    /// Size the server announced, or null when it announced none.
    pub total_bytes: Option<u64>,
    /// Whether the download finished.
    pub done: bool,
    /// The error that stopped the download, or null.
    pub error: Option<String>,
}

/// One built in model of the router the daemon can download.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LocalModelDto {
    /// Identifier of the model, for example `bge-small-en-v1.5`.
    pub id: String,
    /// Name the settings page shows.
    pub name: String,
    /// One sentence about the model.
    pub note: String,
    /// What the model reads: `embeddings`, `reranker`, or `decision`.
    pub role: String,
    /// Size of the download in bytes.
    pub size_bytes: u64,
    /// Whether the model is on disk.
    pub installed: bool,
}

/// The download of one built in model of the router that runs right now.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LocalDownloadDto {
    /// Identifier of the model.
    pub model: String,
    /// Bytes written so far.
    pub received_bytes: u64,
    /// Size the server announced, or null when it announced none.
    pub total_bytes: Option<u64>,
    /// Whether the download finished.
    pub done: bool,
    /// The error that stopped the download, or null.
    pub error: Option<String>,
}

/// State of the built in models of the router.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LocalStoreDto {
    /// Directory that holds the downloaded models.
    pub models_dir: String,
    /// Every model the daemon can download.
    pub models: Vec<LocalModelDto>,
    /// The download that runs right now, or null.
    pub download: Option<LocalDownloadDto>,
}

/// How many labels the intent configuration adds up to.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct LabelBudgetDto {
    /// Number of labels a GLiNER model reads for this configuration.
    /// One label per intent, plus one per entity, plus one per value of
    /// a closed entity.
    pub labels: usize,
    /// Number of labels a GLiNER model reads comfortably.
    pub soft_limit: usize,
    /// Number of labels a GLiNER model reads before it degrades.
    pub hard_limit: usize,
    /// Number of configured intents.
    pub intents: usize,
    /// Number of intents the resolver can offer, or null when it has no
    /// limit. The layered router reads a catalog of any size, because
    /// only the short list of one turn reaches a model.
    pub intent_limit: Option<usize>,
    /// Number of labels that are past the soft limit, or zero.
    pub over_soft: usize,
    /// Number of labels that are past the hard limit, or zero.
    pub over_hard: usize,
    /// The warning the settings page shows, or null.
    pub warning: Option<String>,
}

/// The kind of one entity of an intent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(rename_all = "lowercase")]
pub enum EntityKindDto {
    /// The entity takes a value the user says freely.
    Open,
    /// The entity takes one value of a fixed list.
    Closed,
    /// The entity takes one value of a list a script provides.
    ///
    /// The daemon runs the script, keeps the values in memory, and reads
    /// the value of a message out of that list.
    Script,
}

impl EntityKindDto {
    /// The stored form of the kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Script => "script",
        }
    }

    /// Parse the stored form of the kind. An unknown value reads as open.
    pub fn from_stored(value: &str) -> Self {
        match value {
            "closed" => Self::Closed,
            "script" => Self::Script,
            _ => Self::Open,
        }
    }

    /// Whether the entity carries a list of values the daemon reads from.
    pub fn has_values(&self) -> bool {
        matches!(self, Self::Closed | Self::Script)
    }
}

/// One entity of an intent.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct IntentEntityDto {
    /// Stable entity identifier.
    pub id: Uuid,
    /// Name of the entity, for example `city`.
    pub name: String,
    /// Whether the entity is open or closed.
    pub kind: EntityKindDto,
    /// Values of a closed entity. An open entity has none.
    pub values: Vec<String>,
    /// Shell command that provides the values of a script entity, or none.
    #[serde(default)]
    pub script: Option<String>,
    /// Whether the intent needs a value for this entity.
    ///
    /// A required entity the daemon cannot read makes the turn ask the
    /// user for the value. An optional entity the daemon cannot read lets
    /// the command run without it.
    #[serde(default = "required_by_default")]
    pub required: bool,
}

/// The default of the `required` flag of an entity.
///
/// An entity of a database an older daemon wrote carries no flag, and an
/// entity the daemon must read a value for is the safer reading of it.
pub fn required_by_default() -> bool {
    true
}

/// One intent of the daemon. An intent runs one shell command.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct IntentDto {
    /// Stable intent identifier.
    pub id: Uuid,
    /// Unique name the resolver chooses from, for example `get weather`.
    pub name: String,
    /// What the intent does, in one sentence. The resolver reads it.
    pub description: String,
    /// Shell command the daemon runs for this intent.
    pub command: String,
    /// The entities the intent reads from the message.
    pub entities: Vec<IntentEntityDto>,
    /// Phrases a user may say for this intent, one per entry.
    ///
    /// The resolver reads them next to the name, so "launch firefox"
    /// points at `open application` as well as the name does.
    #[serde(default)]
    pub examples: Vec<String>,
    /// Time the daemon created the intent, in ISO 8601.
    pub created_at: DateTime<Utc>,
    /// Time the daemon last stored the intent, in ISO 8601.
    pub updated_at: DateTime<Utc>,
}

/// Reply of `GET /api/v1/intents`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct IntentListDto {
    /// The intents, in name order.
    pub items: Vec<IntentDto>,
}

/// One entity of the body of `POST /api/v1/intents` and `PUT /api/v1/intents/{id}`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct IntentEntityWriteDto {
    /// Name of the entity.
    pub name: String,
    /// Whether the entity is open or closed.
    pub kind: EntityKindDto,
    /// Values of a closed entity. An open entity sends none.
    #[serde(default)]
    pub values: Vec<String>,
    /// Shell command that provides the values of a script entity.
    #[serde(default)]
    pub script: Option<String>,
    /// Whether the intent needs a value for this entity. It defaults to yes.
    #[serde(default = "required_by_default")]
    pub required: bool,
}

/// Body of `POST /api/v1/intents` and `PUT /api/v1/intents/{id}`.
/// The daemon replaces the entity set of the intent with the sent one.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct IntentWriteDto {
    /// Unique name of the intent.
    pub name: String,
    /// What the intent does, in one sentence.
    pub description: String,
    /// Shell command the daemon runs for this intent.
    pub command: String,
    /// The entities the intent reads from the message.
    #[serde(default)]
    pub entities: Vec<IntentEntityWriteDto>,
    /// Phrases a user may say for this intent, one per entry.
    #[serde(default)]
    pub examples: Vec<String>,
}

/// Body of `POST /api/v1/intents/script/preview`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ScriptPreviewWriteDto {
    /// The shell command the daemon runs for the preview.
    pub script: String,
}

/// Reply of `POST /api/v1/intents/script/preview`.
///
/// The settings page shows the values a script answers with, so a user
/// who writes one reads the list before it reaches a turn.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ScriptPreviewDto {
    /// The values the script wrote, in the order it wrote them.
    pub values: Vec<String>,
    /// The exit code of the script, or none when it did not start.
    pub exit_code: Option<i32>,
    /// How long the script ran, in milliseconds.
    pub duration_ms: u64,
    /// Why the script did not answer, or none when it did.
    pub error: Option<String>,
}

/// Operating system of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemOsDto {
    /// Name of the operating system, for example `Ubuntu`.
    pub name: Option<String>,
    /// Version of the operating system.
    pub version: Option<String>,
    /// Processor architecture, for example `x86_64`.
    pub arch: String,
    /// Kernel version.
    pub kernel_version: Option<String>,
    /// Host name.
    pub hostname: Option<String>,
}

/// One logical processor of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemCpuCoreDto {
    /// Name of the core, for example `cpu0`.
    pub name: String,
    /// Brand of the core, for example `Intel(R) Core(TM) i7`.
    pub brand: String,
    /// Vendor of the core.
    pub vendor: String,
    /// Frequency of the core in MHz.
    pub frequency_mhz: u64,
    /// Usage of the core in percent between 0 and 100.
    pub usage_percent: f32,
}

/// Processor summary of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemCpuDto {
    /// Brand of the processor.
    pub brand: String,
    /// Vendor identifier.
    pub vendor: String,
    /// Number of physical cores, or null when the host does not report it.
    pub physical_cores: Option<usize>,
    /// Number of logical cores.
    pub logical_cores: usize,
    /// Frequency of the processor in MHz.
    pub frequency_mhz: u64,
    /// Average usage of the processor in percent between 0 and 100.
    pub usage_percent: f32,
    /// Every logical core.
    pub cores: Vec<SystemCpuCoreDto>,
}

/// Memory summary of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemMemoryDto {
    /// Total RAM in bytes.
    pub total_bytes: u64,
    /// Available RAM in bytes.
    pub available_bytes: u64,
    /// Used RAM in bytes.
    pub used_bytes: u64,
    /// Total swap in bytes.
    pub total_swap_bytes: u64,
    /// Used swap in bytes.
    pub used_swap_bytes: u64,
}

/// One disk of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemDiskDto {
    /// Name of the disk, for example `/dev/sda1`.
    pub name: String,
    /// Mount point of the disk.
    pub mount_point: String,
    /// File system of the disk, for example `ext4`.
    pub file_system: String,
    /// Total space of the disk in bytes.
    pub total_bytes: u64,
    /// Available space of the disk in bytes.
    pub available_bytes: u64,
    /// Whether the disk is removable.
    pub is_removable: bool,
}

/// Devices the daemon can use for built in models.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemDevicesDto {
    /// Whether this build carries CUDA support.
    pub cuda_build: bool,
    /// Whether ONNX Runtime can use a CUDA device right now.
    pub cuda_available: bool,
    /// Devices this build and this machine offer, best first.
    pub available_devices: Vec<String>,
    /// The device the selected GLiNER model would run on now.
    pub active_device: String,
    /// The device the router models would run on now.
    pub active_local_device: String,
    /// Number of GLiNER models on disk.
    pub gliner_models_installed: usize,
    /// Number of router models on disk.
    pub router_models_installed: usize,
    /// The graphics devices the daemon can reach, best first.
    ///
    /// A graphics device is what makes a built in model fast, so the page
    /// names every device with the memory it holds. A machine without a
    /// graphics device reports an empty list.
    pub gpus: Vec<SystemGpuDto>,
}

/// One graphics device of the host.
///
/// The settings page reads how much memory a device holds, because the
/// memory decides which built in model fits on it. A machine that does not
/// report a value reports null rather than a value of zero.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemGpuDto {
    /// The name of the device, as the machine reports it.
    pub name: String,
    /// The maker of the device: `nvidia`, `amd`, or `intel`.
    pub vendor: String,
    /// True when the device has no memory of its own and draws on the
    /// memory of the system.
    ///
    /// An integrated device shares the memory of the machine, so the page
    /// reads the memory of the host for it rather than a number of zero.
    pub shared_memory: bool,
    /// The memory of the device in bytes, or null when the device holds
    /// none of its own or the machine does not report it.
    pub memory_total_bytes: Option<u64>,
    /// The memory in use in bytes, or null.
    pub memory_used_bytes: Option<u64>,
    /// The memory free in bytes, or null.
    pub memory_free_bytes: Option<u64>,
    /// The version of the driver, or null when the machine does not name
    /// one.
    pub driver_version: Option<String>,
}

/// Resolver state of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemResolverDto {
    /// Whether the llama.cpp server answers.
    pub llama_reachable: bool,
    /// Why the llama server does not answer, or null.
    pub llama_detail: Option<String>,
    /// Whether the embedding server answers.
    pub embeddings_reachable: bool,
    /// Why the embedding server does not answer, or null.
    pub embeddings_detail: Option<String>,
    /// Whether the selected GLiNER model is on disk.
    pub gliner_installed: bool,
    /// Whether the selected embedding model is on disk.
    pub local_embedding_installed: bool,
    /// Whether the selected reranker is on disk.
    pub local_reranker_installed: bool,
    /// Number of configured intents.
    pub intent_count: usize,
    /// Number of labels a GLiNER model reads for this configuration.
    pub label_count: usize,
}

/// One preset that maps a quality value to a router configuration.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct RouterPresetDto {
    /// Quality between 0 and 100. 0 is fastest, 100 is best.
    pub quality: u8,
    /// Speed between 0 and 100. Always 100 - quality.
    pub speed: u8,
    /// Short label of the preset, for example `Balanced`.
    pub label: String,
    /// One sentence about the preset.
    pub description: String,
    /// Router configuration of the preset.
    pub router: RouterStatusDto,
}

/// Recommendation for this host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemRecommendationDto {
    /// Recommended quality between 0 and 100.
    pub quality: u8,
    /// Recommended speed between 0 and 100. Always 100 - quality.
    pub speed: u8,
    /// Why this recommendation fits the host.
    pub reason: String,
    /// Factors that shaped the recommendation.
    pub factors: Vec<String>,
    /// Router configuration of the recommended preset.
    pub router: RouterStatusDto,
}

/// Full system profile of the host.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemProfileDto {
    /// Operating system.
    pub os: SystemOsDto,
    /// Processor.
    pub cpu: SystemCpuDto,
    /// Memory.
    pub memory: SystemMemoryDto,
    /// Disks.
    pub disks: Vec<SystemDiskDto>,
    /// Devices the daemon can use.
    pub devices: SystemDevicesDto,
    /// Resolver state.
    pub resolver: SystemResolverDto,
    /// Recommended preset for this host.
    pub recommendation: SystemRecommendationDto,
    /// Every preset from fastest to best, with the recommended one marked.
    pub presets: Vec<RouterPresetDto>,
}

/// Body of `POST /api/v1/system/preset`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemPresetRequestDto {
    /// Quality between 0 and 100. 0 is fastest, 100 is best.
    pub quality: u8,
}

/// Reply of `POST /api/v1/system/preset`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct SystemPresetDto {
    /// Quality between 0 and 100.
    pub quality: u8,
    /// Speed between 0 and 100. Always 100 - quality.
    pub speed: u8,
    /// Router configuration of the preset.
    pub router: RouterStatusDto,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_without_a_context_field_reads_the_earlier_turns() {
        let request: ChatRequestDto = serde_json::from_str(r#"{"text":"open firefox"}"#)
            .expect("a body with a text and a conversation reads");

        assert_eq!(request.text, "open firefox");
        assert_eq!(request.context, None);
    }

    #[test]
    fn a_message_carries_the_context_switch_of_the_user() {
        let request: ChatRequestDto = serde_json::from_str(r#"{"text":"weather","context":false}"#)
            .expect("a body with a context switch reads");

        assert_eq!(request.context, Some(false));
    }

    #[test]
    fn a_turn_reads_no_time_before_one_is_measured() {
        assert_eq!(MessageMetaDto::default().resolve_ms, None);
    }
}
