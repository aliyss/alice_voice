/**
 * Local types of the web frontend REST bridge.
 *
 * These types describe the client, not the backend contract. The backend
 * contract lives in `src/types/dto.ts`.
 */

/** The method of one REST call. */
export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE';

/** The options of one REST call. */
export interface RequestOptions {
  /** The HTTP method. It defaults to GET. */
  method?: HttpMethod;
  /** The request body. The client serializes it as JSON. */
  body?: unknown;
  /**
   * The time the client waits, in milliseconds. It defaults to the client
   * timeout. A call that handles a message waits longer, because the
   * daemon resolves the intent and runs its command first.
   */
  timeoutMs?: number;
}

/**
 * The result of one `server$` call.
 *
 * The browser never sees an exception from a `server$` function. It sees
 * this union instead, so a view can show the failure in the design system
 * alert component.
 */
export type ApiResult<TData> =
  | { failed: false; data: TData }
  | { failed: true; message: string };
