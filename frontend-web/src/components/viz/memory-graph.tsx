/**
 * `MemoryGraph` draws the memory of the daemon as a cloud of points.
 *
 * One concept is one point. A line joins two concepts when a fact of one
 * names the other, so a reader sees what the memory learned together
 * rather than a list of keys: a concept whose fact reads `brother of Ada`
 * reaches the concept of Ada, and a fact that carries a plain value
 * carries no line.
 *
 * The points take a sunflower: the place of a point follows from the
 * count of the points, so the same memory always draws the same cloud and
 * the daemon draws it without a layout pass. Every point keeps its own
 * room at its radius, which is what a ring of points does not do, so a
 * memory of a dozen concepts reads as a cloud and not as a crowd.
 *
 * The cloud is read only of the data it is given. It reports the concept
 * a reader picks, and the caller opens that concept beside it. Every
 * point is a button with the name of its concept, so the cloud is
 * reachable with the keyboard and readable by a screen reader at any
 * size.
 */
import type { QRL } from '@builder.io/qwik';

import type { MemoryNodeDto } from '~/types/dto';

import { $, component$ } from '@builder.io/qwik';

import { Box } from '~/components/ui/box';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The most points one cloud draws. The rest are named in the list. */
const MAX_POINTS = 40;

/**
 * The number of concepts a cloud names beside its points.
 *
 * A label needs room, and the cloud gives the room of a point only. A
 * small memory therefore names every concept and a large one names the
 * concept a reader picked, so the cloud stays readable at both sizes.
 */
const LABEL_UNTIL = 12;

/** The angle that spreads the points of a sunflower. */
const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

/**
 * The share of the radius a point keeps from the edge.
 *
 * The middle of the cloud is the most crowded part of a sunflower, so the
 * points are moved back from the edge rather than packed against it.
 */
const CLOUD_INSET = 0.06;

/** The longest name the cloud writes beside a point. */
const LABEL_CHARS = 18;

/** One point of the cloud, in a share of the box from 0 to 1. */
interface GraphPoint {
  /** The concept the point stands for. */
  node: MemoryNodeDto;
  /** The share of the width, from the left edge. */
  x: number;
  /** The share of the height, from the top edge. */
  y: number;
}

/** One line of the cloud. */
interface GraphLink {
  /** A stable key. */
  id: string;
  /** The point the line leaves. */
  from: GraphPoint;
  /** The point the line reaches. */
  to: GraphPoint;
}

/**
 * Place the points of the cloud.
 *
 * The points take a sunflower: point `n` sits at the angle `n` golden
 * turns from the first, at the radius of `sqrt(n)`. The square root is
 * what keeps every point its own room: the ring that holds `n` points has
 * the room of `n`, so the room of a point grows with its radius and the
 * points never crowd the middle.
 */
function placePoints(nodes: MemoryNodeDto[]): GraphPoint[] {
  const count = nodes.length;
  return nodes.map((node, index) => {
    const radius =
      count <= 1
        ? 0
        : (Math.sqrt((index + 0.5) / count) * (1 - CLOUD_INSET)) / 2;
    const angle = index * GOLDEN_ANGLE;
    return {
      node,
      x: 0.5 + radius * Math.cos(angle),
      y: 0.5 + radius * Math.sin(angle),
    };
  });
}

/**
 * Read the lines of the cloud.
 *
 * A line joins two concepts when a fact of one carries the key or the
 * name of the other, read without case. The value of a fact is a value
 * the memory learned, so a line is a guess the memory itself made, and a
 * fact that carries a plain value draws nothing.
 */
function readLinks(points: GraphPoint[]): GraphLink[] {
  const byName = new Map<string, GraphPoint>();
  for (const point of points) {
    const key = point.node.key.trim().toLowerCase();
    const title = point.node.title.trim().toLowerCase();
    if (key.length > 0) {
      byName.set(key, point);
    }
    if (title.length > 0) {
      byName.set(title, point);
    }
  }

  const links: GraphLink[] = [];
  for (const point of points) {
    for (const fact of point.node.facts) {
      if (!fact.current) {
        continue;
      }
      const to = byName.get(fact.value.trim().toLowerCase());
      if (!to || to.node.id === point.node.id) {
        continue;
      }
      links.push({ id: fact.id, from: point, to });
    }
  }
  return links;
}

/** Write the name of a concept in the room of a label. */
function shortTitle(title: string): string {
  return title.length > LABEL_CHARS
    ? `${title.slice(0, LABEL_CHARS - 1)}…`
    : title;
}

/** The props of `MemoryGraph`. */
export interface MemoryGraphProps {
  /** The concepts of the memory, newest first. */
  nodes: MemoryNodeDto[];
  /** The identifier of the picked concept, or null. */
  selected: string | null;
  /** The label for a screen reader. It names the cloud. */
  ariaLabel: string;
  /** Report the concept a reader picked. */
  onPick$: QRL<(id: string) => void>;
}

export const MemoryGraph = component$<MemoryGraphProps>((props) => {
  const drawn = props.nodes.slice(0, MAX_POINTS);
  const points = placePoints(drawn);
  const links = readLinks(points);
  const named = points.length <= LABEL_UNTIL;

  if (points.length === 0) {
    return (
      <Text size="hud" tone="faint" block>
        The memory holds no concept yet. A turn that meets no intent teaches the
        daemon the first one.
      </Text>
    );
  }

  return (
    // The cloud is a square, because a sunflower keeps its shape only if
    // the two axes carry the same room. The points are placed by a share
    // of that square, so the drawing and the buttons never drift apart.
    <Box
      role="group"
      ariaLabel={props.ariaLabel}
      class="relative mx-auto aspect-square w-full max-w-72"
    >
      <svg
        aria-hidden="true"
        viewBox="0 0 100 100"
        preserveAspectRatio="none"
        class="absolute inset-0 size-full overflow-visible"
      >
        {links.map((link) => {
          const picked =
            link.from.node.id === props.selected ||
            link.to.node.id === props.selected;
          return (
            <line
              key={link.id}
              x1={link.from.x * 100}
              y1={link.from.y * 100}
              x2={link.to.x * 100}
              y2={link.to.y * 100}
              stroke="currentColor"
              stroke-width={picked ? 0.6 : 0.3}
              class={joinClassNames(
                picked ? 'text-ds-accent-strong' : 'text-ds-line-strong',
                props.selected && !picked ? 'opacity-40' : null,
              )}
            />
          );
        })}
      </svg>

      {points.map((point) => {
        const picked = point.node.id === props.selected;
        return (
          <button
            key={point.node.id}
            type="button"
            aria-pressed={picked}
            aria-label={point.node.title}
            title={point.node.title}
            onClick$={$(() => props.onPick$(point.node.id))}
            class="absolute flex size-6 -translate-x-1/2 -translate-y-1/2 items-center justify-center rounded-ds-full"
            style={{
              left: `${point.x * 100}%`,
              top: `${point.y * 100}%`,
            }}
          >
            <span
              aria-hidden="true"
              class={joinClassNames(
                'size-2.5 rounded-ds-full border transition-colors duration-150 ease-out',
                picked
                  ? 'border-ds-accent-strong bg-ds-accent-strong'
                  : 'border-ds-line-strong bg-ds-surface hover:border-ds-accent-strong',
                props.selected && !picked ? 'opacity-60' : null,
              )}
            />
          </button>
        );
      })}

      {named
        ? points.map((point) => (
            <Box
              key={`${point.node.id}-name`}
              ariaHidden
              class="pointer-events-none absolute -translate-x-1/2 translate-y-[0.5rem]"
              style={{
                left: `${point.x * 100}%`,
                top: `${point.y * 100}%`,
              }}
            >
              <Text
                size="hud"
                tone={point.node.id === props.selected ? 'default' : 'faint'}
                class="whitespace-nowrap"
              >
                {shortTitle(point.node.title)}
              </Text>
            </Box>
          ))
        : null}
    </Box>
  );
});
