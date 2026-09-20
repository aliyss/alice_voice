/**
 * `PanelGroup` is one group of the values of a panel.
 *
 * A panel holds the values of one thing, and those values answer several
 * questions: which reader, how many, from where. A group names one of
 * those questions, keeps its values together behind an outline of its own,
 * so two groups read as two blocks rather than as one long list.
 *
 * Use it inside the body of a `SidePanel`, where the room is narrow and
 * the values of two questions would otherwise run into one another. The
 * sentence about a group waits in the hint of its name, so the panel shows
 * the values and not the prose around them. A group whose values sit in a
 * card of their own (`ModelRow`) keeps the card as the inner block and the
 * group as the outline around it.
 */
import type { CSSProperties } from '@builder.io/qwik';

import { Slot, component$ } from '@builder.io/qwik';

import { InfoHint } from '~/components/ui/info-hint';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `PanelGroup`. */
export interface PanelGroupProps {
  /** The name of the group. It stands above the values. */
  title?: string;
  /** One sentence about the group, behind the hint of its name. */
  hint?: string | null;
  /** The side the hint opens to. It defaults to below the mark. */
  hintSide?: 'top' | 'bottom';
  /** Extra utility classes from the caller. */
  class?: string;
  /** Inline values, usually design tokens. */
  style?: CSSProperties;
}

export const PanelGroup = component$<PanelGroupProps>((props) => {
  return (
    <div
      class={joinClassNames(
        'flex flex-col gap-3 rounded-ds-md border border-ds-line p-3',
        props.class,
      )}
      style={props.style}
    >
      {props.title ? (
        <Stack direction="row" gap="xs" align="center" class="min-w-0">
          <Text size="micro" tone="muted" weight="medium">
            {props.title}
          </Text>
          <InfoHint
            label={`About ${props.title.toLowerCase()}`}
            text={props.hint}
            side={props.hintSide}
            align="left"
          />
        </Stack>
      ) : null}

      <Slot />
    </div>
  );
});
