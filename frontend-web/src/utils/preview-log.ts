/**
 * The log of a sentence the user tried against the resolver.
 *
 * A preview answers with the route a turn would take, so the panel beside
 * the flow can read it in two ways. The reading names what the daemon
 * would do in the words of a user; the technical log names every value the
 * daemon reported, field by field, with one sentence about what the field
 * means. A reader who tunes a pipeline needs both: the reading says whether
 * a sentence works, and the log says which stage and which reader made it
 * work, or failed to.
 *
 * Every value a field carries stands on its own line, because a reader
 * compares two runs by the same field rather than by the same sentence.
 */
import type { TextTone } from '~/components/ui/text';

import type {
  MessageCandidateDto,
  MessageEntityDto,
  MessageMetaDto,
  MessageRouteStepDto,
  ResolverPreviewDto,
} from '~/types/dto';

import type { ChatRouteStep } from '~/utils/chat';

import {
  formatCandidate,
  formatConfidence,
  formatDuration,
  formatEngine,
  formatEntity,
  formatRouteStep,
  formatStage,
} from '~/utils/chat';

/** One value of the technical log of a preview. */
export interface PreviewField {
  /** The name of the value, in the user language. */
  label: string;
  /** The value the daemon reported. */
  value: string;
  /** One sentence about what the value means, or null. */
  hint: string | null;
  /**
   * The color of the value, or null for the color of text. A value takes a
   * tone only when the tone says something: an answer that matched, a
   * stage that refused, a value that was not read.
   */
  tone?: TextTone;
}

/** One part of the technical log of a preview. */
export interface PreviewBlock {
  /** The name of the part, for example `The decision`. */
  title: string;
  /** One sentence about the part, or null. */
  hint: string | null;
  /** The values of the part, in the order the daemon read them. */
  fields: PreviewField[];
  /**
   * The color of the title, or null for the color of small text. A stage
   * takes the tone of the way it ended, so a column of stages reads at a
   * glance.
   */
  tone?: TextTone;
}

/** The plain reading of one preview, in the words of a user. */
export interface PreviewReading {
  /** What the answer is, in two words: `Would run` or `No intent`. */
  status: string;
  /** True when the turn met an intent. */
  matched: boolean;
  /**
   * What the daemon would do as one sentence: the intent it would run, or
   * the reply it would store for a turn that met none.
   */
  answer: string;
  /** The name of the intent the turn met, or null. */
  intent: string | null;
  /** The probability of the choice as a percent, or null. */
  confidence: string | null;
  /** The reply the daemon would store, or null when it would run instead. */
  reply: string | null;
  /** One readable line per entity the intent needs. */
  entities: string[];
  /** The stages the turn passed, in the order they ran. */
  route: ChatRouteStep[];
  /** Why the turn refused, or null when it chose an intent. */
  reason: string | null;
  /** The command the daemon would run, or null. */
  command: string | null;
}

/** What the settings page calls a turn that left the resolver without one. */
const NO_INTENT = 'I could not match that to an intent.';

/** What the daemon calls the stage of a turn that met no intent. */
const NO_STAGE = 'none';

/** The color of one stage of the route, by the way it ended. */
const OUTCOME_TONES: Record<MessageRouteStepDto['outcome'], TextTone> = {
  matched: 'ok',
  passed: 'default',
  fell_back: 'warn',
  refused: 'error',
  skipped: 'faint',
};

/** One sentence about the answer of a turn. */
const ANSWER_HINT =
  'The daemon read the sentence the way it reads a turn, and ran nothing: no message is stored and no command runs.';

/** One sentence about the confidence of a turn. */
const CONFIDENCE_HINT =
  'How sure the stage that decided was. Below the smallest score of the decision stage the turn refuses instead of running a command.';

/** One sentence about the stage that decided a turn. */
const STAGE_HINT =
  'The stage of the pipeline that answered the sentence. Every stage before it read the message and passed it on.';

/** One sentence about the reason of a refusal. */
const REASON_HINT =
  'Why the turn met no intent. A message the catalog does not hold is refused rather than run as the closest intent.';

/** One sentence about one stage of the route. */
const ROUTE_HINT =
  'A stage reads the message and either answers the turn, refuses it, or passes it to the stage below.';

/** One sentence about the reader of a stage. */
const READER_HINT =
  'The reader that really ran. It can differ from the reader the settings pick, because a stage that cannot reach its model falls back to a cheaper reader instead of failing.';

/** One sentence about the outcome of a stage. */
const OUTCOME_HINT =
  'How the stage ended: it answered the turn, refused the message, or passed the turn to the stage below.';

/** One sentence about the time one stage took. */
const DURATION_HINT =
  'How long the stage took. A stage that reads words is quickest, one that reads a model is slowest.';

/** One sentence about the candidates of a stage. */
const CANDIDATES_HINT =
  'The intents the ranking offered, best first, with the score it gave each of them and the evidence that scored it. The decision stage reads this list and chooses one of it.';

/** One sentence about the detail of a stage. */
const DETAIL_HINT = 'What the stage read and how it read it, in its own words.';

/** One sentence about one value of the turn. */
const VALUES_HINT =
  'The values the daemon read for the entities of the intent. A preview reads them without running the command that needs them.';

/** One sentence about the reader of one value. */
const VALUE_READER_HINT =
  'Who read the value: the deterministic pass proves a value the message spells out, a list matches a value the intent owns, the vectors match a value by its meaning, and spans or the model guess one.';

/** One sentence about what a reader read before a match. */
const VALUE_READ_HINT =
  'What the reader read before the daemon matched it to an entry of a list, for example a name it knew by another spelling.';

/** One sentence about the similarity of a value. */
const VALUE_SCORE_HINT =
  'How close the mention was to the entry it matched, between 0 and 1. A match below the smallest similarity of the stage leaves the entity without a value.';

/** One sentence about the command of a turn. */
const COMMAND_HINT =
  'The command the turn would run with the values in place. A preview never runs it, so a user reads the command before a turn depends on it.';

/** One sentence about the time a turn took to read. */
const TURN_DURATION_HINT =
  'How long the daemon took to read the message. Every stage of the route is a part of it.';

/** Read what the daemon would do as one sentence. */
function readAnswer(preview: ResolverPreviewDto): string {
  if (preview.matched) {
    return preview.intent ?? 'an intent of the catalog';
  }
  return preview.reply.length > 0 ? preview.reply : NO_INTENT;
}

/** Read one candidate as one value of the log, at its place in the list. */
function candidateFields(candidates: MessageCandidateDto[]): PreviewField[] {
  return candidates.map((candidate, index) => ({
    label: `#${index + 1} of the short list`,
    value: formatCandidate(candidate),
    hint: index === 0 ? CANDIDATES_HINT : null,
  }));
}

/**
 * Read one stage of the route as a part of the log.
 *
 * The candidates of the stage stand under it, so the list the decision
 * read is read beside the stage that read it rather than in a part of its
 * own that a reader has to match back to a stage.
 */
function stepBlock(
  step: MessageRouteStepDto,
  stage: string | null,
): PreviewBlock {
  // The stage takes the color of the way it ended, in its title and in the
  // one value that says how it ended, so a reader finds the stage that
  // refused or fell back without reading a line of the log.
  const tone = OUTCOME_TONES[step.outcome];
  const fields: PreviewField[] = [
    {
      label: 'The reader that ran',
      value: step.model ? `${step.reader}  ${step.model}` : step.reader,
      hint: READER_HINT,
    },
    {
      label: 'How it ended',
      value: formatRouteStep(step).outcome,
      hint: OUTCOME_HINT,
      tone,
    },
    {
      label: 'The time it took',
      value: formatDuration(step.durationMs),
      hint: DURATION_HINT,
    },
  ];
  if (step.detail) {
    fields.push({
      label: 'What it read',
      value: step.detail,
      hint: DETAIL_HINT,
    });
  }
  fields.push(...candidateFields(step.candidates));

  return {
    title: stage ?? 'the stage',
    hint: ROUTE_HINT,
    fields,
    tone,
  };
}

/** Read one value of the turn as a part of the log. */
function entityBlock(entity: MessageEntityDto): PreviewBlock {
  const read = entity.value.length > 0;
  const fields: PreviewField[] = [
    {
      label: 'The value',
      value: read ? entity.value : '(unread)',
      hint: VALUES_HINT,
      // A value the daemon read nothing for is the thing a user has to
      // fix, so it takes the color of a warning rather than the quiet
      // color of a value that is simply absent.
      tone: read ? undefined : 'warn',
    },
  ];
  if (entity.source) {
    fields.push({
      label: 'The reader',
      value: entity.source,
      hint: VALUE_READER_HINT,
    });
  }
  const engine = formatEngine(entity.engine);
  if (engine) {
    fields.push({
      label: 'The engine',
      value: entity.model ? `${engine}  ${entity.model}` : engine,
      hint: VALUE_READER_HINT,
    });
  }
  if (entity.read && entity.read !== entity.value) {
    fields.push({
      label: 'What it read first',
      value: entity.read,
      hint: VALUE_READ_HINT,
    });
  }
  if (entity.score !== null) {
    fields.push({
      label: 'How close the match was',
      value: entity.score.toFixed(3),
      hint: VALUE_SCORE_HINT,
    });
  }

  return { title: entity.name, hint: VALUES_HINT, fields };
}

/** Read one preview as a plain reading, ready for the panel. */
export function readPreview(preview: ResolverPreviewDto): PreviewReading {
  const meta: MessageMetaDto | null = preview.meta;
  return {
    status: preview.matched ? 'Would run' : 'No intent',
    answer: readAnswer(preview),
    matched: preview.matched,
    intent: preview.intent,
    confidence: formatConfidence(preview.confidence),
    reply: preview.reply.length > 0 && preview.matched ? preview.reply : null,
    entities: (meta?.entities ?? []).map(formatEntity),
    route: (meta?.route?.steps ?? []).map(formatRouteStep),
    reason: meta?.route?.reason ?? null,
    command: meta?.command ?? null,
  };
}

/**
 * Read one preview as a technical log, part by part.
 *
 * The parts follow the way a turn runs: the answer of the whole turn, then
 * every stage of the route with the readers it ran, then the values it
 * read, then the command it would run. A part the turn never reached is
 * left out, so a refusal carries no command and a turn that read no value
 * carries no values.
 */
export function debugPreview(preview: ResolverPreviewDto): PreviewBlock[] {
  const meta: MessageMetaDto | null = preview.meta;
  const blocks: PreviewBlock[] = [];

  // A turn that met an intent is an answer, a turn that refused and said
  // why is a warning, and a turn that refused in silence is a failure.
  const refusal: TextTone = preview.reply.length > 0 ? 'warn' : 'error';
  const answerTone: TextTone = preview.matched ? 'ok' : refusal;
  const answer: PreviewField[] = [
    {
      label: 'The answer',
      value: preview.matched ? 'an intent matched' : 'no intent matched',
      hint: ANSWER_HINT,
      tone: answerTone,
    },
  ];
  if (preview.intent) {
    answer.push({
      label: 'The intent',
      value: preview.intent,
      hint: ANSWER_HINT,
    });
  }
  const confidence = formatConfidence(preview.confidence);
  if (confidence) {
    answer.push({
      label: 'How sure the resolver was',
      value: confidence,
      hint: CONFIDENCE_HINT,
    });
  }
  const stage =
    meta?.stage === null || meta?.stage === undefined
      ? null
      : formatStage(meta.stage);
  if (stage && meta?.stage !== NO_STAGE) {
    answer.push({
      label: 'The stage that decided',
      value: stage,
      hint: STAGE_HINT,
    });
  }
  const engines: string[] = [];
  const intentEngine = formatEngine(meta?.intentEngine ?? null);
  if (intentEngine) {
    engines.push(
      meta?.intentModel
        ? `intent: ${intentEngine}  ${meta.intentModel}`
        : `intent: ${intentEngine}`,
    );
  }
  const valueEngine = formatEngine(meta?.valueEngine ?? null);
  if (valueEngine) {
    engines.push(
      meta?.valueModel
        ? `values: ${valueEngine}  ${meta.valueModel}`
        : `values: ${valueEngine}`,
    );
  }
  for (const [index, engine] of engines.entries()) {
    answer.push({
      label: 'The engine that read',
      value: engine,
      hint: index === 0 ? ANSWER_HINT : null,
    });
  }
  if (meta?.route?.reason) {
    answer.push({
      label: 'Why it refused',
      value: meta.route.reason,
      hint: REASON_HINT,
      tone: refusal,
    });
  }
  if (meta?.durationMs !== null && meta?.durationMs !== undefined) {
    answer.push({
      label: 'The time the read took',
      value: formatDuration(meta.durationMs),
      hint: TURN_DURATION_HINT,
    });
  }
  blocks.push({
    title: 'The answer',
    hint: ANSWER_HINT,
    fields: answer,
    tone: answerTone,
  });

  for (const step of meta?.route?.steps ?? []) {
    blocks.push(stepBlock(step, formatRouteStep(step).stage));
  }

  if ((meta?.entities ?? []).length > 0) {
    for (const entity of meta?.entities ?? []) {
      blocks.push(entityBlock(entity));
    }
  }

  if (meta?.command) {
    blocks.push({
      title: 'The run',
      hint: COMMAND_HINT,
      fields: [
        { label: 'The command', value: meta.command, hint: COMMAND_HINT },
      ],
    });
  }

  return blocks;
}
