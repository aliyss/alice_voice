/**
 * The preferences the web surface keeps in the browser.
 *
 * A preference outlives a reload, so it lives in the store of the browser
 * rather than in a signal of one surface. The store belongs to the client:
 * the server holds none, so the stored value is read when the surface runs
 * in the browser and the first render uses the fallback of the caller. A
 * surface therefore renders the same markup on the server and on the
 * client, and follows the stored value one frame later.
 */
import type { Signal } from '@builder.io/qwik';

import { useSignal, useVisibleTask$ } from '@builder.io/qwik';

/** The preface of every key the surface stores, so the store stays readable. */
const KEY_PREFIX = 'alice.';

/** The stored form of a flag that is on. */
const ON = 'on';

/**
 * The name of the preference that reads a route field by field.
 *
 * The settings page and the chat surface both read the route of a turn, so
 * both read this one name: a user who turns the debug reading on in the
 * settings reads the stored turn of the transcript the same way.
 */
export const ROUTE_DEBUG_PREFERENCE = 'resolverLog';

/** Read one stored preference, or null when the browser holds none. */
export function readPreference(name: string): string | null {
  if (typeof window === 'undefined') {
    return null;
  }
  try {
    return window.localStorage.getItem(`${KEY_PREFIX}${name}`);
  } catch {
    return null;
  }
}

/** Store one preference. A browser that refuses the write keeps the session. */
export function storePreference(name: string, value: string): void {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(`${KEY_PREFIX}${name}`, value);
  } catch {
    // A browser that holds no store answers the same as one that is full:
    // the value of this session stands until the surface reloads.
  }
}

/**
 * Hold one flag of the user over reloads.
 *
 * The flag starts at the value of the caller, and the surface reads the
 * store of the browser when it runs on the client. A name one surface
 * writes and another reads is therefore one preference with two readers,
 * for example the debug reading of a route.
 */
export function useStoredFlag(
  name: string,
  fallback: boolean,
): Signal<boolean> {
  const flag = useSignal(fallback);
  const synced = useSignal(false);

  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ track }) => {
    const current = track(() => flag.value);
    if (!synced.value) {
      // The first run takes the stored value over the fallback. A store
      // that holds nothing leaves the fallback of the caller in place.
      synced.value = true;
      const stored = readPreference(name);
      flag.value = stored === null ? current : stored === ON;
      return;
    }
    storePreference(name, current ? ON : 'off');
  });

  return flag;
}
