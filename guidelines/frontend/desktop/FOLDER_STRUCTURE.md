# Desktop Frontend Folder Structure

## Purpose

This document defines the folder structure of the desktop frontend.
It applies to the Rust desktop project in `frontend-desktop/`.
The desktop frontend is a separate Rust project from the backend and from the web frontend.
It communicates with the backend only through REST and sockets.
It runs as a Rust UI with Tauri (Dioxus is an alternative host).
For the names of files, see [[NAMING_CONVENTIONS]].
For the order inside a file, see [[CODE_STRUCTURE]].

## Agnostic Design

The frontend has three layers.

- The view layer renders the UI.
- The state layer holds the UI state.
- The bridge layer talks to the backend through REST and sockets.

The host (Tauri or Dioxus) lives at the edge.
The views do not import Tauri or Dioxus directly except through the bridge and the app shell.
This rule keeps the migration path open and keeps the backend boundary clean.

## Repository Boundary

The repository has three separate projects: one backend and two frontends.

```
alice_voice/
├── guidelines/                # these documents
│   └── frontend/
│       ├── OVERVIEW.md
│       ├── desktop/           # this document
│       └── web/
├── backend/                   # project 1: daemon + REST/socket server
│   ├── Cargo.toml
│   └── crates/...
├── frontend-desktop/          # project 2: desktop UI (this document)
│   ├── Cargo.toml
│   └── src/...
└── frontend-web/              # project 3: web UI (Qwik)
    ├── package.json
    └── src/...
```

The desktop frontend has its own `Cargo.toml` and its own `Cargo.lock`.
It has no `path` dependency on any `backend/` or `frontend-web/` crate.
It imports only its own crates and external crates (http client, websocket client).
The backend and the frontends share only the API contract (DTOs and OpenAPI).
See [[../../GUIDELINES]] for the boundary rules.

## Crate Structure

```
frontend-desktop/
├── Cargo.toml               # separate manifest, not part of backend workspace
├── Cargo.lock
├── src/
│   ├── main.rs              # Tauri entry or Dioxus launch
│   ├── lib.rs               # public exports for tests
│   ├── app.rs               # app shell and router mount
│   ├── bridge/              # REST and socket client to the backend daemon
│   │   ├── mod.rs
│   │   ├── client.rs        # HTTP client setup (base URL, auth, timeout)
│   │   ├── rest.rs          # typed REST calls to backend
│   │   ├── socket.rs        # WebSocket event subscription
│   │   └── types.rs         # DTOs copied or generated from backend contract
│   ├── state/               # global state
│   │   ├── mod.rs
│   │   ├── app_state.rs     # app status (idle, listening, etc.)
│   │   ├── settings.rs      # user settings and config
│   │   └── history.rs       # recent intents and transcripts
│   ├── pages/               # page components (one per route)
│   │   ├── mod.rs
│   │   ├── home.rs          # status and recent activity
│   │   ├── settings.rs      # configuration page
│   │   ├── plugins.rs       # plugin list and details
│   │   └── history.rs       # transcript and execution history
│   ├── views/               # page-level views shared across pages
│   │   ├── mod.rs
│   │   ├── status/
│   │   │   ├── mod.rs
│   │   │   ├── status-view.rs
│   │   │   └── listening-indicator.rs
│   │   ├── transcript/
│   │   │   ├── mod.rs
│   │   │   ├── transcript-view.rs
│   │   │   └── correction-diff.rs
│   │   └── intent/
│   │       ├── mod.rs
│   │       └── intent-preview.rs
│   ├── components/          # reusable UI components
│   │   ├── mod.rs
│   │   ├── sections/        # mid-level blocks (settings-section, plugin-card)
│   │   ├── forms/           # form components
│   │   ├── partials/        # small pieces (empty-state, badge, spinner)
│   │   ├── layout/          # app layout, sidebar, header
│   │   └── primitives/      # design system primitives (button, card, input)
│   ├── assets/              # icons and static assets
│   ├── styles/              # global styles and tokens
│   │   ├── mod.rs
│   │   ├── tokens.css
│   │   └── global.css
│   └── utils/               # pure helpers (format, time, validation)
│       ├── mod.rs
│       ├── format.rs
│       └── validation.rs
├── index.html               # for Dioxus web build
├── tauri.conf.json          # for Tauri build (optional)
└── tests/
```

If the host is Tauri, `main.rs` starts Tauri and mounts the same views.
If the host is Dioxus, `main.rs` launches Dioxus and mounts the same views.
The `pages` and `views` stay identical.
Only `bridge/client.rs` and `app.rs` know the host.

## Pages, Views, and Components

Use this hierarchy.

- pages: one file per route.
  The page loads data through the bridge, holds page state, and composes views.
  The file name matches the route.
- views: page-level screens that more than one page uses.
  The view receives data as props and emits actions through callbacks.
  The view has no bridge call.
  The page passes the bridge handles to the view.
- sections: mid-level blocks that compose primitives.
  A section has its own props interface.
- partials: small reusable pieces.
  A partial has no business logic.
- primitives: design system components that come from one library crate or folder.
  See [[STYLING]] for the token rules.

Organize by domain, not by type.

```
components/
  sections/
    settings/
      wake-settings-section.rs
      transcribe-settings-section.rs
    plugins/
      plugin-list-section.rs
      plugin-detail-section.rs
```

Do not create `components/buttons/` or `components/inputs/` as top levels.
The primitives folder owns the generic elements.

## Bridge Folder

Put the network boundary in `src/bridge`.
The bridge is the only place that talks to the backend.
It has two channels: REST for request-response and socket for events.

```
bridge/
  mod.rs
  client.rs   # create HTTP client, base URL, reconnect logic
  rest.rs     # typed REST functions: get_status, get_config, set_config
  socket.rs   # WebSocket subscription to /api/events
  types.rs    # DTOs from backend contract (copied or generated, not imported)
```

Rules:

- One REST endpoint maps to one function in `rest.rs`.
  Example: `GET /api/status` maps to `get_status()`.
- One socket subscription maps to one stream in `socket.rs`.
  Example: `WS /api/events` maps to `subscribe_events()`.
- The command functions are async and return a typed `Result<T, BridgeError>`.
- The view never calls the bridge directly.
  The page calls the bridge and passes the result to the view.
- Define the event subscription once in `socket.rs` and share it through app state.
  See [[STATE]].
- Keep base URL and timeout in `client.rs` or in config.
  Do not hard-code URLs in views.
- Generate `types.rs` from `backend/config/openapi.yaml` or copy DTOs manually.
  Do not import `backend/crates/alice-core` with a path dependency.
  The contract is the source of truth, not the backend source code.

Example:

```rust
// bridge/rest.rs
pub async fn get_status(client: &ApiClient) -> Result<AppStatus, BridgeError> { ... }
pub async fn set_config(client: &ApiClient, input: ConfigInput) -> Result<(), BridgeError> { ... }

// bridge/socket.rs
pub async fn subscribe_events(client: &ApiClient) -> Result<EventStream, BridgeError> { ... }
```

## State Folder

Put global state in `src/state`.
One file holds one context or one store.

```
state/
  mod.rs
  app_state.rs      # daemon status, listening state, last transcript (from socket)
  settings.rs       # config values and validation (from REST)
  history.rs        # recent transcripts and executions (from REST + socket)
```

If the host uses contexts (Dioxus context, Tauri store), expose them through this folder.
The views read the state, they do not create it.
See [[STATE]].

## Styles Folder

The frontend has two style files.

- `styles/tokens.css` holds design tokens.
- `styles/global.css` imports tokens and defines base styles.

Do not create a CSS file per component.
Use utility classes and tokens.
See [[STYLING]].

## Utils Folder

Put pure helpers in `src/utils`.
One file holds one helper domain.

- `format.rs` formats time, confidence, and transcript.
- `validation.rs` validates settings inputs.

A util function has no side effects and no bridge access.
See [[CODE_STRUCTURE]] for the function order.

## Asset Rules

- Put icons in `src/assets/icons/`.
- Put images in `src/assets/images/`.
- Reference assets through the `assets` module, not with raw paths in views.

## Import Rules

- A view imports primitives from `crate::components::primitives`.
- A view imports pure helpers from `crate::utils`.
- A page imports views, state, and bridge.
- No view imports `reqwest`, `tokio-tungstenite`, or socket types directly.
  Only `crate::bridge` imports them.
- No file imports from `backend/`.
  The projects are separate.
- Use `crate::` absolute imports for local modules.

Example page import:

```rust
use crate::bridge::rest::get_status;
use crate::bridge::socket::subscribe_events;
use crate::components::primitives::{Button, Card};
use crate::state::app_state::use_app_state;
use crate::views::status::status_view::StatusView;
```

See [[../../GUIDELINES]] for dependency and boundary rules.
See [[../OVERVIEW]] for the two-frontend strategy.
