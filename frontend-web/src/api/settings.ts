/**
 * The settings endpoints of the daemon.
 *
 * A `server$` function runs on the server and calls the backend REST API.
 * The browser never calls the daemon directly, so the API host stays
 * private and the client keeps one integration point.
 */
import type { ApiResult } from '~/types/bridge';
import type { SettingsDto } from '~/types/dto';

import type { SettingsInput } from '~/schemas/settings';

import { server$ } from '@builder.io/qwik-city';

import { safeParse } from 'valibot';

import { BackendError, backendFetch, backendGet } from '~/lib/backend-client';

import { settingsSchema } from '~/schemas/settings';

/** The path of the settings endpoint. */
const SETTINGS_PATH = '/api/v1/settings';

/** The reply of the settings functions. */
export type SettingsResult = ApiResult<SettingsDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/** Read the settings of the daemon. */
export const getSettings = server$(async (): Promise<SettingsResult> => {
  try {
    const settings = await backendGet<SettingsDto>(SETTINGS_PATH);
    return { failed: false, data: settings };
  } catch (error) {
    return { failed: true, message: toFailureMessage(error) };
  }
});

/**
 * Update the settings of the daemon.
 *
 * A field the input does not carry keeps its stored value, so the queue
 * toggle and the resolver form save without touching each other.
 */
export const updateSettings = server$(
  async (input: SettingsInput): Promise<SettingsResult> => {
    const parsed = safeParse(settingsSchema, input);
    if (!parsed.success) {
      return { failed: true, message: parsed.issues[0].message };
    }

    try {
      const settings = await backendFetch<SettingsDto>(SETTINGS_PATH, {
        method: 'PUT',
        body: parsed.output,
      });
      return { failed: false, data: settings };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);
