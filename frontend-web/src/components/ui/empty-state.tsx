/**
 * `EmptyState` fills a view that has no data yet.
 *
 * It holds one headline, one explanation, and no action. The caller adds
 * the action when the view needs one.
 */
import { component$ } from '@builder.io/qwik';

import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `EmptyState`. */
export interface EmptyStateProps {
  /** The headline in the user language. */
  title: string;
  /** The explanation in the user language. */
  description: string;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const EmptyState = component$<EmptyStateProps>((props) => {
  return (
    <div
      class={joinClassNames(
        'flex flex-col items-center justify-center gap-2 py-10 text-center',
        props.class,
      )}
    >
      <Stack gap="xs" align="center">
        <Text size="micro" tone="muted">
          {props.title}
        </Text>
        <Text size="body" tone="faint" block class="max-w-md">
          {props.description}
        </Text>
      </Stack>
    </div>
  );
});
