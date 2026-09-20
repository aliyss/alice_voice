/**
 * `Spinner` shows that a call is pending.
 *
 * It is a hairline ring with one accent arc. The ring turns at the state
 * tempo, so it slows down when the daemon is idle.
 */
import { component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The size of the spinner. */
export type SpinnerSize = 'sm' | 'md';

const SIZES: Record<SpinnerSize, string> = {
  sm: 'size-3 border',
  md: 'size-4 border',
};

/** The props of `Spinner`. */
export interface SpinnerProps {
  /** The size. It defaults to `md`. */
  size?: SpinnerSize;
  /** The label for a screen reader. */
  label: string;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Spinner = component$<SpinnerProps>((props) => {
  const classes = joinClassNames(
    'inline-block shrink-0 animate-ds-spin rounded-ds-full border-ds-line border-t-ds-accent',
    SIZES[props.size ?? 'md'],
    props.class,
  );

  return <span class={classes} role="status" aria-label={props.label} />;
});
