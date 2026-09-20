/**
 * `ScrollArea` is the scroll region of the design system.
 *
 * It owns the DOM node, so a view can keep the newest content in view
 * without touching the DOM itself. The region keeps the end in view only
 * while the reader is at the end: a reader who scrolled up on purpose
 * keeps their place when new content arrives.
 *
 * The region is focusable on purpose. The shell does not scroll the
 * document, so the keyboard reaches a region only when the region takes
 * focus. The label names the region for a screen reader.
 */
import {
  $,
  Slot,
  component$,
  useSignal,
  useVisibleTask$,
} from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** How far from the end the reader may sit and still count as at the end. */
const STICK_DISTANCE_PX = 48;

/** The props of `ScrollArea`. */
export interface ScrollAreaProps {
  /** The name of the region for a screen reader. */
  ariaLabel: string;
  /**
   * Keep the end of the content in view while the reader has not scrolled
   * away from it. A reader who scrolled up keeps their place.
   */
  stickToEnd?: boolean;
  /** Change the value to look at the end again. */
  stickKey?: number;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const ScrollArea = component$<ScrollAreaProps>((props) => {
  const viewport = useSignal<HTMLElement>();
  // The reader starts at the end, so the first render pins the newest turn.
  const atEnd = useSignal(true);

  const handleScroll$ = $((event: Event) => {
    const element = event.target as HTMLElement;
    const distance =
      element.scrollHeight - element.scrollTop - element.clientHeight;
    atEnd.value = distance <= STICK_DISTANCE_PX;
  });

  // The scroll position lives in the DOM, so the work is browser only.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ track }) => {
    track(() => props.stickKey);

    const element = viewport.value;
    if (element && props.stickToEnd && atEnd.value) {
      element.scrollTop = element.scrollHeight;
    }
  });

  return (
    <div
      ref={viewport}
      role="region"
      aria-label={props.ariaLabel}
      tabIndex={0}
      onScroll$={handleScroll$}
      class={joinClassNames(
        'min-h-0 overflow-y-auto overflow-x-hidden',
        props.class,
      )}
    >
      <Slot />
    </div>
  );
});
