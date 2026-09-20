/**
 * `Disclosure` hides secondary detail behind one summary row.
 *
 * The summary row is always on screen and carries the label, the short
 * value a reader needs without a click, and the state of the panel. The
 * detail is on screen only when the reader opened it, so a view can show a
 * lot of machine state without crowding the primary content.
 *
 * The primitive owns the open state. Pass `defaultOpen` to start open.
 *
 * The detail sits flush with the summary row, so the rows of the detail
 * keep the full width of the panel they report on.
 */
import { $, Slot, component$, useSignal } from '@builder.io/qwik';

import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `Disclosure`. */
export interface DisclosureProps {
  /** The name of the panel, for example `Metadata`. */
  label: string;
  /** A short value the summary shows next to the label, or null. */
  summary?: string | null;
  /** Start with the detail on screen. It defaults to closed. */
  defaultOpen?: boolean;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Disclosure = component$<DisclosureProps>((props) => {
  const open = useSignal(props.defaultOpen ?? false);

  const handleToggle$ = $(() => {
    open.value = !open.value;
  });

  return (
    <Stack gap="xs" class={joinClassNames('min-w-0', props.class)}>
      <button
        type="button"
        aria-expanded={open.value}
        onClick$={handleToggle$}
        class="group flex w-full items-center gap-2 rounded-ds-sm py-0.5 text-left transition-colors duration-150 ease-out hover:text-ds-text"
      >
        {/* A caret drawn with two borders, like the rest of the kit. */}
        <span
          aria-hidden="true"
          class={joinClassNames(
            'block size-1.5 shrink-0 border-r border-b border-current text-ds-text-faint transition-transform duration-150 ease-out',
            'group-hover:text-ds-text-muted',
            open.value ? 'rotate-45' : '-rotate-45',
          )}
        />
        <Text size="micro" tone="faint" class="group-hover:text-ds-text-muted">
          {props.label}
        </Text>
        {props.summary ? (
          <Text size="micro" tone="faint" mono>
            {props.summary}
          </Text>
        ) : null}
      </button>

      {open.value ? (
        <Stack gap="xs" role="group" ariaLabel={props.label} class="min-w-0">
          <Slot />
        </Stack>
      ) : null}
    </Stack>
  );
});
