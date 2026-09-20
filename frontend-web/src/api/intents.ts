/**
 * The intent endpoints of the daemon.
 *
 * A `server$` function runs on the server and calls the backend REST API.
 * Every write returns the whole list, so the page shows the stored
 * configuration without a second read of its own.
 */
import type { ApiResult } from '~/types/bridge';
import type {
  IntentDto,
  IntentListDto,
  IntentWriteDto,
  ScriptPreviewDto,
} from '~/types/dto';

import type { IntentInput } from '~/schemas/intent';

import { server$ } from '@builder.io/qwik-city';

import { safeParse } from 'valibot';

import {
  BackendError,
  backendDelete,
  backendFetch,
  backendGet,
} from '~/lib/backend-client';

import { intentSchema } from '~/schemas/intent';

/** The path of the intent endpoints. */
const INTENTS_PATH = '/api/v1/intents';

/** The reply of the intent functions. */
export type IntentListResult = ApiResult<IntentListDto>;

/** The reply of the script preview function. */
export type ScriptPreviewResult = ApiResult<ScriptPreviewDto>;

/** The payload of the update function. */
export interface UpdateIntentPayload {
  /** The intent to replace. */
  id: string;
  /** The new values of the intent. */
  intent: IntentInput;
}

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/** Read the intent configuration. */
export const listIntents = server$(async (): Promise<IntentListResult> => {
  try {
    const list = await backendGet<IntentListDto>(INTENTS_PATH);
    return { failed: false, data: list };
  } catch (error) {
    return { failed: true, message: toFailureMessage(error) };
  }
});

/** Create one intent and return the new list. */
export const createIntent = server$(
  async (input: IntentInput): Promise<IntentListResult> => {
    const parsed = safeParse(intentSchema, input);
    if (!parsed.success) {
      return { failed: true, message: parsed.issues[0].message };
    }

    try {
      await backendFetch<IntentDto>(INTENTS_PATH, {
        method: 'POST',
        body: toWrite(parsed.output),
      });
      return {
        failed: false,
        data: await backendGet<IntentListDto>(INTENTS_PATH),
      };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Replace one intent and return the new list. */
export const updateIntent = server$(
  async (payload: UpdateIntentPayload): Promise<IntentListResult> => {
    const parsed = safeParse(intentSchema, payload.intent);
    if (!parsed.success) {
      return { failed: true, message: parsed.issues[0].message };
    }

    try {
      await backendFetch<IntentDto>(`${INTENTS_PATH}/${payload.id}`, {
        method: 'PUT',
        body: toWrite(parsed.output),
      });
      return {
        failed: false,
        data: await backendGet<IntentListDto>(INTENTS_PATH),
      };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Add the example intents the catalog does not hold yet.
 *
 * A fresh catalog already starts with them, and this adds the ones a
 * workspace that was configured by hand never got. An example that the
 * catalog holds keeps the configuration of the user.
 */
export const addExampleIntents = server$(
  async (): Promise<IntentListResult> => {
    try {
      const list = await backendFetch<IntentListDto>(
        `${INTENTS_PATH}/examples`,
        { method: 'POST' },
      );
      return { failed: false, data: list };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Delete one intent and return the new list. */
export const deleteIntent = server$(
  async (id: string): Promise<IntentListResult> => {
    try {
      await backendDelete(`${INTENTS_PATH}/${id}`);
      return {
        failed: false,
        data: await backendGet<IntentListDto>(INTENTS_PATH),
      };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Run one script and return the values it answers with.
 *
 * The settings page runs the script of a script entity here, so a user
 * reads the list before a turn reads it. The preview ignores the values
 * the daemon holds in memory and runs the script of this moment.
 */
export const previewScript = server$(
  async (script: string): Promise<ScriptPreviewResult> => {
    try {
      const preview = await backendFetch<ScriptPreviewDto>(
        `${INTENTS_PATH}/script/preview`,
        { method: 'POST', body: { script } },
      );
      return { failed: false, data: preview };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Map the form input to the body of the endpoint.
 *
 * An open entity sends no value and no script, because the resolver reads
 * its value out of the message later. A closed entity sends no script and
 * a script entity sends no values: the daemon runs the script for them.
 */
function toWrite(input: IntentInput): IntentWriteDto {
  return {
    name: input.name,
    description: input.description,
    command: input.command,
    entities: input.entities.map((entity) => ({
      name: entity.name,
      kind: entity.kind,
      values: entity.kind === 'closed' ? entity.values : [],
      script: entity.kind === 'script' ? (entity.script ?? '').trim() : null,
      required: entity.required ?? true,
    })),
    examples: input.examples ?? [],
  };
}
