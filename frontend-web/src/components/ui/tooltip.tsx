/**
 * `Tooltip` shows one short value of the content it wraps.
 *
 * The panel appears while the pointer or the focus rests on the content and
 * it takes no room in the layout, so the content stays quiet until a reader
 * asks. Use it for a value that explains what is on screen, for example how
 * long a turn took.
 *
 * The panel is placed where it fits: it opens to the side the caller asks
 * for unless that side has no room, and it shifts along the surface it
 * stands in, so a value in a scrolling transcript is never cut off.
 *
 * The value is text only, and the content stays in the markup as the
 * trigger, so a screen reader reads it as usual. A value may hold several
 * lines, which report one short fact each, for example the model behind
 * each engine of a turn. Detail with more than one field belongs in a
 * disclosure or a card, not in a tooltip.
 */
import type { NoteAlign, NoteSide } from '~/utils/note-placement';

import { $, Slot, component$, useSignal, useStore } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';
import { noteWidthOf, placeNote, rectOf, roomOf } from '~/utils/note-placement';

/** The side of the content the panel opens to. */
export type TooltipSide = NoteSide;

/** The room a pattern keeps from the edge of its surface. */
const NOTE_MARGIN = 8;
/** The height a side needs before the note may open to it. */
const NOTE_MIN_ROOM = 64;

/** The props of `Tooltip`. */
export interface TooltipProps {
  /** The value to show. An empty value shows no panel. */
  text: string | null;
  /** The side the panel opens to. It defaults to above the content. */
  side?: TooltipSide;
  /** The edge the panel hangs from. It defaults to the right edge. */
  align?: NoteAlign;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Tooltip = component$<TooltipProps>((props) => {
  const open = useSignal(false);
  const mark = useSignal<HTMLElement>();
  const note = useStore<{ side: TooltipSide; left: number; width: number }>({
    side: props.side ?? 'top',
    left: 0,
    width: 0,
  });
  const text = props.text;

  const show$ = $(() => {
    const element = mark.value;
    if (!element || !text) {
      return;
    }
    const markRect = rectOf(element);
    const placed = placeNote(markRect, roomOf(element), {
      // The panel is as wide as its longest line, and only the text knows
      // that width before the panel renders.
      width: noteWidthOf(text),
      side: props.side ?? 'top',
      align: props.align ?? 'right',
      margin: NOTE_MARGIN,
      minRoom: NOTE_MIN_ROOM,
    });
    note.side = placed.side;
    note.left = placed.left - markRect.left;
    note.width = placed.width;
    open.value = true;
  });

  return (
    <span
      ref={mark}
      class={joinClassNames('relative inline-flex', props.class)}
      tabIndex={0}
      onMouseEnter$={show$}
      onMouseLeave$={() => {
        open.value = false;
      }}
      onFocusIn$={show$}
      onFocusOut$={() => {
        open.value = false;
      }}
    >
      <Slot />

      {open.value && text ? (
        <span
          role="tooltip"
          style={{
            left: `${note.left}px`,
            width: `${note.width}px`,
          }}
          class={joinClassNames(
            // The note stands over the glass surface, so it takes the one
            // opaque fill of the palette.
            'pointer-events-none absolute z-30 rounded-ds-sm border border-ds-line-strong bg-ds-bg-solid px-2 py-1 font-ds-hud text-[10px] leading-[1.5] tracking-[0.12em] whitespace-pre-line text-ds-text-muted',
            note.side === 'bottom' ? 'top-full mt-1' : 'bottom-full mb-1',
          )}
        >
          {text}
        </span>
      ) : null}
    </span>
  );
});
