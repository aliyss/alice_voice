import type { ChatMessageDto, MessageEntityDto } from '~/types/dto';

import { describe, expect, it } from 'vitest';

import {
  appendRow,
  formatClock,
  formatConfidence,
  formatDuration,
  formatEngine,
  formatEntity,
  formatRouteStep,
  toChatRow,
} from './chat';

function message(overrides: Partial<ChatMessageDto> = {}): ChatMessageDto {
  return {
    id: 'msg-1',
    conversationId: 'conv-1',
    role: 'user',
    text: 'set a timer',
    createdAt: '2026-09-18T10:04:05.000Z',
    intentId: null,
    intentName: null,
    confidence: null,
    meta: null,
    ...overrides,
  };
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

describe('formatClock', () => {
  it('returns a placeholder when the time is not readable', () => {
    expect(formatClock('not a date')).toBe('--:--:--');
  });

  it('pads the hour, the minute, and the second', () => {
    const date = new Date(2026, 8, 18, 4, 5, 6);
    expect(formatClock(date.toISOString())).toBe('04:05:06');
  });
});

describe('formatConfidence', () => {
  it('returns null for a missing confidence', () => {
    expect(formatConfidence(null)).toBeNull();
  });

  it('rounds the confidence to a whole percent', () => {
    expect(formatConfidence(0.876)).toBe('88%');
    expect(formatConfidence(1)).toBe('100%');
  });
});

describe('formatRouteStep', () => {
  it('names the stage, the reader, and the time of one stage', () => {
    expect(
      formatRouteStep({
        stage: 'decide',
        outcome: 'fell_back',
        reader: 'scores',
        model: null,
        detail: 'the model did not answer',
        candidates: [],
        durationMs: 3,
      }),
    ).toEqual({
      stage: 'decision',
      reader: 'scores',
      outcome: 'fell back to a cheaper reader',
      detail: 'the model did not answer',
      duration: '3 ms',
    });
  });

  it('keeps the model a reader ran beside the reader', () => {
    const step = formatRouteStep({
      stage: 'extract',
      outcome: 'matched',
      reader: 'model',
      model: 'qwen3.5-4b',
      detail: null,
      candidates: [],
      durationMs: 412,
    });

    expect(step.reader).toBe('model  qwen3.5-4b');
    expect(step.duration).toBe('412 ms');
  });
});

describe('toChatRow', () => {
  it('maps the DTO fields to the row fields', () => {
    const row = toChatRow(
      message({
        role: 'assistant',
        intentId: 'set_timer',
        intentName: 'set a timer',
        confidence: 0.5,
      }),
    );

    expect(row.id).toBe('msg-1');
    expect(row.role).toBe('assistant');
    expect(row.text).toBe('set a timer');
    expect(row.intentId).toBe('set_timer');
    expect(row.intentName).toBe('set a timer');
    expect(row.confidence).toBe('50%');
  });
});

describe('formatEngine', () => {
  it('names each engine in user language', () => {
    expect(formatEngine('llama')).toBe('llama.cpp');
    expect(formatEngine('gliner')).toBe('GLiNER');
  });

  it('reads a missing engine as null', () => {
    expect(formatEngine(null)).toBeNull();
  });
});

describe('formatDuration', () => {
  it('reads a run under a second in milliseconds', () => {
    expect(formatDuration(412)).toBe('412 ms');
  });

  it('reads a longer run in seconds', () => {
    expect(formatDuration(2100)).toBe('2.1 s');
  });
});

describe('formatEntity', () => {
  it('names the reader that read a value', () => {
    expect(
      formatEntity(entity({ source: 'list', read: 'firefox browser' })),
    ).toBe('city = Berlin  list (read firefox browser)');
  });

  it('reads an entity with its value', () => {
    expect(formatEntity(entity())).toBe('city = Berlin');
  });

  it('marks an entity without a value', () => {
    expect(formatEntity(entity({ value: '' }))).toBe('city = (unread)');
  });
});

describe('toChatRow metadata', () => {
  it('keeps the engine and the model of each step', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'llama',
          intentModel: 'qwen3.5-4b',
          valueEngine: 'gliner',
          valueModel: 'gliner_small-v2.1',
          entities: [entity()],
          command: 'curl -s wttr.in/Berlin?format=3',
          exitCode: 0,
          durationMs: 412,
          stage: null,
          candidates: [],
          route: null,
        },
      }),
    );

    expect(row.meta).toEqual({
      resolver: 'llama.cpp  ·  GLiNER',
      resolverModels: 'llama.cpp  qwen3.5-4b\nGLiNER  gliner_small-v2.1',
      stage: null,
      candidates: [],
      route: [],
      reason: null,
      entities: ['city = Berlin'],
      command: 'curl -s wttr.in/Berlin?format=3',
      exitCode: 'exit 0',
      exitFailed: false,
      duration: '412 ms',
    });
  });

  it('names the stage of the layered router and the short list', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'router',
          intentModel: 'qwen3.5-4b',
          valueEngine: 'gliner',
          valueModel: 'gliner_small-v2.1',
          stage: 'rerank',
          candidates: [
            { name: 'get weather', score: 0.62, evidence: 'words' },
            { name: 'tell time', score: 0.41, evidence: 'embedding' },
          ],
          entities: [],
          route: null,
          command: null,
          exitCode: null,
          durationMs: null,
        },
      }),
    );

    expect(row.meta?.resolver).toBe('Router  ·  GLiNER');
    expect(row.meta?.stage).toBe('model over the short list');
    expect(row.meta?.candidates).toEqual([
      'get weather  62%  words',
      'tell time  41%  embedding',
    ]);
  });

  it('names the engine of a turn that chose no intent', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'gliner',
          intentModel: 'gliner_small-v2.1',
          valueEngine: null,
          valueModel: null,
          entities: [],
          command: null,
          exitCode: null,
          durationMs: null,
          stage: null,
          candidates: [],
          route: null,
        },
      }),
    );

    expect(row.meta?.resolver).toBe('GLiNER');
    expect(row.meta?.resolverModels).toBe('GLiNER  gliner_small-v2.1');
    expect(row.meta?.entities).toEqual([]);
    expect(row.meta?.command).toBeNull();
  });

  it('lists the model of one engine once when it read both steps', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'gliner',
          intentModel: 'gliner_small-v2.1',
          valueEngine: 'gliner',
          valueModel: 'gliner_small-v2.1',
          entities: [],
          command: null,
          exitCode: null,
          durationMs: null,
          stage: null,
          candidates: [],
          route: null,
        },
      }),
    );

    expect(row.meta?.resolver).toBe('GLiNER  ·  GLiNER');
    expect(row.meta?.resolverModels).toBe('GLiNER  gliner_small-v2.1');
  });

  it('reports the engines without a model when the daemon cannot name one', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'llama',
          intentModel: null,
          valueEngine: null,
          valueModel: null,
          entities: [],
          command: null,
          exitCode: null,
          durationMs: null,
          stage: null,
          candidates: [],
          route: null,
        },
      }),
    );

    expect(row.meta?.resolver).toBe('llama.cpp');
    expect(row.meta?.resolverModels).toBeNull();
  });

  it('marks an entity the daemon could not fill', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'llama',
          intentModel: null,
          valueEngine: null,
          valueModel: null,
          entities: [entity({ value: '' })],
          command: null,
          exitCode: null,
          durationMs: null,
          stage: null,
          candidates: [],
          route: null,
        },
      }),
    );

    expect(row.meta?.entities).toEqual(['city = (unread)']);
    expect(row.meta?.duration).toBeNull();
  });

  it('reads a long run in seconds', () => {
    const row = toChatRow(
      message({
        meta: {
          intentEngine: 'gliner',
          intentModel: null,
          valueEngine: 'gliner',
          valueModel: null,
          entities: [],
          command: 'sleep 2',
          exitCode: 1,
          durationMs: 2100,
          stage: null,
          candidates: [],
          route: null,
        },
      }),
    );

    expect(row.meta?.exitCode).toBe('exit 1');
    expect(row.meta?.exitFailed).toBe(true);
    expect(row.meta?.duration).toBe('2.1 s');
  });

  it('reports no metadata for a turn that read nothing', () => {
    expect(toChatRow(message()).meta).toBeNull();
  });
});

describe('appendRow', () => {
  it('appends a new message at the end', () => {
    const rows = appendRow([], message());
    const next = appendRow(rows, message({ id: 'msg-2', role: 'assistant' }));

    expect(next.map((row) => row.id)).toEqual(['msg-1', 'msg-2']);
  });

  it('ignores a message that is already in the transcript', () => {
    const rows = appendRow([], message());
    expect(appendRow(rows, message())).toHaveLength(1);
  });
});
