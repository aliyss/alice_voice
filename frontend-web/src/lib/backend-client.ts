/**
 * The REST client of the backend API.
 *
 * This module is the only place in the web frontend that calls `fetch`.
 * A route loader calls it from the server. A `server$` function calls it
 * from the server. No component imports this module.
 *
 * Set `PUBLIC_ALICE_API_URL` to point the client at another host.
 */

import type { HttpMethod, RequestOptions } from '~/types/bridge';

/** The default host of the daemon REST API. */
const DEFAULT_BASE_URL = 'http://127.0.0.1:8787';

/** The time the client waits for an answer before it gives up. */
const REQUEST_TIMEOUT_MS = 10_000;

/** A failed REST call. The message is safe to show to the user. */
export class BackendError extends Error {
  /** The HTTP status, or zero when the daemon did not answer. */
  readonly status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = 'BackendError';
    this.status = status;
  }
}

/** Read the configured base URL, or the default when it is not set. */
function resolveBaseUrl(): string {
  const configured = import.meta.env.PUBLIC_ALICE_API_URL;
  if (typeof configured === 'string' && configured.length > 0) {
    return configured;
  }
  return DEFAULT_BASE_URL;
}

/** Build the absolute URL of one endpoint path. */
function buildRequestUrl(path: string): string {
  const base = resolveBaseUrl();
  const withSlash = base.endsWith('/') ? base : `${base}/`;
  return new URL(path.replace(/^\/+/, ''), withSlash).toString();
}

/** Turn a thrown fetch error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof Error && error.name === 'AbortError') {
    return 'The daemon did not answer in time.';
  }
  return 'The daemon is not reachable.';
}

/** Send one request and return the raw response. */
async function requestResponse(
  path: string,
  method: HttpMethod,
  body: unknown,
  timeoutMs: number,
): Promise<Response> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  const hasBody = body !== undefined;
  try {
    return await fetch(buildRequestUrl(path), {
      method,
      signal: controller.signal,
      headers: hasBody
        ? { accept: 'application/json', 'content-type': 'application/json' }
        : { accept: 'application/json' },
      body: hasBody ? JSON.stringify(body) : undefined,
    });
  } catch (error) {
    throw new BackendError(toFailureMessage(error), 0);
  } finally {
    clearTimeout(timeout);
  }
}

/**
 * Call one backend REST endpoint and return the parsed DTO.
 *
 * @throws {BackendError} when the daemon does not answer, answers with a
 * failure status, or sends a body that is not JSON.
 */
export async function backendFetch<TDto>(
  path: string,
  options: RequestOptions = {},
): Promise<TDto> {
  const method = options.method ?? 'GET';
  const response = await requestResponse(
    path,
    method,
    options.body,
    options.timeoutMs ?? REQUEST_TIMEOUT_MS,
  );

  if (!response.ok) {
    throw new BackendError(await readFailureMessage(response), response.status);
  }

  // A delete answers with no body.
  if (response.status === 204) {
    return undefined as TDto;
  }

  try {
    return (await response.json()) as TDto;
  } catch {
    throw new BackendError(
      'The daemon sent an answer that is not JSON.',
      response.status,
    );
  }
}

/**
 * Read the message of a failure reply.
 *
 * The daemon explains the writes it rejects, for example a name that is
 * already in use, so the caller shows that message instead of a status.
 */
async function readFailureMessage(response: Response): Promise<string> {
  try {
    const body = (await response.json()) as { error?: unknown };
    if (typeof body.error === 'string' && body.error.length > 0) {
      return body.error;
    }
  } catch {
    // The reply carries no message that a reader can show.
  }
  return `The daemon answered with status ${response.status}.`;
}

/** Call one backend REST endpoint with GET. */
export function backendGet<TDto>(path: string): Promise<TDto> {
  return backendFetch<TDto>(path);
}

/** Call one backend REST endpoint with POST. */
export function backendPost<TDto>(
  path: string,
  body: unknown,
  options: Omit<RequestOptions, 'method' | 'body'> = {},
): Promise<TDto> {
  return backendFetch<TDto>(path, { ...options, method: 'POST', body });
}

/** Call one backend REST endpoint with PUT. */
export function backendPut<TDto>(path: string, body: unknown): Promise<TDto> {
  return backendFetch<TDto>(path, { method: 'PUT', body });
}

/** Call one backend REST endpoint with DELETE. */
export function backendDelete(path: string): Promise<void> {
  return backendFetch<void>(path, { method: 'DELETE' });
}


