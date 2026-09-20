/**
 * `PageHeader` opens a page with a title, a description, and optional
 * machine metadata.
 *
 * Every page uses the same header, so the hierarchy stays the same on
 * every route. The primary action of the page sits at the right edge.
 */
import { Slot, component$ } from '@builder.io/qwik';

import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `PageHeader`. */
export interface PageHeaderProps {
  /** The page title. */
  title: string;
  /** One sentence that explains the page in the user language. */
  description?: string;
  /** Machine metadata shown at the right edge. */
  meta?: string;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const PageHeader = component$<PageHeaderProps>((props) => {
  return (
    <header
      class={joinClassNames(
        'flex flex-wrap items-center justify-between gap-4',
        props.class,
      )}
    >
      <div class="flex flex-col gap-1">
        <h1 class="font-ds-body text-[20px] leading-tight font-medium tracking-[-0.01em] text-ds-text">
          {props.title}
        </h1>
        {props.description ? (
          <Text size="body" tone="muted" block>
            {props.description}
          </Text>
        ) : null}
      </div>
      <div class="flex items-center gap-3">
        {props.meta ? (
          <Text size="micro" tone="faint">
            {props.meta}
          </Text>
        ) : null}
        <Slot />
      </div>
    </header>
  );
});
