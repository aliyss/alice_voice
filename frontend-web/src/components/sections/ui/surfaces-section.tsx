/**
 * `SurfacesSection` shows the layout of the design system.
 *
 * It covers the glass `Card`, the `ScrollArea`, the `Box` and `Stack`
 * layout primitives, and the `PageHeader`. The `AppLayout` is not in a
 * frame here, because it is the shell that renders this page.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { PageHeader } from '~/components/ui/page-header';
import { ScrollArea } from '~/components/ui/scroll-area';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The sample rows of the scroll region. */
const ROWS = [
  'wake detected',
  'listening started',
  'transcript final',
  'intent resolved',
];

/** The sample rows of a pickable panel. */
const PICKS = ['get weather', 'open application', 'lock screen'];

export const SurfacesSection = component$(() => {
  const chosen = useSignal(PICKS[0]);

  return (
    <Card label="Card, ScrollArea, Stack, PageHeader">
      <Stack gap="lg">
        <ShowcaseBlock title="Card tone" note="frame and label are optional">
          <Stack direction="row" gap="md" wrap class="w-full">
            <Card class="min-w-[180px] flex-1">
              <Text size="micro" tone="muted">
                default
              </Text>
            </Card>
            <Card tone="sunken" class="min-w-[180px] flex-1">
              <Text size="micro" tone="muted">
                sunken
              </Text>
            </Card>
            <Card tone="clear" class="min-w-[180px] flex-1">
              <Text size="micro" tone="muted">
                clear
              </Text>
            </Card>
            <Card frame={false} class="min-w-[180px] flex-1">
              <Text size="micro" tone="muted">
                no frame
              </Text>
            </Card>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Card as one pick"
          note="a press over the whole panel, so a list of panels is a list of choices"
        >
          <Stack gap="sm" class="w-full max-w-sm">
            {PICKS.map((pick) => (
              <Card
                key={pick}
                tone="sunken"
                frame={false}
                pickable
                picked={chosen.value === pick}
                pickLabel={`Pick ${pick}`}
                bodyClass="p-3"
                onPick$={$(() => {
                  chosen.value = pick;
                })}
              >
                <Text size="body" weight="medium">
                  {pick}
                </Text>
              </Card>
            ))}
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="ScrollArea"
          note="the region owns the scrollbar and the pin"
        >
          <ScrollArea ariaLabel="Sample log" class="h-32 w-full max-w-sm">
            <Stack gap="sm" class="p-1">
              {ROWS.map((row) => (
                <Text key={row} size="micro" tone="muted">
                  {row}
                </Text>
              ))}
            </Stack>
          </ScrollArea>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Stack direction and gap"
          note="row, column, wrap, align, and justify"
        >
          <Stack direction="row" gap="sm" wrap align="center">
            <Stack direction="row" gap="xs">
              <Text size="micro" tone="faint">
                xs
              </Text>
              <Text size="micro" tone="faint">
                xs
              </Text>
            </Stack>
            <Stack direction="row" gap="lg">
              <Text size="micro" tone="faint">
                lg
              </Text>
              <Text size="micro" tone="faint">
                lg
              </Text>
            </Stack>
            <Stack direction="column" gap="sm">
              <Text size="micro" tone="faint">
                column
              </Text>
              <Text size="micro" tone="faint">
                column
              </Text>
            </Stack>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="PageHeader"
          note="title, description, meta, and one action at the right"
        >
          <PageHeader
            title="Sample page"
            description="One sentence that explains the page."
            meta="updated 12:04:05"
          >
            <Button variant="solid" size="sm">
              Action
            </Button>
          </PageHeader>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
