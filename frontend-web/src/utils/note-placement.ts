/**
 * Where the note of a hint or a tooltip stands.
 *
 * A note floats over the glass surface, so the surface it stands in decides
 * how much room it has: a panel scrolls and clips what leaves it, and the
 * window ends at its own edge. The note therefore opens to the side the
 * caller asked for, takes the other side when that one has no room, and
 * shifts along the surface until it fits, so a note is never cut off.
 *
 * The rule is pure: it reads the two rectangles and returns where the note
 * goes. The caller reads the rectangles of the mark and of the surface with
 * `roomOf`, and the two stay apart, so the rule is testable on its own.
 */

/** The rectangle of a mark or of a note, in window coordinates. */
export interface NoteRect {
  /** The left edge. */
  left: number;
  /** The right edge. */
  right: number;
  /** The top edge. */
  top: number;
  /** The bottom edge. */
  bottom: number;
}

/** The key of the room a note may take. */
export interface NoteBounds {
  /** The left edge of the room. */
  left: number;
  /** The right edge of the room. */
  right: number;
  /** The top edge of the room. */
  top: number;
  /** The bottom edge of the room. */
  bottom: number;
}

/** The side a note opens to. */
export type NoteSide = 'top' | 'bottom';

/** The edge of the mark the note hangs from. */
export type NoteAlign = 'left' | 'right';

/** What the caller asks of a note. */
export interface NoteRequest {
  /** The width the note wants. It shrinks to the room it has. */
  width: number;
  /** The side the caller prefers. */
  side: NoteSide;
  /** The edge the note hangs from. */
  align: NoteAlign;
  /** The room the note keeps from the edge of the surface. */
  margin: number;
  /** The room a side needs before the note may open to it. */
  minRoom: number;
}

/** Where the note of a mark stands. */
export interface NotePlacement {
  /** The side the note opens to. */
  side: NoteSide;
  /** The left edge of the note, in window coordinates. */
  left: number;
  /** The width of the note. */
  width: number;
}

/**
 * Place the note of one mark.
 *
 * The vertical side is a choice between the two sides of the mark: the
 * caller's side wins unless it has too little room and the other side has
 * more. The horizontal place keeps the note inside the surface, so a mark
 * near the edge of a narrow panel takes a note that shifts towards the
 * middle rather than one that leaves the panel.
 */
export function placeNote(
  mark: NoteRect,
  room: NoteBounds,
  request: NoteRequest,
): NotePlacement {
  const available = room.right - room.left - 2 * request.margin;
  const width = Math.max(0, Math.min(request.width, available));

  const roomOf = (side: NoteSide): number =>
    side === 'bottom' ? room.bottom - mark.bottom : mark.top - room.top;
  const other: NoteSide = request.side === 'top' ? 'bottom' : 'top';
  const side =
    roomOf(request.side) < request.minRoom &&
    roomOf(other) > roomOf(request.side)
      ? other
      : request.side;

  const anchored = request.align === 'left' ? mark.left : mark.right - width;
  const lowest = room.left + request.margin;
  const highest = room.right - width - request.margin;
  const left =
    highest < lowest ? lowest : Math.min(Math.max(anchored, lowest), highest);

  return { side, left, width };
}

/**
 * Read the room a note of one element may take.
 *
 * The room is the smallest surface that clips it: the closest ancestor that
 * hides what leaves it, and the window. A note that stays inside that room is
 * never cut off, however the surface scrolls.
 */
export function roomOf(element: Element): NoteBounds {
  const windowed = {
    left: 0,
    right: window.innerWidth,
    top: 0,
    bottom: window.innerHeight,
  };
  let parent: Element | null = element.parentElement;
  while (parent) {
    const style = window.getComputedStyle(parent);
    const clips =
      style.overflowX !== 'visible' || style.overflowY !== 'visible';
    if (clips) {
      const rect = parent.getBoundingClientRect();
      return {
        left: Math.max(windowed.left, rect.left),
        right: Math.min(windowed.right, rect.right),
        top: Math.max(windowed.top, rect.top),
        bottom: Math.min(windowed.bottom, rect.bottom),
      };
    }
    parent = parent.parentElement;
  }

  return windowed;
}

/** The rectangle of one element in window coordinates. */
export function rectOf(element: Element): NoteRect {
  const rect = element.getBoundingClientRect();
  return {
    left: rect.left,
    right: rect.right,
    top: rect.top,
    bottom: rect.bottom,
  };
}

/**
 * Read the width a note of one text needs.
 *
 * The note of a tooltip is as wide as the longest line it holds, and its
 * width is not known before it renders. The face is the monospaced HUD face,
 * so the count of the characters is the width it takes, and the padding and
 * the border are added on top.
 */
export function noteWidthOf(text: string): number {
  const longest = text
    .split('\n')
    .reduce((widest, line) => Math.max(widest, line.length), 0);
  return Math.ceil(longest * 6.2) + 18;
}
