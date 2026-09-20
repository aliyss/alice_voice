/**
 * `FeedbackSection` shows `Alert`, `Spinner`, and `EmptyState`.
 *
 * These primitives answer a pending call and a failed call, so the samples
 * use the same wording the application uses.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Alert } from '~/components/ui/alert';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { EmptyState } from '~/components/ui/empty-state';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

export const FeedbackSection = component$(() => {
  const pending = useSignal(false);

  const handleToggle$ = $(() => {
    pending.value = !pending.value;
  });

  return (
    <Card label="Alert, Spinner, EmptyState">
      <Stack gap="lg">
        <ShowcaseBlock title="Alert tone" note="the retry action is optional">
          <Stack gap="md" class="w-full">
            <Alert
              tone="error"
              title="Send failed"
              message="The daemon is not reachable."
              onRetry$={handleToggle$}
            />
            <Alert
              tone="warn"
              title="Slow link"
              message="The daemon answered after three seconds."
            />
            <Alert
              tone="info"
              title="Offline"
              message="The conversation stays on this machine until the daemon returns."
            />
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Spinner"
          note="the ring turns at the tempo of the current state"
        >
          <Spinner size="sm" label="Pending" />
          <Spinner size="md" label="Pending" />
          <Stack direction="row" gap="sm" align="center">
            <Button
              variant={pending.value ? 'solid' : 'outline'}
              size="sm"
              onClick$={handleToggle$}
            >
              {pending.value ? 'Pending' : 'Idle'}
            </Button>
            <Text size="body" tone="muted">
              {pending.value ? 'Waiting for the daemon' : 'The daemon answered'}
            </Text>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock title="EmptyState">
          <EmptyState
            title="No turns yet"
            description="The conversation with the daemon appears here. Type a message or use the wake word."
          />
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
