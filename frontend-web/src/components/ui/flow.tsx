/**
 * `FlowGraph` draws a node graph: blocks joined by drawn links.
 *
 * The caller gives every block a column and a row, which is the layering of
 * the graph rather than a place on a canvas. `layoutOf` turns that layering
 * into places: a column is as far from the one before it as the longest
 * value a link carries between them needs, and a row is as far from the row
 * below it as the values that run down the chain between them need. A step
 * that several values cross therefore breathes, and a step that none cross
 * stays close, so the graph reads as a hierarchy rather than an even grid.
 *
 * A link leaves the bottom of a block and reaches the top of the block
 * below it, or it leaves the side of a block and reaches the side of a
 * block in the next column through the gutter between the columns, so two
 * links never cross a block. The caller keeps every link forward: the
 * blocks the chain reads stand in the first column, the chain in the
 * middle, and the turns that leave it in the last one, so a reader never
 * walks a link backwards.
 *
 * The graph is a canvas. The reader drags it to move it, the wheel or the
 * controls change the zoom, and the first view fits the whole graph into
 * the room it is given. A graph wider than its column therefore needs no
 * scrollbar, and a reader who wants the detail of one block zooms in
 * rather than losing the overview.
 *
 * The graph is read only. It reports the block a reader picks, and the
 * caller shows the values of that block in a panel beside the graph.
 *
 * The blocks are buttons and the links are one SVG behind them, so the
 * whole graph is reachable with the keyboard and readable by a screen
 * reader.
 */
import type { QRL } from '@builder.io/qwik';

import type { BadgeTone } from '~/components/ui/badge';
import type { TextTone } from '~/components/ui/text';

import {
  $,
  component$,
  useSignal,
  useStore,
  useVisibleTask$,
} from '@builder.io/qwik';

import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The width of one block. */
const NODE_W = 256;
/** The smallest height of one block. */
const NODE_H = 64;
/** The room the name of a block takes. */
const NODE_TITLE_H = 22;
/** The room one line of the note of a block takes. */
const NODE_NOTE_H = 17;
/** The room a block keeps above and below its lines. */
const NODE_PAD_Y = 26;
/**
 * The width one character of a note takes.
 *
 * A note is drawn in the HUD face at 11px with its tracking, so the count of
 * its characters is the width it takes. The layout wraps the note with it to
 * learn how many lines it needs, and the block grows to hold them rather than
 * cutting the note off.
 */
const NODE_CHAR_W = 8.4;
/** The room a block keeps on its left and right. */
const NODE_PAD_X = 30;
/** The room around the whole graph. */
const PAD = 20;
/** The smallest room between two rows. */
const GAP_Y = 56;
/** The smallest room between two columns. */
const GAP_X = 128;
/** The height of one value on a link. */
const LABEL_H = 14;
/**
 * The width one character of a value takes.
 *
 * A value is drawn in the monospaced HUD face, so its width is the count of
 * its characters times this value. The layout reads it to make the room
 * between two columns hold the longest value that turns there, and to keep
 * two values apart.
 */
const LABEL_W = 8;
/**
 * The tone a state of a block takes as small text.
 *
 * A state is one short value, so it reads as text beside the note of the
 * block rather than as a badge that would push the name of the block
 * aside.
 */
const STATUS_TONES: Record<BadgeTone, TextTone> = {
  neutral: 'faint',
  accent: 'accent',
  ok: 'ok',
  warn: 'warn',
  error: 'error',
};

/** The room a value keeps between itself and the link or block beside it. */
const LABEL_PAD = 12;
/**
 * The room a value keeps under itself.
 *
 * A value sits above the link it names, so the room under it keeps the
 * arrow, the link and the block below clear of the text.
 */
const LABEL_BELOW = 8;
/** The room between two values that share the same row of the layout. */
const LABEL_GAP = 8;
/**
 * The room between two links that share the side of a block.
 *
 * Two places can read into the same step, and two links can leave a step for
 * the same place. Every one of them takes its own place on that side, so a
 * reader can follow a single arrow instead of a bundle.
 */
const ARROW_GAP = 16;
/**
 * The room between the end of a link and the block it reaches.
 *
 * The arrow head takes this room, so a value that ends at the arrow keeps
 * clear of both the arrow and the block.
 */
const ARROW_ROOM = 6;
/** The room the fit view keeps between the graph and the edge of its frame. */
const FIT_PAD = 12;
/** The smallest zoom the reader can reach. */
const MIN_ZOOM = 0.2;
/** The largest zoom the reader can reach. */
const MAX_ZOOM = 2.5;
/** How much one press of a zoom control changes the zoom. */
const ZOOM_STEP = 1.25;
/** How far a pointer travels before a press counts as a drag. */
const DRAG_PX = 4;

/** What a block stands for. */
export type FlowNodeKind = 'stage' | 'place' | 'exit';

/**
 * What a link carries.
 *
 * The two channels are drawn in two colors, because they mean two
 * different things: `flow` is the turn itself, the message that moves from
 * one stage to the next, and `setting` is the configuration of a stage,
 * which points at the place that reads it. A reader tells the shape of a
 * turn from the wiring of the settings by the color of the line alone.
 */
export type FlowChannel = 'flow' | 'setting';

/**
 * The side of a block a link leaves or reaches.
 *
 * An input sits on the left or the top and an output on the right or the
 * bottom, so the direction of a connection reads from where it touches a
 * block and not from the arrow alone. A link between two columns runs
 * right to left and a link inside one column runs bottom to top, which is
 * the two cases the layout really draws.
 */
export type FlowSide = 'left' | 'right' | 'top' | 'bottom';

/** One block of the graph. */
export interface FlowNodeSpec {
  /** A stable id. The links name it. */
  id: string;
  /** The name of the block. */
  title: string;
  /** One line about what the block does or reads. */
  detail?: string;
  /** The reader the block would run right now, or null. */
  status?: { label: string; tone?: BadgeTone } | null;
  /** What the block stands for. */
  kind: FlowNodeKind;
  /** The 0-based column, left to right. */
  column: number;
  /** The 0-based row, top to bottom. */
  row: number;
  /** The position of the block in the chain, for a stage. */
  stage?: number;
  /**
   * The connection points of the block.
   *
   * A point the caller does not give is read from the links of the graph:
   * a link that leaves a block makes an output of its channel and a link
   * that reaches one makes an input. A caller therefore declares the
   * points a block may accept rather than the ones it uses, and a point
   * with no link yet is still drawn, which is where a reader starts a
   * connection to it.
   */
  ports?: FlowPortSpec[];
  /**
   * Whether a link may be drawn from an output point of the block.
   *
   * A block that does not set it starts no connection, so only the blocks
   * the caller names take a press and the rest keep their points as marks.
   * It decides nothing unless the caller also gives `onConnect$`.
   */
  dragFrom?: boolean;
}

/** One connection point of a block. */
export interface FlowPortSpec {
  /** The channel the point carries. */
  channel: FlowChannel;
  /** The side of the block the point sits on. */
  side: FlowSide;
}

/** One link of the graph. */
export interface FlowEdgeSpec {
  /** A stable id. */
  id: string;
  /** The block the link leaves. */
  from: string;
  /** The block the link reaches. */
  to: string;
  /** The value the link carries. */
  label?: string;
  /**
   * The look of the link. A main link is the chain the message follows, a
   * branch link is a turn that leaves the chain, and a setting link is the
   * place a stage reads.
   */
  kind?: 'main' | 'branch' | 'setting';
}

/** How a block of a route reads: it answered, it stumbled, or it failed. */
export type FlowTone = 'ok' | 'warn' | 'error';

/**
 * One route of a message, read onto the graph.
 *
 * The tone of a block says how the stage the block draws ended, so a
 * reader sees a route that fell back and a route that refused apart
 * before reading a single step. A link that led to nothing the route
 * reached carries no tone, which is the same as not being in the route.
 */
export interface FlowRoute {
  /** The tone of every block the route reached, by block id. */
  nodes: Record<string, FlowTone>;
  /** The tone of every link the route followed, by link id. */
  edges: Record<string, FlowTone>;
}

/** The props of `FlowGraph`. */
export interface FlowGraphProps {
  /** The id of the graph. It names the arrow markers of the links. */
  id: string;
  /** The blocks. */
  nodes: FlowNodeSpec[];
  /** The links between the blocks. */
  edges: FlowEdgeSpec[];
  /** The block a reader picked, or null. */
  selected?: string | null;
  /**
   * The blocks and links one route reached, with the tone of every one of
   * them. Every other block and link of the graph steps back, so a reader
   * sees the way one message really went instead of the whole graph at
   * once. Null shows every block the same way.
   */
  route?: FlowRoute | null;
  /**
   * Take the height of the room the caller leaves, rather than the height
   * of the graph. A caller that gives the graph the rest of a page uses it,
   * so the pipeline fills the screen instead of standing in a box of its
   * own size.
   */
  fill?: boolean;
  /** The label for a screen reader. */
  ariaLabel: string;
  /** Report the block a reader picked. */
  onSelect$?: QRL<(id: string) => void>;
  /**
   * Report a connection the reader drew from one block to another.
   *
   * The caller decides what a connection means and whether it is allowed:
   * the graph reports the two ids and draws nothing of its own. A caller
   * that gives no handler draws no connection points to grab.
   */
  onConnect$?: QRL<(from: string, to: string) => void>;
}

/** The grid one graph lays out on. */
interface FlowLayout {
  /** The left edge of every column. */
  lefts: number[];
  /** The top edge of every row. */
  tops: number[];
  /** The height of every row. */
  heights: number[];
  /** The middle x of the gutter in front of a column. */
  gutters: number[];
  /** The width of the whole graph. */
  width: number;
  /** The height of the whole graph. */
  height: number;
}

/** The room one value needs. */
interface LabelRoom {
  /** The left edge of the value. */
  left: number;
  /** The top edge the value reads best at. */
  top: number;
  /** The rows the value shares its room with, or null. */
  band: { id: number; top: number; height: number } | null;
}

/** The drawing of one link. */
interface LinkGeometry {
  /** The path of the link. */
  path: string;
  /** Where the value of the link sits. */
  label: LabelRoom | null;
}

/** The room the graph is drawn in. */
interface FlowView {
  /** The width of the frame. */
  width: number;
  /** The height of the frame. */
  height: number;
}

/** Where the graph sits inside its frame. */
interface FlowOffset {
  /** How far the graph is moved right. */
  x: number;
  /** How far the graph is moved down. */
  y: number;
}

/** The pointer gesture that moves the graph, kept apart from the render. */
interface FlowGesture {
  /** The pointer that is down, or null. */
  pointerId: number | null;
  /** Where the pointer went down. */
  x: number;
  /** Where the pointer went down. */
  y: number;
  /** The offset the pointer took hold of. */
  originX: number;
  /** The offset the pointer took hold of. */
  originY: number;
  /** Whether the press travelled far enough to count as a drag. */
  dragging: boolean;
  /** The block a connection is being drawn from, or null. */
  connectFrom: string | null;
  /** The graph point the connection starts at. */
  connectX: number;
  connectY: number;
  /** Whether the press moved the graph, so its click is not a pick. */
  swallowed: boolean;
}

/** How many rows and columns a graph uses. */
function extentOf(nodes: FlowNodeSpec[]): {
  columns: number;
  rows: number;
} {
  if (nodes.length === 0) {
    return { columns: 1, rows: 1 };
  }
  return {
    columns: Math.max(...nodes.map((node) => node.column)) + 1,
    rows: Math.max(...nodes.map((node) => node.row)) + 1,
  };
}

/**
 * How many lines one note takes inside a block.
 *
 * The words wrap as a browser wraps them, so the count is read by walking
 * the words and folding them at the width of the block. The face of the
 * note is the monospaced HUD face, so the count of the characters is the
 * width it takes.
 */
function linesOfNote(note: string, width: number): number {
  const perLine = Math.max(1, Math.floor(width / NODE_CHAR_W));
  let lines = 1;
  let used = 0;
  for (const word of note.split(' ')) {
    const length = used === 0 ? word.length : word.length + 1;
    if (used > 0 && used + length > perLine) {
      lines += 1;
      used = word.length;
      continue;
    }
    used += length;
  }
  return lines;
}

/**
 * The height one block asks for.
 *
 * A block reads in three parts: its name, the state it would run with, and
 * the note that explains it. The state takes a line of its own, because a
 * state that runs on inside a sentence gives no overview of a column of
 * blocks, and the note wraps. The height follows from the lines the two
 * take, so the row around the block grows rather than cutting it off.
 */
function heightOf(node: FlowNodeSpec): number {
  const state = node.status ? NODE_NOTE_H : 0;
  const note = node.detail ? linesOfNote(node.detail, NODE_W - NODE_PAD_X) : 0;
  return Math.max(
    NODE_H,
    NODE_PAD_Y + NODE_TITLE_H + state + note * NODE_NOTE_H,
  );
}

/** The height of the block that stands in one row. */
function heightIn(node: FlowNodeSpec, layout: FlowLayout): number {
  return layout.heights[node.row] ?? NODE_H;
}

/**
 * Lay the blocks of one graph out.
 *
 * The layering of the caller is the input: the columns run left to right
 * and the rows top to bottom. The algorithm sizes the room between them
 * from the links that cross it, so a gap holds everything that travels
 * through it and no value ever lands on a block:
 *
 * 1. A gutter holds the longest value that ends in it on both sides of its
 *    turn, so a value sits at the end of its link with the room it needs,
 *    and it is at least as wide as `GAP_X`.
 * 2. A row is as tall as its tallest block, and its band is tall enough
 *    for every value that runs down the chain across it, at least as tall
 *    as `GAP_Y`.
 * 3. The rows take their places in order, so a band that carries more is
 *    the only one that grows.
 */
function layoutOf(nodes: FlowNodeSpec[], edges: FlowEdgeSpec[]): FlowLayout {
  const { columns, rows } = extentOf(nodes);
  const byId = new Map(nodes.map((node) => [node.id, node]));

  // A value sits at the end of its link, so the room in front of a column
  // has to hold a value on each side of its turn: the one that ends there
  // coming in, and the one that leaves. The widest value of the gutter
  // therefore sizes both halves of it.
  const gutterNeed = Array.from({ length: columns }, () => 0);
  const bandNeed = Array.from({ length: Math.max(rows - 1, 0) }, () => 0);
  for (const edge of edges) {
    const from = byId.get(edge.from);
    const to = byId.get(edge.to);
    if (!from || !to || !edge.label) {
      continue;
    }
    const width = edge.label.length * LABEL_W + LABEL_PAD;
    if (from.column === to.column) {
      const band = Math.min(from.row, to.row);
      bandNeed[band] = Math.max(
        bandNeed[band] ?? 0,
        LABEL_H + LABEL_PAD + LABEL_BELOW,
      );
    } else {
      const column = Math.max(from.column, to.column);
      gutterNeed[column] = Math.max(
        gutterNeed[column] ?? 0,
        2 * (width + ARROW_ROOM),
      );
    }
  }

  const lefts: number[] = [PAD];
  const gutters: number[] = [0];
  for (let column = 1; column < columns; column += 1) {
    const gap = Math.max(GAP_X, gutterNeed[column] ?? 0);
    lefts.push(lefts[column - 1] + NODE_W + gap);
    gutters.push(lefts[column] - gap / 2);
  }

  // Every block of one row stands on the same line, so the row is as tall
  // as the tallest block in it and the blocks of that row share the height.
  const heights = Array.from({ length: rows }, (_, row) =>
    nodes
      .filter((node) => node.row === row)
      .reduce((tallest, node) => Math.max(tallest, heightOf(node)), NODE_H),
  );

  const tops: number[] = [];
  let y = PAD;
  for (let row = 0; row < rows; row += 1) {
    tops.push(y);
    const band = row < rows - 1 ? Math.max(GAP_Y, bandNeed[row] ?? 0) : 0;
    y += heights[row] + band;
  }

  return {
    lefts,
    tops,
    heights,
    gutters,
    width: lefts[columns - 1] + NODE_W + PAD,
    height: tops[rows - 1] + heights[rows - 1] + PAD,
  };
}

/** The place of a block on the grid. */
function place(
  node: FlowNodeSpec,
  layout: FlowLayout,
): { left: number; top: number } {
  return {
    left: layout.lefts[node.column] ?? PAD,
    top: layout.tops[node.row] ?? PAD,
  };
}

/** Add one link to the group of links that share a side of a block. */
function groupAnchor(
  groups: Map<string, { key: string; order: number }[]>,
  side: string,
  key: string,
  order: number,
): void {
  const list = groups.get(side) ?? [];
  list.push({ key, order });
  groups.set(side, list);
}

/**
 * Spread the links that share a side of a block.
 *
 * The links are grouped by the block and the side they touch, ordered by
 * the row of the far end, which keeps two links from crossing more than
 * they have to, and then given a place around the middle of that side. The
 * result names the offset of every end of every link.
 */
function spreadOf(
  nodes: FlowNodeSpec[],
  edges: FlowEdgeSpec[],
): Map<string, number> {
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const groups = new Map<string, { key: string; order: number }[]>();
  for (const edge of edges) {
    const from = byId.get(edge.from);
    const to = byId.get(edge.to);
    if (!from || !to) {
      continue;
    }
    if (from.column === to.column) {
      groupAnchor(groups, `${from.id}:bottom`, `${edge.id}:start`, to.row);
      groupAnchor(groups, `${to.id}:top`, `${edge.id}:end`, from.row);
      continue;
    }
    const forward = from.column < to.column;
    groupAnchor(
      groups,
      `${from.id}:${forward ? 'right' : 'left'}`,
      `${edge.id}:start`,
      to.row,
    );
    groupAnchor(
      groups,
      `${to.id}:${forward ? 'left' : 'right'}`,
      `${edge.id}:end`,
      from.row,
    );
  }

  const offsets = new Map<string, number>();
  for (const group of groups.values()) {
    group.sort((a, b) => a.order - b.order || a.key.localeCompare(b.key));
    group.forEach((entry, index) => {
      offsets.set(entry.key, (index - (group.length - 1) / 2) * ARROW_GAP);
    });
  }
  return offsets;
}

/** Where the two ends of one link touch their blocks. */
interface LinkEnds {
  /** The end at the block the link leaves. */
  start: { x: number; y: number };
  /** The end at the block the link reaches. */
  end: { x: number; y: number };
}

/**
 * Read the two ends of one link.
 *
 * A link that runs down a column keeps the middle of the column and takes
 * its own place along it. A link between two columns touches the middle of
 * the side of a block, moved by the place the link has on that side.
 */
function endsOf(
  from: FlowNodeSpec,
  to: FlowNodeSpec,
  layout: FlowLayout,
  away: number,
  toward: number,
): LinkEnds {
  const a = place(from, layout);
  const b = place(to, layout);
  const aH = heightIn(from, layout);
  const bH = heightIn(to, layout);
  if (from.column === to.column) {
    return {
      start: { x: a.left + NODE_W / 2 + away, y: a.top + aH },
      end: { x: b.left + NODE_W / 2 + toward, y: b.top - ARROW_ROOM },
    };
  }
  const forward = from.column < to.column;
  return {
    start: {
      x: forward ? a.left + NODE_W : a.left,
      y: a.top + aH / 2 + away,
    },
    end: {
      x: forward ? b.left - ARROW_ROOM : b.left + NODE_W + ARROW_ROOM,
      y: b.top + bH / 2 + toward,
    },
  };
}

/**
 * Read the drawing of one link.
 *
 * A link inside one column is a straight line through the middle of the
 * column and its value belongs to the band between the two rows. A link
 * between two columns turns inside the gutter between them and its value
 * ends at the side of the block it reaches.
 *
 * A value sits at the end of its link, just before the arrow, because that
 * is where a reader looks to see what a link carries. The layout gave the
 * gutter the room for it, so a value never lands on the link it names or
 * on a block.
 */
function linkOf(
  from: FlowNodeSpec,
  to: FlowNodeSpec,
  layout: FlowLayout,
  chars: number,
  away: number,
  toward: number,
): LinkGeometry {
  const { start, end } = endsOf(from, to, layout, away, toward);
  const width = chars * LABEL_W;
  if (from.column === to.column) {
    const bandTop = layout.tops[from.row] + heightIn(from, layout);
    const height = layout.tops[to.row] - bandTop;
    return {
      path: `M ${start.x} ${start.y} L ${end.x} ${end.y}`,
      label: {
        left: end.x + LABEL_PAD,
        top: end.y - LABEL_H - LABEL_BELOW,
        band: { id: Math.min(from.row, to.row), top: bandTop, height },
      },
    };
  }

  const forward = from.column < to.column;
  const gutter = layout.gutters[Math.max(from.column, to.column)] ?? PAD;
  return {
    path: `M ${start.x} ${start.y} H ${gutter} V ${end.y} H ${end.x}`,
    label: {
      // A value that comes in ends at the arrow, and a value that leaves
      // starts at it, so a reader reads the value where the link lands.
      left: forward ? end.x - width - LABEL_PAD : end.x + LABEL_PAD,
      top: end.y - LABEL_H - LABEL_BELOW,
      band: null,
    },
  };
}

/** A rectangle on the graph, for the rule that keeps two values apart. */
interface LabelRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** Whether two rectangles sit on each other, with a margin between them. */
function onEachOther(a: LabelRect, b: LabelRect): boolean {
  return (
    a.left < b.right + 4 &&
    a.right + 4 > b.left &&
    a.top < b.bottom + 4 &&
    a.bottom + 4 > b.top
  );
}

/** A value of a link, placed. */
interface PlacedLabel {
  /** A stable key. */
  id: string;
  /** The value. */
  label: string;
  /** The left edge. */
  left: number;
  /** The top edge. */
  top: number;
}

/** The border a tone paints. */
const TONE_BORDERS: Record<FlowTone, string> = {
  ok: 'border-ds-ok',
  warn: 'border-ds-warn',
  error: 'border-ds-error',
};

/** The color a tone paints a link with. */
const TONE_COLORS: Record<FlowTone, string> = {
  ok: 'text-ds-ok',
  warn: 'text-ds-warn',
  error: 'text-ds-error',
};

/**
 * The color of one channel a link carries.
 *
 * The turn takes the neutral line and the settings take the accent, so the
 * two read apart at a glance and neither one borrows the color the tones
 * of a route use for answered, stumbled, and failed.
 */
const CHANNEL_COLORS: Record<FlowChannel, string> = {
  flow: 'text-ds-line-strong',
  setting: 'text-ds-accent',
};

/** The color of the point of one channel. */
const PORT_COLORS: Record<FlowChannel, string> = {
  flow: 'bg-ds-line-strong',
  setting: 'bg-ds-accent',
};

/** The dashes of one channel, so the color is not the only difference. */
const CHANNEL_DASHES: Record<FlowChannel, string | undefined> = {
  flow: undefined,
  setting: '2 3',
};

/** Read the channel of one link. */
function channelOf(edge: FlowEdgeSpec): FlowChannel {
  return edge.kind === 'setting' ? 'setting' : 'flow';
}

/**
 * Read the connection points of one block.
 *
 * A caller that declares the points gets the ones it declared, so a block
 * shows the connection it may accept before that connection exists. A
 * caller that declares none gets the points its links already use, so the
 * graph needs no second description of what it draws.
 */
function portsOf(
  node: FlowNodeSpec,
  nodes: FlowNodeSpec[],
  edges: FlowEdgeSpec[],
): FlowPortSpec[] {
  if (node.ports && node.ports.length > 0) {
    return node.ports;
  }
  const used: FlowPortSpec[] = [];
  for (const edge of edges) {
    const from = nodes.find((other) => other.id === edge.from);
    const to = nodes.find((other) => other.id === edge.to);
    if (!from || !to) {
      continue;
    }
    const sides = sidesOf(from, to);
    if (edge.from === node.id) {
      used.push({ channel: channelOf(edge), side: sides.out });
    }
    if (edge.to === node.id) {
      used.push({ channel: channelOf(edge), side: sides.in });
    }
  }
  return used.filter(
    (port, index) =>
      used.findIndex(
        (other) => other.channel === port.channel && other.side === port.side,
      ) === index,
  );
}

/**
 * Read the two sides of one link.
 *
 * This mirrors the layout: a link that stays inside a column runs down it,
 * so it leaves the bottom of a block and reaches the top of the next one,
 * and a link that crosses the columns runs sideways.
 */
function sidesOf(
  from: FlowNodeSpec,
  to: FlowNodeSpec,
): {
  out: FlowSide;
  in: FlowSide;
} {
  if (from.column === to.column) {
    return { out: 'bottom', in: 'top' };
  }
  return from.column < to.column
    ? { out: 'right', in: 'left' }
    : { out: 'left', in: 'right' };
}

/**
 * Place the point of one block along the side it sits on.
 *
 * Two points of the same side spread along the side rather than stack on
 * one another, so a stage that reads a setting and takes a turn at once
 * shows both of them.
 */
function portOffset(
  port: FlowPortSpec,
  node: FlowNodeSpec,
  nodes: FlowNodeSpec[],
  edges: FlowEdgeSpec[],
): string {
  const side = portsOf(node, nodes, edges).filter(
    (other) => other.side === port.side,
  );
  const at = side.findIndex((other) => other.channel === port.channel);
  return `${((at + 1) / (side.length + 1)) * 100}%`;
}

/** The placement of the point of one channel on one side of a block. */
function portPlacement(
  port: FlowPortSpec,
  node: FlowNodeSpec,
  nodes: FlowNodeSpec[],
  edges: FlowEdgeSpec[],
): { className: string; style: Record<string, string> } {
  const at = portOffset(port, node, nodes, edges);
  switch (port.side) {
    case 'left':
      return { className: 'left-0 -translate-x-1/2', style: { top: at } };
    case 'right':
      return { className: 'right-0 translate-x-1/2', style: { top: at } };
    case 'top':
      return { className: 'top-0 -translate-y-1/2', style: { left: at } };
    case 'bottom':
      return { className: 'bottom-0 translate-y-1/2', style: { left: at } };
    default:
      return { className: '', style: {} };
  }
}

/**
 * Read one browser point as a point of the graph.
 *
 * The graph is drawn through one transform, so a point of the frame has to
 * have the pan taken off it and the zoom divided out before it means
 * anything to the layout. The room is the box of the frame on the screen.
 */
function toGraph(
  clientX: number,
  clientY: number,
  room: DOMRect | undefined,
  offset: FlowOffset,
  scale: number,
): { x: number; y: number } {
  if (!room) {
    return { x: 0, y: 0 };
  }
  return {
    x: (clientX - room.left - offset.x) / scale,
    y: (clientY - room.top - offset.y) / scale,
  };
}

/** The block under one point of the graph, or null. */
function nodeAt(
  point: { x: number; y: number },
  nodes: FlowNodeSpec[],
  layout: FlowLayout,
): FlowNodeSpec | null {
  for (const node of nodes) {
    const at = place(node, layout);
    const room = heightIn(node, layout);
    if (
      point.x >= at.left &&
      point.x <= at.left + NODE_W &&
      point.y >= at.top &&
      point.y <= at.top + room
    ) {
      return node;
    }
  }
  return null;
}

/** The graph point of one connection point of one block. */
function portPoint(
  node: FlowNodeSpec,
  port: FlowPortSpec,
  nodes: FlowNodeSpec[],
  edges: FlowEdgeSpec[],
  layout: FlowLayout,
): { x: number; y: number } {
  const at = place(node, layout);
  const room = heightIn(node, layout);
  const side = portsOf(node, nodes, edges).filter(
    (other) => other.side === port.side,
  );
  const at_ = side.findIndex((other) => other.channel === port.channel);
  const along = (at_ + 1) / (side.length + 1);
  switch (port.side) {
    case 'right':
      return { x: at.left + NODE_W, y: at.top + room * along };
    case 'left':
      return { x: at.left, y: at.top + room * along };
    case 'top':
      return { x: at.left + NODE_W * along, y: at.top };
    default:
      return { x: at.left + NODE_W * along, y: at.top + room };
  }
}

/** The suffix the id of the value of a link carries. */
const LABEL_SUFFIX = '-label';

/**
 * The color of the value of one link.
 *
 * The value stands on its link, and the link already carries the color of
 * the route, so the value itself stays in the palette of text: it steps up
 * from faint to default on a link of the route. Tinting it would leave a
 * short machine value in a status color, which is hard to read at 11px and
 * says the same thing twice. The id of a value is the id of its link with
 * a suffix, so the place of the value on the route is read off the link.
 */
function labelTone(
  tones: Record<string, FlowTone> | null,
  id: string,
): TextTone {
  const link = id.endsWith(LABEL_SUFFIX)
    ? id.slice(0, -LABEL_SUFFIX.length)
    : id;
  if (!tones) {
    return 'faint';
  }
  return link in tones ? 'default' : 'faint';
}

/**
 * The look of one block.
 *
 * A block of the route takes the color of its tone, so the way a message
 * went reads on the graph itself: the stages it passed stay green, one
 * that fell back turns amber, and the stage that refused turns red. The
 * pick keeps its own mark beside the tone, so a block of the route is
 * still the block whose values are open.
 */
function nodeClasses(
  kind: FlowNodeKind,
  selected: boolean,
  tone: FlowTone | null,
): string {
  return joinClassNames(
    'absolute flex flex-col justify-center gap-1 rounded-ds-sm border px-3.5 py-2.5 text-left transition-colors duration-150 ease-out',
    kind === 'place' ? 'bg-ds-surface-sunken' : 'bg-ds-surface',
    kind === 'exit' ? 'border-dashed' : null,
    tone
      ? TONE_BORDERS[tone]
      : selected
        ? 'border-ds-accent-strong'
        : 'border-ds-line hover:border-ds-line-strong',
    // A picked block of the route keeps its tone and takes a second mark,
    // so a reader never loses the color of the route to the pick.
    selected && tone
      ? 'outline-2 outline-offset-2 outline-ds-accent-edge'
      : null,
  );
}

/**
 * Hold one axis of the graph inside its frame.
 *
 * The graph moves between the two ends of that axis and never leaves the
 * frame: a graph smaller than the frame may sit anywhere inside it, and a
 * graph larger than the frame shows its far edge at the edge of the frame
 * at worst. So a drag always moves the graph a reader can see, and never
 * throws the graph away.
 */
function holdAxis(value: number, content: number, frame: number): number {
  const span = frame - content;
  return Math.min(Math.max(span, 0), Math.max(Math.min(span, 0), value));
}

/** Hold the whole graph inside its frame. */
function holdOffset(
  offset: FlowOffset,
  scale: number,
  graph: FlowView,
  frame: FlowView,
): FlowOffset {
  return {
    x: holdAxis(offset.x, graph.width * scale, frame.width),
    y: holdAxis(offset.y, graph.height * scale, frame.height),
  };
}

/**
 * The zoom at which the whole graph is in view.
 *
 * The graph is never blown up past its own size, because the blocks read
 * best at rest, and the fit keeps the smallest zoom reachable so a tiny
 * frame still shows a graph rather than a speck.
 */
function fitOf(graph: FlowView, frame: FlowView): number {
  if (frame.width <= 0 || frame.height <= 0) {
    return 1;
  }
  const scale = Math.min(
    1,
    (frame.width - FIT_PAD * 2) / graph.width,
    (frame.height - FIT_PAD * 2) / graph.height,
  );
  return Math.max(MIN_ZOOM, scale);
}

/**
 * The room the graph reads its frame at.
 *
 * The frame is read from the layout box rather than the painted box, so a
 * zoom transform on the content never feeds back into the fit.
 */
function readView(frame: HTMLElement | undefined): FlowView {
  if (!frame) {
    return { width: 0, height: 0 };
  }
  return { width: frame.clientWidth, height: frame.clientHeight };
}

export const FlowGraph = component$<FlowGraphProps>((props) => {
  const layout = layoutOf(props.nodes, props.edges);
  const spreads = spreadOf(props.nodes, props.edges);
  // The route of one message, when the caller read one: the blocks and
  // links it reached keep their place and their tone, and every other one
  // steps back.
  const route = props.route;
  const nodeTones = route?.nodes ?? null;
  const edgeTones = route?.edges ?? null;
  const routed = route
    ? new Set([...Object.keys(route.nodes), ...Object.keys(route.edges)])
    : null;
  const byId = new Map(props.nodes.map((node) => [node.id, node]));
  const links = props.edges.flatMap((edge) => {
    const from = byId.get(edge.from);
    const to = byId.get(edge.to);
    if (!from || !to) {
      return [];
    }
    return [
      {
        edge,
        geometry: linkOf(
          from,
          to,
          layout,
          edge.label?.length ?? 0,
          spreads.get(`${edge.id}:start`) ?? 0,
          spreads.get(`${edge.id}:end`) ?? 0,
        ),
      },
    ];
  });

  // The values that run down the chain share the room between two rows, so
  // every one of them takes a place in that room rather than pushing into
  // the block below it. The room was sized for them, so they always fit,
  // and they stack upward from the block they reach, which is the end of
  // their link.
  const bands = new Map<
    number,
    { top: number; height: number; entries: PlacedLabel[] }
  >();
  const labels: PlacedLabel[] = [];
  for (const { edge, geometry } of links) {
    const label = edge.label;
    const room = geometry.label;
    if (!label || !room) {
      continue;
    }
    const entry = {
      id: `${edge.id}-label`,
      label,
      left: room.left,
      top: room.top,
    };
    if (room.band) {
      const band = bands.get(room.band.id) ?? {
        top: room.band.top,
        height: room.band.height,
        entries: [],
      };
      band.entries.push(entry);
      bands.set(room.band.id, band);
      continue;
    }
    labels.push(entry);
  }
  for (const band of bands.values()) {
    band.entries.sort((a, b) => a.top - b.top);
    const last = band.entries.length - 1;
    band.entries.forEach((entry, index) => {
      labels.push({
        ...entry,
        top:
          band.top +
          band.height -
          ARROW_ROOM -
          LABEL_H -
          4 -
          (last - index) * (LABEL_H + LABEL_GAP),
      });
    });
  }

  // A value that turns in a gutter sits beside the turn it names, and two
  // of them can claim the same place. One that lands on another moves down
  // until it has its own room, which the open gutter always offers.
  const taken: LabelRect[] = [];
  const placed = labels.map((entry) => {
    const rect: LabelRect = {
      left: entry.left,
      top: entry.top,
      right: entry.left + entry.label.length * LABEL_W,
      bottom: entry.top + LABEL_H,
    };
    let guard = 0;
    while (taken.some((other) => onEachOther(rect, other)) && guard < 16) {
      rect.top += LABEL_H + 2;
      rect.bottom += LABEL_H + 2;
      guard += 1;
    }
    taken.push(rect);
    return { ...entry, top: rect.top };
  });

  const frame = useSignal<HTMLElement>();
  const scale = useSignal(1);
  const offset = useSignal<FlowOffset>({ x: 0, y: 0 });
  // The reader took the view by hand, so a resize leaves it alone.
  const held = useSignal(false);
  const grabbing = useSignal(false);
  const gesture = useStore<FlowGesture>({
    pointerId: null,
    x: 0,
    y: 0,
    originX: 0,
    originY: 0,
    dragging: false,
    swallowed: false,
    connectFrom: null,
    connectX: 0,
    connectY: 0,
  });
  // The point the connection under the pointer reaches, in graph units.
  const connectPoint = useSignal<{ x: number; y: number }>({ x: 0, y: 0 });

  const graphView = { width: layout.width, height: layout.height };

  // The view lives in a transform on the browser, so the work is browser
  // only. The graph is drawn at rest on the server and the first paint fits
  // it, which is the only view the server could not know.
  const fit = $(() => {
    const room = readView(frame.value);
    const next = fitOf(graphView, room);
    scale.value = next;
    offset.value = holdOffset(
      {
        x: (room.width - graphView.width * next) / 2,
        y: (room.height - graphView.height * next) / 2,
      },
      next,
      graphView,
      room,
    );
  });

  const zoomAt$ = $((next: number, x: number, y: number) => {
    const room = readView(frame.value);
    const wanted = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next));
    if (wanted === scale.value) {
      return;
    }
    // The point under the pointer keeps its place, so a zoom reads as the
    // graph coming closer rather than as a jump.
    const ratio = wanted / scale.value;
    held.value = true;
    scale.value = wanted;
    offset.value = holdOffset(
      {
        x: x - (x - offset.value.x) * ratio,
        y: y - (y - offset.value.y) * ratio,
      },
      wanted,
      graphView,
      room,
    );
  });

  const handleZoomIn$ = $(() => {
    const room = readView(frame.value);
    void zoomAt$(scale.value * ZOOM_STEP, room.width / 2, room.height / 2);
  });

  const handleZoomOut$ = $(() => {
    const room = readView(frame.value);
    void zoomAt$(scale.value / ZOOM_STEP, room.width / 2, room.height / 2);
  });

  const handleFit$ = $(() => {
    held.value = false;
    fit();
  });

  /** Start drawing a connection from one block. */
  const handlePortDown$ = $(
    (event: PointerEvent, node: FlowNodeSpec, port: FlowPortSpec) => {
      if (event.pointerType === 'mouse' && event.button !== 0) {
        return;
      }
      // The press belongs to the connection and not to the pan of the graph.
      event.stopPropagation();
      const start = portPoint(node, port, props.nodes, props.edges, layout);
      gesture.connectFrom = node.id;
      gesture.connectX = start.x;
      gesture.connectY = start.y;
      connectPoint.value = toGraph(
        event.clientX,
        event.clientY,
        frame.value?.getBoundingClientRect(),
        offset.value,
        scale.value,
      );
      gesture.pointerId = event.pointerId;
      gesture.dragging = false;
      gesture.swallowed = true;
      frame.value?.setPointerCapture(event.pointerId);
    },
  );

  const handlePointerDown$ = $((event: PointerEvent) => {
    if (event.pointerType === 'mouse' && event.button !== 0) {
      return;
    }
    gesture.pointerId = event.pointerId;
    gesture.x = event.clientX;
    gesture.y = event.clientY;
    gesture.originX = offset.value.x;
    gesture.originY = offset.value.y;
    gesture.dragging = false;
    // A press that follows a drag is a new press and reads as a pick again.
    gesture.swallowed = false;
  });

  const handlePointerMove$ = $((event: PointerEvent) => {
    if (gesture.pointerId !== event.pointerId) {
      return;
    }
    // A connection under the pointer follows the pointer and never pans.
    if (gesture.connectFrom) {
      connectPoint.value = toGraph(
        event.clientX,
        event.clientY,
        frame.value?.getBoundingClientRect(),
        offset.value,
        scale.value,
      );
      return;
    }
    const dx = event.clientX - gesture.x;
    const dy = event.clientY - gesture.y;
    if (!gesture.dragging && Math.hypot(dx, dy) < DRAG_PX) {
      return;
    }
    if (!gesture.dragging) {
      gesture.dragging = true;
      grabbing.value = true;
      // The capture starts with the drag, so a plain press still reaches
      // the block under the pointer and picks it.
      frame.value?.setPointerCapture(event.pointerId);
    }
    held.value = true;
    offset.value = holdOffset(
      { x: gesture.originX + dx, y: gesture.originY + dy },
      scale.value,
      graphView,
      readView(frame.value),
    );
  });

  const handlePointerUp$ = $((event: PointerEvent) => {
    if (gesture.pointerId !== event.pointerId) {
      return;
    }
    if (gesture.connectFrom) {
      const from = gesture.connectFrom;
      const to = nodeAt(
        toGraph(
          event.clientX,
          event.clientY,
          frame.value?.getBoundingClientRect(),
          offset.value,
          scale.value,
        ),
        props.nodes,
        layout,
      );
      gesture.connectFrom = null;
      gesture.pointerId = null;
      gesture.dragging = false;
      grabbing.value = false;
      // A connection onto the block it starts at says nothing, and the
      // caller decides whether the two ends may be joined at all.
      if (to && to.id !== from) {
        void props.onConnect$?.(from, to.id);
      }
      return;
    }
    if (gesture.dragging) {
      gesture.swallowed = true;
    }
    gesture.pointerId = null;
    gesture.dragging = false;
    grabbing.value = false;
  });

  const handleSelect$ = $((id: string) => {
    if (gesture.swallowed) {
      gesture.swallowed = false;
      return;
    }
    props.onSelect$?.(id);
  });

  // The view lives in a transform on the browser, so the work is browser
  // only. The wheel is a native listener because a zoom has to stop the
  // page from scrolling under the graph, and only a listener that is not
  // passive may do that.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ cleanup }) => {
    const element = frame.value;
    if (!element) {
      return;
    }

    const handleWheel = (event: WheelEvent) => {
      event.preventDefault();
      const rect = element.getBoundingClientRect();
      void zoomAt$(
        scale.value * Math.exp(-event.deltaY * 0.0015),
        event.clientX - rect.left,
        event.clientY - rect.top,
      );
    };
    element.addEventListener('wheel', handleWheel, { passive: false });

    // Wheel zoom only - height is pure CSS min-h-[600px] with no observer.
    cleanup(() => {
      element.removeEventListener('wheel', handleWheel);
    });
  });

  return (
    <div
      ref={frame}
      role="group"
      aria-label={props.ariaLabel}
      onPointerDown$={handlePointerDown$}
      onPointerMove$={handlePointerMove$}
      onPointerUp$={handlePointerUp$}
      onPointerCancel$={handlePointerUp$}
      class={joinClassNames(
        'relative w-full touch-none overflow-hidden select-none',
        props.fill ? 'flex-1 min-h-0' : null,
        grabbing.value ? 'cursor-grabbing' : 'cursor-grab',
      )}
      style={props.fill ? undefined : { height: `${layout.height}px` }}
    >
      <div
        class="absolute top-0 left-0 origin-top-left will-change-transform"
        style={{
          width: `${layout.width}px`,
          height: `${layout.height}px`,
          transform: `translate3d(${offset.value.x}px, ${offset.value.y}px, 0) scale(${scale.value})`,
        }}
      >
        <svg
          aria-hidden="true"
          viewBox={`0 0 ${layout.width} ${layout.height}`}
          class="absolute top-0 left-0 overflow-visible"
          style={{ width: `${layout.width}px`, height: `${layout.height}px` }}
        >
          <defs>
            <marker
              id={`${props.id}-arrow`}
              viewBox="0 0 8 8"
              refX="7"
              refY="4"
              markerWidth="6"
              markerHeight="6"
              orient="auto-start-reverse"
            >
              <path d="M 0 0 L 8 4 L 0 8 z" fill="currentColor" />
            </marker>
          </defs>

          {links.map(({ edge, geometry }) => {
            const tone = edgeTones?.[edge.id] ?? null;
            return (
              <g
                key={edge.id}
                data-edge={edge.id}
                data-route={tone ?? undefined}
                class={joinClassNames(
                  // A link of the route takes the color of the block it
                  // leads into, so the way the message went reads along the
                  // link as well as on the blocks at its two ends. A link
                  // outside the route takes the color of its channel, so a
                  // reader tells the turn from the wiring of the settings
                  // before any route is read at all.
                  tone
                    ? TONE_COLORS[tone]
                    : edge.kind === 'branch'
                      ? 'text-ds-text-faint'
                      : CHANNEL_COLORS[channelOf(edge)],
                  routed && !routed.has(edge.id) ? 'opacity-25' : null,
                )}
              >
                <path
                  d={geometry.path}
                  fill="none"
                  stroke="currentColor"
                  stroke-width={tone ? 1.5 : 1}
                  stroke-dasharray={
                    edge.kind === 'branch'
                      ? '3 3'
                      : CHANNEL_DASHES[channelOf(edge)]
                  }
                  marker-end={`url(#${props.id}-arrow)`}
                />
              </g>
            );
          })}

          {gesture.connectFrom ? (
            <path
              d={`M ${gesture.connectX} ${gesture.connectY} L ${connectPoint.value.x} ${connectPoint.value.y}`}
              fill="none"
              stroke="currentColor"
              stroke-width={1.5}
              stroke-dasharray="4 3"
              class={CHANNEL_COLORS.setting}
              marker-end={`url(#${props.id}-arrow)`}
            />
          ) : null}
        </svg>

        {placed.map((entry) => (
          <Box
            key={entry.id}
            class={joinClassNames(
              'pointer-events-none absolute',
              routed && !routed.has(entry.id) ? 'opacity-25' : null,
            )}
            style={{ left: `${entry.left}px`, top: `${entry.top}px` }}
          >
            {/* The value of a link of the route carries the color of the
                link, so a reader follows one value the message carried. */}
            <Text size="hud" mono tone={labelTone(edgeTones, entry.id)}>
              {entry.label}
            </Text>
          </Box>
        ))}

        {props.nodes.map((node) => {
          const point = place(node, layout);
          const selected = props.selected === node.id;
          const tone = nodeTones?.[node.id] ?? null;
          const onRoute = routed?.has(node.id) ?? false;
          return (
            <button
              key={node.id}
              type="button"
              aria-pressed={selected}
              data-route={tone ?? undefined}
              onClick$={$(() => handleSelect$(node.id))}
              class={joinClassNames(
                nodeClasses(node.kind, selected, tone),
                routed && !onRoute ? 'opacity-40' : null,
              )}
              style={{
                left: `${point.left}px`,
                top: `${point.top}px`,
                width: `${NODE_W}px`,
                height: `${heightIn(node, layout)}px`,
              }}
            >
              {/* The name takes the first line on its own, with the room of
                  a word between it and the number of the block. */}
              <Stack direction="row" gap="sm" align="center" class="min-w-0">
                <Box
                  ariaHidden
                  class={joinClassNames(
                    'flex size-5 shrink-0 items-center justify-center rounded-ds-full border',
                    // The number of a block of the route takes the tone of
                    // the route too, so a reader finds the stage that
                    // refused by the number as well as by the border.
                    tone
                      ? TONE_BORDERS[tone]
                      : node.kind === 'stage'
                        ? 'border-ds-accent-edge bg-ds-accent-veil'
                        : 'border-ds-line',
                  )}
                >
                  {/* `leading-none` keeps the number on the middle of its
                      circle instead of on the line the text sits on. */}
                  <Text
                    size="hud"
                    mono
                    tone={tone ?? 'accent'}
                    class="leading-none"
                  >
                    {node.stage ?? '·'}
                  </Text>
                </Box>
                <Text size="body" weight="medium" block class="truncate">
                  {node.title}
                </Text>
              </Stack>

              {/* The state takes a line of its own: a reader scans a column
                  of blocks by their names and their states, and a state
                  inside a sentence is read by nobody. */}
              {node.status ? (
                <Text
                  size="hud"
                  tone={STATUS_TONES[node.status.tone ?? 'neutral']}
                  block
                  class="truncate leading-[17px]"
                >
                  {node.status.label}
                </Text>
              ) : null}

              {node.detail ? (
                <Text size="hud" tone="faint" block class="leading-[17px]">
                  {node.detail}
                </Text>
              ) : null}

              {/* The connection points of the block. An input sits on the
                  left edge and an output on the right edge, which is where
                  the links of the graph already leave and reach it, and the
                  points of one edge spread along it rather than stacking on
                  one another. */}
              {portsOf(node, props.nodes, props.edges).map((port) => {
                const point = portPlacement(
                  port,
                  node,
                  props.nodes,
                  props.edges,
                );
                // A point a connection may be drawn from takes the press,
                // and a point a link reaches is a mark only. The header
                // decides which one a point is: an output carries a
                // connection onward and an input receives one.
                const output = port.side === 'right' || port.side === 'bottom';
                const grab =
                  props.onConnect$ !== undefined &&
                  output &&
                  node.dragFrom === true;
                return (
                  <span
                    key={`${port.side}-${port.channel}`}
                    aria-hidden
                    onPointerDown$={
                      grab
                        ? $((event: PointerEvent) =>
                            handlePortDown$(event, node, port),
                          )
                        : undefined
                    }
                    class={joinClassNames(
                      'absolute rounded-full',
                      PORT_COLORS[port.channel],
                      grab ? 'size-2.5 cursor-crosshair' : 'size-1.5',
                      point.className,
                    )}
                    style={point.style}
                  />
                );
              })}
            </button>
          );
        })}
      </div>

      <div
        class="absolute right-2 bottom-2 flex gap-1"
        onPointerDown$={$((event: PointerEvent) => event.stopPropagation())}
      >
        <Button
          size="sm"
          variant="quiet"
          ariaLabel="Zoom out"
          onClick$={handleZoomOut$}
        >
          −
        </Button>
        <Button
          size="sm"
          variant="quiet"
          ariaLabel="Zoom in"
          onClick$={handleZoomIn$}
        >
          +
        </Button>
        <Button
          size="sm"
          variant="quiet"
          ariaLabel="Fit the graph"
          onClick$={handleFit$}
        >
          Fit
        </Button>
      </div>
    </div>
  );
});
