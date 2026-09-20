/**
 * `PanelSection` shows the surfaces a settings panel is built from.
 *
 * `SidePanel` holds the values of the thing a reader picked: a column
 * beside the content on a wide screen, where a pick shifts the content, and
 * a cover over it on a narrow one, where a reader closes the panel to go
 * back. `PanelGroup` groups those values behind an outline of its own, and
 * `FieldLabel` names one value and keeps its sentence behind a hint.
 */
import { $, component$, useSignal } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { FieldLabel } from '~/components/ui/field-label';
import { PanelGroup } from '~/components/ui/panel-group';
import { SidePanel } from '~/components/ui/side-panel';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The picks of the sample, in the order they are shown. */
const PICKS = ['Retrieval', 'Decision', 'Extraction'];

export const PanelSection = component$(() => {
  const open = useSignal(true);
  const chosen = useSignal(PICKS[0]);

  return (
    <Card label="SidePanel">
      <Stack gap="lg">
        <ShowcaseBlock
          title="SidePanel"
          note="a column beside the content on a wide screen, a cover over it on a narrow one"
        >
          <Stack direction="row" gap="md" class="w-full">
            <Stack
              gap="sm"
              class="min-h-[168px] flex-1 rounded-ds-sm border border-ds-line bg-ds-surface p-3"
            >
              <Text size="micro" tone="faint">
                Content
              </Text>
              <Stack direction="row" gap="xs" wrap>
                {PICKS.map((name) => (
                  <Button
                    key={name}
                    variant={chosen.value === name ? 'solid' : 'outline'}
                    size="sm"
                    onClick$={$(() => {
                      chosen.value = name;
                      open.value = true;
                    })}
                  >
                    {name}
                  </Button>
                ))}
              </Stack>
              <Text size="micro" tone="faint">
                {open.value
                  ? 'A pick fills the panel and the content gives it the column.'
                  : 'The panel is closed, so the content takes the whole width.'}
              </Text>
            </Stack>

            <SidePanel
              ariaLabel="Values of the sample step"
              title={chosen.value}
              note="the values of the step a reader picked"
              open={open.value}
              onClose$={$(() => {
                open.value = false;
              })}
            >
              <Stack gap="md">
                <PanelGroup
                  title="Reader"
                  hint="How the stage reads the values of an intent. The lists need no model, the spans use a built in model, and the model reads every value."
                >
                  <Stack gap="sm">
                    <FieldLabel
                      label="How the values are read"
                      hint="One value of a form is a switch, not a set of views, so it stands in view rather than behind a dropdown."
                    />
                    <ContentSwitcher
                      ariaLabel="How the values are read"
                      value={chosen.value.toLowerCase()}
                      options={PICKS.map((name) => ({
                        value: name.toLowerCase(),
                        label: name,
                      }))}
                      onPick$={$((value: string) => {
                        chosen.value =
                          PICKS.find((name) => name.toLowerCase() === value) ??
                          PICKS[0];
                      })}
                    />
                  </Stack>
                </PanelGroup>

                <PanelGroup
                  title="The run"
                  hint="A group of its own, so the values of one question stand apart from the values of the next."
                >
                  <Text size="body" tone="muted">
                    The values of {chosen.value} live here, so the step and its
                    values are one view.
                  </Text>
                </PanelGroup>
              </Stack>
            </SidePanel>
          </Stack>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
