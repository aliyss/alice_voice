/**
 * The view model of the daemon status.
 *
 * The backend sends a status snapshot over REST and a stream of events
 * over the socket. These pure functions fold the two into the single
 * state that the aura, the status strip, and the live turn render.
 */
import type {
  DaemonStateDto,
  MessageEntityDto,
  ResolverEngineDto,
  StatusDto,
  SystemEventDto,
} from '~/types/dto';

/**
 * The visual state of the aura.
 *
 * It adds two states to the daemon ones: `offline`, for a daemon that does
 * not answer, and `success`, for a turn that has just run. A turn that ran
 * and a turn that failed both leave the daemon idle, so the state machine
 * alone cannot tell them apart once the moment has passed.
 */
export type AuraState =
  | 'offline'
  | 'idle'
  | 'listening'
  | 'transcribing'
  | 'resolving'
  | 'executing'
  | 'success'
  | 'error';

/**
 * What completed the last turn of the daemon.
 *
 * The state machine reports `Idle` for a turn that ran and `Error` for one
 * that failed, so the state alone cannot tell the two apart once the moment
 * has passed. The outcome keeps it, and the aura holds its `success` state
 * on it.
 */
export type TurnOutcome = 'ok' | 'failed';

/**
 * The turn that is running right now.
 *
 * The daemon streams every stage of a turn over the socket: the tokens of
 * the resolver, the intent it chose, the command it started, and the
 * output of that command. The live turn keeps those stages so the chat
 * page shows the work while it happens.
 */
export interface LiveTurnState {
  /** The answer the resolver has read so far. */
  thinking: string;
  /** The name of the resolved intent, or null. */
  intent: string | null;
  /** The probability of the chosen option, or null. */
  confidence: number | null;
  /** The engine that chose the intent, or null before it chose one. */
  intentEngine: ResolverEngineDto | null;
  /** The model that engine ran, or null when the daemon cannot name it. */
  intentModel: string | null;
  /** The engine that read the entity values, or null when none did. */
  valueEngine: ResolverEngineDto | null;
  /** The model that engine ran, or null when the daemon cannot name it. */
  valueModel: string | null;
  /** The entities of the intent with the values the resolver read. */
  entities: MessageEntityDto[];
  /** The command the daemon started, or null. */
  command: string | null;
  /** The output lines of the command, newest last. */
  output: string[];
}

/** The live status of the daemon and of the socket link. */
export interface AppStatusState {
  /** The state machine state of the daemon. */
  state: DaemonStateDto;
  /** The time the daemon entered the state, in ISO 8601 format. */
  since: string;
  /** The daemon version string. */
  version: string;
  /** True while the event stream is open. */
  connected: boolean;
  /** What completed the last turn, or null before the first one. */
  outcome: TurnOutcome | null;
  /** How many turns have completed since the shell started. */
  turns: number;
  /** The stages of the turn that runs right now. */
  live: LiveTurnState;
}

/** The aura state of each daemon state. */
const AURA_STATE_BY_DAEMON_STATE: Record<DaemonStateDto, AuraState> = {
  Idle: 'idle',
  Listening: 'listening',
  Transcribing: 'transcribing',
  Resolving: 'resolving',
  Executing: 'executing',
  Error: 'error',
};

/** Every visual state of the aura, in the order the interface shows them. */
export const AURA_STATES: AuraState[] = [
  'offline',
  'idle',
  'listening',
  'transcribing',
  'resolving',
  'executing',
  'success',
  'error',
];

/** The user language label of each aura state. */
export const AURA_STATE_LABELS: Record<AuraState, string> = {
  offline: 'No link',
  idle: 'Ready',
  listening: 'Listening',
  transcribing: 'Transcribing',
  resolving: 'Thinking',
  executing: 'Working',
  success: 'Done',
  error: 'Attention',
};

/** The meter energy of each daemon state, between 0 and 1. */
const ENERGY_BY_DAEMON_STATE: Record<DaemonStateDto, number> = {
  Idle: 0.12,
  Listening: 0.86,
  Transcribing: 0.6,
  Resolving: 0.92,
  Executing: 1,
  Error: 0.4,
};

/** The largest part of the model answer the live turn keeps. */
const MAX_THINKING_CHARS = 2000;

/** The largest number of command output lines the live turn keeps. */
const MAX_OUTPUT_LINES = 24;

/** Build the live turn of a turn that has not started. */
export function createEmptyLiveTurn(): LiveTurnState {
  return {
    thinking: '',
    intent: null,
    confidence: null,
    intentEngine: null,
    intentModel: null,
    valueEngine: null,
    valueModel: null,
    entities: [],
    command: null,
    output: [],
  };
}

/** Keep the end of a text, which is the part a reader is watching. */
function keepTail(text: string, max: number): string {
  return text.length <= max ? text : text.slice(text.length - max);
}

/** Keep the newest lines of a command output. */
function keepTailLines(lines: string[], max: number): string[] {
  return lines.length <= max ? lines : lines.slice(lines.length - max);
}

/** Build the first state from the REST snapshot. The link starts closed. */
export function createInitialStatus(snapshot: StatusDto): AppStatusState {
  return {
    state: snapshot.state,
    since: snapshot.since,
    version: snapshot.version,
    connected: false,
    outcome: null,
    turns: 0,
    live: createEmptyLiveTurn(),
  };
}

/**
 * Replace the state fields with a REST snapshot.
 *
 * A successful read proves that the link is open, so the function marks
 * the daemon as connected. The socket client uses it after a reconnect
 * to catch the events it missed.
 *
 * A snapshot says nothing about the turns, so the count survives it. The
 * aura must not celebrate a turn it has already reported.
 */
export function mergeSnapshot(
  current: AppStatusState,
  snapshot: StatusDto,
): AppStatusState {
  return {
    ...createInitialStatus(snapshot),
    connected: true,
    outcome: current.outcome,
    turns: current.turns,
    live: current.live,
  };
}

/** Read the daemon state that one event implies, or null. */
export function stateForEvent(event: SystemEventDto): DaemonStateDto | null {
  switch (event.payload.type) {
    case 'WakeDetected':
    case 'ListeningStarted':
      return 'Listening';
    case 'ListeningStopped':
      return 'Idle';
    case 'TranscriptInterim':
    case 'TranscriptFinal':
    case 'TranscriptCorrected':
      return 'Transcribing';
    case 'IntentThinking':
    case 'IntentResolved':
    case 'IntentValuesRead':
    case 'IntentNotFound':
      return 'Resolving';
    case 'ExecutionStarted':
    case 'ExecutionOutput':
      return 'Executing';
    case 'ExecutionCompleted':
    case 'MessageReplied':
      return 'Idle';
    case 'ExecutionFailed':
      return 'Error';
    default:
      return null;
  }
}

/** Read the turn outcome that one event reports, or null. */
export function outcomeForEvent(event: SystemEventDto): TurnOutcome | null {
  switch (event.payload.type) {
    case 'ExecutionCompleted':
      return 'ok';
    case 'ExecutionFailed':
      return 'failed';
    default:
      return null;
  }
}

/**
 * Fold one event into the stages of the live turn.
 *
 * A queued message starts a turn, so it clears what the last one left.
 * The stages then arrive in the order of the pipeline.
 */
export function applyLiveEvent(
  live: LiveTurnState,
  event: SystemEventDto,
): LiveTurnState {
  switch (event.payload.type) {
    case 'MessageQueued':
      return createEmptyLiveTurn();
    case 'IntentThinking':
      return {
        ...live,
        thinking: keepTail(
          live.thinking + event.payload.delta,
          MAX_THINKING_CHARS,
        ),
      };
    case 'IntentResolved':
      return {
        ...live,
        intent: event.payload.intent,
        confidence: event.payload.confidence,
        intentEngine: event.payload.engine,
        intentModel: event.payload.model,
      };
    case 'IntentValuesRead':
      return {
        ...live,
        valueEngine: event.payload.engine,
        valueModel: event.payload.model,
        entities: event.payload.entities,
      };
    case 'ExecutionStarted':
      return { ...live, command: event.payload.command, output: [] };
    case 'ExecutionOutput':
      return {
        ...live,
        output: keepTailLines(
          [...live.output, event.payload.line],
          MAX_OUTPUT_LINES,
        ),
      };
    default:
      return live;
  }
}

/** Fold one socket event into the live status. */
export function applyEvent(
  current: AppStatusState,
  event: SystemEventDto,
): AppStatusState {
  const next = stateForEvent(event);
  const outcome = outcomeForEvent(event);
  // A failed turn stays in the error state until the next turn starts.
  const holdsError =
    event.payload.type === 'MessageReplied' && current.state === 'Error';
  const state = holdsError ? current.state : (next ?? current.state);

  return {
    ...current,
    connected: true,
    state,
    since: state === current.state ? current.since : event.at,
    outcome: outcome ?? current.outcome,
    turns: outcome ? current.turns + 1 : current.turns,
    live: applyLiveEvent(current.live, event),
  };
}

/** Mark the link as closed. The daemon state stays as the last known one. */
export function markLinkClosed(current: AppStatusState): AppStatusState {
  return { ...current, connected: false };
}

/**
 * Map the daemon status to the visual state of the aura.
 *
 * The `success` state is not here: a turn that has just run is a moment and
 * not a status, so the aura holds it on its own. See
 * `components/viz/aura.tsx`.
 */
export function toAuraState(status: AppStatusState): AuraState {
  if (!status.connected) {
    return 'offline';
  }
  return AURA_STATE_BY_DAEMON_STATE[status.state];
}

/** Map the daemon status to the energy of the level meter, between 0 and 1. */
export function toEnergy(status: AppStatusState): number {
  if (!status.connected) {
    return 0;
  }
  return ENERGY_BY_DAEMON_STATE[status.state];
}

/** A short label for the daemon state. */
export function toStateLabel(status: AppStatusState): string {
  return status.connected ? status.state : 'Offline';
}
