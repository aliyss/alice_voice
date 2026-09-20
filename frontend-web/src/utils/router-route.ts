/**
 * The route of one message, read back onto the flow of the router.
 *
 * The daemon answers a preview with the route a message took: the stages
 * it passed, the reader every stage ran, and whether it met an intent. The
 * flow of the settings page draws those same stages, so this module turns
 * one route into the ids of the blocks and links that carried the message.
 * A reader then sees which stage answered a message and which stage
 * refused it, rather than a graph of every stage at once.
 *
 * The ids are the ids the graph uses, so the mapping lives beside the
 * flow, and a stage this module does not know is left out rather than
 * guessed at. Every link of the graph is named after the two blocks it
 * joins, so a link this module can name is a link the graph draws.
 */
import type { FlowRoute, FlowTone } from '~/components/ui/flow';

import type {
  MessageRouteStepDto,
  ResolverPreviewDto,
  RouteStageDto,
} from '~/types/dto';

/** The id of the block that draws one stage of the router. */
const STAGE_BLOCKS: Record<RouteStageDto, string> = {
  fast_path: 'deterministic',
  retrieve: 'retrieval',
  decide: 'decision',
  extract: 'extraction',
};

/** The chain of the graph, in the order a message walks it. */
const CHAIN = [
  'message',
  'deterministic',
  'retrieval',
  'decision',
  'extraction',
  'run',
] as const;

/** The link between two blocks of the chain, by the two names. */
const CHAIN_LINKS: string[] = [
  'message-deterministic',
  'deterministic-retrieval',
  'retrieval-decision',
  'decision-extraction',
  'extraction-run',
];

/** The block that answers one reader of the route, if the graph draws it. */
export interface RoutePlaces {
  /** The block that ranks the catalog by its vectors, or null. */
  vectors: string | null;
  /** The block that ran the decision with a built in model, or null. */
  reranker: string | null;
  /** The block that ran a language model, or null. */
  model: string | null;
  /** The block that found the spans of the values, or null. */
  spans: string | null;
}

/**
 * The blocks and links one route reached, with the tone of each.
 *
 * The ids say what the route touched and the tones say how each part of it
 * ended, so the graph draws a turn that fell back in amber and a turn that
 * refused in red rather than in one flat color.
 */
export interface RouteHighlight {
  /** The ids of the blocks the route reached. */
  nodes: string[];
  /** The ids of the links the route followed. */
  edges: string[];
  /** The ids above, with the tone of each one, for the graph. */
  route: FlowRoute;
}

/**
 * How one stage of the route ended.
 *
 * A stage that refused ended the turn and reads as a failure; a stage
 * that fell back to another reader read the message less surely than the
 * settings promise and reads as a warning; every other stage answered what
 * it was asked to answer.
 */
function toneOf(step: MessageRouteStepDto): FlowTone {
  if (step.outcome === 'refused') {
    return 'error';
  }
  return step.outcome === 'fell_back' ? 'warn' : 'ok';
}

/**
 * The link the graph draws out of the chain, by the exit it reaches.
 *
 * The graph hangs each exit off the stage that can leave the chain, so the
 * link is walked only when the turn really reached that stage: a refusal
 * the retrieval stage answered leaves the chain nowhere the graph draws a
 * link, and its exit block stands on its own.
 */
const EXIT_LINKS = {
  refused: { link: 'decision-refused', from: 'decision' },
  asks: { link: 'extraction-asks', from: 'extraction' },
} as const;

/** One highlight while it is built. */
interface HighlightDraft {
  /** The blocks the route reached, in the order it reached them. */
  nodes: Set<string>;
  /** The links the route followed, in the order it followed them. */
  edges: string[];
  /** The tone of every block and link, by id. */
  tones: Record<string, FlowTone>;
}

/** The tone of the message itself, which every route walks through. */
const MESSAGE_TONE: FlowTone = 'ok';

/**
 * Close a highlight.
 *
 * A link takes the tone of the block it leads into, so a reader follows
 * one color from the message to the answer, and the link that carries a
 * refusal turns red with the block it reaches.
 */
function close(draft: HighlightDraft): RouteHighlight {
  const nodes: Record<string, FlowTone> = {};
  for (const id of draft.nodes) {
    nodes[id] = draft.tones[id] ?? MESSAGE_TONE;
  }
  const edges: Record<string, FlowTone> = {};
  for (const id of draft.edges) {
    const to = id.split('-').at(-1) ?? '';
    edges[id] = nodes[to] ?? MESSAGE_TONE;
  }
  return {
    nodes: [...draft.nodes],
    edges: draft.edges,
    route: { nodes, edges },
  };
}

/**
 * Read the blocks and links one route reached.
 *
 * A stage the route skipped keeps out of the highlight, so a turn the
 * deterministic pass answered does not light up the stages below it. The
 * place a stage read stands with the reader that really answered it, so a
 * turn the embedding server ranked lights the server and a turn the words
 * alone ranked lights no place at all. The link that place feeds the chain
 * is walked with it, because the graph draws every place that way around:
 * the dashed link a reader follows into the stage that read it.
 */
export function routeHighlight(
  preview: ResolverPreviewDto | null,
  places: RoutePlaces,
): RouteHighlight | null {
  const route = preview?.meta?.route ?? null;
  if (!route || route.steps.length === 0) {
    return null;
  }

  const draft: HighlightDraft = {
    nodes: new Set<string>(['message']),
    edges: [],
    tones: {},
  };
  // The places the stages read, each with the link it feeds the chain. A
  // place answers a stage rather than carrying the message, so its link is
  // drawn last: the chain a reader follows is read first.
  const readers: string[] = [];
  for (const step of route.steps) {
    if (step.outcome === 'skipped') {
      continue;
    }
    const block = STAGE_BLOCKS[step.stage];
    draft.nodes.add(block);
    draft.tones[block] = toneOf(step);
    const place = placeOf(step, places);
    if (place) {
      draft.nodes.add(place);
      draft.tones[place] = toneOf(step);
      const link = `${place}-${block}`;
      if (!readers.includes(link)) {
        readers.push(link);
      }
    }
  }

  // The chain carries the message from the first block it reached to the
  // last one, so every link between two reached stages counts as walked.
  const walked = CHAIN.filter((id) => draft.nodes.has(id));
  for (let index = 1; index < walked.length; index += 1) {
    const link = `${walked[index - 1]}-${walked[index]}`;
    if (CHAIN_LINKS.includes(link)) {
      draft.edges.push(link);
    }
  }

  // The run the turn reached, the exit it took, or nothing when it stopped
  // inside the chain.
  let tail: string | null = null;
  if (!route.matched) {
    // The turn stopped at the stage that refused it, and the graph draws
    // one exit for that: a message that meets no intent leaves the chain.
    draft.nodes.add('refused');
    draft.tones.refused = 'error';
    // The link out of the chain belongs to the graph, so it is walked only
    // when the turn really reached the stage that link leaves.
    tail = draft.nodes.has(EXIT_LINKS.refused.from)
      ? EXIT_LINKS.refused.link
      : null;
  } else if ((preview?.reply.length ?? 0) > 0) {
    // A matched turn that answered the user rather than running the command
    // stopped at the extraction, because a value the intent needs is
    // missing. The reply carries that answer, so the exit follows from it,
    // and a turn without one would run its command.
    draft.nodes.add('asks');
    draft.tones.asks = 'warn';
    tail = draft.nodes.has(EXIT_LINKS.asks.from) ? EXIT_LINKS.asks.link : null;
  } else {
    draft.nodes.add('run');
    draft.tones.run = 'ok';
    const run = `${walked.at(-1) ?? ''}-run`;
    tail = CHAIN_LINKS.includes(run) ? run : null;
  }
  if (tail) {
    draft.edges.push(tail);
  }
  draft.edges.push(...readers);
  return close(draft);
}

/**
 * The block that stands beside one stage of the route.
 *
 * A stage that reads no model of its own lights no place, and a stage
 * that fell back names the reader it really ran rather than the reader
 * the settings picked.
 */
function placeOf(
  step: MessageRouteStepDto,
  places: RoutePlaces,
): string | null {
  if (step.stage === 'retrieve') {
    return step.reader === 'words' || step.reader === 'lexical'
      ? null
      : places.vectors;
  }
  if (step.stage === 'decide') {
    if (step.reader === 'reranker') {
      return places.reranker;
    }
    return step.reader === 'model' ? places.model : null;
  }
  if (step.stage === 'extract') {
    if (step.reader === 'spans') {
      return places.spans;
    }
    return step.reader === 'model' ? places.model : null;
  }
  return null;
}
