/**
 * `AuraSection` shows the aura running through every state of the daemon,
 * with every state also standing still below it.
 *
 * The aura is a WebGL2 shader, so this section cannot read a state out of
 * the document the way `TokensSection` does. It reads the table the shader
 * reads, `~/utils/aura`, and prints the numbers of the state at the top.
 *
 * The state at the top is picked at random rather than shown as a row of
 * stills, because a still of each state can show what a state looks like but
 * not what it does: how one state becomes the next. It holds each state for
 * `AURA_STATE_HOLD` and lets `~/utils/aura-motion` carry it into the next
 * one, so the move is the same one the shell makes.
 */
import type { AuraState } from '~/utils/status';

import { component$, useSignal, useVisibleTask$ } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { Aura } from '~/components/viz/aura';

import { AURA_BY_STATE } from '~/utils/aura';
import { AURA_STATES, AURA_STATE_LABELS } from '~/utils/status';

/** How long one state is held before the aura moves to another, in ms. */
const AURA_STATE_HOLD = 5000;

/** Pick any state but the one on screen, so every move is a visible move. */
function anotherState(current: AuraState): AuraState {
  const others = AURA_STATES.filter((state) => state !== current);
  return others[Math.floor(Math.random() * others.length)];
}

export const AuraSection = component$(() => {
  const state = useSignal<AuraState>('idle');

  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ cleanup }) => {
    const timer = setInterval(() => {
      state.value = anotherState(state.value);
    }, AURA_STATE_HOLD);
    cleanup(() => clearInterval(timer));
  });

  const look = AURA_BY_STATE[state.value];

  return (
    <Card label="Aura">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Every state"
          note="the presence of the daemon: one state at a time at the top, and every state at rest below it"
        >
          <Stack gap="xl">
            <Stack direction="row" gap="xl" align="center" wrap>
              <Box class="w-[240px] shrink-0">
                <Aura state={state.value} />
              </Box>

              <Stack gap="xs">
                <Text size="micro" tone="muted">
                  {`${AURA_STATE_LABELS[state.value]} · ${state.value}`}
                </Text>
                <Text size="micro" tone="faint">
                  {`tone ${look.tone}`}
                </Text>
                <Text size="micro" tone="faint">
                  {`speed ${look.speed} · gain ${look.gain}`}
                </Text>
                <Text size="micro" tone="faint">
                  {`scale ${look.scale} · blur ${look.blur} · shift ${look.shift}`}
                </Text>
                <Text size="micro" tone="faint">
                  {`breath ${look.pulse.period}s · ${Math.round(
                    look.pulse.depth * 100,
                  )}%`}
                </Text>
              </Stack>
            </Stack>

            <Stack direction="row" gap="lg" wrap>
              {AURA_STATES.map((item) => (
                <Stack key={item} gap="xs" align="center" class="w-[124px]">
                  <Box class="w-[116px]">
                    <Aura state={item} />
                  </Box>
                  <Text size="micro" tone="muted">
                    {AURA_STATE_LABELS[item]}
                  </Text>
                  <Text size="micro" tone="faint">
                    {item}
                  </Text>
                </Stack>
              ))}
            </Stack>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
