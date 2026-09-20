/**
 * `Box` is the neutral container of the design system.
 *
 * It paints nothing and adds no spacing. Use it to place a background, a
 * border, or a size utility around a group of elements.
 */
import type { CSSProperties } from '@builder.io/qwik';

import { Slot, component$ } from '@builder.io/qwik';

/** The props of `Box`. */
export interface BoxProps {
  /** The ARIA role of the region. */
  role?: string;
  /** The label for a screen reader. It names the region. */
  ariaLabel?: string;
  /**
   * Hide the box from a screen reader.
   *
   * Use it for a box that carries a mark the text already says, for example
   * the numbered disc of a block of a chain: the mark is for the eye, and a
   * reader that reads it twice learns nothing.
   */
  ariaHidden?: boolean;
  /** Extra utility classes from the caller. */
  class?: string;
  /** Inline values, usually design tokens. */
  style?: CSSProperties;
}

export const Box = component$<BoxProps>((props) => {
  return (
    <div
      role={props.role}
      aria-label={props.ariaLabel}
      aria-hidden={props.ariaHidden ? 'true' : undefined}
      class={props.class}
      style={props.style}
    >
      <Slot />
    </div>
  );
});
