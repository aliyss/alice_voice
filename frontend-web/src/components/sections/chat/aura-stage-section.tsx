/**
 * `AuraStageSection` is the presence of the daemon on the page.
 *
 * The aura is the largest thing on the stage and it keeps that place when a
 * conversation starts: the voice of the daemon does not shrink because a
 * turn ran. It takes the height the stage has to give, so it stays as large
 * as the window allows and yields room while a turn runs.
 */
import type { AuraState, TurnOutcome } from '~/utils/status';

import { component$ } from '@builder.io/qwik';

import { Box } from '~/components/ui/box';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { Aura } from '~/components/viz/aura';

import { AURA_STATE_LABELS } from '~/utils/status';

/** The user language caption of each aura state. */
const STATE_CAPTIONS: Record<AuraState, string> = {
  offline: 'The daemon does not answer. Start the listening service.',
  idle: 'Say “Alice” to wake me, or type below.',
  listening: 'I am listening.',
  transcribing: 'I am writing down what I heard.',
  resolving: 'I am finding the right action.',
  executing: 'I am running the action.',
  success: 'The action ran. Say “Alice” again when you need me.',
  error: 'The last action failed. Check the transcript.',
};

/** The props of `AuraStageSection`. */
export interface AuraStageSectionProps {
  /** The visual state of the aura. */
  state: AuraState;
  /** How many turns have completed. A change starts the moment of success. */
  turns: number;
  /** What completed the last turn. */
  outcome: TurnOutcome | null;
}

export const AuraStageSection = component$<AuraStageSectionProps>((props) => {
  return (
    <Stack
      gap="sm"
      align="center"
      justify="center"
      class="min-h-0 w-full flex-1"
    >
      {/* The square takes the smaller of the space it is given and its own
          limit, so the aura never pushes the field out of the window. */}
      <Box class="aspect-square h-[min(52vh,420px)] max-h-full max-w-full transition-[height] duration-700 ease-out">
        <Aura
          state={props.state}
          turns={props.turns}
          outcome={props.outcome}
          class="size-full"
        />
      </Box>

      <Stack gap="xs" align="center" class="shrink-0">
        <Text size="micro" tone="muted">
          {AURA_STATE_LABELS[props.state]}
        </Text>
        <Text size="body" tone="faint" block class="max-w-md text-center">
          {STATE_CAPTIONS[props.state]}
        </Text>
      </Stack>
    </Stack>
  );
});
