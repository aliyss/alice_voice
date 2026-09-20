/**
 * The event stream client of the daemon.
 *
 * This module is the only place in the web frontend that opens a
 * `WebSocket`. The app shell opens the stream once and pushes the events
 * into the app status context.
 *
 * The socket reconnects with a backoff delay. `onOpen` fires after every
 * successful connect, so the caller can re-read the snapshot with REST
 * and catch the events it missed.
 */

import type { SystemEventDto } from '~/types/dto';

/** The socket path of the daemon event stream. */
const EVENT_PATH = '/api/v1/events';

/** The first reconnect delay in milliseconds. */
const BASE_RETRY_MS = 500;

/** The longest reconnect delay in milliseconds. */
const MAX_RETRY_MS = 15_000;

/** The callbacks of one event stream. */
export interface EventStreamHandlers {
  /** Run for every event the daemon pushes. */
  onEvent: (event: SystemEventDto) => void;
  /** Run after the socket connects or reconnects. */
  onOpen?: () => void;
  /** Run after the socket closes, before the reconnect delay. */
  onClose?: () => void;
}

/** A live event stream. Call `close` to stop the reconnect loop. */
export interface EventStream {
  close: () => void;
}

/** Read the base URL of the socket, or the page origin when it is not set. */
function resolveSocketBase(): string {
  const configured = import.meta.env.PUBLIC_ALICE_API_URL;
  if (typeof configured === 'string' && configured.length > 0) {
    return configured;
  }
  return window.location.origin;
}

/** Build the absolute WebSocket URL of the event stream. */
function resolveSocketUrl(): string {
  const url = new URL(EVENT_PATH, resolveSocketBase());
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

/** Grow the delay with every attempt and add jitter. */
function backoffDelay(attempt: number): number {
  const ceiling = Math.min(MAX_RETRY_MS, BASE_RETRY_MS * 2 ** attempt);
  return ceiling / 2 + Math.random() * (ceiling / 2);
}

/** Parse one socket frame, or return null when the frame is not an event. */
function parseEvent(data: unknown): SystemEventDto | null {
  if (typeof data !== 'string') {
    return null;
  }
  try {
    return JSON.parse(data) as SystemEventDto;
  } catch {
    return null;
  }
}

/** Open the event stream and keep it open. */
export function createEventStream(
  handlers: EventStreamHandlers,
): EventStream {
  let socket: WebSocket | null = null;
  let attempt = 0;
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const scheduleReconnect = (): void => {
    if (stopped) {
      return;
    }
    timer = setTimeout(connect, backoffDelay(attempt));
    attempt += 1;
  };

  function connect(): void {
    if (stopped) {
      return;
    }
    socket = new WebSocket(resolveSocketUrl());

    socket.onopen = () => {
      attempt = 0;
      handlers.onOpen?.();
    };

    socket.onmessage = (message: MessageEvent) => {
      const event = parseEvent(message.data);
      if (event) {
        handlers.onEvent(event);
      }
    };

    socket.onclose = () => {
      handlers.onClose?.();
      scheduleReconnect();
    };

    socket.onerror = () => {
      socket?.close();
    };
  }

  connect();

  return {
    close: () => {
      stopped = true;
      clearTimeout(timer);
      socket?.close();
    },
  };
}
