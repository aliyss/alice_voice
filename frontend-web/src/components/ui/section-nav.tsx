/**
 * `SectionNav` lists the sections of one surface and reports the pick.
 *
 * A surface whose sections are long shows one section at a time, so a
 * reader reaches a setting without scrolling past every other one.
 *
 * The bar follows the settings layout of GitHub: on a wide screen it is a
 * quiet column of rows beside the content, where the row in view carries
 * the accent color and a thin left bar, and every other row stays muted
 * until it is hovered. On a narrow screen the same rows become a scrollable
 * strip above the content, with the row in view kept in the accent color.
 *
 * An entry carries the state of its section, so the bar reads as a summary
 * of the surface as well as a way to move through it. The entries are
 * buttons and the chosen one carries `aria-current`, so the bar is
 * reachable with the keyboard and readable by a screen reader.
 */
import type { QRL } from '@builder.io/qwik';

import type { BadgeTone } from '~/components/ui/badge';

import { $, component$ } from '@builder.io/qwik';

import { Badge } from '~/components/ui/badge';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The state of one section. */
export interface SectionNavStatus {
  /** The short status value. */
  label: string;
  /** The tone of the value. It defaults to `neutral`. */
  tone?: BadgeTone;
}

/** One section of the surface. */
export interface SectionNavItem {
  /** A stable id. The caller names the section with it. */
  id: string;
  /** The name of the section. */
  label: string;
  /** The state of the section, or null when it has none. */
  status?: SectionNavStatus | null;
}

/** The props of `SectionNav`. */
export interface SectionNavProps {
  /** The label of the bar for a screen reader. */
  ariaLabel: string;
  /** The sections, in the order the bar shows them. */
  items: SectionNavItem[];
  /** The id of the section in view. */
  selected: string;
  /** Report the section a reader picked. */
  onSelect$: QRL<(id: string) => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

/** The look of one entry. */
function entryClasses(selected: boolean): string {
  return joinClassNames(
    'group flex shrink-0 items-center justify-between gap-2 rounded-ds-xs px-2.5 py-1.5 text-left transition-colors duration-150 ease-out',
    'lg:w-full lg:border-l-2 lg:rounded-none',
    selected
      ? 'lg:border-ds-accent-strong'
      : 'lg:border-transparent lg:hover:border-ds-line-strong',
  );
}

export const SectionNav = component$<SectionNavProps>((props) => {
  return (
    <Stack
      direction="row"
      gap="sm"
      role="navigation"
      ariaLabel={props.ariaLabel}
      class={joinClassNames(
        'shrink-0 overflow-x-auto pb-1 lg:w-56 lg:flex-col lg:gap-0.5 lg:overflow-x-visible lg:py-0 lg:pr-4',
        props.class,
      )}
    >
      {props.items.map((item) => {
        const selected = props.selected === item.id;
        return (
          <button
            key={item.id}
            type="button"
            aria-current={selected ? 'true' : undefined}
            onClick$={$(() => props.onSelect$(item.id))}
            class={entryClasses(selected)}
          >
            <Text
              block
              tone={selected ? 'accent' : 'muted'}
              weight={selected ? 'medium' : 'regular'}
              class={selected ? undefined : 'group-hover:text-ds-text'}
            >
              {item.label}
            </Text>
            {item.status ? (
              <Badge
                tone={item.status.tone ?? 'neutral'}
                label={item.status.label}
              />
            ) : null}
          </button>
        );
      })}
    </Stack>
  );
});
