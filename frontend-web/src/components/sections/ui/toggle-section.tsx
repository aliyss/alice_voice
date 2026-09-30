/**
 * `ToggleSection` shows the toggle switch of the design system.
 *
 * `ToggleSwitch` turns one setting on or off, so the sample reads as one
 * control that carries its state in the color of the track and in the
 * place of the knob. The sample also shows a disabled switch, because a
 * value whose place does not answer cannot be flipped.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Card } from '~/components/ui/card';
import { FieldLabel } from '~/components/ui/field-label';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { ToggleSwitch } from '~/components/ui/toggle-switch';

export const ToggleSection = component$(() => {
  const context = useSignal(true);

  return (
    <Card label="ToggleSwitch">
      <Stack gap="lg">
        <ShowcaseBlock
          title="ToggleSwitch"
          note="one value on or off, the track takes the color and the knob moves"
        >
          <Stack gap="md" class="w-full">
            <Stack gap="sm">
              <FieldLabel
                label="Context"
                hint="Whether the daemon reads the earlier turns of a conversation as the context of a message."
              />
              <ToggleSwitch
                label="Context"
                hint="Read the earlier turns as the context of the message."
                checked={context.value}
                onChange$={$((next: boolean) => {
                  context.value = next;
                })}
              />
            </Stack>
            <Text size="hud" tone="faint">
              {`The switch is ${context.value ? 'on' : 'off'}.`}
            </Text>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Disabled"
          note="a value whose place does not answer cannot be flipped"
        >
          <Stack gap="md" wrap>
            <ToggleSwitch
              label="On"
              checked
              disabled
              onChange$={$((): void => {
                /* The sample cannot change. */
              })}
            />
            <ToggleSwitch
              label="Off"
              checked={false}
              disabled
              onChange$={$((): void => {
                /* The sample cannot change. */
              })}
            />
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
