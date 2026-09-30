/**
 * The view model of the daemon status.
 *
 * The backend sends a status snapshot over REST and a stream of events
 * over the socket. These pure functions fold the two into the single
 * state that the aura, the status strip, and the live turn render.
 */
import type {
  DaemonStateDto,
  MemorySavedFactDto,
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
 * One shell script the model wrote for a message no intent matched.
 *
 * The daemon runs no script of its own accord, so the surface reads the
 * script and its rating and waits for the decision of the user.
 */
export interface ProposedScript {
  /** The stable identifier of the stored script. */
  id: string;
  /** One sentence about what the script does. */
  summary: string;
  /** The shell script. */
  script: string;
  /** How rough the script is on the machine, between 0 and 100. */
  destructiveness: number;
}

/**
 * The turn that is running right now.
 *
 * The daemon streams every stage of a turn over the socket: the tokens of
 * the resolver, the intent it chose, the command it started, and the
 * output of that command. The live turn keeps those stages so the chat
 * page shows the work while it happens.
 */
/**
 * What the memory learned from one stored turn.
 *
 * The memory reads a turn after the daemon answered it, so this arrives on
 * its own and belongs to the message of the turn rather than to the stages
 * of the live one.
 */
export interface LiveMemory {
  /** The message the memory learned from. */
  messageId: string;
  /** The facts the turn taught, in the order the memory wrote them. */
  facts: MemorySavedFactDto[];
}

/**
 * One stage of the turn, as the resolver named it while it ran.
 *
 * The resolver reports a stage as it starts, so the surface reads the step
 * the daemon is on rather than one label that stands still for the whole
 * read. A stage that reads a model takes seconds, so the list is what
 * tells a reader that the turn moves.
 */
export interface LiveStage {
  /**
   * The stage: `fast_path`, `retrieve`, `decide`, `extract`, `answer`, or
   * `script`.
   */
  stage: string;
  /** One sentence about what the stage is doing. */
  detail: string;
  /**
   * The time the stage started, in ISO 8601 format.
   *
   * The resolver reports a stage when it starts it, so the gap between two
   * stages is the time the first one took and the field is what a reader
   * sees when they ask where the seconds of a turn went.
   */
  at: string;
}

/** The stages of the turn the daemon handles right now. */
export interface LiveTurnState {
  /** The answer the resolver has read so far. */
  thinking: string;
  /**
   * The stages the resolver has entered, oldest first.
   *
   * The list grows as the turn runs, so the last entry is the step the
   * daemon is on right now.
   */
  stages: LiveStage[];
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
  /** The script the model wrote and the user has not decided about. */
  script: ProposedScript | null;
  /** What the memory learned last, or null when it learned nothing yet. */
  memory: LiveMemory | null;
  /**
   * The time the turn started, in ISO 8601 format, or null before the
   * daemon accepted a message. The surface counts from it, so a user
   * reads how long the turn has been running and not only that it runs.
   */
  startedAt: string | null;
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

/**
 * The largest number of stages the live turn keeps.
 *
 * A turn passes a handful of stages even when it falls back, so the list
 * holds one turn and drops the oldest entry of a turn that runs long.
 */
const MAX_STAGES = 8;

/** The largest number of command output lines the live turn keeps. */
const MAX_OUTPUT_LINES = 24;

/** Build the live turn of a turn that has not started. */
export function createEmptyLiveTurn(): LiveTurnState {
  return {
    thinking: '',
    stages: [],
    intent: null,
    confidence: null,
    intentEngine: null,
    intentModel: null,
    valueEngine: null,
    valueModel: null,
    entities: [],
    command: null,
    output: [],
    script: null,
    memory: null,
    startedAt: null,
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
    case 'IntentStage':
    case 'IntentResolved':
    case 'IntentValuesRead':
    case 'IntentNotFound':
      return 'Resolving';
    case 'ExecutionStarted':
    case 'ExecutionOutput':
    case 'ScriptApproved':
      return 'Executing';
    case 'ScriptDenied':
      return 'Idle';
    case 'ScriptProposed':
      return 'Resolving';
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
      // The queued event is the start of the turn, so the surface counts
      // the wait from it.
      return { ...createEmptyLiveTurn(), startedAt: event.at };
    case 'IntentThinking':
      return {
        ...live,
        thinking: keepTail(
          live.thinking + event.payload.delta,
          MAX_THINKING_CHARS,
        ),
      };
    case 'IntentStage': {
      const stages = [
        ...live.stages,
        {
          stage: event.payload.stage,
          detail: event.payload.detail,
          at: event.at,
        },
      ];
      return {
        ...live,
        stages:
          stages.length <= MAX_STAGES ? stages : stages.slice(-MAX_STAGES),
      };
    }
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
    case 'ScriptProposed':
      return {
        ...live,
        script: {
          id: event.payload.id,
          summary: event.payload.summary,
          script: event.payload.script,
          destructiveness: event.payload.destructiveness,
        },
      };
    case 'ScriptApproved':
    case 'ScriptDenied':
      return { ...live, script: null };
    case 'MemorySaved':
      // The memory reads the turn in the background, so this arrives after
      // the turn ended and names the message it learned from.
      return {
        ...live,
        memory: {
          messageId: event.payload.message_id,
          facts: event.payload.facts,
        },
      };
    default:
      return live;
  }
}

/**
 * The time every stage of a running turn has taken, oldest first.
 *
 * A stage ends when the next one starts, so the gap between two stages is
 * the time of the first one. The last stage has no next one yet, so it is
 * measured down to `now` and grows while the daemon works. A stage whose
 * time cannot be read reports zero rather than a number a reader would
 * mistake for a measurement.
 */
export function stageDurations(stages: LiveStage[], now: number): number[] {
  return stages.map((stage, index) => {
    const next = index + 1 < stages.length ? stages[index + 1] : null;
    const start = Date.parse(stage.at);
    const end = next === null ? now : Date.parse(next.at);
    if (Number.isNaN(start) || Number.isNaN(end)) {
      return 0;
    }
    return Math.max(0, end - start);
  });
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
