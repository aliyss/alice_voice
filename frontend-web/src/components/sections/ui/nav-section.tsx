/**
 * `NavSection` shows the section bar of the design system.
 *
 * `SectionNav` lists the sections of one surface and reports the pick. An
 * entry carries the state of its section, and the chosen entry is the one
 * the surface shows, so a long surface needs no scroll to change section.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { SectionNav } from '~/components/ui/section-nav';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

export const NavSection = component$(() => {
  const selected = useSignal('resolver');

  return (
    <Card label="SectionNav">
      <Stack gap="lg">
        <ShowcaseBlock
          title="SectionNav"
          note="a row above the content on a narrow screen, a column beside it on a wide one"
        >
          <Stack direction="row" gap="md" class="w-full">
            <SectionNav
              ariaLabel="Sample sections"
              selected={selected.value}
              onSelect$={$((id: string) => {
                selected.value = id;
              })}
              items={[
                {
                  id: 'resolver',
                  label: 'Intent resolver',
                  status: { label: 'Router' },
                },
                {
                  id: 'queue',
                  label: 'Queue',
                  status: { label: 'On', tone: 'ok' },
                },
                { id: 'intents', label: 'Intents', status: { label: '40' } },
                { id: 'places', label: 'Places', status: { label: '2 up' } },
              ]}
            />
            <Box class="min-h-[132px] flex-1 rounded-ds-sm border border-ds-line bg-ds-surface p-3">
              <Text size="micro" tone="faint">
                {selected.value}
              </Text>
            </Box>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
