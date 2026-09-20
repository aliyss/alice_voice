# Web State with Qwik useContext

## Purpose

This document defines how the web frontend shares state.
It applies to `frontend-web/` with Qwik.
It covers Qwik contexts, loaders, and the REST and socket sync.
The web frontend has no local database; all persistent state lives in the backend.
See [[FOLDER_STRUCTURE]] for the folder layout and [[ROUTING]] for the loader rules.

## Where the Contexts Live

Each context has one file.
The file is `src/context/<name>.context.ts`.
The name follows [[NAMING_CONVENTIONS]].

The file exports two things:

- The context id with `createContextId<T>(name)`.
- The provider component that uses `useContextProvider`.

A context that the web app needs lives in `src/context`.

```
context/
├── app.context.ts        # app status from socket (idle, listening, etc.)
├── settings.context.ts   # config from REST
└── history.context.ts    # recent history from REST + socket updates
```

## How to Provide the Context

Mount the provider once at the top of the relevant layout.

- The app status provider goes in `src/routes/layout.tsx`.
  Feed it with the socket stream from `WS /api/v1/events` through `~/lib/socket-client`.
- The settings provider goes in `src/routes/layout.tsx` or in `src/root.tsx`.
  Feed it with the data from `GET /api/v1/config`.

The value of a context must be serializable.
Qwik serializes the context between the server and the client.
Do not put a function, a WebSocket instance, or a Prisma object in the context.
Put the socket instance in `lib/socket-client.ts` and push values into the context.

```
 // src/routes/layout.tsx
export default component$(() => {
  const appState = useSignal<AppState>({ status: 'Idle' });
  useContextProvider(appContext, appState);
  useVisibleTask$(() => {
    const stream = createEventStream();
    stream.on('event', (e) => { appState.value = mapEvent(e); });
  });
  return <Slot />;
});
```

## When to Use a Context

| Use a context | Do not use a context |
| --- | --- |
| Daemon status and listening state from socket | Route data from `routeLoader$` that belongs to one page (pass as props) |
| Settings and config from REST that many routes need | One-off fetch result |
| History that the sidebar and the home page share | Form draft (use modular-forms) |
| Theme and locale | WebSocket instance or fetch client |

Route data that belongs to one page stays in `routeLoader$` and props.
Do not move it to a context.

## Loader and server$ vs Context

- A `routeLoader$` fetches a snapshot from the backend REST through `lib/backend-client.ts`.
  The snapshot is the initial render.
- The socket pushes deltas through `lib/socket-client.ts` and updates the context.
  The context is the live state.

Flow:

1. The loader fetches `GET /api/v1/status` and returns it as props for the first paint.
2. The layout subscribes to `WS /api/v1/events` once and writes to `app.context`.
3. The views read the context for live updates and read loader props for page-specific data.

Do not fetch REST in a loop from a context.
Do not subscribe to the socket in every page.

## Qwik Patterns

A component is `component$`.
An event handler is a QRL with the `$` suffix: `onClick$`, `onInput$`.
A `server$` function is a QRL that runs on the server and then calls backend REST.
See [[ROUTING]] for the server$ convention.

A QRL does not close over the component.
Pass the values that the QRL needs as arguments.
A QRL and its arguments must be serializable.
Do not pass a function or a class instance.

Use `useSignal` for the state of one component.
Use `useStore` for a group of related values.
Use a context for state that many components need.
See the table above.

A form has its own state.
Use `@modular-forms/qwik` for the form state.
Do not use `useSignal` for form fields.
See the Qwik form guide and [[CODE_STRUCTURE]] for the form order.

Use `useTask$` for work that runs at mount or when a signal changes.
Use `useVisibleTask$` for work that runs only in the browser.
The socket subscription runs in `useVisibleTask$` because WebSocket is browser-only.
A `useVisibleTask$` runs only in the browser; use it only for browser work.

Keep the code in a component file in the order from [[CODE_STRUCTURE]].

## Provider and Consumer Names

The provider name starts with the concept.
Use `AppProvider` for the app status.
Use `SettingsProvider` for the settings.

The consumer calls `useContext` with the context id.
All files import the same context id from `src/context`.

## Socket Reconnection

The socket can drop.
Reconnect with backoff.
After reconnect, re-fetch the snapshot with REST to catch missed events.

```
// lib/socket-client.ts
let retries = 0;
function connect() {
  const ws = new WebSocket('/api/v1/events');
  ws.onclose = () => { setTimeout(connect, backoff(retries++)); };
  ws.onopen = () => { retries = 0; refreshSnapshotWithRest(); };
}
```

Do not put reconnection logic in a page.
Keep it in `lib/socket-client.ts`.

## Example

```
// context/app.context.ts
export const appContext = createContextId<Signal<AppState>>('app-context');
export const AppProvider = component$(() => {
  const state = useSignal<AppState>({ status: 'Idle' });
  useContextProvider(appContext, state);
  return <Slot />;
});
```

The consumer:

```
const appState = useContext(appContext);
```

See [[FOLDER_STRUCTURE]] for the full layout.
