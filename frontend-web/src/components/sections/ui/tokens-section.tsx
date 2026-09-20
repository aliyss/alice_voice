/**
 * `TokensSection` prints the design tokens and their computed values.
 *
 * The section reads the value from the document instead of the token
 * file, so a typo in a token is visible at once. The swatch takes the
 * value as an inline background, because the point of the block is to
 * show the token itself and not a second copy of it.
 */
import {
  component$,
  useContext,
  useSignal,
  useVisibleTask$,
} from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { displayContext } from '~/context/display.context';

/** The color tokens of the interface. */
const COLOR_TOKENS = [
  '--ds-color-bg',
  '--ds-color-bg-solid',
  '--ds-color-surface',
  '--ds-color-surface-raised',
  '--ds-color-surface-sunken',
  '--ds-color-line',
  '--ds-color-line-strong',
  '--ds-color-text',
  '--ds-color-text-muted',
  '--ds-color-text-faint',
  '--ds-color-accent',
  '--ds-color-accent-strong',
  '--ds-color-accent-deep',
  '--ds-color-ok',
  '--ds-color-warn',
  '--ds-color-error',
];

/** The radius tokens. */
const RADIUS_TOKENS = [
  '--ds-radius-sm',
  '--ds-radius-md',
  '--ds-radius-lg',
  '--ds-radius-full',
];

/** The duration tokens. */
const MOTION_TOKENS = [
  '--ds-duration-fast',
  '--ds-duration-base',
  '--ds-duration-slow',
];

/** The space tokens. They serve the global rules, not the utilities. */
const SPACE_TOKENS = [
  '--ds-space-2xs',
  '--ds-space-xs',
  '--ds-space-sm',
  '--ds-space-md',
  '--ds-space-lg',
  '--ds-space-xl',
];

/** Every token the section reads. */
const ALL_TOKENS = [
  ...COLOR_TOKENS,
  ...RADIUS_TOKENS,
  ...MOTION_TOKENS,
  ...SPACE_TOKENS,
];

export const TokensSection = component$(() => {
  const backdrop = useContext(displayContext);
  const values = useSignal<Record<string, string>>({});

  // The tokens are plain CSS, so the section reads them from the document.
  // The work is browser only.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(() => {
    const style = getComputedStyle(document.documentElement);
    const next: Record<string, string> = {};
    for (const name of ALL_TOKENS) {
      next[name] = style.getPropertyValue(name).trim();
    }
    values.value = next;
  });

  return (
    <Card label="Tokens">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Backdrop"
          note="the display context writes data-backdrop on the document element"
        >
          <Text size="micro" tone="muted">
            {`data-backdrop "${backdrop.value}" → --ds-color-bg "${values.value['--ds-color-bg'] ?? ''}"`}
          </Text>
        </ShowcaseBlock>

        <ShowcaseBlock title="Color" note="the swatch takes the computed value">
          <Stack gap="sm" class="w-full">
            {COLOR_TOKENS.map((name) => (
              <Stack key={name} direction="row" gap="md" align="center">
                <Box
                  class="size-6 shrink-0 rounded-ds-sm border border-ds-line"
                  style={{
                    backgroundColor: values.value[name] ?? 'transparent',
                  }}
                />
                <Text size="micro" tone="muted" class="w-[220px] shrink-0">
                  {name}
                </Text>
                <Text size="micro" tone="faint">
                  {values.value[name] ?? ''}
                </Text>
              </Stack>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock title="Radius">
          <Stack direction="row" gap="lg" wrap align="center">
            {RADIUS_TOKENS.map((name) => (
              <Stack key={name} gap="xs" align="center">
                <Box
                  class="size-14 border border-ds-line-strong bg-ds-surface"
                  style={{ borderRadius: values.value[name] ?? '0' }}
                />
                <Text size="micro" tone="faint">
                  {name}
                </Text>
              </Stack>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Space"
          note="components use the Tailwind numeric scale instead"
        >
          <Stack gap="sm" class="w-full">
            {SPACE_TOKENS.map((name) => (
              <Stack key={name} direction="row" gap="md" align="center">
                <Text size="micro" tone="muted" class="w-[220px] shrink-0">
                  {name}
                </Text>
                <Box
                  class="h-3 rounded-ds-sm bg-ds-accent"
                  style={{ width: values.value[name] ?? '0' }}
                />
                <Text size="micro" tone="faint">
                  {values.value[name] ?? ''}
                </Text>
              </Stack>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock title="Motion">
          <Stack direction="row" gap="lg" wrap align="center">
            {MOTION_TOKENS.map((name) => (
              <Stack key={name} gap="xs" align="center">
                <Text size="micro" tone="muted">
                  {values.value[name] ?? ''}
                </Text>
                <Text size="micro" tone="faint">
                  {name}
                </Text>
              </Stack>
            ))}
            <Stack gap="xs" align="center">
              <Text size="body" mono>
                Aa 0123 listening
              </Text>
              <Text size="micro" tone="faint">
                --ds-font-hud
              </Text>
            </Stack>
            <Stack gap="xs" align="center">
              <Text size="body">Aa 0123 listening</Text>
              <Text size="micro" tone="faint">
                --ds-font-body
              </Text>
            </Stack>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
