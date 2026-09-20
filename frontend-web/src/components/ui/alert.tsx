/**
 * `Alert` shows a failure or a warning in the design system language.
 *
 * Use it for a backend REST error. Put the message in the user language.
 */
import type { QRL } from '@builder.io/qwik';

import { component$ } from '@builder.io/qwik';

import { Button } from '~/components/ui/button';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The tone of the alert. */
export type AlertTone = 'error' | 'warn' | 'info';

const TONES: Record<AlertTone, string> = {
  error: 'border-ds-error text-ds-error',
  warn: 'border-ds-warn text-ds-warn',
  info: 'border-ds-line text-ds-text-muted',
};

/** The props of `Alert`. */
export interface AlertProps {
  /** The tone. It defaults to `error`. */
  tone?: AlertTone;
  /** The short headline. */
  title: string;
  /** The explanation in the user language. */
  message: string;
  /** The label of the retry action. It defaults to `Retry`. */
  retryLabel?: string;
  /** The retry action. The button is hidden when it is absent. */
  onRetry$?: QRL<() => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Alert = component$<AlertProps>((props) => {
  const classes = joinClassNames(
    'rounded-ds-sm border-l-2 bg-ds-surface px-4 py-3',
    TONES[props.tone ?? 'error'],
    props.class,
  );

  return (
    <div class={classes} role="alert">
      <Stack gap="sm">
        <Text size="micro" mono tone="inherit">
          {props.title}
        </Text>
        <Text size="body" tone="muted" block>
          {props.message}
        </Text>
        {props.onRetry$ ? (
          <div class="pt-1">
            <Button size="sm" variant="outline" onClick$={props.onRetry$}>
              {props.retryLabel ?? 'Retry'}
            </Button>
          </div>
        ) : null}
      </Stack>
    </div>
  );
});
