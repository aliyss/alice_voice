/**
 * `DataSection` shows the primitives that report machine state.
 *
 * It covers the `Tooltip`, which shows one value of the content it wraps,
 * the `Disclosure`, which hides a list behind one summary row, and the
 * `MetadataRow`, which renders one labelled field of that list. They are
 * the pair a view uses to report how the daemon read a turn.
 */
import { component$ } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Badge } from '~/components/ui/badge';
import { Card } from '~/components/ui/card';
import { Disclosure } from '~/components/ui/disclosure';
import { MetadataRow } from '~/components/ui/metadata-row';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { Tooltip } from '~/components/ui/tooltip';

export const DataSection = component$(() => {
  return (
    <Card label="Tooltip, Disclosure, MetadataRow">
      <Stack gap="lg">
        <ShowcaseBlock
          title="Tooltip"
          note="the pointer or the focus opens the value, and it takes no room"
        >
          <Stack direction="row" gap="lg" align="center" class="py-6">
            <Tooltip text="412 ms" class="shrink-0">
              <Text size="micro" tone="faint">
                12:04:05
              </Text>
            </Tooltip>
            <Tooltip text="no value yet" class="shrink-0">
              <Text size="micro" tone="faint">
                12:04:05
              </Text>
            </Tooltip>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Tooltip, more than one line"
          note="one short fact per line, for a value a row cannot carry"
        >
          <Stack direction="row" gap="lg" align="center" class="py-6">
            <Tooltip
              text={'llama.cpp  qwen3.5-4b\nGLiNER  gliner_small-v2.1'}
              side="bottom"
              class="shrink-0"
            >
              <Text size="hud" tone="muted">
                llama.cpp · GLiNER
              </Text>
            </Tooltip>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Disclosure"
          note="closed by default, open on demand, one field per row"
        >
          <Stack gap="md" class="w-full max-w-sm">
            <Disclosure label="Metadata" class="w-full">
              <Stack gap="none" class="w-full divide-y divide-ds-line-faint">
                <MetadataRow label="Intent" class="py-1.5">
                  <Text size="body">get weather</Text>
                  <Badge tone="neutral" label="81%" />
                </MetadataRow>
                <MetadataRow label="Resolver" class="py-1.5">
                  <Text size="hud" tone="muted">
                    llama.cpp · GLiNER
                  </Text>
                </MetadataRow>
                <MetadataRow label="Command" align="start" class="py-1.5">
                  <Text size="hud" tone="muted" block class="break-all">
                    curl -s "wttr.in/Berlin?format=3"
                  </Text>
                </MetadataRow>
                <MetadataRow label="Exit" class="py-1.5">
                  <Text size="hud" tone="ok">
                    exit 0
                  </Text>
                </MetadataRow>
              </Stack>
            </Disclosure>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="Disclosure, a turn with no intent"
          note="the table names who read the message even without an intent"
        >
          <Stack gap="md" class="w-full max-w-sm">
            <Disclosure label="Metadata" class="w-full">
              <Stack gap="none" class="w-full divide-y divide-ds-line-faint">
                <MetadataRow label="Intent" class="py-1.5">
                  <Text size="body" tone="muted">
                    no intent matched
                  </Text>
                </MetadataRow>
                <MetadataRow label="Resolver" class="py-1.5">
                  <Tooltip text="qwen3.5-4b" side="bottom">
                    <Text size="hud" tone="muted">
                      llama.cpp
                    </Text>
                  </Tooltip>
                </MetadataRow>
              </Stack>
            </Disclosure>
          </Stack>
        </ShowcaseBlock>

        <ShowcaseBlock
          title="MetadataRow alignment"
          note="center for a one line value, start for a value that wraps"
        >
          <Stack gap="sm" class="w-full max-w-sm">
            <MetadataRow label="Intent" labelWidth="w-20">
              <Text size="body">get weather</Text>
              <Badge tone="neutral" label="81%" />
            </MetadataRow>
            <MetadataRow label="Entities" labelWidth="w-20">
              <Text size="hud">city = Berlin</Text>
            </MetadataRow>
            <MetadataRow label="Output" labelWidth="w-20" align="start">
              <Text size="hud" tone="muted" block class="break-all">
                Berlin: cloud +17 C
              </Text>
              <Text size="hud" tone="muted" block class="break-all">
                2 lines of output
              </Text>
            </MetadataRow>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
