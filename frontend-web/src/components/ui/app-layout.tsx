/**
 * `AppLayout` is the application shell.
 *
 * It fills the viewport, paints the backdrop token, and lays a vignette
 * over the backdrop. The backdrop is transparent by default, so the
 * window behind the page stays visible. The display context switches the
 * backdrop to a solid color when the user asks for it.
 */
import { Slot, component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The props of `AppLayout`. */
export interface AppLayoutProps {
  /** Extra utility classes from the caller. */
  class?: string;
}

export const AppLayout = component$<AppLayoutProps>((props) => {
  return (
    <div
      class={joinClassNames(
        'relative isolate flex h-dvh min-h-0 w-full flex-col overflow-hidden bg-ds-bg',
        props.class,
      )}
    >
      <span
        aria-hidden="true"
        class="pointer-events-none fixed inset-0 [background-image:var(--ds-shell-vignette)]"
      />
      <div class="relative flex min-h-0 flex-1 flex-col">
        <Slot />
      </div>
    </div>
  );
});
