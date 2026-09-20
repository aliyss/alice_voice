/**
 * The application shell.
 *
 * The layout reads one daemon status snapshot over REST, then provides
 * the display preference and the live app status to the whole subtree.
 * The status strip is the only chrome of the shell.
 *
 * A missing daemon must not break the page, so the loader falls back to
 * an offline snapshot. The status strip then shows the offline state and
 * the socket client keeps trying to connect.
 */
import type { StatusDto } from '~/types/dto';

import { Slot, component$ } from '@builder.io/qwik';
import { routeLoader$ } from '@builder.io/qwik-city';

import { StatusStripSection } from '~/components/sections/status/status-strip-section';
import { AppLayout } from '~/components/ui/app-layout';

import { backendGet } from '~/lib/backend-client';

import { AppStatusProvider } from '~/context/app-status.context';
import { DisplayProvider } from '~/context/display.context';

/** The path of the daemon status endpoint. */
const STATUS_PATH = '/api/v1/status';

/** The snapshot the shell paints when the daemon does not answer. */
const OFFLINE_SNAPSHOT: StatusDto = {
  state: 'Idle',
  since: '1970-01-01T00:00:00.000Z',
  version: 'unknown',
};

/** Read the first status snapshot of the daemon. */
export const useStatusSnapshot = routeLoader$(async (): Promise<StatusDto> => {
  try {
    return await backendGet<StatusDto>(STATUS_PATH);
  } catch {
    return OFFLINE_SNAPSHOT;
  }
});

export default component$(() => {
  const snapshot = useStatusSnapshot();

  return (
    <AppLayout>
      <AppStatusProvider snapshot={snapshot.value}>
        <DisplayProvider>
          <StatusStripSection />
          <Slot />
        </DisplayProvider>
      </AppStatusProvider>
    </AppLayout>
  );
});
