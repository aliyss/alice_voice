/**
 * `ShowcaseBlock` holds one sample of the catalog.
 *
 * It draws the caption, an optional note, and a sunken frame around the
 * sample. The frame is dark on purpose, so a sample that only works on a
 * bright backdrop is easy to spot.
 */
import { Slot, component$ } from '@builder.io/qwik';

import { Card } from '~/components/ui/card';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The props of `ShowcaseBlock`. */
export interface ShowcaseBlockProps {
  /** The name of the sample. */
  title: string;
  /** The note under the name. It holds the states the sample shows. */
  note?: string;
}

export const ShowcaseBlock = component$<ShowcaseBlockProps>((props) => {
  return (
    <Stack gap="sm">
      <Stack gap="xs">
        <Text size="micro" tone="muted">
          {props.title}
        </Text>
        {props.note ? (
          <Text size="micro" tone="faint">
            {props.note}
          </Text>
        ) : null}
      </Stack>
      <Card
        tone="sunken"
        frame={false}
        bodyClass="flex flex-wrap items-center gap-4 p-4"
      >
        <Slot />
      </Card>
    </Stack>
  );
});
