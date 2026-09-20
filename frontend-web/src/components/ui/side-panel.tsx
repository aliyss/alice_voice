/**
 * `SidePanel` holds the values of the thing a reader picked.
 *
 * A surface that edits one thing at a time -- the step of a graph, the row
 * of a list -- puts the values of the pick in a panel rather than under the
 * content, so a reader reads the thing and edits it in one view.
 *
 * On a wide screen the panel is a column beside the content: a pick shifts
 * the content and shows the values of the pick in the same view, and a
 * reader who closes the panel gives the content the whole width again. The
 * column grows with the screen, so a wide screen gives a long form the room
 * to read, and it stops at twice its narrow width, so the content keeps the
 * larger half of the surface at every size. The
 * column keeps the height of the content it stands beside, and a value list
 * longer than that height scrolls inside it, so a reader never scrolls the
 * page to reach the values of the thing they picked. On a narrow screen
 * there is no room for a column, so the panel covers the content over a
 * backdrop and a reader closes it to go back.
 *
 * The actions of the values take the footer, which stays at the end of the
 * column while the body scrolls, so a reader who changed a value at the
 * top of a long list still reaches the save without scrolling.
 *
 * The panel is a region with its own label and its own way out, so it is
 * reachable with the keyboard and readable by a screen reader at both
 * widths.
 */
import type { QRL } from '@builder.io/qwik';

import { Slot, component$ } from '@builder.io/qwik';

import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `SidePanel`. */
export interface SidePanelProps {
  /** The label of the panel for a screen reader. */
  ariaLabel: string;
  /** The name of the panel. It names what the values belong to. */
  title: string;
  /** One line about what the panel changes, or null. */
  note?: string | null;
  /** Show the panel. A panel that is closed takes no room. */
  open: boolean;
  /** Close the panel. The page defines the handle. */
  onClose$: QRL<() => void>;
  /** Give the panel a footer bar for the actions of the values. */
  footer?: boolean;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const SidePanel = component$<SidePanelProps>((props) => {
  return (
    // A closed panel keeps no room, so the content takes the whole width
    // until a pick opens the panel beside it.
    <Box
      class={joinClassNames(
        props.open
          ? 'lg:w-[clamp(22rem,30vw,44rem)] lg:min-h-0 lg:shrink-0'
          : null,
        props.class,
      )}
    >
      {props.open ? (
        <>
          {/* The backdrop turns a tap outside the panel into a close. It is
              decoration for a screen reader, because the panel carries its
              own close button. */}
          <button
            type="button"
            aria-label="Close"
            tabIndex={-1}
            onClick$={props.onClose$}
            class="fixed inset-0 z-40 cursor-default bg-ds-bg/70 lg:hidden"
          />

          <Box
            role="region"
            ariaLabel={props.ariaLabel}
            class={joinClassNames(
              'z-50 flex flex-col border-ds-line bg-ds-surface p-4',
              'fixed inset-y-0 right-0 w-[min(26rem,100%)] border-l',
              'lg:sticky lg:top-0 lg:z-auto lg:h-full lg:w-full lg:rounded-ds-md lg:border',
            )}
          >
            <Stack
              direction="row"
              gap="sm"
              align="start"
              justify="between"
              class="shrink-0"
            >
              <Stack gap="xs" class="min-w-0">
                <Text size="body" weight="medium" block>
                  {props.title}
                </Text>
                {props.note ? (
                  <Text size="micro" tone="faint" block>
                    {props.note}
                  </Text>
                ) : null}
              </Stack>
              <Button variant="quiet" size="sm" onClick$={props.onClose$}>
                Close
              </Button>
            </Stack>

            <Box class="min-h-0 flex-1 overflow-y-auto pt-4">
              <Slot />
            </Box>

            {props.footer ? (
              <Box class="shrink-0 border-t border-ds-line pt-4">
                <Slot name="footer" />
              </Box>
            ) : null}
          </Box>
        </>
      ) : null}
    </Box>
  );
});
