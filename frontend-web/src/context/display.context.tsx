/**
 * The display preference of the shell.
 *
 * The interface runs over a transparent backdrop by default, because a
 * HUD is usually composited over something else. The user can switch to
 * a solid backdrop when the page stands alone in a browser tab.
 *
 * The provider writes the choice to the `data-backdrop` attribute of the
 * document element. The tokens in `src/styles/tokens.css` react to it,
 * so `--ds-color-bg` becomes opaque and every `bg-ds-bg` follows.
 */
import type { Signal } from '@builder.io/qwik';

import {
  Slot,
  component$,
  useContextProvider,
  useSignal,
  useVisibleTask$,
} from '@builder.io/qwik';
import { createContextId } from '@builder.io/qwik';

/** The backdrop of the shell. */
export type DisplayBackdrop = 'transparent' | 'solid';

/** The context identifier of the display preference. */
export const displayContext =
  createContextId<Signal<DisplayBackdrop>>('display-context');

/**
 * Read the backdrop of the first paint.
 *
 * The default is transparent, because the shell is usually composited
 * over something else. The `backdrop` search parameter starts a page that
 * stands alone in a browser tab with a solid backdrop.
 */
function readInitialBackdrop(): DisplayBackdrop {
  if (typeof window === 'undefined') {
    return 'transparent';
  }
  const requested = new URLSearchParams(window.location.search).get('backdrop');
  return requested === 'solid' ? 'solid' : 'transparent';
}

export const DisplayProvider = component$(() => {
  const backdrop = useSignal<DisplayBackdrop>(readInitialBackdrop());
  useContextProvider(displayContext, backdrop);

  // The document element exists in the browser only, so the attribute
  // write runs in a visible task.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ track }) => {
    const value = track(() => backdrop.value);
    document.documentElement.dataset.backdrop = value;
  });

  return <Slot />;
});
