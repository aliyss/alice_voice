/**
 * `QueueToggleSection` is the queue feature control of the settings page.
 *
 * It shows the current toggle value as a badge and offers one action to
 * flip the toggle. The page holds the pending flag and the failure text,
 * so this section stays pure. The sentence about what the toggle changes
 * and the route it calls wait in hints, because the card holds one value
 * and one action.
 */
import type { QRL } from '@builder.io/qwik';

import { component$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { PanelGroup } from '~/components/ui/panel-group';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The props of `QueueToggleSection`. */
export interface QueueToggleSectionProps {
  /** Whether the queue is on. */
  enabled: boolean;
  /** True while the toggle request is in flight. */
  pending: boolean;
  /** The last failure message, or null. */
  error: string | null;
  /**
   * True when the queue setting cannot take effect.
   *
   * The queue is a database setting, so a database that does not answer
   * makes the toggle useless.
   */
  blocked: boolean;
  /** Why the setting cannot take effect, or null. */
  blockedReason: string | null;
  /** Flip the toggle. The page defines the handle. */
  onToggle$: QRL<(next: boolean) => void>;
}

export const QueueToggleSection = component$<QueueToggleSectionProps>(
  (props) => {
    return (
      <Card class="w-full">
        <Stack gap="md">
          <Stack direction="row" gap="md" align="center" justify="between" wrap>
            <Stack gap="xs" class="min-w-0 flex-1">
              <Text size="body" weight="medium">
                Message queue
              </Text>
              <Text size="hud" tone="faint">
                A turn is stored in the database before the daemon answers it.
              </Text>
            </Stack>
            <Badge
              tone={props.enabled ? 'ok' : 'neutral'}
              label={props.enabled ? 'On' : 'Off'}
            />
          </Stack>

          {props.error ? (
            <Alert tone="error" title="Save failed" message={props.error} />
          ) : null}

          {props.blocked && props.blockedReason ? (
            <Alert
              tone="warn"
              title="Cannot be changed"
              message={props.blockedReason}
            />
          ) : null}

          <PanelGroup
            title="The store of every turn"
            hint="With the queue on, the daemon stores each turn in the database and moves it out of the queue once it is handled. With the queue off, the daemon replies without a store."
          >
            <Button
              variant={props.enabled ? 'outline' : 'solid'}
              size="md"
              disabled={props.pending || props.blocked}
              onClick$={() => props.onToggle$(!props.enabled)}
            >
              {props.pending
                ? 'Saving...'
                : props.enabled
                  ? 'Turn off'
                  : 'Turn on'}
            </Button>
          </PanelGroup>
        </Stack>
      </Card>
    );
  },
);
