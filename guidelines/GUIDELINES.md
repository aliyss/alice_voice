# Alice Voice Project Guidelines

## Purpose

This document defines the general structure of the Alice Voice project.
It applies to all crates and packages in the workspace.
Read this document before you write code.

Alice Voice is a Rust listening service.
The service runs silently in the background.
It uses few resources when it is idle.
It wakes on a wake command.
After wake, it listens to the user.
It uses a fast transcriber for real-time text.
It uses a slow transcriber to correct the text.
Then it identifies the intent.
Then it executes the intent through plugins.

The project has three documentation areas.

- [[frontend/OVERVIEW|Frontend overview]] defines the two frontends and the sharing strategy.
- [[frontend/desktop/FOLDER_STRUCTURE|Desktop documentation]] defines the Tauri desktop app.
- [[frontend/web/FOLDER_STRUCTURE|Web documentation]] defines the Qwik web app.
- [[backend/ARCHITECTURE|Backend documentation]] defines the listening service.

## System Overview

The system has three layers in the backend and one separate layer for the frontend.

- The audio layer captures audio and detects the wake command.
- The intelligence layer transcribes the audio and resolves the intent.
- The execution layer runs the action through the plugin system.
- The frontend layer shows the state and the configuration.

```
audio capture -> wake detection -> listening -> fast transcribe -\
                                                                -> intent resolve -> execution
                                              slow transcribe ---/
```

The frontend does not belong to the pipeline.
The frontend shows the state and the configuration.
The frontend communicates with the backend only through a network boundary.
The boundary uses REST for request-response and sockets (WebSocket) for events.
There is no direct crate import between frontend and backend.

## Project Structure

The repository holds three separate projects.
Each project has its own manifest and its own dependencies.
A project never imports code from another project.
All projects communicate only through REST and sockets.

```
alice_voice/
├── guidelines/                # these documents
│   ├── backend/               # backend docs
│   └── frontend/
│       ├── OVERVIEW.md        # two frontend strategy
│       ├── desktop/           # desktop docs (Tauri/Rust)
│       └── web/               # web docs (Qwik)
├── backend/                   # Rust project 1: the listening service
│   ├── Cargo.toml             # backend workspace manifest
│   ├── crates/
│   │   ├── alice-core         # shared backend types, errors, config, events
│   │   ├── alice-audio        # capture and wake detection
│   │   ├── alice-transcribe   # fast and slow transcribers
│   │   ├── alice-intent       # intent database and resolver
│   │   ├── alice-plugin-host  # plugin interface and registry
│   │   ├── alice-executor     # action execution
│   │   └── alice-daemon       # background service binary + API server
│   ├── plugins/
│   │   └── ...                # out-of-tree plugins (optional)
│   └── config/
│       ├── example.toml
│       └── openapi.yaml       # REST and socket contract
├── frontend-desktop/          # Rust project 2: the desktop UI (Tauri)
│   ├── Cargo.toml
│   ├── src/
│   │   ├── bridge/            # REST and socket client
│   │   ├── state/
│   │   ├── pages/
│   │   ├── views/
│   │   └── components/
│   └── tauri.conf.json
└── frontend-web/              # TypeScript project 3: the web UI (Qwik)
    ├── package.json
    └── src/
        ├── routes/            # Qwik City routes
        ├── api/               # server$ that call backend REST
        ├── schemas/           # valibot schemas
        ├── components/
        ├── context/
        ├── types/
        └── lib/
```

If the backend uses a single crate, keep the same hierarchy inside `backend/src/`.
See [[backend/FOLDER_STRUCTURE]] for the single-crate layout.
The module structure mirrors the crate structure inside each project.
See [[frontend/OVERVIEW]] for the frontend split.

## Project Responsibilities

| Project | Crate/Package | Responsibility | Audio | UI |
| --- | --- | --- | --- | --- |
| backend | alice-core | Shared backend types, errors, config, events, DTOs | No | No |
| backend | alice-audio | Audio capture and wake word detection | Yes | No |
| backend | alice-transcribe | Fast transcriber and slow transcriber | No | No |
| backend | alice-intent | Intent database and intent resolver | No | No |
| backend | alice-plugin-host | Plugin trait, registry, and isolation | No | No |
| backend | alice-executor | Intent execution and side effects | No | No |
| backend | alice-daemon | Wiring, lifecycle, background task, REST/socket server | Yes | No |
| frontend-desktop | alice-frontend | Desktop UI, REST/socket client, state, views (Rust/Tauri) | No | Yes |
| frontend-web | frontend-web | Web UI, REST/socket client, state, views (Qwik/TypeScript) | No | Yes |

## Dependency and Boundary Rules

The two projects are isolated.
The isolation is strict.

Backend internal rules:

- alice-core has no dependency on other backend crates.
  All backend crates import alice-core.
- alice-audio imports only alice-core.
- alice-transcribe imports only alice-core and alice-audio types.
- alice-intent imports alice-core and alice-transcribe types.
  It does not import alice-audio.
- alice-plugin-host imports only alice-core.
- alice-executor imports alice-core, alice-intent, and alice-plugin-host.
- alice-daemon wires all backend crates.
  It is the only backend crate that imports all backend crates.
- A plugin imports alice-core and alice-plugin-host.
  It never imports the daemon.
- No backend crate forms a cycle.
  The graph is a directed acyclic graph.

Frontend-backend boundary rules:

- No frontend imports a backend crate, not even alice-core.
  All projects have separate dependencies.
- The frontends and the backend communicate only through REST and sockets.
  REST handles request-response (get status, get config, set config, list plugins).
  Sockets (WebSocket) handle streaming events (wake, transcript, intent, execution).
- The API contract lives in the backend and is documented as OpenAPI and as typed DTOs with JSON.
  The desktop frontend copies or generates the DTOs into `frontend-desktop/src/bridge/types.rs`.
  The web frontend copies or generates the DTOs into `frontend-web/src/types/dto.ts`.
  No frontend uses backend source code to get the types.
- The backend owns the API and the event schema.
  The frontends follow the schema.
  A breaking change needs a version bump (`/api/v1/` to `/api/v2/`).
- Each project has its own lock file and its own format and lint.
  The backend has `Cargo.lock`, `cargo fmt`, and `cargo clippy`.
  The desktop has `Cargo.lock`, `cargo fmt`, and `cargo clippy`.
  The web has `package-lock.json`, `eslint`, and `prettier`.
  See [[CODE_QUALITY]] and [[frontend/web/CODE_QUALITY]] for the baselines.

## Project Boundary Rule

The backend and the frontends are three separate projects.
They run as separate processes.
The backend runs as a daemon with a REST and socket server.
The desktop frontend runs as a native app with a REST and socket client.
The web frontend runs as a browser app that also uses REST and sockets.

- Do not add a workspace that contains more than one project.
  Keep `backend/Cargo.toml`, `frontend-desktop/Cargo.toml`, and `frontend-web/package.json` separate.
- Do not share code through `path` dependencies across the boundary.
  Share only the API contract (`backend/config/openapi.yaml`).
- Use REST for commands and queries.
  Use sockets for continuous events.
  See [[backend/ARCHITECTURE]] for the boundary protocol.
- The two frontends do not share code except for design tokens when you choose the token sharing option.
  See [[frontend/OVERVIEW]] for the sharing strategy.
  The optional URL sharing (desktop shows web URL) is allowed only for selected views and must not replace native capabilities.

## Single File Responsibility

One file has one concept.
One module has one domain.
One function has one action.

If a file does two things, split the file.
If a module covers two domains, split the module.
If a function needs two verbs, split the function.

See [[backend/CODE_STRUCTURE]], [[frontend/desktop/CODE_STRUCTURE]], and [[frontend/web/CODE_STRUCTURE]] for the file order.

## Documentation Language

All documentation prose follows ASD-STE100 Simplified Technical English.
It has short sentences, approved words, and the active voice.
It has no contractions and no Latin abbreviations.
The rule does not apply to code, identifiers, or command syntax.

User-facing text uses the user language.
Developer-facing messages and documentation use STE English.

## Definition of Done

A change is done when it passes all checks for its project.

Backend and desktop Rust checks:

- The code passes `cargo check` and `cargo clippy` with no warnings.
- The code passes `cargo fmt --check`.
- The unit tests pass with `cargo test`.
- The names follow [[backend/NAMING_CONVENTIONS]] or [[frontend/desktop/NAMING_CONVENTIONS]].
- The code follows the order in [[backend/CODE_STRUCTURE]] or [[frontend/desktop/CODE_STRUCTURE]].

Web TypeScript checks:

- The code passes `tsc --noEmit`.
- The code passes `eslint` with no warnings.
- The code passes `prettier --check`.
- The names follow [[frontend/web/NAMING_CONVENTIONS]].
- The code follows the order in [[frontend/web/CODE_STRUCTURE]].

All projects:

- The file has one responsibility.
  See [[backend/CODE_STRUCTURE]] section Single Responsibility.
- The API contract check passes when you change a DTO.
  See [[CODE_QUALITY]].
- The documentation is up to date and uses STE.

## Documentation Map

| File | Content |
| --- | --- |
| [[backend/ARCHITECTURE]] | Pipeline, state machine, events, and resource rules |
| [[backend/FOLDER_STRUCTURE]] | Backend folder and crate structure |
| [[backend/NAMING_CONVENTIONS]] | Names for files, types, functions, and modules |
| [[backend/CONVENTIONS]] | Data types, errors, async, and configuration |
| [[backend/CODE_STRUCTURE]] | Single responsibility and order inside a file |
| [[frontend/OVERVIEW]] | Two frontends strategy, boundary, and sharing options |
| [[frontend/desktop/FOLDER_STRUCTURE]] | Desktop folder layout (Tauri/Rust, REST/socket client) |
| [[frontend/desktop/NAMING_CONVENTIONS]] | Desktop names for files, types, and bridge |
| [[frontend/desktop/CODE_STRUCTURE]] | Desktop single responsibility and order inside a file |
| [[frontend/desktop/STATE]] | Desktop local and global state with REST/socket |
| [[frontend/desktop/STYLING]] | Desktop tokens and primitives |
| [[frontend/web/FOLDER_STRUCTURE]] | Web folder layout with Qwik routes, api, schemas |
| [[frontend/web/NAMING_CONVENTIONS]] | Web names for Qwik files and components |
| [[frontend/web/CODE_STRUCTURE]] | Web single responsibility and callback handle pattern |
| [[frontend/web/ROUTING]] | Web routes, layouts, loaders, and server$ |
| [[frontend/web/STATE]] | Web shared state with Qwik useContext and socket sync |
| [[frontend/web/STYLING]] | Web tokens and primitives with Tailwind |
| [[frontend/web/CODE_QUALITY]] | Web eslint, prettier, and testing baseline |
| [[CODE_QUALITY]] | Shared Rust and API contract quality baseline |
