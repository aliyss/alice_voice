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
      type: 'IntentStage';
      /**
       * The stage: `fast_path`, `retrieve`, `decide`, `extract`, `answer`,
       * or `script`.
       */
      stage: string;
      /** One sentence about what the stage is doing. */
      detail: string;
    }
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
  | { type: 'MessageReplied'; id: string }
  | {
      type: 'ScriptProposed';
      id: string;
      summary: string;
      script: string;
      destructiveness: number;
    }
  | { type: 'ScriptApproved'; id: string }
  | { type: 'ScriptDenied'; id: string }
  | {
      type: 'MemorySaved';
      message_id: string;
      facts: MemorySavedFactDto[];
    };

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
  /**
   * What the turn taught the long term memory, or null when the memory
   * learned nothing from it. The memory reads a turn after the daemon
   * answered it, so the field is filled when a reader reads the message
   * back rather than when the turn is stored.
   */
  memory: MessageMemoryDto | null;
}

/** What one turn taught the long term memory. */
export interface MessageMemoryDto {
  /** The facts the turn taught, in the order the memory wrote them. */
  facts: MemorySavedFactDto[];
}

/** One fact that one turn taught the long term memory. */
export interface MemorySavedFactDto {
  /** The name of the concept the fact belongs to. */
  concept: string;
  /** The relation the fact names. */
  relation: string;
  /** The value the fact carries. */
  value: string;
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
  | 'lists'
  | 'choice';

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
  /**
   * The identifier of the script the model wrote for this turn, or null
   * when the model answered in words. The store keeps the script, so a
   * user can still decide about a turn that a restart left behind.
   */
  scriptId: string | null;
  /**
   * How long the daemon needed to read the turn, in milliseconds, or null
   * when it did not measure the read. The read runs from the queued
   * message to the command, so it is told apart from `durationMs`.
   */
  resolveMs: number | null;
  /**
   * The concepts the daemon read from the long term memory for the turn,
   * as plain text, or null when the memory carried none. The memory
   * belongs to the branch that no intent matches, so only a turn without
   * an intent reports what the daemon read of it.
   */
  memorySeed: string | null;
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

/**
 * The device a built in model of the router runs on.
 *
 * The router may read a built in model on an OpenVINO device, so `gpu`
 * belongs to this list even though the GLiNER model does not offer it.
 */
export type LocalDevice = 'auto' | 'cpu' | 'cuda' | 'gpu';

/** Where the router reads the vectors of the catalog. */
export type EmbedSource = 'server' | 'local';

/** The evidence the retrieval stage of the router ranks the catalog with. */
export type RetrieveEngine = 'lexical' | 'dense' | 'hybrid';

/** How the decision stage of the router chooses one of the short list. */
export type DecideEngine = 'score' | 'rerank' | 'generative' | 'laya';

/** How the extraction stage of the router reads the entity values. */
export type ExtractEngine = 'lists' | 'spans' | 'laya' | 'generative';

/** How a mention is read against the values of an entity. */
export type ListMatch = 'lexical' | 'dense' | 'both';

/** One shell script that waits for the decision of the user. */
export interface ScriptDto {
  /** The stable identifier of the script. */
  id: string;
  /** The conversation of the turn, or null when the queue is off. */
  conversationId: string | null;
  /** The message the model wrote the script for. */
  requestText: string;
  /** One sentence about what the script does. */
  summary: string;
  /** The shell script. */
  script: string;
  /** How rough the script is on the machine, between 0 and 100. */
  destructiveness: number;
  /** The decision of the user: pending, approved, denied, or ran. */
  status: string;
  /** The time the daemon stored the script, in ISO 8601 format. */
  createdAt: string;
}

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
  /** The identifier of the built in decision model. */
  routerLayaModel: string;
  /** The device a built in model of the router runs on. */
  routerLocalDevice: LocalDevice;
  /** Whether a phrase counts only when the message shares its action. */
  routerPhraseGate: boolean;
  /** How a mention is read against the values of an entity. */
  routerListMatch: ListMatch;
  /** The smallest cosine similarity an embedding match of a value needs. */
  routerListFloor: number;
  /** Whether the language model answers a message no intent matched. */
  routerFallbackLlm: boolean;
  /**
   * Whether the language model may write a shell script for a message no
   * intent matched. The daemon runs no script without the approval of the
   * user, whatever this value holds.
   */
  routerScriptFallback: boolean;
  /**
   * Whether the language model reads an open value the built in reader
   * found none of. Off reads no such value and asks the user for it.
   */
  routerOpenValuesLlm: boolean;
  /** Response quality between 0 and 100. 0 is fastest, 100 is best. */
  responseQuality: number;
  /** Response speed between 0 and 100. Always 100 - quality. */
  responseSpeed: number;
  /**
   * The sentences the settings page tries against the resolver. They are
   * the tests of a pipeline, so the daemon keeps them beside the settings
   * even though a turn never reads them.
   */
  previewSentences: string[];
  /** Whether the librarian keeps a long term memory. */
  librarianEnabled: boolean;
  /** The base URL of the model server the librarian reads. */
  librarianBaseUrl: string;
  /** The model the librarian reads. */
  librarianModel: string;
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
  /** The identifier of the built in decision model. */
  routerLayaModel?: string;
  /** The device a built in model of the router runs on. */
  routerLocalDevice?: LocalDevice;
  /** Whether a phrase counts only when the message shares its action. */
  routerPhraseGate?: boolean;
  /** How a mention is read against the values of an entity. */
  routerListMatch?: ListMatch;
  /** The smallest cosine similarity an embedding match of a value needs. */
  routerListFloor?: number;
  /** Whether the language model answers a message no intent matched. */
  routerFallbackLlm?: boolean;
  /** Whether the language model may write a shell script for a message. */
  routerScriptFallback?: boolean;
  /** Whether the language model reads an open value no reader else read. */
  routerOpenValuesLlm?: boolean;
  /** Response quality between 0 and 100. */
  responseQuality?: number;
  /** Response speed between 0 and 100. */
  responseSpeed?: number;
  /** The sentences the settings page tries against the resolver. */
  previewSentences?: string[];
  /** Whether the librarian keeps a long term memory. */
  librarianEnabled?: boolean;
  /** The base URL of the model server the librarian reads. */
  librarianBaseUrl?: string;
  /** The model the librarian reads. */
  librarianModel?: string;
}

/** The state of the librarian as the settings page sees it. */
export interface LibrarianStatusDto {
  /** Whether the daemon keeps a memory at all. */
  enabled: boolean;
  /** The address of the model server the librarian reads. */
  baseUrl: string;
  /** The model the librarian reads. */
  model: string;
  /** Whether the model server answers. */
  reachable: boolean;
  /** Why the server does not answer, or null. */
  detail: string | null;
  /** Number of stored memory nodes. */
  nodes: number;
  /** Number of stored memory edges, closed edges included. */
  edges: number;
  /** Number of stored edges that are true now. */
  currentEdges: number;
  /** Number of stored edges a later turn taught again. */
  confirmedEdges: number;
  /** Number of episodes the worker has not read yet. */
  pendingEpisodes: number;
  /** Number of episodes the worker read. */
  ingestedEpisodes: number;
}

/** One dated relation of one memory node. */
export interface MemoryFactDto {
  /** The stable identifier of the fact. */
  id: string;
  /** The relation the fact names, for example `lives_in`. */
  relation: string;
  /** The value the fact carries. */
  value: string;
  /** Whether the fact is true now. */
  current: boolean;
  /** How sure the reader was of the fact, from zero to one. */
  confidence: number;
  /** How much the fact is worth, from zero to one. */
  importance: number;
  /** Number of later turns that taught the same fact again. */
  confirmations: number;
  /** The time the memory last saw the fact, in ISO 8601 format. */
  lastConfirmedAt: string;
  /** The time the fact became true, in ISO 8601 format. */
  validAt: string;
  /** The time the fact stopped being true, or null while it is current. */
  invalidAt: string | null;
}

/** One memory node with its facts. */
export interface MemoryNodeDto {
  /** The stable identifier of the node. */
  id: string;
  /** The key that names the node, for example `user`. */
  key: string;
  /** The name the memory shows. */
  title: string;
  /** The body of the concept in plain text. */
  body: string;
  /** The relations of the node, current first. */
  facts: MemoryFactDto[];
  /** The time the daemon stored the node, in ISO 8601 format. */
  createdAt: string;
  /** The time the daemon last changed the node, in ISO 8601 format. */
  updatedAt: string;
}

/** The body of a memory query. */
export interface MemoryQueryRequestDto {
  /** The words to search the memory for. Empty returns the recent nodes. */
  text: string;
  /** The largest number of nodes to return. */
  limit?: number;
}

/** The reply of a memory query. */
export interface MemoryQueryDto {
  /** The nodes the query matched, best first. */
  nodes: MemoryNodeDto[];
}

/** One relation a write gives to a node. */
export interface MemoryFactWriteDto {
  /** The relation the fact names. */
  relation: string;
  /** The value the fact carries. */
  value: string;
}

/** The body of a memory write. */
export interface MemoryWriteRequestDto {
  /** The key that names the node. An existing key is enriched. */
  key: string;
  /** The name the memory shows. */
  title: string;
  /** The body of the concept. */
  body: string;
  /** The relations to write. */
  facts: MemoryFactWriteDto[];
}

/** The body of a memory merge. */
export interface MemoryMergeRequestDto {
  /** The concept that absorbs the other. */
  into: string;
}

/** The report of the memory linter. */
export interface LibrarianLintDto {
  /** Nodes two keys name, so the memory holds one thing twice. */
  duplicates: MemoryNodeDto[];
  /** Nodes that no relation reaches, so nothing links to them. */
  orphans: MemoryNodeDto[];
  /** Relations a newer relation replaced. They stay readable. */
  closed: MemoryFactDto[];
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
  /** What the model reads: `embeddings`, `reranker`, or `decision`. */
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
  /** The identifier of the built in decision model. */
  layaModel: string;
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

/** Operating system of the host. */
export interface SystemOsDto {
  /** Name of the operating system, for example `Ubuntu`. */
  name: string | null;
  /** Version of the operating system. */
  version: string | null;
  /** Processor architecture, for example `x86_64`. */
  arch: string;
  /** Kernel version. */
  kernelVersion: string | null;
  /** Host name. */
  hostname: string | null;
}

/** One logical processor of the host. */
export interface SystemCpuCoreDto {
  /** Name of the core, for example `cpu0`. */
  name: string;
  /** Brand of the core, for example `Intel(R) Core(TM) i7`. */
  brand: string;
  /** Vendor of the core. */
  vendor: string;
  /** Frequency of the core in MHz. */
  frequencyMhz: number;
  /** Usage of the core in percent between 0 and 100. */
  usagePercent: number;
}

/** Processor summary of the host. */
export interface SystemCpuDto {
  /** Brand of the processor. */
  brand: string;
  /** Vendor identifier. */
  vendor: string;
  /** Number of physical cores, or null when the host does not report it. */
  physicalCores: number | null;
  /** Number of logical cores. */
  logicalCores: number;
  /** Frequency of the processor in MHz. */
  frequencyMhz: number;
  /** Average usage of the processor in percent between 0 and 100. */
  usagePercent: number;
  /** Every logical core. */
  cores: SystemCpuCoreDto[];
}

/** Memory summary of the host. */
export interface SystemMemoryDto {
  /** Total RAM in bytes. */
  totalBytes: number;
  /** Available RAM in bytes. */
  availableBytes: number;
  /** Used RAM in bytes. */
  usedBytes: number;
  /** Total swap in bytes. */
  totalSwapBytes: number;
  /** Used swap in bytes. */
  usedSwapBytes: number;
}

/** One disk of the host. */
export interface SystemDiskDto {
  /** Name of the disk, for example `/dev/sda1`. */
  name: string;
  /** Mount point of the disk. */
  mountPoint: string;
  /** File system of the disk, for example `ext4`. */
  fileSystem: string;
  /** Total space of the disk in bytes. */
  totalBytes: number;
  /** Available space of the disk in bytes. */
  availableBytes: number;
  /** Whether the disk is removable. */
  isRemovable: boolean;
}

/** Devices the daemon can use for built in models. */
export interface SystemDevicesDto {
  /** Whether this build carries CUDA support. */
  cudaBuild: boolean;
  /** Whether ONNX Runtime can use a CUDA device right now. */
  cudaAvailable: boolean;
  /** Devices this build and this machine offer, best first. */
  availableDevices: string[];
  /** The device the selected GLiNER model would run on now. */
  activeDevice: string;
  /** The device the router models would run on now. */
  activeLocalDevice: string;
  /** Number of GLiNER models on disk. */
  glinerModelsInstalled: number;
  /** Number of router models on disk. */
  routerModelsInstalled: number;
  /**
   * The graphics devices the daemon can reach, best first. The memory of a
   * device decides which built in model fits on it.
   */
  gpus: SystemGpuDto[];
}

/** One graphics device of the host. */
export interface SystemGpuDto {
  /** The name of the device, as the machine reports it. */
  name: string;
  /** The maker of the device: `nvidia`, `amd`, or `intel`. */
  vendor: string;
  /**
   * True when the device has no memory of its own and draws on the memory
   * of the system, which is what an integrated device does.
   */
  sharedMemory: boolean;
  /** The memory of the device in bytes, or null when it is not reported. */
  memoryTotalBytes: number | null;
  /** The memory in use in bytes, or null. */
  memoryUsedBytes: number | null;
  /** The memory free in bytes, or null. */
  memoryFreeBytes: number | null;
  /** The version of the driver, or null when the machine does not name one. */
  driverVersion: string | null;
}

/** Resolver state of the host. */
export interface SystemResolverDto {
  /** Whether the llama.cpp server answers. */
  llamaReachable: boolean;
  /** Why the llama server does not answer, or null. */
  llamaDetail: string | null;
  /** Whether the embedding server answers. */
  embeddingsReachable: boolean;
  /** Why the embedding server does not answer, or null. */
  embeddingsDetail: string | null;
  /** Whether the selected GLiNER model is on disk. */
  glinerInstalled: boolean;
  /** Whether the selected embedding model is on disk. */
  localEmbeddingInstalled: boolean;
  /** Whether the selected reranker is on disk. */
  localRerankerInstalled: boolean;
  /** Number of configured intents. */
  intentCount: number;
  /** Number of labels a GLiNER model reads for this configuration. */
  labelCount: number;
}

/** One preset that maps a quality value to a router configuration. */
export interface RouterPresetDto {
  /** Quality between 0 and 100. 0 is fastest, 100 is best. */
  quality: number;
  /** Speed between 0 and 100. Always 100 - quality. */
  speed: number;
  /** Short label of the preset, for example `Balanced`. */
  label: string;
  /** One sentence about the preset. */
  description: string;
  /** Router configuration of the preset. */
  router: RouterStatusDto;
}

/** Recommendation for this host. */
export interface SystemRecommendationDto {
  /** Recommended quality between 0 and 100. */
  quality: number;
  /** Recommended speed between 0 and 100. Always 100 - quality. */
  speed: number;
  /** Why this recommendation fits the host. */
  reason: string;
  /** Factors that shaped the recommendation. */
  factors: string[];
  /** Router configuration of the recommended preset. */
  router: RouterStatusDto;
}

/** Full system profile of the host. */
export interface SystemProfileDto {
  /** Operating system. */
  os: SystemOsDto;
  /** Processor. */
  cpu: SystemCpuDto;
  /** Memory. */
  memory: SystemMemoryDto;
  /** Disks. */
  disks: SystemDiskDto[];
  /** Devices the daemon can use. */
  devices: SystemDevicesDto;
  /** Resolver state. */
  resolver: SystemResolverDto;
  /** Recommended preset for this host. */
  recommendation: SystemRecommendationDto;
  /** Every preset from fastest to best, with the recommended one marked. */
  presets: RouterPresetDto[];
}

/** Body of `POST /api/v1/system/preset`. */
export interface SystemPresetRequestDto {
  /** Quality between 0 and 100. 0 is fastest, 100 is best. */
  quality: number;
}

/** Reply of `POST /api/v1/system/preset`. */
export interface SystemPresetDto {
  /** Quality between 0 and 100. */
  quality: number;
  /** Speed between 0 and 100. Always 100 - quality. */
  speed: number;
  /** Router configuration of the preset. */
  router: RouterStatusDto;
}
