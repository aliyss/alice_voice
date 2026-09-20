# Frontend Overview

## Purpose

This document defines the two frontends of Alice Voice.
It explains when to use each frontend and how they share with the backend.
Read this document before you write frontend code.
See [[../GUIDELINES]] for the project boundary.

## Two Frontends

The project has two separate frontend projects.
Both frontends are separate Rust or TypeScript projects from the backend.
Both frontends talk to the backend only through REST and sockets.

| Frontend | Stack | Purpose | Build |
| --- | --- | --- | --- |
| Desktop | Rust with Tauri (or Dioxus) | Native app for local control, tray, system settings, and hotkeys | `frontend-desktop/` |
| Web | Qwik with Qwik City | Browser app for remote access, history, intent management, and plugin admin | `frontend-web/` |

The backend is the same for both frontends.
The backend exposes one REST API and one socket stream.
Both frontends consume the same contract.
See [[../backend/ARCHITECTURE]] for the backend boundary.

```
backend daemon ── REST /api/* ──► desktop (Tauri) ──┐
              └─ WS /api/events ─►                   ├─► user
              ── REST /api/* ──► web (Qwik) ────────┘
              └─ WS /api/events ─►
```

## Repository Structure

```
alice_voice/
├── guidelines/                      # these documents
│   └── frontend/
│       ├── OVERVIEW.md              # this file
│       ├── desktop/                 # desktop guidelines (Rust/Tauri)
│       └── web/                     # web guidelines (Qwik)
├── backend/                         # Rust daemon + REST/socket server
│   ├── Cargo.toml
│   └── crates/...
├── frontend-desktop/                # Rust project (Tauri)
│   ├── Cargo.toml
│   ├── src/
│   └── tauri.conf.json
└── frontend-web/                    # Qwik project (TypeScript)
    ├── package.json
    └── src/
```

Each frontend has its own manifest and its own lock file.
A frontend never imports code from the other frontend.
A frontend never imports backend source code.
The API contract is the only shared artifact.
See [[../GUIDELINES]] for the dependency rules.

## Choice of Frontend

Use desktop when you need native capabilities.
Examples: tray icon, global hotkeys, window focus, local config file, offline mode.
Use web when you need remote access.
Examples: history view, intent editor, plugin admin, multi-user access.

Both frontends show the same core state: daemon status, transcript, and execution result.
Each frontend may show extra views that fit its platform.
Do not duplicate a platform-specific view in the other frontend without reason.

## Backend Contract

Both frontends use the same backend contract.

- REST for request-response: `GET /api/status`, `GET /api/config`, `PUT /api/config`, `GET /api/plugins`, `GET /api/history`.
- Sockets for push: `WS /api/events` streams `SystemEvent` JSON.

The backend defines the DTOs in `backend/crates/alice-core/src/dto.rs` and documents them in `backend/config/openapi.yaml`.
Each frontend copies or generates the DTOs into its own types folder.
The desktop copies into `frontend-desktop/src/bridge/types.rs`.
The web copies into `frontend-web/src/types/dto.ts` or generates with `openapi-typescript`.
See [[desktop/FOLDER_STRUCTURE]] and [[web/FOLDER_STRUCTURE]].

Version the API as `/api/v1/`.
A breaking change needs a new version.

## Desktop Frontend

The desktop frontend is Rust with Tauri.
The docs for the desktop frontend live in `frontend/desktop/`.

Use the desktop guidelines when you work in `frontend-desktop/`.

- [[desktop/FOLDER_STRUCTURE]] defines the desktop folder layout.
- [[desktop/CODE_STRUCTURE]] defines file order and single responsibility.
- [[desktop/STATE]] defines local state, global state, and socket sync.
- [[desktop/STYLING]] defines tokens and primitives.

The desktop bridge owns `reqwest` and `tokio-tungstenite` (or `tauri` fetch).
Only `src/bridge/` imports them.
See [[desktop/FOLDER_STRUCTURE]] for the agnostic view rule.

## Web Frontend

The web frontend is Qwik with Qwik City.
The docs for the web frontend live in `frontend/web/`.
The web guidelines follow the Qwik conventions from sp-guidelines with Alice Voice specifics.

Use the web guidelines when you work in `frontend-web/`.

- [[web/FOLDER_STRUCTURE]] defines the Qwik folder layout with `src/routes`, `src/api`, `src/schemas`, `src/components`.
- [[web/CODE_STRUCTURE]] defines file order, single responsibility, and the callback handle pattern.
- [[web/ROUTING]] defines routes, layouts, loaders, and server$ usage.
- [[web/STATE]] defines Qwik `useContext`, `useSignal`, and `useStore`.
- [[web/STYLING]] defines the design tokens and primitives (`frontend-web/src/components/ui`).
- [[web/NAMING_CONVENTIONS]] defines file and symbol names for Qwik.

The web follows the same REST and socket rule as the desktop.
The web never talks to the backend through Qwik server$ to the backend directly on the same host except through REST fetch and WebSocket.
The Qwik `routeLoader$` fetches from the backend REST API.
The Qwik `server$` triggers backend REST mutations and then revalidates.

## Sharing Strategy

The desktop and the web share the backend contract but they do not share code by default.
Do not share components through a `path` dependency.
The codebases have different languages and runtimes.

Sharing is theoretical and optional.
Use it only when it reduces effort and keeps quality.

Two sharing options exist.

### Option One: Share Design Tokens and Primitives

Share the visual language, not the code.

- Keep one source of truth for tokens in `frontend-web/src/styles/tokens.css` or in a neutral `design/` folder.
- Copy or publish the tokens to `frontend-desktop/src/styles/tokens.css` when they change.
- Keep primitives in each frontend with the same API and the same tokens.
  A button looks the same on desktop and on web but the implementation stays separate.

Use this option when the UIs should look consistent but the code should stay isolated.
This is the recommended default.

### Option Two: Desktop Shows the Web URL

Let the desktop embed the web frontend for selected views.

- The web frontend can run as a standalone app on the backend host.
- The desktop Tauri app can show a `Webview` that loads the web URL for views such as history or plugin admin.
- The desktop then shows only native views locally and delegates the rest to the web URL.
- The desktop config holds the web URL and falls back to a local placeholder when the URL is not reachable.

Use this option only when it works reliably and when the embedded view needs no native access.
Do not embed the web URL for tray, hotkeys, or local settings.
Keep those native.
If the option does not work, keep the two frontends separate.
Do not force URL sharing.

In both options, the REST and socket contract stays the single integration point.
Do not add a second integration such as direct file access or shared database.

## Single File Responsibility

The rule applies to both frontends.
One file has one concept.
One component renders one section or one form.
One function does one action.
See [[desktop/CODE_STRUCTURE]] and [[web/CODE_STRUCTURE]].

## Documentation Map

| File | Content |
| --- | --- |
| [[desktop/FOLDER_STRUCTURE]] | Desktop folder layout (Tauri/Rust) |
| [[desktop/CODE_STRUCTURE]] | Desktop code order and single responsibility |
| [[desktop/STATE]] | Desktop local and global state with REST/socket |
| [[desktop/STYLING]] | Desktop tokens and primitives |
| [[desktop/NAMING_CONVENTIONS]] | Desktop file and symbol names |
| [[web/FOLDER_STRUCTURE]] | Web folder layout with routes, api, schemas, components |
| [[web/CODE_STRUCTURE]] | Web code order and callback handle pattern |
| [[web/ROUTING]] | Web routes, layouts, loaders, and server$ |
| [[web/STATE]] | Web shared state with Qwik useContext |
| [[web/STYLING]] | Web tokens and primitives with Tailwind |
| [[web/NAMING_CONVENTIONS]] | Web file and symbol names |
| [[web/CODE_QUALITY]] | Web lint and format baseline (eslint, prettier) |
