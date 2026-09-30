/**
 * `RouteLog` reports a route field by field.
 *
 * One part of the log is one stage of the route: the reader that ran, the
 * way the stage ended, the time it took, and what it read. Every value
 * stands on a line of its own with one sentence about what the field
 * means, because a reader who tunes the pipeline compares two runs by the
 * same field rather than by the same sentence.
 *
 * The settings page reads the sentence it tried with this, and the chat
 * surface reads the route of a stored turn with it, so both report one
 * route the same way. The parts come from `debugPreview` and `debugRoute`,
 * which name the fields and the sentences.
 */
import type { PreviewBlock } from '~/utils/preview-log';

import { component$ } from '@builder.io/qwik';

import { InfoHint } from '~/components/ui/info-hint';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The props of `RouteLog`. */
export interface RouteLogProps {
  /** The parts of the log, in the order the daemon read them. */
  blocks: PreviewBlock[];
  /** Extra utility classes from the caller. */
  class?: string;
}

export const RouteLog = component$<RouteLogProps>((props) => {
  return (
    <Stack gap="sm" class={props.class}>
      {props.blocks.map((block, position) => (
        <Stack key={`${block.title}-${position}`} gap="xs">
          <Stack direction="row" gap="xs" align="center">
            <Text size="hud" mono tone={block.tone ?? 'muted'}>
              {block.title}
            </Text>
            <InfoHint
              label={`About ${block.title}`}
              text={block.hint}
              side="top"
              align="left"
            />
          </Stack>
          <Stack gap="none">
            {block.fields.map((field, at) => (
              <Stack
                key={`${field.label}-${at}`}
                direction="row"
                gap="sm"
                align="start"
                justify="between"
                class="border-t border-ds-line py-1 first:border-t-0"
              >
                <Stack direction="row" gap="xs" align="center" class="shrink-0">
                  <Text size="hud" tone="faint">
                    {field.label}
                  </Text>
                  <InfoHint
                    label={`About ${field.label}`}
                    text={field.hint}
                    side="top"
                    align="left"
                  />
                </Stack>
                <Text
                  size="hud"
                  mono
                  tone={field.tone ?? 'default'}
                  block
                  class="min-w-0 flex-1 text-right break-all"
                >
                  {field.value}
                </Text>
              </Stack>
            ))}
          </Stack>
        </Stack>
      ))}
    </Stack>
  );
});
