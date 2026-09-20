import type { MessageEntityDto, StatusDto, SystemEventDto } from '~/types/dto';

import type { AppStatusState } from './status';

import { describe, expect, it } from 'vitest';

import {
  applyEvent,
  applyLiveEvent,
  createEmptyLiveTurn,
  createInitialStatus,
  markLinkClosed,
  mergeSnapshot,
  outcomeForEvent,
  stateForEvent,
  toAuraState,
  toEnergy,
  toStateLabel,
} from './status';

const SNAPSHOT: StatusDto = {
  state: 'Idle',
  since: '2026-09-18T10:00:00.000Z',
  version: '0.1.0',
};

function connectedStatus(): AppStatusState {
  return { ...createInitialStatus(SNAPSHOT), connected: true };
}

/** Build one entity with the reader that read its value. */
function entity(overrides: Partial<MessageEntityDto> = {}): MessageEntityDto {
  return {
    name: 'city',
    value: 'Berlin',
    source: null,
    engine: null,
    model: null,
    read: null,
    score: null,
    ...overrides,
  };
}

function event(payload: SystemEventDto['payload']): SystemEventDto {
  return { at: '2026-09-18T10:00:05.000Z', payload };
}

/** The payload of a command that ran. */
function completed(intent: string): SystemEventDto['payload'] {
  return {
    type: 'ExecutionCompleted',
    intent,
    exitCode: 0,
    durationMs: 12,
    output: 'done',
  };
}

describe('createInitialStatus', () => {
  it('starts with a closed link', () => {
    const status = createInitialStatus(SNAPSHOT);
    expect(status.connected).toBe(false);
    expect(status.state).toBe('Idle');
    expect(status.version).toBe('0.1.0');
  });
});

describe('mergeSnapshot', () => {
  it('opens the link after a successful read', () => {
    const next = mergeSnapshot(createInitialStatus(SNAPSHOT), {
      ...SNAPSHOT,
      state: 'Listening',
    });

    expect(next.connected).toBe(true);
    expect(next.state).toBe('Listening');
  });

  it('keeps the turn count, because a snapshot knows nothing about them', () => {
    const next = mergeSnapshot(connectedStatus(), SNAPSHOT);

    expect(next.turns).toBe(0);
    expect(next.outcome).toBeNull();
  });
});

describe('outcomeForEvent', () => {
  it('reports the two events that end a turn', () => {
    expect(outcomeForEvent(event(completed('set_timer')))).toBe('ok');
    expect(
      outcomeForEvent(
        event({ type: 'ExecutionFailed', intent: 'set_timer', error: 'nope' }),
      ),
    ).toBe('failed');
    expect(outcomeForEvent(event({ type: 'ListeningStarted' }))).toBeNull();
  });
});

describe('stateForEvent', () => {
  it('maps the transcript events to Transcribing', () => {
    expect(
      stateForEvent(event({ type: 'TranscriptInterim', text: 'hey' })),
    ).toBe('Transcribing');
    expect(stateForEvent(event({ type: 'TranscriptFinal', text: 'hey' }))).toBe(
      'Transcribing',
    );
  });

  it('maps the execution failure to Error', () => {
    const payload = {
      type: 'ExecutionFailed' as const,
      intent: 'set_timer',
      error: 'no plugin',
    };
    expect(stateForEvent(event(payload))).toBe('Error');
  });
});

describe('applyEvent', () => {
  it('opens the link and stores the transition time', () => {
    const next = applyEvent(
      createInitialStatus(SNAPSHOT),
      event({ type: 'WakeDetected', timestamp: '2026-09-18T10:00:05.000Z' }),
    );

    expect(next.connected).toBe(true);
    expect(next.state).toBe('Listening');
    expect(next.since).toBe('2026-09-18T10:00:05.000Z');
  });

  it('counts a turn that ran, and keeps what it reported', () => {
    const next = applyEvent(connectedStatus(), event(completed('set_timer')));

    expect(next.state).toBe('Idle');
    expect(next.outcome).toBe('ok');
    expect(next.turns).toBe(1);
  });

  it('counts a turn that failed', () => {
    const next = applyEvent(
      connectedStatus(),
      event({
        type: 'ExecutionFailed',
        intent: 'set_timer',
        error: 'no plugin',
      }),
    );

    expect(next.state).toBe('Error');
    expect(next.outcome).toBe('failed');
    expect(next.turns).toBe(1);
  });

  it('leaves the turn count alone for an event that is not a turn', () => {
    const next = applyEvent(
      connectedStatus(),
      event({ type: 'ListeningStarted' }),
    );

    expect(next.turns).toBe(0);
    expect(next.outcome).toBeNull();
  });

  it('returns to Idle when the daemon stops listening', () => {
    const listening = { ...connectedStatus(), state: 'Listening' as const };
    const next = applyEvent(
      listening,
      event({ type: 'ListeningStopped', reason: 'SilenceTimeout' }),
    );

    expect(next.state).toBe('Idle');
  });
});

describe('applyLiveEvent', () => {
  it('clears the stages when a message enters the queue', () => {
    const live = {
      thinking: 'old',
      intent: 'get weather',
      confidence: 0.9,
      intentEngine: 'llama' as const,
      intentModel: 'qwen3.5-4b',
      valueEngine: 'gliner' as const,
      valueModel: 'gliner_small-v2.1',
      entities: [entity()],
      command: 'echo hi',
      output: ['hi'],
    };
    const next = applyLiveEvent(
      live,
      event({ type: 'MessageQueued', id: 'msg-1' }),
    );

    expect(next).toEqual(createEmptyLiveTurn());
  });

  it('appends the tokens the resolver read', () => {
    const first = applyLiveEvent(
      createEmptyLiveTurn(),
      event({ type: 'IntentThinking', delta: '{"choice"' }),
    );
    const next = applyLiveEvent(
      first,
      event({ type: 'IntentThinking', delta: ':"A"}' }),
    );

    expect(next.thinking).toBe('{"choice":"A"}');
  });

  it('keeps the name and the probability of the resolved intent', () => {
    const next = applyLiveEvent(
      createEmptyLiveTurn(),
      event({
        type: 'IntentResolved',
        intent: 'get weather',
        confidence: 0.93,
        engine: 'llama',
        model: 'qwen3.5-4b',
      }),
    );

    expect(next.intent).toBe('get weather');
    expect(next.confidence).toBe(0.93);
    expect(next.intentEngine).toBe('llama');
    expect(next.intentModel).toBe('qwen3.5-4b');
  });

  it('keeps the engine that read the entity values', () => {
    const next = applyLiveEvent(
      createEmptyLiveTurn(),
      event({
        type: 'IntentValuesRead',
        engine: 'gliner',
        model: 'gliner_small-v2.1',
        entities: [entity()],
      }),
    );

    expect(next.valueEngine).toBe('gliner');
    expect(next.valueModel).toBe('gliner_small-v2.1');
    expect(next.entities).toEqual([entity()]);
  });

  it('keeps a missing probability as null', () => {
    const next = applyLiveEvent(
      createEmptyLiveTurn(),
      event({
        type: 'IntentResolved',
        intent: 'get weather',
        confidence: null,
        engine: 'gliner',
        model: 'gliner_small-v2.1',
      }),
    );

    expect(next.confidence).toBeNull();
    expect(next.intentModel).toBe('gliner_small-v2.1');
  });

  it('starts the output again when the command starts', () => {
    const stale = { ...createEmptyLiveTurn(), output: ['stale'] };
    const next = applyLiveEvent(
      stale,
      event({
        type: 'ExecutionStarted',
        intent: 'get weather',
        command: 'echo hi',
      }),
    );

    expect(next.command).toBe('echo hi');
    expect(next.output).toEqual([]);
  });

  it('appends the output lines of the command', () => {
    const started = applyLiveEvent(
      createEmptyLiveTurn(),
      event({ type: 'ExecutionStarted', intent: 'x', command: 'y' }),
    );
    const first = applyLiveEvent(
      started,
      event({ type: 'ExecutionOutput', stream: 'stdout', line: 'one' }),
    );
    const next = applyLiveEvent(
      first,
      event({ type: 'ExecutionOutput', stream: 'stderr', line: 'two' }),
    );

    expect(next.output).toEqual(['one', 'two']);
  });

  it('keeps the end of a long model answer', () => {
    const next = applyLiveEvent(
      createEmptyLiveTurn(),
      event({ type: 'IntentThinking', delta: 'a'.repeat(2500) }),
    );

    expect(next.thinking).toHaveLength(2000);
  });
});

describe('applyEvent with a live turn', () => {
  it('carries the stages of the turn and reports Resolving', () => {
    const queued = applyEvent(
      connectedStatus(),
      event({ type: 'MessageQueued', id: 'msg-1' }),
    );
    const thinking = applyEvent(
      queued,
      event({ type: 'IntentThinking', delta: 'Because' }),
    );
    const resolved = applyEvent(
      thinking,
      event({
        type: 'IntentResolved',
        intent: 'get weather',
        confidence: 0.9,
        engine: 'llama',
        model: 'qwen3.5-4b',
      }),
    );

    expect(resolved.live.thinking).toBe('Because');
    expect(resolved.live.intent).toBe('get weather');
    expect(resolved.state).toBe('Resolving');
  });

  it('leaves the resolver state for idle when the reply lands', () => {
    const resolving = { ...connectedStatus(), state: 'Resolving' as const };
    const next = applyEvent(
      resolving,
      event({ type: 'MessageReplied', id: 'msg-1' }),
    );

    expect(next.state).toBe('Idle');
  });

  it('keeps a failed turn in the error state until the next one', () => {
    const failed = applyEvent(
      connectedStatus(),
      event({ type: 'ExecutionFailed', intent: 'x', error: 'nope' }),
    );
    const next = applyEvent(
      failed,
      event({ type: 'MessageReplied', id: 'msg-1' }),
    );

    expect(next.state).toBe('Error');
    expect(next.outcome).toBe('failed');
  });
});

describe('markLinkClosed', () => {
  it('closes the link and keeps the last state', () => {
    const next = markLinkClosed(connectedStatus());
    expect(next.connected).toBe(false);
    expect(next.state).toBe('Idle');
  });
});

describe('toAuraState', () => {
  it('returns offline while the link is closed', () => {
    expect(toAuraState(createInitialStatus(SNAPSHOT))).toBe('offline');
  });

  it('returns the lower case daemon state while the link is open', () => {
    const status = { ...connectedStatus(), state: 'Executing' as const };
    expect(toAuraState(status)).toBe('executing');
  });
});

describe('toEnergy', () => {
  it('returns zero while the link is closed', () => {
    expect(toEnergy(createInitialStatus(SNAPSHOT))).toBe(0);
  });

  it('returns the highest energy while the daemon executes', () => {
    const status = { ...connectedStatus(), state: 'Executing' as const };
    expect(toEnergy(status)).toBe(1);
  });
});

describe('toStateLabel', () => {
  it('shows Offline while the link is closed', () => {
    expect(toStateLabel(createInitialStatus(SNAPSHOT))).toBe('Offline');
  });

  it('shows the daemon state while the link is open', () => {
    expect(toStateLabel(connectedStatus())).toBe('Idle');
  });
});
