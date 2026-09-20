import type { NoteBounds, NoteRequest } from '~/utils/note-placement';

import { describe, expect, it } from 'vitest';

import { noteWidthOf, placeNote } from '~/utils/note-placement';

/** The room of a wide window, so only the mark decides the note. */
const WINDOW: NoteBounds = { left: 0, right: 1200, top: 0, bottom: 800 };

/** The request of a note that wants the width of the hint. */
const REQUEST: NoteRequest = {
  width: 224,
  side: 'bottom',
  align: 'right',
  margin: 8,
  minRoom: 64,
};

/** A mark at one place, with its own size. */
function mark(left: number, top: number) {
  return { left, right: left + 16, top, bottom: top + 16 };
}

describe('placeNote', () => {
  it('opens to the side the caller asked for when it has room', () => {
    const placed = placeNote(mark(600, 300), WINDOW, REQUEST);
    expect(placed.side).toBe('bottom');
  });

  it('opens to the other side when the asked side is at the edge', () => {
    const placed = placeNote(mark(600, 780), WINDOW, REQUEST);
    expect(placed.side).toBe('top');
  });

  it('keeps the note inside the surface it stands in', () => {
    const panel: NoteBounds = { left: 400, right: 640, top: 0, bottom: 800 };
    const placed = placeNote(mark(620, 300), panel, REQUEST);
    // The note hangs from the right edge of the mark, and the panel is
    // narrower than the note wants, so it takes the room of the panel.
    expect(placed.width).toBe(224);
    expect(placed.left).toBe(408);
    expect(placed.left + placed.width).toBeLessThanOrEqual(632);
  });

  it('shrinks the note when the surface is narrower than it wants', () => {
    const narrow: NoteBounds = { left: 100, right: 260, top: 0, bottom: 800 };
    const placed = placeNote(mark(120, 300), narrow, REQUEST);
    expect(placed.width).toBe(144);
    expect(placed.left).toBe(108);
  });

  it('hangs the note from the left edge when the caller asks for it', () => {
    const placed = placeNote(mark(600, 300), WINDOW, {
      ...REQUEST,
      align: 'left',
    });
    expect(placed.left).toBe(600);
  });
});

describe('noteWidthOf', () => {
  it('reads the width of the longest line', () => {
    expect(noteWidthOf('bge-small\nresolver: router')).toBeGreaterThan(
      noteWidthOf('bge-small'),
    );
  });

  it('adds the padding of the note', () => {
    expect(noteWidthOf('')).toBe(18);
  });
});
