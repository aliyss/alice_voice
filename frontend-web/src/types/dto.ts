/**
 * Data transfer objects of the Alice Voice backend contract.
 *
 * The backend owns these types. This file copies them from
 * `backend/config/openapi.yaml`. Do not rename a field without a contract
 * change and a version bump.
 */

/** The state machine of the daemon. It mirrors `backend/crates/alice-core`. */
export type DaemonStateDto =
  | 'Idle'
  | 'Listening'
  | 'Transcribing'
  | 'Resolving'
  | 'Executing'
  | 'Error';

/** Why the daemon left the Listening state. */
export type StopReasonDto =
  | 'SilenceTimeout'
  | 'UserStopped'
  | 'WakeDetected'
  | 'Error';

/** The reply of `GET /api/v1/status`. */
export interface StatusDto {
  /** The current state machine state. */
  state: DaemonStateDto;
  /** The time the daemon entered the state, in ISO 8601 format. */
  since: string;
  /** The daemon version string. */
  version: string;
}

/** One event of the socket stream. It wraps one backend `SystemEvent`. */
export interface SystemEventDto {
  /** The time the daemon emitted the event, in ISO 8601 format. */
  at: string;
  /** The event payload, tagged by the name of the backend variant. */
  payload: SystemEventPayloadDto;
}

/** The stream a command wrote one line to. */
export type OutputStreamDto = 'stdout' | 'stderr';

/** The payload of a `SystemEventDto`. */
export type SystemEventPayloadDto =
  | { type: 'WakeDetected'; timestamp: string }
  | { type: 'ListeningStarted' }
  | { type: 'ListeningStopped'; reason: StopReasonDto }
  | { type: 'TranscriptInterim'; text: string }
  | { type: 'TranscriptFinal'; text: string }
  | { type: 'TranscriptCorrected'; original: string; corrected: string }
  | { type: 'IntentThinking'; delta: string }
  | {
      type: 'IntentResolved';
      intent: string;
      confidence: number | null;
      /** The engine that chose the intent. */
      engine: ResolverEngineDto;
      /** The model that engine ran, or null when the daemon cannot name it. */
      model: string | null;
    }
  | {
      type: 'IntentValuesRead';
      /** The engine that read the values. */
      engine: ResolverEngineDto;
      /** The model that engine ran, or null when the daemon cannot name it. */
      model: string | null;
      /** The entities of the intent with the values the engine read. */
      entities: MessageEntityDto[];
    }
  | { type: 'IntentNotFound'; text: string }
  | { type: 'ExecutionStarted'; intent: string; command: string }
  | { type: 'ExecutionOutput'; stream: OutputStreamDto; line: string }
  | {
      type: 'ExecutionCompleted';
      intent: string;
      exitCode: number;
      durationMs: number;
      output: string;
    }
  | { type: 'ExecutionFailed'; intent: string; error: string }
  | { type: 'MessageQueued'; id: string }
  | { type: 'MessageReplied'; id: string };

/** The speaker of one chat message. */
export type ChatRoleDto = 'user' | 'assistant';

/** One stored turn of a conversation. */
export interface ChatMessageDto {
  /** The stable message identifier. */
  id: string;
  /** The conversation of the message, or null when the queue is off. */
  conversationId: string | null;
  /** The speaker. */
  role: ChatRoleDto;
  /** The message text. */
  text: string;
  /** The time the daemon stored the message, in ISO 8601 format. */
  createdAt: string;
  /** The identifier of the resolved intent, or null when none was found. */
  intentId: string | null;
  /** The name of the resolved intent, or null when none was found. */
  intentName: string | null;
  /** The probability of the chosen option, or null when the model sent none. */
  confidence: number | null;
  /** How the daemon read the turn, or null when it read nothing. */
  meta: MessageMetaDto | null;
}

/** One engine that read a part of a handled turn. */
export type ResolverEngineDto = 'llama' | 'gliner' | 'router';

/** The stage of the layered router that answered a turn. */
export type RouterStageDto = 'fast_path' | 'retrieve' | 'rerank' | 'none';

/** The stage of the router that read one stage of a turn. */
export type RouteStageDto = 'fast_path' | 'retrieve' | 'decide' | 'extract';

/** How one stage of the router ended. */
export type RouteOutcomeDto =
  | 'matched'
  | 'refused'
  | 'passed'
  | 'fell_back'
  | 'skipped';

/** The reader that read the value of one entity. */
export type EntitySourceDto =
  | 'fast_path'
  | 'list'
  | 'embedding'
  | 'spans'
  | 'model'
  | 'lists';

/** One entity of an intent with the value the resolver read for it. */
export interface MessageEntityDto {
  /** The name of the entity, for example `city`. */
  name: string;
  /** The value the resolver read, empty when it read none. */
  value: string;
  /** The reader that read the value, or null when the daemon read none. */
  source: EntitySourceDto | null;
  /** The engine that ran the reader, or null when the daemon read it. */
  engine: ResolverEngineDto | null;
  /** The model the reader ran, or null when it ran none. */
  model: string | null;
  /**
   * What the reader read before the value was matched to an entry of a
   * list, or null when the value is the entry itself.
   */
  read: string | null;
  /**
   * The similarity an embedding match read, between 0 and 1, or null when
   * the reader reported none.
   */
  score: number | null;
}

/** One stage of the layered router in one turn. */
export interface MessageRouteStepDto {
  /** The stage: `fast_path`, `retrieve`, `decide`, or `extract`. */
  stage: RouteStageDto;
  /** How the stage ended. */
  outcome: RouteOutcomeDto;
  /** The reader the stage ran, for example `rules` or `reranker`. */
  reader: string;
  /** The model the reader ran, or null when it ran none. */
  model: string | null;
  /** One sentence about what the stage read, or null. */
  detail: string | null;
  /** The candidates the stage read, best first. */
  candidates: MessageCandidateDto[];
  /** How long the stage took, in milliseconds. */
  durationMs: number;
}

/** How the layered router read one turn, stage by stage. */
export interface MessageRouteDto {
  /** The stage that decided the turn, or null when the router refused. */
  stage: RouterStageDto | null;
  /** True when the turn met an intent. */
  matched: boolean;
  /** Why the router refused, or null when it chose. */
  reason: string | null;
  /** The stages the turn passed, in the order they ran. */
  steps: MessageRouteStepDto[];
}

/** One intent the retrieval stage offered for a turn. */
export interface MessageCandidateDto {
  /** The name of the intent. */
  name: string;
  /** The score of the intent for this turn, between 0 and 1. */
  score: number;
  /** Which evidence read the score, for example `words` or `embedding`. */
  evidence: string;
}

/** How the daemon read and ran one handled turn. */
export interface MessageMetaDto {
  /** The engine that chose the intent, or null when none chose one. */
  intentEngine: ResolverEngineDto | null;
  /** The model that engine ran, or null when the daemon cannot name it. */
  intentModel: string | null;
  /** The engine that read the entity values, or null when none did. */
  valueEngine: ResolverEngineDto | null;
  /** The model that engine ran, or null when the daemon cannot name it. */
  valueModel: string | null;
  /**
   * The stage of the layered router that decided the turn, or null when the
   * backend of the turn is not the router.
   */
  stage: RouterStageDto | null;
  /** The intents the retrieval stage kept for this turn, best first. */
  candidates: MessageCandidateDto[];
  /** The entities of the intent with the values the daemon read. */
  entities: MessageEntityDto[];
  /**
   * How the layered router read the turn stage by stage, or null when the
   * backend of the turn is not the router.
   */
  route: MessageRouteDto | null;
  /** The command after the daemon put the values in, or null. */
  command: string | null;
  /** The exit code of the command, or null when none ran. */
  exitCode: number | null;
  /** How long the command ran, in milliseconds, or null when none ran. */
  durationMs: number | null;
}

/** Body of `POST /api/v1/resolver/preview`. */
export interface ResolverPreviewRequestDto {
  /** The message to read. */
  text: string;
}

/** Reply of `POST /api/v1/resolver/preview`. */
export interface ResolverPreviewDto {
  /** The message the daemon read. */
  text: string;
  /** True when the resolver met an intent. */
  matched: boolean;
  /** The name of the intent the resolver met, or null when it met none. */
  intent: string | null;
  /** The probability of the choice, or null when the resolver sent none. */
  confidence: number | null;
  /** The reply the daemon would store, empty when it would run a command. */
  reply: string;
  /** How the daemon read the turn, or null when it read nothing at all. */
  meta: MessageMetaDto | null;
}

/** One conversation. The conversation owns its own message history. */
export interface ConversationDto {
  /** The stable conversation identifier. */
  id: string;
  /** The title the daemon generated from the first message. */
  title: string;
  /** The time the daemon created the conversation, in ISO 8601 format. */
  createdAt: string;
  /** The time the daemon stored the last message, in ISO 8601 format. */
  updatedAt: string;
}

/** The reply of `GET /api/v1/conversations`. */
export interface ConversationListDto {
  /** The conversations, most recently updated first. */
  items: ConversationDto[];
}

/** The reply of `GET /api/v1/conversations/{id}`. */
export interface ConversationDetailDto {
  /** The conversation. */
  conversation: ConversationDto;
  /** The full message history, oldest first. */
  messages: ChatMessageDto[];
}

/** The body of `POST /api/v1/chat`. */
export interface ChatRequestDto {
  /** The text the user typed. */
  text: string;
  /** The conversation to append to. Null starts a new conversation. */
  conversationId: string | null;
}

/** The reply of `POST /api/v1/chat`. */
export interface ChatReplyDto {
  /** True when the daemon stored the turn, false when the queue is off. */
  stored: boolean;
  /** The conversation of the turn, or null when the queue is off. */
  conversation: ConversationDto | null;
  /** The stored user message. */
  user: ChatMessageDto;
  /** The daemon reply. */
  reply: ChatMessageDto;
}

/**
 * How an entity takes its value.
 *
 * `open` reads the words of the message, `closed` takes one value of a
 * fixed list, and `script` takes one value of a list a shell command
 * answers with, which the daemon keeps in memory.
 */
export type EntityKindDto = 'open' | 'closed' | 'script';

/** One entity of an intent. */
export interface IntentEntityDto {
  /** The stable entity identifier. */
  id: string;
  /** The name of the entity, for example `city`. */
  name: string;
  /** How the entity takes its value. */
  kind: EntityKindDto;
  /** The values of a closed entity. An open entity has none. */
  values: string[];
  /** The shell command a script entity runs, or null. */
  script: string | null;
  /**
   * Whether the intent needs a value for this entity.
   *
   * The daemon asks the user for a required value it could not read, and
   * runs the command of an optional one without the value.
   */
  required: boolean;
}

/** One intent. An intent runs one shell command. */
export interface IntentDto {
  /** The stable intent identifier. */
  id: string;
  /** The unique name the resolver chooses from, for example `get weather`. */
  name: string;
  /** What the intent does, in one sentence. The resolver reads it. */
  description: string;
  /** The shell command the daemon runs for this intent. */
  command: string;
  /** The entities the intent reads from the message. */
  entities: IntentEntityDto[];
  /** The phrases a user may say for this intent, one per entry. */
  examples: string[];
  /** The time the daemon created the intent, in ISO 8601 format. */
  createdAt: string;
  /** The time the daemon last stored the intent, in ISO 8601 format. */
  updatedAt: string;
}

/** The reply of `GET /api/v1/intents`. */
export interface IntentListDto {
  /** The intents, in name order. */
  items: IntentDto[];
}

/** One entity of the body of an intent write. */
export interface IntentEntityWriteDto {
  /** The name of the entity. */
  name: string;
  /** How the entity takes its value. */
  kind: EntityKindDto;
  /** The values of a closed entity. An open entity sends none. */
  values?: string[];
  /** The shell command of a script entity. */
  script?: string | null;
  /** Whether the intent needs a value for this entity. */
  required?: boolean;
}

/** The body of `POST /api/v1/intents` and `PUT /api/v1/intents/{id}`. */
export interface IntentWriteDto {
  /** The unique name of the intent. */
  name: string;
  /** What the intent does, in one sentence. */
  description: string;
  /** The shell command the daemon runs for this intent. */
  command: string;
  /** The entities the intent reads from the message. */
  entities: IntentEntityWriteDto[];
  /** The phrases a user may say for this intent, one per entry. */
  examples?: string[];
}

/** The body of `POST /api/v1/intents/script/preview`. */
export interface ScriptPreviewWriteDto {
  /** The shell command the daemon runs for the preview. */
  script: string;
}

/** The reply of `POST /api/v1/intents/script/preview`. */
export interface ScriptPreviewDto {
  /** The values the script wrote, in the order it wrote them. */
  values: string[];
  /** The exit code of the script, or null when it did not start. */
  exitCode: number | null;
  /** How long the script ran, in milliseconds. */
  durationMs: number;
  /** Why the script did not answer, or null when it did. */
  error: string | null;
}

/**
 * The engine that reads the intent of a message.
 *
 * `router` reads the message in stages: a deterministic pass, a retrieval
 * pass over the whole catalog, a decision over the short list it keeps,
 * and an extraction pass for the values of the intent that won.
 */
export type ResolverBackend = 'llama' | 'gliner' | 'hybrid' | 'router';

/** The device a built in model runs on. */
export type GlinerDevice = 'auto' | 'cpu' | 'cuda';

/** The device a built in model of the router runs on. */
export type LocalDevice = GlinerDevice;

/** Where the router reads the vectors of the catalog. */
export type EmbedSource = 'server' | 'local';

/** The evidence the retrieval stage of the router ranks the catalog with. */
export type RetrieveEngine = 'lexical' | 'dense' | 'hybrid';

/** How the decision stage of the router chooses one of the short list. */
export type DecideEngine = 'score' | 'rerank' | 'generative';

/** How the extraction stage of the router reads the entity values. */
export type ExtractEngine = 'lists' | 'spans' | 'generative';

/** How a mention is read against the values of an entity. */
export type ListMatch = 'lexical' | 'dense' | 'both';

/** The reply of `GET /api/v1/settings` and `PUT /api/v1/settings`. */
export interface SettingsDto {
  /** Whether the queue stores messages. */
  queueEnabled: boolean;
  /** The engine that reads the intent of a message. */
  resolverBackend: ResolverBackend;
  /** The base URL of the intent resolver. */
  resolverBaseUrl: string;
  /** The model name the intent resolver asks the server for. */
  resolverModel: string;
  /** The identifier of the built in GLiNER model. */
  glinerModel: string;
  /** The device the built in GLiNER model runs on. */
  glinerDevice: GlinerDevice;
  /** The smallest probability a GLiNER label needs to count. */
  glinerThreshold: number;
  /** Whether the deterministic pass of the router reads the message first. */
  routerFastPath: boolean;
  /** The evidence the retrieval stage of the router ranks the catalog with. */
  routerRetrieve: RetrieveEngine;
  /** How the decision stage of the router chooses one of the short list. */
  routerDecide: DecideEngine;
  /** How the values of the entities of the chosen intent are read. */
  routerExtract: ExtractEngine;
  /** The size of the short list the retrieval stage keeps. */
  routerTopK: number;
  /** The smallest score the decision stage accepts. */
  routerFloor: number;
  /** The smallest distance between the best and the second best. */
  routerMargin: number;
  /** The weight of the words of the catalog. */
  routerLexicalWeight: number;
  /** The weight of the embeddings of the catalog. */
  routerDenseWeight: number;
  /** The model name the embedding server answers to. */
  routerEmbedModel: string;
  /** The directory that holds the downloaded models of the router. */
  routerModelsDir: string;
  /** Where the router reads the vectors of the catalog. */
  routerEmbedSource: EmbedSource;
  /** The identifier of the built in embedding model. */
  routerEmbedLocalModel: string;
  /** The identifier of the built in reranker. */
  routerRerankModel: string;
  /** The device a built in model of the router runs on. */
  routerLocalDevice: LocalDevice;
  /** Whether a phrase counts only when the message shares its action. */
  routerPhraseGate: boolean;
  /** How a mention is read against the values of an entity. */
  routerListMatch: ListMatch;
  /** The smallest cosine similarity an embedding match of a value needs. */
  routerListFloor: number;
  /**
   * The sentences the settings page tries against the resolver. They are
   * the tests of a pipeline, so the daemon keeps them beside the settings
   * even though a turn never reads them.
   */
  previewSentences: string[];
}

/** The body of `PUT /api/v1/settings`. A null field keeps the stored value. */
export interface SettingsUpdateDto {
  /** Whether the queue stores messages. */
  queueEnabled?: boolean;
  /** The engine that reads the intent of a message. */
  resolverBackend?: ResolverBackend;
  /** The base URL of the intent resolver. */
  resolverBaseUrl?: string;
  /** The model name the intent resolver asks the server for. */
  resolverModel?: string;
  /** The identifier of the built in GLiNER model. */
  glinerModel?: string;
  /** The device the built in GLiNER model runs on. */
  glinerDevice?: GlinerDevice;
  /** The smallest probability a GLiNER label needs to count. */
  glinerThreshold?: number;
  /** Whether the deterministic pass of the router reads the message first. */
  routerFastPath?: boolean;
  /** The evidence the retrieval stage of the router ranks the catalog with. */
  routerRetrieve?: RetrieveEngine;
  /** How the decision stage of the router chooses one of the short list. */
  routerDecide?: DecideEngine;
  /** How the values of the entities of the chosen intent are read. */
  routerExtract?: ExtractEngine;
  /** The size of the short list the retrieval stage keeps. */
  routerTopK?: number;
  /** The smallest score the decision stage accepts. */
  routerFloor?: number;
  /** The smallest distance between the best and the second best. */
  routerMargin?: number;
  /** The weight of the words of the catalog. */
  routerLexicalWeight?: number;
  /** The weight of the embeddings of the catalog. */
  routerDenseWeight?: number;
  /** The model name the embedding server answers to. */
  routerEmbedModel?: string;
  /** The directory that holds the downloaded models of the router. */
  routerModelsDir?: string;
  /** Where the router reads the vectors of the catalog. */
  routerEmbedSource?: EmbedSource;
  /** The identifier of the built in embedding model. */
  routerEmbedLocalModel?: string;
  /** The identifier of the built in reranker. */
  routerRerankModel?: string;
  /** The device a built in model of the router runs on. */
  routerLocalDevice?: LocalDevice;
  /** Whether a phrase counts only when the message shares its action. */
  routerPhraseGate?: boolean;
  /** How a mention is read against the values of an entity. */
  routerListMatch?: ListMatch;
  /** The smallest cosine similarity an embedding match of a value needs. */
  routerListFloor?: number;
  /** The sentences the settings page tries against the resolver. */
  previewSentences?: string[];
}

/** One GLiNER model the daemon can download. */
export interface GlinerModelDto {
  /** The identifier of the model, for example `gliner_small-v2.1`. */
  id: string;
  /** The name the settings page shows. */
  name: string;
  /** One sentence about the model. */
  note: string;
  /** The size of the download in bytes. */
  sizeBytes: number;
  /** Whether the model is on disk. */
  installed: boolean;
}

/** One built in model of the router the daemon can download. */
export interface LocalModelDto {
  /** The identifier of the model, for example `bge-small-en-v1.5`. */
  id: string;
  /** The name the settings page shows. */
  name: string;
  /** One sentence about the model. */
  note: string;
  /** What the model reads: `embeddings` or `reranker`. */
  role: string;
  /** The size of the download in bytes. */
  sizeBytes: number;
  /** Whether the model is on disk. */
  installed: boolean;
}

/** The download of one built in model of the router that runs right now. */
export interface LocalDownloadDto {
  /** The identifier of the model. */
  model: string;
  /** The bytes written so far. */
  receivedBytes: number;
  /** The size the server announced, or null. */
  totalBytes: number | null;
  /** Whether the download finished. */
  done: boolean;
  /** The error that stopped the download, or null. */
  error: string | null;
}

/** The state of the built in models of the router. */
export interface LocalStoreDto {
  /** The directory that holds the downloaded models. */
  modelsDir: string;
  /** Every model the daemon can download. */
  models: LocalModelDto[];
  /** The download that runs right now, or null. */
  download: LocalDownloadDto | null;
}

/** The GLiNER download that runs right now. */
export interface GlinerDownloadDto {
  /** The identifier of the model. */
  model: string;
  /** The bytes written so far. */
  receivedBytes: number;
  /** The size the server announced, or null. */
  totalBytes: number | null;
  /** Whether the download finished. */
  done: boolean;
  /** The error that stopped the download, or null. */
  error: string | null;
}

/** The state of the built in GLiNER models. */
export interface GlinerStoreDto {
  /** The identifier of the selected model. */
  model: string;
  /** The device the selected model should run on. */
  device: GlinerDevice;
  /** The device the selected model would run on now. */
  activeDevice: string;
  /** Whether the selected model is installed. */
  installed: boolean;
  /** The smallest probability a label needs to count. */
  threshold: number;
  /** The directory that holds the downloaded models. */
  modelsDir: string;
  /** The devices this build and this machine can run on, best first. */
  devices: string[];
  /** Whether this build carries CUDA support. */
  cudaBuild: boolean;
  /** Every model the daemon can download. */
  models: GlinerModelDto[];
  /** The download that runs right now, or null. */
  download: GlinerDownloadDto | null;
}

/** The configuration of the llama.cpp resolver. */
export interface LlamaStatusDto {
  /** The base URL of the llama.cpp server. */
  baseUrl: string;
  /** The model name the server answers to. */
  model: string;
  /** The number of intents the resolver can offer. */
  intentLimit: number;
}

/** How many labels the intent configuration adds up to. */
export interface LabelBudgetDto {
  /** The number of labels a GLiNER model reads for this configuration. */
  labels: number;
  /** The number of labels a GLiNER model reads comfortably. */
  softLimit: number;
  /** The number of labels a GLiNER model reads before it degrades. */
  hardLimit: number;
  /** The number of configured intents. */
  intents: number;
  /**
   * The number of intents the resolver can offer, or null when it has no
   * limit. The layered router reads a catalog of any size.
   */
  intentLimit: number | null;
  /** The number of labels past the soft limit, or zero. */
  overSoft: number;
  /** The number of labels past the hard limit, or zero. */
  overHard: number;
  /** The warning the settings page shows, or null. */
  warning: string | null;
}

/**
 * One dependency a setting needs.
 *
 * A setting belongs to a place that stores it and to a service that makes
 * it work. The settings page keeps a control usable only when every place
 * it needs answers.
 */
export interface DependencyDto {
  /** The short key of the dependency, for example `database`. */
  key: string;
  /** The name the settings page shows. */
  label: string;
  /** Whether the dependency answers. */
  reachable: boolean;
  /** Why it does not answer, or what it reports about itself. */
  detail: string | null;
}

/** The llama.cpp server as the settings page sees it. */
export interface LlamaDependencyDto {
  /** Whether the server answers. */
  reachable: boolean;
  /** Why it does not answer, or null. */
  detail: string | null;
  /** The address the daemon asked. */
  baseUrl: string;
  /** The model the daemon stored. */
  model: string;
  /** The models the server offers. Empty when the server is down. */
  models: string[];
}

/**
 * The state of the dependencies of the settings surface.
 *
 * The reply needs no database, so the settings page can tell a database
 * that is down from a daemon that is down.
 */
export interface DependenciesDto {
  /** The values the daemon starts from. */
  configured: SettingsDto;
  /** The database that stores every setting. */
  database: DependencyDto;
  /** The llama.cpp server that reads intents and values. */
  llama: LlamaDependencyDto;
  /** The built in GLiNER resolver. */
  gliner: DependencyDto;
}

/** The state of the layered router. */
export interface RouterStatusDto {
  /** Whether the deterministic pass reads the message first. */
  fastPath: boolean;
  /** The evidence the retrieval stage ranks the catalog with. */
  retrieve: RetrieveEngine;
  /** How the decision stage chooses one of the short list. */
  decide: DecideEngine;
  /** How the values of the entities are read. */
  extract: ExtractEngine;
  /** The size of the short list the retrieval stage keeps. */
  topK: number;
  /** The smallest score the decision stage accepts. */
  floor: number;
  /** The smallest distance between the best and the second best. */
  margin: number;
  /** The weight of the words of the catalog. */
  lexicalWeight: number;
  /** The weight of the embeddings of the catalog. */
  denseWeight: number;
  /** The model name the embedding server answers to. */
  embedModel: string;
  /** Where the router reads the vectors of the catalog. */
  embedSource: EmbedSource;
  /** The identifier of the built in embedding model. */
  embedLocalModel: string;
  /** The identifier of the built in reranker. */
  rerankModel: string;
  /** The device a built in model of the router should run on. */
  localDevice: LocalDevice;
  /** The device a built in model of the router would run on now. */
  activeLocalDevice: string;
  /** The state of the built in models of the router. */
  local: LocalStoreDto;
  /** Whether a phrase counts only when the message shares its action. */
  phraseGate: boolean;
  /** How a mention is read against the values of an entity. */
  listMatch: ListMatch;
  /** The smallest cosine similarity an embedding match of a value needs. */
  listFloor: number;
  /** Whether the place the vectors come from answers right now. */
  embeddingsReachable: boolean;
  /** Why that place answers with no embeddings, or null. */
  embeddingsDetail: string | null;
}

/** The reply of `GET /api/v1/resolver`. */
export interface ResolverStatusDto {
  /** The engine that reads the intent of a message. */
  backend: ResolverBackend;
  /** The configuration of the llama.cpp resolver. */
  llama: LlamaStatusDto;
  /** The state of the built in GLiNER models. */
  gliner: GlinerStoreDto;
  /** How many labels the intent configuration adds up to. */
  budget: LabelBudgetDto;
  /** The state of the layered router. */
  router: RouterStatusDto;
}
