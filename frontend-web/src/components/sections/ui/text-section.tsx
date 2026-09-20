/**
 * `TextSection` shows every size, tone, and weight of `Text`.
 *
 * The section renders the sample twice, once on the dark glass frame and
 * once in the plain page, because a tone has to stay readable on both.
 */
import type { TextSize, TextTone, TextWeight } from '~/components/ui/text';

import { component$ } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Card } from '~/components/ui/card';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The sizes of `Text`, from the smallest to the largest. */
const SIZES: TextSize[] = ['micro', 'hud', 'body', 'lead', 'title', 'display'];

/** The tones of `Text`. */
const TONES: TextTone[] = [
  'default',
  'muted',
  'faint',
  'accent',
  'ok',
  'warn',
  'error',
];

/** The weights of `Text`. */
const WEIGHTS: TextWeight[] = ['regular', 'medium', 'semibold'];

export const TextSection = component$(() => {
  return (
    <Card label="Text">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Size"
          note="micro and hud use the display font and upper case"
        >
          <Stack gap="sm" class="w-full">
            {SIZES.map((size) => (
              <Stack key={size} direction="row" gap="md" align="baseline">
                <Text size="micro" tone="faint" class="w-[76px] shrink-0">
                  {size}
                </Text>
                <Text size={size}>The daemon is listening</Text>
              </Stack>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock title="Tone">
          <Stack gap="sm" class="w-full">
            {TONES.map((tone) => (
              <Stack key={tone} direction="row" gap="md" align="baseline">
                <Text size="micro" tone="faint" class="w-[76px] shrink-0">
                  {tone}
                </Text>
                <Text size="body" tone={tone}>
                  The daemon is listening
                </Text>
              </Stack>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock title="Weight and font">
          <Stack direction="row" gap="lg" wrap align="baseline">
            {WEIGHTS.map((weight) => (
              <Text key={weight} size="lead" weight={weight}>
                {weight}
              </Text>
            ))}
            <Text size="lead" mono>
              body font
            </Text>
            <Text size="lead" mono>
              display font
            </Text>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
