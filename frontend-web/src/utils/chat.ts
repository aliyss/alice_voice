/**
 * The view model of the chat transcript.
 *
 * A DTO carries the raw backend fields. A row carries the fields the
 * transcript renders, so the view never formats a date or a percentage.
 */
import type {
  ChatMessageDto,
  ChatRoleDto,
  EntitySourceDto,
  MessageCandidateDto,
  MessageEntityDto,
  MessageMetaDto,
  MessageRouteStepDto,
  ResolverEngineDto,
  RouterStageDto,
} from '~/types/dto';

/** One stage of the route of a turn, ready to render. */
export interface ChatRouteStep {
  /** The name of the stage in words, for example `ranking of the catalog`. */
  stage: string;
  /** The reader the stage ran, for example `rules`. */
  reader: string;
  /** How the stage ended in words, for example `fell back`. */
  outcome: string;
  /** One sentence about what the stage read, or null. */
  detail: string | null;
  /** How long the stage took, for example `4 ms`. */
  duration: string;
}

/** How the daemon read and ran one turn, ready to render. */
export interface ChatRowMeta {
  /**
   * The engines that read the turn, in words, for example
   * `llama.cpp  ·  GLiNER`. A turn the resolver read and chose no intent
   * for carries the engine of that read, so the surface always names who
   * read the message.
   */
  resolver: string | null;
  /**
   * The model each of those engines ran, one per line, or null when the
   * daemon cannot name them.
   */
  resolverModels: string | null;
  /**
   * The stage of the layered router that decided the turn, in words, or
   * null when the resolver of the turn is not the router.
   */
  stage: string | null;
  /**
   * The intents the retrieval stage offered, best first, one per line.
   * The list is the evidence of the decision, so a refusal still shows
   * what the catalog had on offer.
   */
  candidates: string[];
  /** One readable entry per entity, for example `city = Berlin`. */
  entities: string[];
  /**
   * The stages the layered router passed, in the order they ran, or empty
   * when the resolver of the turn is not the router.
   */
  route: ChatRouteStep[];
  /** Why the router refused the message, or null when it chose or ran. */
  reason: string | null;
  /** The command the daemon ran, or null. */
  command: string | null;
  /** The exit code of the run in words, for example `exit 0`, or null. */
  exitCode: string | null;
  /** True when the command ran and reported a failure. */
  exitFailed: boolean;
  /** How long the command ran, for example `412 ms`, or null. */
  duration: string | null;
}

/** One rendered turn of the chat transcript. */
export interface ChatRow {
  /** The stable message identifier. */
  id: string;
  /** The speaker. */
  role: ChatRoleDto;
  /** The message text. */
  text: string;
  /** The local time of the message as `HH:MM:SS`. */
  clock: string;
  /** The resolved intent identifier, or null. */
  intentId: string | null;
  /** The name of the resolved intent, or null. */
  intentName: string | null;
  /** The resolver confidence as a whole percent, or null. */
  confidence: string | null;
  /** How the daemon read and ran the turn, or null when it read nothing. */
  meta: ChatRowMeta | null;
}

/** The user language name of each resolver engine. */
export const ENGINE_LABELS: Record<ResolverEngineDto, string> = {
  llama: 'llama.cpp',
  gliner: 'GLiNER',
  router: 'Router',
};

/** The user language name of each stage of the layered router. */
export const STAGE_LABELS: Record<RouterStageDto, string> = {
  fast_path: 'deterministic pass',
  retrieve: 'ranking of the catalog',
  rerank: 'model over the short list',
  none: 'nothing matched',
};

/** The user language name of one stage of the route of a turn. */
const STEP_LABELS: Record<MessageRouteStepDto['stage'], string> = {
  fast_path: 'deterministic pass',
  retrieve: 'ranking of the catalog',
  decide: 'decision',
  extract: 'extraction',
};

/** How one stage ended, in words. */
const OUTCOME_LABELS: Record<MessageRouteStepDto['outcome'], string> = {
  matched: 'answered the turn',
  refused: 'refused the message',
  passed: 'passed the turn on',
  fell_back: 'fell back to a cheaper reader',
  skipped: 'did not run',
};

/** The user language name of the reader that read one entity value. */
const SOURCE_LABELS: Record<EntitySourceDto, string> = {
  fast_path: 'proved',
  list: 'list',
  embedding: 'vectors',
  spans: 'spans',
  model: 'model',
  lists: 'lists',
};

/**
 * Read one stage of the route with the reader it ran and the time it took.
 *
 * The detailed sentence of the stage waits with the step, so the row of
 * the route stays one line a reader can scan. The settings panel and the
 * transcript both read a route with this, so the two name a stage the
 * same way.
 */
export function formatRouteStep(step: MessageRouteStepDto): ChatRouteStep {
  return {
    stage: STEP_LABELS[step.stage],
    reader: step.model ? `${step.reader}  ${step.model}` : step.reader,
    outcome: OUTCOME_LABELS[step.outcome],
    detail: step.detail,
    duration: formatDuration(step.durationMs),
  };
}

/** The name of one stage in user language, or null. */
export function formatStage(stage: RouterStageDto | null): string | null {
  return stage ? STAGE_LABELS[stage] : null;
}

/** Read one candidate as `name 72%`, with the evidence in words. */
export function formatCandidate(candidate: MessageCandidateDto): string {
  const score = `${Math.round(candidate.score * 100)}%`;
  return `${candidate.name}  ${score}  ${candidate.evidence}`;
}

/** The name of one engine in user language, or null. */
export function formatEngine(engine: ResolverEngineDto | null): string | null {
  return engine ? ENGINE_LABELS[engine] : null;
}

/**
 * Read one entity as `name = value`, and as `name = (unread)` when empty.
 *
 * A value the daemon read carries the reader that read it, because a value
 * a list proved and a value a model guessed read the same in a row and
 * mean very different things. What the reader read before the value was
 * matched to an entry of a list waits behind it as
 * `name = value  list (read what the reader read)`.
 */
export function formatEntity(entity: MessageEntityDto): string {
  const value = entity.value.length > 0 ? entity.value : '(unread)';
  if (!entity.source) {
    return `${entity.name} = ${value}`;
  }
  const source = SOURCE_LABELS[entity.source];
  const read =
    entity.read && entity.read !== entity.value ? ` (read ${entity.read})` : '';
  return `${entity.name} = ${value}  ${source}${read}`;
}

/** Read how long one command ran. */
export function formatDuration(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

/**
 * Read the engines of one turn with the model each of them ran.
 *
 * One engine can read both steps, so the second line is dropped when it
 * names the same model as the first one.
 */
function formatResolver(meta: MessageMetaDto): {
  resolver: string | null;
  resolverModels: string | null;
} {
  const steps: { engine: ResolverEngineDto | null; model: string | null }[] = [
    { engine: meta.intentEngine, model: meta.intentModel },
    { engine: meta.valueEngine, model: meta.valueModel },
  ];

  const engines: string[] = [];
  const models: string[] = [];
  for (const step of steps) {
    const label = formatEngine(step.engine);
    if (!label) {
      continue;
    }
    engines.push(label);
    if (step.model && !models.includes(`${label}  ${step.model}`)) {
      models.push(`${label}  ${step.model}`);
    }
  }

  return {
    resolver: engines.length > 0 ? engines.join('  ·  ') : null,
    resolverModels: models.length > 0 ? models.join('\n') : null,
  };
}

/** Map the stored metadata of one turn to the rendered metadata. */
export function toRowMeta(meta: MessageMetaDto | null): ChatRowMeta | null {
  if (!meta) {
    return null;
  }
  return {
    ...formatResolver(meta),
    stage: formatStage(meta.stage),
    candidates: meta.candidates.map(formatCandidate),
    entities: meta.entities.map(formatEntity),
    route: (meta.route?.steps ?? []).map(formatRouteStep),
    reason: meta.route?.reason ?? null,
    command: meta.command,
    exitCode: meta.exitCode === null ? null : `exit ${meta.exitCode}`,
    exitFailed: meta.exitCode !== null && meta.exitCode !== 0,
    duration: meta.durationMs === null ? null : formatDuration(meta.durationMs),
  };
}

/** The placeholder of an unreadable time. */
const UNKNOWN_CLOCK = '--:--:--';

/** Pad one number to two digits. */
function pad(value: number): string {
  return String(value).padStart(2, '0');
}

/** Format a timestamp as the local time `HH:MM:SS`. */
export function formatClock(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return UNKNOWN_CLOCK;
  }
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

/** Format a confidence between 0 and 1 as a whole percent. */
export function formatConfidence(value: number | null): string | null {
  if (value === null || Number.isNaN(value)) {
    return null;
  }
  return `${Math.round(value * 100)}%`;
}

/** Map one message DTO to one transcript row. */
export function toChatRow(message: ChatMessageDto): ChatRow {
  return {
    id: message.id,
    role: message.role,
    text: message.text,
    clock: formatClock(message.createdAt),
    intentId: message.intentId,
    intentName: message.intentName,
    confidence: formatConfidence(message.confidence),
    meta: toRowMeta(message.meta),
  };
}

/**
 * Build the row of a message the user just sent.
 *
 * The page shows this row immediately and replaces it with the stored
 * message when the daemon answers.
 */
export function toPendingRow(text: string, now: Date): ChatRow {
  return {
    id: `pending-${now.getTime()}`,
    role: 'user',
    text,
    clock: formatClock(now.toISOString()),
    intentId: null,
    intentName: null,
    confidence: null,
    meta: null,
  };
}

/** Append one message to the transcript, newest last. */
export function appendRow(rows: ChatRow[], message: ChatMessageDto): ChatRow[] {
  if (rows.some((row) => row.id === message.id)) {
    return rows;
  }
  return [...rows, toChatRow(message)];
}
