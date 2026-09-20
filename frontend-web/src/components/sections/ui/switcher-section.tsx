/**
 * `SwitcherSection` shows the content switcher of the design system.
 *
 * `ContentSwitcher` swaps between alternate views of the same content, so
 * the sample reads as one strip: the ends carry the rounding, the values
 * between them keep square corners, and only the chosen value is filled.
 * The sample also shows a value that cannot be picked. */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Card } from '~/components/ui/card';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { FieldLabel } from '~/components/ui/field-label';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

export const SwitcherSection = component$(() => {
  const reader = useSignal('spans');
  const device = useSignal('auto');

  return (
    <Card label="ContentSwitcher">
      <Stack gap="lg">
        <ShowcaseBlock
          title="ContentSwitcher"
          note="alternate views of the same content, one strip, one value filled"
        >
          <Stack gap="sm" class="w-full">
            <FieldLabel
              label="Reader"
              hint="How the stage reads the values of an intent. The lists need no model, the spans use a built in model, and the model reads every value."
            />
            <ContentSwitcher
              ariaLabel="Reader"
              value={reader.value}
              options={[
                { value: 'lists', label: 'Lists' },
                { value: 'spans', label: 'Spans' },
                { value: 'generative', label: 'Model' },
              ]}
              onPick$={$((value: string) => {
                reader.value = value;
              })}
            />
            <Text size="hud" tone="faint">
              {`The chosen reader is ${reader.value}. The stage shows one reader at a time, because the values replace one another.`}
            </Text>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Ends and middle"
          note="the ends are rounded, the values between them are square"
        >
          <Stack gap="md">
            <ContentSwitcher
              ariaLabel="Device"
              size="sm"
              value={device.value}
              options={[
                { value: 'auto', label: 'Auto' },
                { value: 'cpu', label: 'CPU' },
                { value: 'cuda', label: 'CUDA', disabled: true },
              ]}
              onPick$={$((value: string) => {
                device.value = value;
              })}
            />
            <ContentSwitcher
              ariaLabel="Sample switch of two values"
              value="off"
              options={[
                { value: 'on', label: 'On' },
                { value: 'off', label: 'Off' },
              ]}
              onPick$={$((): void => {
                /* The sample keeps its value. */
              })}
            />
            <ContentSwitcher
              ariaLabel="Sample switch that cannot be stored"
              disabled
              value="spans"
              options={[
                { value: 'lists', label: 'Lists' },
                { value: 'spans', label: 'Spans' },
                { value: 'generative', label: 'Model' },
              ]}
              onPick$={$((): void => {
                /* The switch is disabled while the value cannot be stored. */
              })}
            />
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
