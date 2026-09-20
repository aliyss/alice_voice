/**
 * `FieldLabel` is the caption of one field.
 *
 * A field needs a name, and a name often needs a sentence: what the value
 * changes, why the choice exists, what a reader loses with one option. The
 * sentence takes room the values need, so the caption keeps it behind a
 * hint and the field reads as a name and a control.
 *
 * Use it above a field, a switch, or a group of buttons. Use `PanelGroup`
 * when several fields belong to one question and need an outline around
 * them.
 */
import { component$ } from '@builder.io/qwik';

import { InfoHint } from '~/components/ui/info-hint';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `FieldLabel`. */
export interface FieldLabelProps {
  /** The name of the field. */
  label: string;
  /** One sentence about the field, behind the hint of its name. */
  hint?: string | null;
  /** The side the hint opens to. It defaults to below the mark. */
  hintSide?: 'top' | 'bottom';
  /** Extra utility classes from the caller. */
  class?: string;
}

export const FieldLabel = component$<FieldLabelProps>((props) => {
  return (
    <Stack
      direction="row"
      gap="xs"
      align="center"
      class={joinClassNames('min-w-0', props.class)}
    >
      <Text size="micro" tone="faint">
        {props.label}
      </Text>
      <InfoHint
        label={`About ${props.label.toLowerCase()}`}
        text={props.hint}
        side={props.hintSide}
        align="left"
      />
    </Stack>
  );
});
