/**
 * The librarian endpoints of the daemon.
 *
 * A `server$` function runs on the server and calls the backend REST API.
 * The settings page reads the state of the memory here, searches it, and
 * writes one node by hand.
 */
import type { ApiResult } from '~/types/bridge';
import type {
  LibrarianLintDto,
  LibrarianStatusDto,
  MemoryMergeRequestDto,
  MemoryNodeDto,
  MemoryQueryDto,
  MemoryQueryRequestDto,
  MemoryWriteRequestDto,
} from '~/types/dto';

import { server$ } from '@builder.io/qwik-city';

import {
  BackendError,
  backendDelete,
  backendFetch,
  backendGet,
  backendPost,
} from '~/lib/backend-client';

/** The path of the librarian endpoint. */
const LIBRARIAN_PATH = '/api/v1/librarian';

/** The reply of the librarian status function. */
export type LibrarianStatusResult = ApiResult<LibrarianStatusDto>;

/** The reply of a memory query. */
export type MemoryQueryResult = ApiResult<MemoryQueryDto>;

/** The reply of a memory write. */
export type MemoryWriteResult = ApiResult<MemoryNodeDto>;

/** The reply of the memory linter. */
export type LibrarianLintResult = ApiResult<LibrarianLintDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The daemon did not answer.';
}

/** Read the state of the librarian. */
export const getLibrarianStatus = server$(
  async (): Promise<LibrarianStatusResult> => {
    try {
      const status = await backendGet<LibrarianStatusDto>(LIBRARIAN_PATH);
      return { failed: false, data: status };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Search the memory for the nodes a text names. */
export const queryMemory = server$(
  async (input: MemoryQueryRequestDto): Promise<MemoryQueryResult> => {
    try {
      const result = await backendPost<MemoryQueryDto>(
        `${LIBRARIAN_PATH}/query`,
        input,
      );
      return { failed: false, data: result };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Write one memory node and its facts by hand. */
export const writeMemory = server$(
  async (input: MemoryWriteRequestDto): Promise<MemoryWriteResult> => {
    try {
      const node = await backendPost<MemoryNodeDto>(
        `${LIBRARIAN_PATH}/memories`,
        input,
      );
      return { failed: false, data: node };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Retire one fact and return the concept it belongs to. */
export const retireFact = server$(
  async (id: string): Promise<MemoryWriteResult> => {
    try {
      const node = await backendFetch<MemoryNodeDto>(
        `${LIBRARIAN_PATH}/facts/${encodeURIComponent(id)}`,
        { method: 'DELETE' },
      );
      return { failed: false, data: node };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Move one concept into another.
 *
 * The concept in the path moves into the concept the body names: its facts
 * move as they stand, its key becomes a name of the target, and the empty
 * concept goes. The reply is the concept that keeps them.
 */
export const mergeConcept = server$(
  async (id: string, into: string): Promise<MemoryWriteResult> => {
    try {
      const body: MemoryMergeRequestDto = { into };
      const node = await backendPost<MemoryNodeDto>(
        `${LIBRARIAN_PATH}/memories/${encodeURIComponent(id)}/merge`,
        body,
      );
      return { failed: false, data: node };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Delete one concept and every fact of it. */
export const deleteConcept = server$(
  async (id: string): Promise<ApiResult<null>> => {
    try {
      await backendDelete(
        `${LIBRARIAN_PATH}/memories/${encodeURIComponent(id)}`,
      );
      return { failed: false, data: null };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/** Read the report of the memory linter. */
export const lintMemory = server$(async (): Promise<LibrarianLintResult> => {
  try {
    const report = await backendGet<LibrarianLintDto>(`${LIBRARIAN_PATH}/lint`);
    return { failed: false, data: report };
  } catch (error) {
    return { failed: true, message: toFailureMessage(error) };
  }
});
