/**
 * The live status of the daemon.
 *
 * The layout reads one snapshot over REST and provides it here. The
 * provider then opens the socket stream once and folds every event into
 * the same signal, so every view reads one source of truth.
 *
 * The socket is browser only. The subscription runs in a visible task.
 */
import type { Signal } from '@builder.io/qwik';

import type { ApiResult } from '~/types/bridge';
import type { StatusDto } from '~/types/dto';

import type { AppStatusState } from '~/utils/status';

import {
  Slot,
  component$,
  useContextProvider,
  useSignal,
  useVisibleTask$,
} from '@builder.io/qwik';
import { createContextId } from '@builder.io/qwik';
import { server$ } from '@builder.io/qwik-city';

import { BackendError, backendGet } from '~/lib/backend-client';
import { createEventStream } from '~/lib/socket-client';

import {
  applyEvent,
  createInitialStatus,
  markLinkClosed,
  mergeSnapshot,
} from '~/utils/status';

/** Refresh the daemon status — defined here so the QRL is found in dev (no cross-file dynamic import). */
const refreshStatus = server$(async (): Promise<ApiResult<StatusDto>> => {
  try {
    const status = await backendGet<StatusDto>('/api/v1/status');
    return { failed: false, data: status };
  } catch (error) {
    const msg =
      error instanceof BackendError
        ? error.message
        : 'The daemon did not answer.';
    return { failed: true, message: msg };
  }
});

/** The context identifier of the app status. */
export const appStatusContext =
  createContextId<Signal<AppStatusState>>('app-status-context');

/** The props of `AppStatusProvider`. */
export interface AppStatusProviderProps {
  /** The REST snapshot the shell painted on the server. */
  snapshot: StatusDto;
}

export const AppStatusProvider = component$<AppStatusProviderProps>((props) => {
  const status = useSignal<AppStatusState>(createInitialStatus(props.snapshot));
  useContextProvider(appStatusContext, status);

  // The socket exists in the browser only, so the subscription starts in a
  // visible task and stops in the cleanup.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ cleanup }) => {
    const stream = createEventStream({
      onOpen: async () => {
        const result = await refreshStatus();
        if (!result.failed) {
          status.value = mergeSnapshot(status.value, result.data);
        }
      },
      onEvent: (event) => {
        status.value = applyEvent(status.value, event);
      },
      onClose: () => {
        status.value = markLinkClosed(status.value);
      },
    });

    cleanup(() => stream.close());
  });

  return <Slot />;
});
