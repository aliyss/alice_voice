/**
 * `Badge` shows one status value.
 *
 * It carries a tone, a short label, and an optional indicator dot. The
 * label is machine state, so the badge uses the display font.
 */
import { component$ } from '@builder.io/qwik';

import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The tone of the badge. */
export type BadgeTone = 'neutral' | 'accent' | 'ok' | 'warn' | 'error';

const TONES: Record<BadgeTone, string> = {
  neutral: 'border-ds-line text-ds-text-muted',
  accent: 'border-ds-accent-strong text-ds-accent-strong',
  ok: 'border-ds-ok text-ds-ok',
  warn: 'border-ds-warn text-ds-warn',
  error: 'border-ds-error text-ds-error',
};

/** The props of `Badge`. */
export interface BadgeProps {
  /** The tone. It defaults to `neutral`. */
  tone?: BadgeTone;
  /** The status value. It is short and in the user language. */
  label: string;
  /** Blink the indicator dot. Use it for an active state. */
  pulse?: boolean;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Badge = component$<BadgeProps>((props) => {
  const classes = joinClassNames(
    'inline-flex items-center gap-2 rounded-ds-full border bg-ds-surface px-2.5 py-1',
    TONES[props.tone ?? 'neutral'],
    props.class,
  );

  return (
    <span class={classes}>
      <span
        aria-hidden="true"
        class={joinClassNames(
          'size-1.5 shrink-0 rounded-ds-full bg-current',
          props.pulse ? 'animate-ds-blink' : undefined,
        )}
      />
      <Text size="micro" mono tone="inherit">
        {props.label}
      </Text>
    </span>
  );
});
