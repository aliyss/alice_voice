/**
 * The status endpoint of the daemon.
 *
 * The route loader reads the first snapshot. This function re-reads the
 * snapshot after the socket reconnects, so the app catches the events it
 * missed while the link was down.
 */
import type { ApiResult } from '~/types/bridge';
import type { StatusDto } from '~/types/dto';

import { server$ } from '@builder.io/qwik-city';

import { BackendError, backendGet } from '~/lib/backend-client';

/** The path of the status endpoint. */
const STATUS_PATH = '/api/v1/status';

/** The reply of the refresh status function. */
export type RefreshStatusResult = ApiResult<StatusDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/** Read the current daemon status again. */
export const refreshStatus = server$(async (): Promise<RefreshStatusResult> => {
  try {
    const status = await backendGet<StatusDto>(STATUS_PATH);
    return { failed: false, data: status };
  } catch (error) {
    return { failed: true, message: toFailureMessage(error) };
  }
});
