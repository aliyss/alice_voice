/**
 * `TabsSection` shows the tabs of the design system.
 *
 * `Tabs` names the parts of one context that do not replace one another,
 * so the sample pairs the strip with a panel the way a surface does and
 * the ids of the two belong together. Use `ContentSwitcher` when the
 * values are alternate views of the same content instead.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Card } from '~/components/ui/card';
import { Stack } from '~/components/ui/stack';
import { Tabs, tabId, tabPanelId } from '~/components/ui/tabs';
import { Text } from '~/components/ui/text';

/** The views of the sample, in the order the strip shows them. */
const VIEWS = [
  { id: 'intent', label: 'Intent' },
  { id: 'command', label: 'Command' },
  { id: 'entities', label: 'Entities', status: '2' },
];

export const TabsSection = component$(() => {
  const view = useSignal(VIEWS[0].id);

  return (
    <Card label="Tabs">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Tabs"
          note="the parts of one context, each part with content of its own"
        >
          <Stack gap="none">
            <Tabs
              ariaLabel="Sample views"
              items={VIEWS}
              selected={view.value}
              onSelect$={$((id: string) => {
                view.value = id;
              })}
            />
            <div
              role="tabpanel"
              id={tabPanelId(view.value)}
              aria-labelledby={tabId(view.value)}
              class="pt-3"
            >
              <Text size="body" tone="muted">
                {`The ${view.value} view of the surface stands here, wired to the tab above it.`}
              </Text>
            </div>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Wrapping"
          note="a strip that wraps instead of scrolling on a narrow surface"
        >
          <Stack gap="none" class="max-w-xs">
            <Tabs
              ariaLabel="Sample views that wrap"
              items={VIEWS}
              selected={view.value}
              wrap
              onSelect$={$((id: string) => {
                view.value = id;
              })}
            />
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
