/**
 * `InfoHint` is the small note of the design system.
 *
 * A value often needs a sentence more than it needs a label: what a reader
 * changes, what a file costs, why a choice exists. The sentence takes room
 * that the values themselves need, so the hint keeps it out of the way and
 * shows it while the pointer or the focus rests on the mark, and a press
 * opens it for good on a screen without a pointer.
 *
 * The note is placed where it fits: it opens to the side the caller asks for
 * unless that side has no room, and it shifts along the surface it stands
 * in, so a mark near the edge of a narrow panel takes a note inside the
 * panel rather than one that is cut off.
 *
 * Use the hint for one short fact. A paragraph that explains a whole
 * surface belongs in the documentation of that surface, and a hint that
 * long is read by nobody.
 */
import type { NoteAlign, NoteSide } from '~/utils/note-placement';

import { $, component$, useSignal, useStore } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';
import { placeNote, rectOf, roomOf } from '~/utils/note-placement';

/** The side the note opens to. */
export type InfoHintSide = NoteSide;

/** The edge of the mark the note hangs from. */
export type InfoHintAlign = NoteAlign;

/** The width the note wants. */
const NOTE_W = 224;
/** The room a pattern keeps from the edge of its surface. */
const NOTE_MARGIN = 8;
/** The height a side needs before the note may open to it. */
const NOTE_MIN_ROOM = 64;

/** The props of `InfoHint`. */
export interface InfoHintProps {
  /** The label for a screen reader. It names the value the note is about. */
  label: string;
  /** The note. An empty note draws no mark. */
  text: string | null | undefined;
  /** The side the note opens to. It defaults to below the mark. */
  side?: InfoHintSide;
  /**
   * The edge the note hangs from. It defaults to the right edge, so the
   * note grows to the left. A mark near the left edge of a narrow surface
   * takes `left`, and the note grows into the room the surface has.
   */
  align?: InfoHintAlign;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const InfoHint = component$<InfoHintProps>((props) => {
  const open = useSignal(false);
  const mark = useSignal<HTMLElement>();
  const note = useStore<{
    side: InfoHintSide;
    left: number;
    width: number;
  }>({
    side: props.side ?? 'bottom',
    left: 0,
    width: NOTE_W,
  });

  // The room a note has is a question about the layout, so it is read when
  // the note opens and never on the server.
  const show$ = $(() => {
    const element = mark.value;
    if (!element) {
      return;
    }
    const markRect = rectOf(element);
    const placed = placeNote(markRect, roomOf(element), {
      width: NOTE_W,
      side: props.side ?? 'bottom',
      align: props.align ?? 'right',
      margin: NOTE_MARGIN,
      minRoom: NOTE_MIN_ROOM,
    });
    note.side = placed.side;
    note.left = placed.left - markRect.left;
    note.width = placed.width;
    open.value = true;
  });

  if (!props.text) {
    return null;
  }

  return (
    <span
      ref={mark}
      role="button"
      tabIndex={0}
      aria-label={props.label}
      class={joinClassNames(
        'relative inline-flex size-4 shrink-0 cursor-help items-center justify-center rounded-ds-full border border-ds-line font-ds-hud text-[9px] leading-none text-ds-text-faint select-none hover:border-ds-line-strong hover:text-ds-text-muted',
        props.class,
      )}
      onMouseEnter$={show$}
      onMouseLeave$={() => {
        open.value = false;
      }}
      onFocusIn$={show$}
      onFocusOut$={() => {
        open.value = false;
      }}
      onClick$={() => {
        if (open.value) {
          open.value = false;
          return;
        }
        void show$();
      }}
    >
      ?
      {open.value ? (
        <span
          role="tooltip"
          style={{
            left: `${note.left}px`,
            width: `${note.width}px`,
          }}
          class={joinClassNames(
            // The note stands over the glass surface, so it takes the one
            // opaque fill of the palette: a reader has to be able to read it
            // against whatever stands behind it.
            'pointer-events-none absolute z-30 rounded-ds-sm border border-ds-line-strong bg-ds-bg-solid px-2.5 py-1.5 text-left font-ds-body text-[11px] leading-[1.5] font-normal tracking-normal text-ds-text-muted normal-case',
            note.side === 'top' ? 'bottom-full mb-1' : 'top-full mt-1',
          )}
        >
          {props.text}
        </span>
      ) : null}
    </span>
  );
});
