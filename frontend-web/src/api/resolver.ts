/**
 * The resolver endpoints of the daemon.
 *
 * A `server$` function runs on the server and calls the backend REST API.
 * The settings page reads the state of the resolver here, and downloads
 * or removes a built in model.
 */
import type { ApiResult } from '~/types/bridge';
import type { ResolverPreviewDto, ResolverStatusDto } from '~/types/dto';

import { server$ } from '@builder.io/qwik-city';

import {
  BackendError,
  backendFetch,
  backendGet,
  backendPost,
} from '~/lib/backend-client';

/** The path of the resolver endpoint. */
const RESOLVER_PATH = '/api/v1/resolver';

/**
 * The time the client waits for one preview, in milliseconds.
 *
 * A preview reads a message the way a turn would, so it waits for the
 * model server the same way a turn does.
 */
const PREVIEW_TIMEOUT_MS = 60_000;

/** The reply of the resolver functions. */
export type ResolverStatusResult = ApiResult<ResolverStatusDto>;

/** The reply of the preview function. */
export type ResolverPreviewResult = ApiResult<ResolverPreviewDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/**
 * Read the state of the resolver.
 *
 * The reply says which engine reads a message, which models are on disk,
 * which devices this build offers, and how many labels the intent
 * configuration adds up to.
 */
export const getResolverStatus = server$(
  async (): Promise<ResolverStatusResult> => {
    try {
      const status = await backendGet<ResolverStatusDto>(RESOLVER_PATH);
      return { failed: false, data: status };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Download one built in GLiNER model.
 *
 * The download runs in the background, so the reply carries the state the
 * settings page polls while it runs.
 */
export const downloadGlinerModel = server$(
  async (id: string): Promise<ResolverStatusResult> => {
    try {
      const status = await backendFetch<ResolverStatusDto>(
        `${RESOLVER_PATH}/gliner/models/${encodeURIComponent(id)}/download`,
        { method: 'POST' },
      );
      return { failed: false, data: status };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Remove the files of one built in GLiNER model. */
export const deleteGlinerModel = server$(
  async (id: string): Promise<ResolverStatusResult> => {
    try {
      const status = await backendFetch<ResolverStatusDto>(
        `${RESOLVER_PATH}/gliner/models/${encodeURIComponent(id)}`,
        { method: 'DELETE' },
      );
      return { failed: false, data: status };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Download one built in model of the router.
 *
 * The download runs in the background, so the reply carries the state the
 * settings page polls while it runs.
 */
export const downloadLocalModel = server$(
  async (id: string): Promise<ResolverStatusResult> => {
    try {
      const status = await backendFetch<ResolverStatusDto>(
        `${RESOLVER_PATH}/local/models/${encodeURIComponent(id)}/download`,
        { method: 'POST' },
      );
      return { failed: false, data: status };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Read one message without running anything.
 *
 * The settings page tries the sentences of the user with this, so a user
 * reads the route a sentence would take before a turn runs it. The daemon
 * stores nothing and publishes nothing on the event stream.
 */
export const previewMessage = server$(
  async (text: string): Promise<ResolverPreviewResult> => {
    try {
      const preview = await backendPost<ResolverPreviewDto>(
        `${RESOLVER_PATH}/preview`,
        { text },
        { timeoutMs: PREVIEW_TIMEOUT_MS },
      );
      return { failed: false, data: preview };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Remove the files of one built in model of the router. */
export const deleteLocalModel = server$(
  async (id: string): Promise<ResolverStatusResult> => {
    try {
      const status = await backendFetch<ResolverStatusDto>(
        `${RESOLVER_PATH}/local/models/${encodeURIComponent(id)}`,
        { method: 'DELETE' },
      );
      return { failed: false, data: status };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);
