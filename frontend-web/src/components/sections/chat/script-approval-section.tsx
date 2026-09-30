/**
 * `ScriptApprovalSection` shows a shell script the model wrote and asks the
 * user to decide about it.
 *
 * A message that no intent matched may be answered with a script the model
 * wrote against the commands of the machine. The daemon runs no script of
 * its own accord, so the surface reads the script, the sentence about what
 * it does, and how rough it is on the machine, and waits. The decision is
 * the only way the script ever runs.
 *
 * The rating is information and not a gate: the badge names how rough the
 * script is on the machine, and the user decides with that in view.
 */
import type { QRL } from '@builder.io/qwik';

import type { ProposedScript } from '~/utils/status';

import { component$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { MetadataRow } from '~/components/ui/metadata-row';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The props of `ScriptApprovalSection`. */
export interface ScriptApprovalSectionProps {
  /** The script the model wrote. */
  script: ProposedScript;
  /** True while the decision is on its way to the daemon. */
  pending: boolean;
  /** The last failure message of a decision, or null. */
  error: string | null;
  /** Run the script. */
  onApprove$: QRL<() => void>;
  /** Close the script without running it. */
  onDeny$: QRL<() => void>;
}

/** The tone of the badge that reports how rough a script is. */
function destructivenessTone(value: number): 'ok' | 'warn' | 'error' {
  if (value >= 67) {
    return 'error';
  }
  return value >= 34 ? 'warn' : 'ok';
}

export const ScriptApprovalSection = component$<ScriptApprovalSectionProps>(
  (props) => {
    const tone = destructivenessTone(props.script.destructiveness);
    const summary =
      props.script.summary.trim().length > 0
        ? props.script.summary
        : 'The model wrote a script for this message.';

    return (
      <Card label="Script" class="w-full shrink-0">
        <Stack gap="md">
          <MetadataRow label="What it does">
            <Text size="body" tone="default" block>
              {summary}
            </Text>
            <Badge
              tone={tone}
              label={`roughness ${props.script.destructiveness}`}
            />
          </MetadataRow>

          <Text size="body" mono tone="muted" block class="break-all">
            {props.script.script}
          </Text>

          {props.error ? (
            <Alert
              tone="error"
              title="The decision failed"
              message={props.error}
            />
          ) : null}

          <Stack direction="row" gap="sm">
            <Button
              variant="solid"
              size="md"
              disabled={props.pending}
              onClick$={props.onApprove$}
            >
              {props.pending ? 'Working...' : 'Approve and run'}
            </Button>
            <Button
              variant="outline"
              size="md"
              disabled={props.pending}
              onClick$={props.onDeny$}
            >
              Deny
            </Button>
          </Stack>
        </Stack>
      </Card>
    );
  },
);
