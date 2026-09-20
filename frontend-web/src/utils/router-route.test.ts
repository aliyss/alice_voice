import type {
  MessageRouteDto,
  MessageRouteStepDto,
  ResolverPreviewDto,
  RouterStageDto,
} from '~/types/dto';

import { describe, expect, it } from 'vitest';

import { routeHighlight } from '~/utils/router-route';

/** The places the tests can light up. */
const PLACES = {
  vectors: 'server',
  reranker: 'builtin',
  model: 'server',
  spans: 'gliner',
};

/** Build one step of a route. */
function step(
  stage: MessageRouteStepDto['stage'],
  reader: string,
  outcome: MessageRouteStepDto['outcome'] = 'passed',
): MessageRouteStepDto {
  return {
    stage,
    outcome,
    reader,
    model: null,
    detail: null,
    candidates: [],
    durationMs: 3,
  };
}

/** Build one route with the steps and the answer of the daemon. */
function route(
  steps: MessageRouteStepDto[],
  options: { matched?: boolean; reply?: string } = {},
): ResolverPreviewDto {
  const matched = options.matched ?? true;
  const stage: RouterStageDto | null =
    steps.at(-1)?.stage === 'extract'
      ? 'rerank'
      : ((steps.at(-1)?.stage ?? null) as RouterStageDto | null);
  const route: MessageRouteDto = {
    stage,
    matched,
    reason: matched ? null : 'below the floor',
    steps,
  };
  return {
    text: 'open firefox',
    matched,
    intent: matched ? 'open application' : null,
    confidence: matched ? 0.7 : null,
    reply: options.reply ?? '',
    meta: {
      intentEngine: 'router',
      intentModel: null,
      valueEngine: null,
      valueModel: null,
      stage: route.stage,
      candidates: [],
      entities: [],
      route,
      command: null,
      exitCode: null,
      durationMs: null,
    },
  };
}

describe('routeHighlight', () => {
  it('reads nothing out of a turn without a route', () => {
    expect(routeHighlight(null, PLACES)).toBeNull();
    expect(routeHighlight(route([]), PLACES)).toBeNull();
  });

  it('keeps the stages a turn skipped out of the highlight', () => {
    const highlight = routeHighlight(
      route([step('fast_path', 'rules', 'matched')]),
      PLACES,
    );

    // A proven message runs without the stages below it, and the graph
    // draws no link between the two, so only the stages it walked light
    // up beside the run the message reaches.
    expect(highlight?.nodes).toEqual(['message', 'deterministic', 'run']);
    expect(highlight?.edges).toEqual(['message-deterministic']);
    // The order of the ids is the order the route reached them, so the
    // caller reads the same list it draws.
    expect(highlight?.nodes[0]).toBe('message');
  });

  it('walks the chain the words of the catalog ranked', () => {
    const highlight = routeHighlight(
      route([
        step('fast_path', 'rules'),
        step('retrieve', 'lexical'),
        step('decide', 'model'),
        step('extract', 'spans'),
      ]),
      PLACES,
    );

    expect([...(highlight?.nodes ?? [])].sort()).toEqual([
      'decision',
      'deterministic',
      'extraction',
      'gliner',
      'message',
      'retrieval',
      'run',
      'server',
    ]);
    // The chain reads first, then the places that answered a stage: the
    // server decided the turn and the built in model found the spans.
    expect(highlight?.edges).toEqual([
      'message-deterministic',
      'deterministic-retrieval',
      'retrieval-decision',
      'decision-extraction',
      'extraction-run',
      'server-decision',
      'gliner-extraction',
    ]);
  });

  it('walks the links of the places a stage read', () => {
    const highlight = routeHighlight(
      route([
        step('fast_path', 'rules'),
        step('retrieve', 'dense'),
        step('decide', 'reranker'),
        step('extract', 'spans'),
      ]),
      PLACES,
    );

    expect(highlight?.edges).toContain('server-retrieval');
    expect(highlight?.edges).toContain('builtin-decision');
    expect(highlight?.edges).toContain('gliner-extraction');
    expect(highlight?.route.edges['server-retrieval']).toBe('ok');
    expect(highlight?.route.edges['builtin-decision']).toBe('ok');
  });

  it('lights the vectors the ranking really read', () => {
    const highlight = routeHighlight(
      route([step('fast_path', 'rules'), step('retrieve', 'dense')]),
      PLACES,
    );

    expect(highlight?.nodes).toContain('server');
    expect(highlight?.edges).toContain('deterministic-retrieval');
  });

  it('reads the exit of a refusal', () => {
    const highlight = routeHighlight(
      route(
        [step('fast_path', 'rules'), step('retrieve', 'lexical', 'refused')],
        { matched: false },
      ),
      PLACES,
    );

    expect(highlight?.nodes).toContain('refused');
    expect(highlight?.nodes).not.toContain('asks');
    // The graph draws the exit off the decision stage, and this turn
    // never reached it, so no link carries it out.
    expect(highlight?.edges).not.toContain('decision-refused');
  });

  it('leaves the chain through the decision when the decision refused', () => {
    const highlight = routeHighlight(
      route(
        [
          step('fast_path', 'rules'),
          step('retrieve', 'lexical'),
          step('decide', 'scores', 'refused'),
        ],
        { matched: false },
      ),
      PLACES,
    );

    expect(highlight?.nodes).toContain('refused');
    expect(highlight?.edges).toContain('decision-refused');
  });

  it('gives a refusal the tone of a failure', () => {
    const highlight = routeHighlight(
      route(
        [
          step('fast_path', 'rules'),
          step('retrieve', 'lexical'),
          step('decide', 'scores', 'refused'),
        ],
        { matched: false },
      ),
      PLACES,
    );

    expect(highlight?.route.nodes.refused).toBe('error');
    expect(highlight?.route.nodes.decision).toBe('error');
    // The link carries the color of the block it leads into, so the way a
    // turn left the chain reads along the link as well.
    expect(highlight?.route.edges['decision-refused']).toBe('error');
    expect(highlight?.route.nodes.message).toBe('ok');
  });

  it('gives a stage that fell back the tone of a warning', () => {
    const highlight = routeHighlight(
      route([
        step('fast_path', 'rules'),
        step('retrieve', 'words', 'fell_back'),
        step('decide', 'scores', 'matched'),
      ]),
      PLACES,
    );

    expect(highlight?.route.nodes.retrieval).toBe('warn');
    expect(highlight?.route.edges['deterministic-retrieval']).toBe('warn');
    expect(highlight?.route.nodes.decision).toBe('ok');
  });

  it('reads the exit that asks with the tone of a warning', () => {
    const highlight = routeHighlight(
      route(
        [
          step('fast_path', 'rules'),
          step('retrieve', 'lexical'),
          step('decide', 'scores', 'matched'),
          step('extract', 'lists', 'matched'),
        ],
        { reply: 'To run open application I need a value for applications.' },
      ),
      PLACES,
    );

    expect(highlight?.route.nodes.asks).toBe('warn');
    expect(highlight?.route.edges['extraction-asks']).toBe('warn');
    expect(highlight?.route.nodes.run).toBeUndefined();
  });

  it('reads the exit that asks for a missing value', () => {
    const highlight = routeHighlight(
      route(
        [
          step('fast_path', 'rules'),
          step('retrieve', 'lexical'),
          step('decide', 'scores', 'matched'),
          step('extract', 'lists', 'matched'),
        ],
        { reply: 'To run open application I need a value for applications.' },
      ),
      PLACES,
    );

    expect(highlight?.nodes).toContain('asks');
    expect(highlight?.nodes).not.toContain('gliner');
    expect(highlight?.edges).toContain('extraction-asks');
  });

  it('lights the place the reader that fell back really is', () => {
    const highlight = routeHighlight(
      route([
        step('fast_path', 'rules'),
        step('retrieve', 'words', 'fell_back'),
        step('decide', 'scores'),
      ]),
      PLACES,
    );

    expect(highlight?.nodes).not.toContain('server');
    expect([...(highlight?.nodes ?? [])].sort()).toEqual([
      'decision',
      'deterministic',
      'message',
      'retrieval',
      'run',
    ]);
  });
});
