# Web Routing

## Purpose

This document defines the routing of the web frontend.
It applies to `frontend-web/` with Qwik City.
It covers routes, index files, layouts, and the REST and socket handling.
Qwik City defines the routes from the filesystem.
The web frontend has no database access; all data comes from the backend through REST and sockets.
See [[FOLDER_STRUCTURE]] for the folder layout.

## Routes

Each web route lives in `frontend-web/src/routes`.
The filesystem structure defines the URL structure.

- One folder per URL segment.
- The folder index renders the base URL. The file is `index.tsx`.
- A dynamic segment uses a folder with square brackets. The folder `plugins/[id]` renders `/plugins/:id`.
- A route group uses parentheses. The group does not add a URL segment.
  Put authenticated pages in `(app)` if a guard is needed.

```
src/routes/
├── layout.tsx           # global layout (app shell)
├── index.tsx            # /  home: status and last transcript
├── settings/
│   └── index.tsx        # /settings
├── plugins/
│   ├── index.tsx        # /plugins
│   └── [id]/
│       └── index.tsx    # /plugins/:id
└── history/
    └── index.tsx        # /history
```

Use `routeLoader$` to get data for a page.
A `routeLoader$` in the web frontend fetches from the backend REST through `~/lib/backend-client`.
Call the backend, not a local database.

Use `server$` to change data from a page.
A `server$` function calls backend REST with a mutation.
Call it from an event handler or from a Form action.

A loader returns a serializable DTO from `~/types/dto`.
A loader never returns a backend internal type.

## Index Files

Every route folder has one entry file.
A page folder has `index.tsx`.
Do not put named files in a route folder.

A package uses a barrel `index.ts` for its public exports.
Do not put a barrel `index.ts` inside `routes`.
Qwik City owns the `routes` folder.

The index file is thin.
It has only the loaders and the default component.
A loader fetches the data from the backend REST.
The default component renders the page component.
The page component lives in `src/components/pages`.
See [[FOLDER_STRUCTURE]] for the pages folder.

The loader returns the data only.
It does not return the callback props.
The default component passes the loader data to the page.
The default component passes the handles as callback props.
Put all the page-specific logic in the page component.
Do not put page logic in the index file.
See [[CODE_STRUCTURE]] for the callback handle pattern.

Example index file for a page that fetches from backend REST:

```
import { routeLoader$, server$ } from '@builder.io/qwik-city';
import { component$, $ } from '@builder.io/qwik';

import { HomePage } from '~/components/pages/home-page';
import type { HomePageData } from '~/components/pages/home-page';
import { backendFetch } from '~/lib/backend-client';
import { updateConfig } from '~/api/settings';

export const useHomeData = routeLoader$(async (): Promise<HomePageData> => {
  const status = await backendFetch<AppStatusDto>('/api/v1/status');
  const list = await backendFetch<ConversationListDto>('/api/v1/conversations?limit=10');
  return { status, conversations: list.items.map(toRow) };
});

export default component$(() => {
  const home = useHomeData();
  const handleSave = $((data: ConfigInput) => updateConfig(data));
  return <HomePage status={home.value.status} conversations={home.value.conversations} onSave$={handleSave} />;
});
```

A route that needs no data has only the default component.
It renders the page without props.

## Layouts

A layout is `layout.tsx`.
It wraps every route in its folder.
The layout uses `Slot` to render the child content.

Use two levels of layout for the web:

- `src/root.tsx`: the global root with `QwikCityProvider`, head, body, `RouterOutlet`.
  Mount the global context providers here. See [[STATE]].
- `src/routes/layout.tsx`: the app shell with header, sidebar, and socket provider.
  The shell subscribes to `WS /api/v1/events` once and writes to the global context.
  See [[STATE]] for the socket rule.

A layout can declare its own `routeLoader$`.
The loader runs once for the whole subtree.
Use it to fetch global config from `GET /api/v1/config`.

## Mutations with server$

Use `server$` for all data changes that the user triggers.
A `server$` function in the web frontend calls backend REST.

- Put the `server$` function in `src/api`.
  Do not put a `server$` function in a route file.
  Organize the `api` folder by domain.
  One route domain maps to one `api` file.
  See [[FOLDER_STRUCTURE]] for the api folder.
- Import `server$` from `@builder.io/qwik-city`.
  Call a `server$` function from an event handler such as `onClick$`.
- Do not pass a `server$` function directly to a component prop.
  Pass a handle instead. A handle is a `$()` wrapper.
  See [[CODE_STRUCTURE]] for the callback handle pattern.
- Put validation in the `server$` function before the fetch.
  Import the valibot schema from `src/schemas`.
  See [[CODE_STRUCTURE]] for the validation order.

Example:

```
export const updateConfig = server$(async (input: ConfigInput) => {
  const data = parse(configSchema, input);
  const res = await backendFetch('/api/v1/config', { method: 'PUT', body: data });
  if (!res.ok) return { failed: true, message: res.error };
  return { failed: false };
});
```

The `server$` never imports the backend crates.
It uses text-based REST through `lib/backend-client.ts`.

## Sockets and Real-time

The backend pushes real-time state through a WebSocket.
The web frontend subscribes once in the app layout.

- The socket client lives in `src/lib/socket-client.ts`.
  It connects to `WS /api/v1/events` and emits `SystemEvent` JSON.
- The layout or a global context provider calls `createEventStream` and writes to the global context.
  See [[STATE]] for the context setup.
- The route loader does not subscribe to the socket.
  The route loader uses REST.
  The socket updates the context after the loader returns the snapshot.

Do not poll the backend in a loop from a route.
Use the socket for pushes and REST for queries.

## Error Handling and Validation

Validate the input of a `server$` function with valibot.
Validate before you call the backend REST.
The valibot schema lives in `src/schemas`.
Do not define a schema inside a route or an `api` file.

Return a result object from a `server$` function.
The result object has a `failed` flag.
Set the flag to `true` when the action fails.
Include a message for the user.
Do not throw for an expected failure.
Throw only for an unexpected error or a redirect.

Show an error to the user with the alert components from the design system.
Show a validation error next to the field.
The alert text uses the user language.
See [[STYLING]] for the UI language rule.

## Local API Handlers

Qwik allows local `onGet` handlers in `src/routes/api/.../index.ts`.
Use them only for web-local needs such as health checks.
Do not use them as a proxy to the backend unless you document the reason.
The default path is direct fetch from `routeLoader$` and `server$` to `backend` through `lib/backend-client.ts`.
If you use a proxy, keep it in one place and mark it as proxy.
