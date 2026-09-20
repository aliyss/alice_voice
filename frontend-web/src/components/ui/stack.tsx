/**
 * `Stack` places elements in a row or a column with one gap.
 *
 * Use it for every flex layout. A component does not write `flex` and
 * `gap` utilities by hand.
 */
import type { CSSProperties } from '@builder.io/qwik';

import { Slot, component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The axis of the stack. */
export type StackDirection = 'row' | 'column';

/** The gap between the children. */
export type StackGap = 'none' | 'xs' | 'sm' | 'md' | 'lg' | 'xl';

/** The cross axis alignment. */
export type StackAlign = 'start' | 'center' | 'end' | 'stretch' | 'baseline';

/** The main axis alignment. */
export type StackJustify = 'start' | 'center' | 'end' | 'between';

const DIRECTIONS: Record<StackDirection, string> = {
  row: 'flex-row',
  column: 'flex-col',
};

const GAPS: Record<StackGap, string> = {
  none: 'gap-0',
  xs: 'gap-1',
  sm: 'gap-2',
  md: 'gap-4',
  lg: 'gap-6',
  xl: 'gap-8',
};

const ALIGNMENTS: Record<StackAlign, string> = {
  start: 'items-start',
  center: 'items-center',
  end: 'items-end',
  stretch: 'items-stretch',
  baseline: 'items-baseline',
};

const JUSTIFICATIONS: Record<StackJustify, string> = {
  start: 'justify-start',
  center: 'justify-center',
  end: 'justify-end',
  between: 'justify-between',
};

/** The props of `Stack`. */
export interface StackProps {
  /** The axis. It defaults to a column. */
  direction?: StackDirection;
  /** The gap between the children. It defaults to `md`. */
  gap?: StackGap;
  /** The cross axis alignment. */
  align?: StackAlign;
  /** The main axis alignment. */
  justify?: StackJustify;
  /** Wrap the children onto more than one line. */
  wrap?: boolean;
  /** The ARIA role of the region. */
  role?: string;
  /** The label for a screen reader. It names the region. */
  ariaLabel?: string;
  /** Extra utility classes from the caller. */
  class?: string;
  /** Inline values, usually design tokens. */
  style?: CSSProperties;
}

export const Stack = component$<StackProps>((props) => {
  const classes = joinClassNames(
    'flex',
    DIRECTIONS[props.direction ?? 'column'],
    GAPS[props.gap ?? 'md'],
    ALIGNMENTS[props.align ?? 'stretch'],
    JUSTIFICATIONS[props.justify ?? 'start'],
    props.wrap ? 'flex-wrap' : undefined,
    props.class,
  );

  return (
    <div
      role={props.role}
      aria-label={props.ariaLabel}
      class={classes}
      style={props.style}
    >
      <Slot />
    </div>
  );
});
