import type {
  MessageEntityDto,
  MessageMetaDto,
  MessageRouteStepDto,
  ResolverPreviewDto,
} from '~/types/dto';

import { describe, expect, it } from 'vitest';

import { debugPreview, readPreview } from '~/utils/preview-log';

/** Build one step of a route. */
function step(
  stage: MessageRouteStepDto['stage'],
  options: Partial<MessageRouteStepDto> = {},
): MessageRouteStepDto {
  return {
    stage,
    outcome: 'passed',
    reader: 'rules',
    model: null,
    detail: null,
    candidates: [],
    durationMs: 2,
    ...options,
  };
}

/** Build one value of a turn. */
function entity(
  name: string,
  options: Partial<MessageEntityDto> = {},
): MessageEntityDto {
  return {
    name,
    value: '',
    source: null,
    engine: null,
    model: null,
    read: null,
    score: null,
    ...options,
  };
}

/** Build the metadata of one preview. */
function meta(options: Partial<MessageMetaDto> = {}): MessageMetaDto {
  return {
    intentEngine: 'router',
    intentModel: null,
    valueEngine: null,
    valueModel: null,
    stage: 'none',
    candidates: [],
    entities: [],
    route: null,
    command: null,
    exitCode: null,
    durationMs: 12,
    ...options,
  };
}

/** Build one preview with the steps and the answer of the daemon. */
function preview(
  options: Partial<ResolverPreviewDto> = {},
): ResolverPreviewDto {
  return {
    text: 'open firefox',
    matched: false,
    intent: null,
    confidence: null,
    reply: '',
    meta: meta(),
    ...options,
  };
}

describe('readPreview', () => {
  it('names what the daemon would run', () => {
    const reading = readPreview(
      preview({
        matched: true,
        intent: 'open application',
        confidence: 0.72,
        meta: meta({
          stage: 'rerank',
          entities: [
            entity('applications', {
              value: 'firefox',
              source: 'embedding',
              engine: 'router',
              model: 'bge-small-en-v1.5',
              read: 'the browser',
              score: 0.91,
            }),
          ],
        }),
      }),
    );

    expect(reading.status).toBe('Would run');
    expect(reading.answer).toBe('open application');
    expect(reading.confidence).toBe('72%');
    expect(reading.entities).toEqual([
      'applications = firefox  vectors (read the browser)',
    ]);
    expect(reading.command).toBeNull();
  });

  it('reads a refusal that answered the user', () => {
    const reading = readPreview(
      preview({
        reply: 'I need the city of the weather.',
        meta: meta({ durationMs: null }),
      }),
    );

    expect(reading.status).toBe('No intent');
    expect(reading.answer).toBe('I need the city of the weather.');
    expect(reading.matched).toBe(false);
    expect(reading.reply).toBeNull();
  });

  it('answers a refusal the daemon sent no reply for', () => {
    const reading = readPreview(preview());

    expect(reading.status).toBe('No intent');
    expect(reading.answer).toBe('I could not match that to an intent.');
  });

  it('keeps the reply of a match beside the intent', () => {
    const reading = readPreview(
      preview({
        matched: true,
        intent: 'open application',
        reply: 'I need a value for applications.',
      }),
    );

    expect(reading.status).toBe('Would run');
    expect(reading.answer).toBe('open application');
    expect(reading.reply).toBe('I need a value for applications.');
  });
});

describe('debugPreview', () => {
  it('reads the answer, the stages, the values, and the run', () => {
    const blocks = debugPreview(
      preview({
        matched: true,
        intent: 'open application',
        confidence: 0.72,
        meta: meta({
          stage: 'rerank',
          entities: [entity('applications', { value: 'firefox' })],
          command: 'firefox --new-window',
          durationMs: 26,
          route: {
            stage: 'rerank',
            matched: true,
            reason: null,
            steps: [
              step('fast_path', { outcome: 'passed', reader: 'rules' }),
              step('retrieve', {
                reader: 'dense',
                model: 'bge-small-en-v1.5',
                candidates: [
                  {
                    name: 'open application',
                    score: 0.8,
                    evidence: 'words and vectors',
                  },
                ],
              }),
            ],
          },
        }),
      }),
    );

    expect(blocks.map((block) => block.title)).toEqual([
      'The answer',
      'deterministic pass',
      'ranking of the catalog',
      'applications',
      'The run',
    ]);

    const answer = blocks[0];
    expect(blocks[0].fields).toContainEqual({
      label: 'The answer',
      value: 'an intent matched',
      hint: expect.any(String),
      tone: 'ok',
    });
    expect(answer.fields).toContainEqual({
      label: 'The stage that decided',
      value: 'model over the short list',
      hint: expect.any(String),
    });
    expect(answer.fields).toContainEqual({
      label: 'The time the read took',
      value: '26 ms',
      hint: expect.any(String),
    });

    // The list the decision read stands with the stage that read it.
    expect(blocks[2].fields).toContainEqual({
      label: '#1 of the short list',
      value: 'open application  80%  words and vectors',
      hint: expect.any(String),
    });
    expect(blocks[2].fields).toContainEqual({
      label: 'The reader that ran',
      value: 'dense  bge-small-en-v1.5',
      hint: expect.any(String),
    });

    // A value the daemon read nothing for says so rather than standing empty.
    expect(blocks[3].fields[0]).toEqual({
      label: 'The value',
      value: 'firefox',
      hint: expect.any(String),
    });
  });

  it('colors the answer, the stage, and the value that needs a reader', () => {
    const blocks = debugPreview(
      preview({
        matched: false,
        reply: 'I need a value for applications.',
        meta: meta({
          entities: [entity('applications', { value: 'firefox' })],
          route: {
            stage: 'none',
            matched: false,
            reason: 'below the floor',
            steps: [
              step('retrieve', { outcome: 'fell_back', reader: 'words' }),
              step('decide', {
                outcome: 'refused',
                reader: 'model',
                model: 'fake-4b',
              }),
            ],
          },
        }),
      }),
    );

    // A refusal that answered the user is a warning, not a failure.
    expect(blocks[0].tone).toBe('warn');
    expect(blocks[0].fields[0].tone).toBe('warn');
    expect(blocks[0].fields).toContainEqual(
      expect.objectContaining({ label: 'Why it refused', tone: 'warn' }),
    );
    // A stage takes the color of the way it ended, in its title and in the
    // value that says how it ended.
    expect(blocks[1].tone).toBe('warn');
    expect(blocks[2].tone).toBe('error');
    expect(blocks[2].fields).toContainEqual(
      expect.objectContaining({ label: 'How it ended', tone: 'error' }),
    );
    // A value that simply says what the daemon ran takes no color.
    expect(blocks[2].fields[0].tone).toBeUndefined();
  });

  it('marks a value the daemon read nothing for', () => {
    const blocks = debugPreview(
      preview({
        matched: true,
        intent: 'open application',
        meta: meta({
          entities: [
            entity('applications', { value: 'firefox' }),
            entity('window'),
          ],
        }),
      }),
    );

    expect(blocks[0].tone).toBe('ok');
    expect(blocks[1].fields[0]).toEqual({
      label: 'The value',
      value: 'firefox',
      hint: expect.any(String),
      tone: undefined,
    });
    expect(blocks[2].fields[0]).toEqual({
      label: 'The value',
      value: '(unread)',
      hint: expect.any(String),
      tone: 'warn',
    });
  });

  it('leaves out the parts a turn never reached', () => {
    const blocks = debugPreview(
      preview({
        matched: false,
        meta: meta({
          intentEngine: null,
          durationMs: null,
          route: {
            stage: 'none',
            matched: false,
            reason: 'below the floor',
            steps: [step('fast_path', { outcome: 'refused', reader: 'rules' })],
          },
        }),
      }),
    );

    expect(blocks.map((block) => block.title)).toEqual([
      'The answer',
      'deterministic pass',
    ]);
    expect(blocks[0].fields).toContainEqual({
      label: 'Why it refused',
      value: 'below the floor',
      hint: expect.any(String),
      tone: 'error',
    });
    // A turn that met no intent runs nothing and read no value.
    expect(blocks[0].fields).not.toContainEqual(
      expect.objectContaining({ label: 'The time the read took' }),
    );
  });

  it('reads a turn with no metadata at all', () => {
    const blocks = debugPreview(preview({ meta: null }));

    expect(blocks).toHaveLength(1);
    expect(blocks[0].title).toBe('The answer');
    expect(blocks[0].fields).toHaveLength(1);
  });
});
