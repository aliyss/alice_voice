/**
 * `MetadataRow` renders one labelled field of machine metadata.
 *
 * The rows of one block form a table: the label column has a fixed width,
 * the value column takes the rest, and the two line up on the baseline, so
 * a reader can scan down the labels and still read every value. Give the
 * rows of a block a vertical padding and a hairline between them, and keep
 * the value in the text hand and not in the HUD hand: a value is meant to
 * be read, the label is meant to be skimmed.
 *
 * Use `align="start"` when the value is longer than one line, for example a
 * command.
 *
 * The live turn and the stored turn of the chat surface both report how the
 * daemon read and ran a turn, and both use this row.
 */
import { Slot, component$ } from '@builder.io/qwik';

import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `MetadataRow`. */
export interface MetadataRowProps {
  /** The name of the field, for example `Intent`. */
  label: string;
  /** Align a one line value with the label, or stack a longer value. */
  align?: 'center' | 'start';
  /** The width class of the label column. It defaults to `w-20`. */
  labelWidth?: string;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const MetadataRow = component$<MetadataRowProps>((props) => {
  const align = props.align ?? 'center';

  return (
    <Stack
      direction="row"
      gap="md"
      align={align}
      class={joinClassNames('min-w-0', props.class)}
    >
      <Text
        size="micro"
        tone="faint"
        class={joinClassNames(
          'shrink-0',
          props.labelWidth ?? 'w-20',
          align === 'center' ? '' : 'pt-0.5',
        )}
      >
        {props.label}
      </Text>
      {align === 'start' ? (
        <Stack gap="xs" class="min-w-0 flex-1">
          <Slot />
        </Stack>
      ) : (
        <Stack
          direction="row"
          gap="sm"
          align="baseline"
          wrap
          class="min-w-0 flex-1"
        >
          <Slot />
        </Stack>
      )}
    </Stack>
  );
});
