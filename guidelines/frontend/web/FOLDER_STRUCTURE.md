# Web Folder Structure

## Purpose

This document defines the folder structure of the web frontend.
It applies to `frontend-web/` which uses Qwik with Qwik City.
It follows the Qwik conventions from sp-guidelines and adapts them for Alice Voice.
The web frontend talks to the backend only through REST and sockets.
For the names of files, see [[NAMING_CONVENTIONS]].
For the route structure, see [[ROUTING]].
For the shared state, see [[STATE]].

## Repository Boundary

The web project is separate from the backend and from the desktop frontend.

```
alice_voice/
├── backend/                 # Rust daemon + REST/socket server
├── frontend-desktop/        # Rust + Tauri
└── frontend-web/            # Qwik (this document)
    ├── package.json
    └── src/
```

The web project has its own `package.json` and `tsconfig.json`.
It has no `path` dependency on `backend/` or on `frontend-desktop/`.
It communicates with the backend only through REST and WebSocket.
The API types come from the backend contract (`backend/config/openapi.yaml`) or are copied to `src/types/dto.ts`.
See [[../../GUIDELINES]] and [[../OVERVIEW]] for the boundary.

## Application Structure

Each Qwik application in `frontend-web` has the same `src` folder.

```
frontend-web/src/
├── routes/              # Qwik City routes. See [[ROUTING]].
│   ├── layout.tsx       # root layout with providers. See [[ROUTING]].
│   ├── index.tsx        # home: status and last transcript
│   ├── settings/
│   │   └── index.tsx    # settings form
│   ├── plugins/
│   │   ├── index.tsx    # plugin list
│   │   └── [id]/
│   │       └── index.tsx# plugin detail
│   ├── history/
│   │   └── index.tsx    # transcript and execution history
│   └── api/             # local Qwik API handlers if needed (proxy excluded)
│       └── health/
│           └── index.ts # local health, not backend proxy
├── api/                 # server$ functions that call the backend. See section Api Folder.
├── schemas/             # valibot schemas. See section Schemas Folder.
├── components/          # UI that belongs only to the web frontend
│   └── pages/           # page components. See section Pages Folder.
├── context/             # shared state contexts. See [[STATE]].
├── types/               # DTO types from backend contract
│   ├── dto.ts           # AppStatus, Config, PluginDto, HistoryItem
│   └── bridge.ts        # REST client types
├── utils/               # pure logic for the web frontend
├── lib/                 # runtime helpers (REST client, socket client)
│   ├── backend-client.ts# fetch wrapper for backend REST
│   └── socket-client.ts # WebSocket wrapper for /api/events
├── root.tsx             # global root with QwikCityProvider. See [[ROUTING]].
└── global.css           # single global CSS file. See [[STYLING]].
```

Keep the web structure identical to sp-guidelines where it fits.
Adapt the `api/` and `lib/` usage to REST fetch instead of Prisma.

## The Api Folder

Put the `server$` functions in `src/api`.
Do not put a `server$` function in a route file.
The route file imports the functions from `src/api`.

Organize the `api` folder by domain.
Mirror the route structure in the `api` folder.

```
api/
├── status.ts            # getStatus, subscribe not here
├── settings.ts          # getConfig, updateConfig
├── plugins.ts           # listPlugins, setPluginEnabled
└── history.ts           # listHistory
```

Rules:

- One route domain maps to one api file.
  The file name matches the route folder name.
  The file name is kebab-case and ends with `.ts`.
  The file has no JSX.
- The file exports the `server$` functions and their result types.
- A `server$` function calls the backend REST API through `lib/backend-client.ts`.
  It does not access a database.
  Example: `getStatus` calls `GET /api/v1/status` on the backend.
- The web frontend has no direct database access.
  The backend owns all data.
  See [[../../backend/ARCHITECTURE]] for the backend boundary.
- The `server$` function validates input with a valibot schema from `src/schemas` before it calls the backend.
  See [[CODE_STRUCTURE]] for the validation order.
- A shared view that more than one route uses receives the actions as callback props.
  The route file wraps the `server$` in a handle and passes the handle to the view.
  See [[CODE_STRUCTURE]] for the callback handle pattern.
  See [[ROUTING]] for the server$ convention.

## The Schemas Folder

Put the valibot schemas in `src/schemas`.
The schemas live outside the `api` folder.
The `api` file imports the schema from `src/schemas`.
A schema file is a `.ts` file with no JSX.

One schema file covers one domain.
The file name matches the domain name.
The file name is kebab-case.

```
schemas/
├── config.ts            # configSchema
├── plugin.ts            # pluginToggleSchema
└── history.ts           # historyFilterSchema
```

Export the schema and its input type from the file.
The `api` file uses the input type for the `server$` function.
See [[NAMING_CONVENTIONS]] for the schema names.

## The Components Folder

Put the components that are specific to the web frontend in `src/components`.
Organize the components by domain.
Do not organize by type.

```
components/
├── pages/               # page components. See section Pages Folder.
├── sections/
│   ├── status/          # status sections (status-hero, listening-indicator)
│   ├── settings/        # settings sections
│   ├── plugins/         # plugin sections
│   └── history/         # history sections
├── forms/               # form components. See [[NAMING_CONVENTIONS]].
├── viz/                 # timeline or chart components for history
└── router-head/         # head tags
```

A section is one block of a page.
The section has its own props interface.
The page component composes the sections.
See [[NAMING_CONVENTIONS]] for the section names.
See [[CODE_STRUCTURE]] for the order inside a file.

## The Pages Folder

A page component renders one complete page.
Put the page components in `src/components/pages`.
The file name ends with `-page.tsx`.
The component name is `<Domain>Page`.

```
components/pages/
├── home-page.tsx        # HomePage
├── settings-page.tsx    # SettingsPage
├── plugins-page.tsx     # PluginsPage
└── history-page.tsx     # HistoryPage
```

The route index file has only the loaders and the default component.
The loader passes the data to the page component.
The page component has all the page-specific logic.
See [[ROUTING]] for the index file rule.

A page component file exports:

- The page component `<Domain>Page`
- The props interface `<Domain>PageProps`
- The data type `<Domain>PageData`
- The row interfaces that the page uses

The route file imports the page component and the data type.
The loader returns the data only.
The loader calls the backend REST through `lib/backend-client.ts`.
The data type omits the callback props.
The default component renders the page with the loader data and passes handles as callback props.
See [[CODE_STRUCTURE]] for the callback handle pattern.

Mirror the route structure in the pages folder.

## The Lib Folder

Put the runtime helpers in `src/lib`.

- `backend-client.ts` wraps `fetch` for backend REST.
  It holds base URL, timeout, and error mapping.
  It exports `get`, `put`, `post` helpers that return typed DTOs.
- `socket-client.ts` wraps the WebSocket for `WS /api/v1/events`.
  It exports `createEventStream` that re-connects with backoff.

No component imports `fetch` or `WebSocket` directly.
Only `lib/` imports them.
See [[STATE]] for the socket subscription rule.

## The Types Folder

Put the DTO types in `src/types`.

- `dto.ts` holds `AppStatus`, `ConfigDto`, `PluginDto`, `HistoryItem`, `SystemEvent`.
  Generate it from `backend/config/openapi.yaml` or copy it manually.
  Do not import from `backend/crates/...`.
- `bridge.ts` holds local types for the REST client such as `ApiResult<T>`.

The loader maps the DTO to a row type if the view needs a different shape.
The row type lives in the page file.

## Imports

The web imports follow the sp-guidelines alias `~/*` plus `~/lib/*` for backend client.

```
import type { HistoryPageData } from '~/components/pages/history-page';
import { HistoryPage } from '~/components/pages/history-page';
import { listHistory } from '~/api/history';
import { backendFetch } from '~/lib/backend-client';
```

A component uses only primitives from the design system for structure.
See [[STYLING]] for the primitive rule.
