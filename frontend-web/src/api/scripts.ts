/**
 * The script endpoints of the daemon.
 *
 * A message that no intent matched may be answered with a shell script the
 * model wrote. The daemon runs no script of its own accord: it stores the
 * script and waits for the decision of the user. These functions read the
 * stored script back, run one the user approved, and close one the user
 * denied. The daemon owns the decision, so a page that closes before the
 * answer still leaves a decision behind.
 */
import type { ApiResult } from '~/types/bridge';
import type { ScriptDto } from '~/types/dto';

import { server$ } from '@builder.io/qwik-city';

import { BackendError, backendFetch, backendGet } from '~/lib/backend-client';

/** The path of the script endpoints. */
const SCRIPTS_PATH = '/api/v1/scripts';

/** The reply of the script functions. */
export type ScriptResult = ApiResult<ScriptDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/** Read one stored script by its identifier. */
export const getScript = server$(async (id: string): Promise<ScriptResult> => {
  try {
    const script = await backendGet<ScriptDto>(`${SCRIPTS_PATH}/${id}`);
    return { failed: false, data: script };
  } catch (error) {
    return { failed: true, message: toFailureMessage(error) };
  }
});

/**
 * Approve one script and let the daemon run it.
 *
 * The answer carries the stored script after the run, so the page reads
 * the decision the daemon made rather than the one it asked for.
 */
export const approveScript = server$(
  async (id: string): Promise<ScriptResult> => {
    try {
      const script = await backendFetch<ScriptDto>(
        `${SCRIPTS_PATH}/${id}/approve`,
        { method: 'POST' },
      );
      return { failed: false, data: script };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Deny one script, so the daemon never runs it. */
export const denyScript = server$(async (id: string): Promise<ScriptResult> => {
  try {
    const script = await backendFetch<ScriptDto>(`${SCRIPTS_PATH}/${id}/deny`, {
      method: 'POST',
    });
    return { failed: false, data: script };
  } catch (error) {
    return { failed: true, message: toFailureMessage(error) };
  }
});
