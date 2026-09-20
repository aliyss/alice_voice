/**
 * `ButtonsSection` shows every variant and size of `Button`.
 *
 * The section counts the clicks on the primary sample, so the action of
 * the primitive is easy to see.
 */
import type { ButtonSize, ButtonVariant } from '~/components/ui/button';

import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The variants of `Button`. */
const VARIANTS: ButtonVariant[] = ['solid', 'outline', 'quiet'];

/** The sizes of `Button`. */
const SIZES: ButtonSize[] = ['sm', 'md', 'lg'];

export const ButtonsSection = component$(() => {
  const clicks = useSignal(0);

  const handleCount$ = $(() => {
    clicks.value += 1;
  });

  return (
    <Card label="Button">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Variant and size"
          note="one solid button per view, at the rightmost position"
        >
          <Stack gap="md" class="w-full">
            {VARIANTS.map((variant) => (
              <Stack key={variant} direction="row" gap="md" align="center" wrap>
                <Text size="micro" tone="faint" class="w-[76px] shrink-0">
                  {variant}
                </Text>
                {SIZES.map((size) => (
                  <Button key={size} variant={variant} size={size}>
                    {size}
                  </Button>
                ))}
              </Stack>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="State"
          note="hover, pressed, and focus come from the primitive"
        >
          <Button variant="solid">Default</Button>
          <Button variant="solid" disabled>
            Disabled
          </Button>
          <Button variant="outline" disabled>
            Disabled
          </Button>
          <Button variant="quiet" disabled>
            Disabled
          </Button>
        </ShowcaseBlock>

        <ShowcaseBlock title="Action and width">
          <Stack gap="md" class="w-full max-w-sm">
            <Button variant="solid" fullWidth onClick$={handleCount$}>
              {`Clicks ${clicks.value}`}
            </Button>
            <Button variant="outline" fullWidth>
              Full width
            </Button>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
