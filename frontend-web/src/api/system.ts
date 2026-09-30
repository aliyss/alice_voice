/**
 * The system endpoints of the daemon.
 *
 * `GET /api/v1/system` returns the host profile and the recommended
 * router preset. The settings page uses the recommendation to place the
 * two linked sliders at the value that fits the machine. `POST
 * /api/v1/system/preset` turns a quality value into the router
 * configuration the daemon would store, so a slider moves without
 * writing the settings until the user saves.
 */
import type { ApiResult } from '~/types/bridge';
import type { SystemPresetDto, SystemProfileDto } from '~/types/dto';

import { server$ } from '@builder.io/qwik-city';

import { BackendError, backendFetch, backendGet } from '~/lib/backend-client';

/** The path of the system endpoint. */
const SYSTEM_PATH = '/api/v1/system';

/** The reply of the system functions. */
export type SystemProfileResult = ApiResult<SystemProfileDto>;

/** The reply of the preset function. */
export type SystemPresetResult = ApiResult<SystemPresetDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/**
 * Read the system profile of the host.
 *
 * The profile holds the operating system, the processor, the memory, the
 * disks, the devices the daemon can use, and the recommended router
 * preset. The settings page shows the profile and places the sliders at
 * the recommended quality.
 */
export const getSystemProfile = server$(
  async (): Promise<SystemProfileResult> => {
    try {
      const profile = await backendGet<SystemProfileDto>(SYSTEM_PATH);
      return { failed: false, data: profile };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Turn a quality value into the router configuration the daemon would
 * store.
 *
 * Quality is 0..100 where 0 is fastest and 100 is best. The daemon
 * lerps between the minimum successful settings and the maximum quality
 * settings, so every value in between is a linear interpolation.
 */
export const getSystemPreset = server$(
  async (quality: number): Promise<SystemPresetResult> => {
    try {
      const preset = await backendFetch<SystemPresetDto>(
        `${SYSTEM_PATH}/preset`,
        {
          method: 'POST',
          body: JSON.stringify({ quality }),
        },
      );
      return { failed: false, data: preset };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);
