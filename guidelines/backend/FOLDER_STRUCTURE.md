# Backend Folder Structure

## Purpose

This document defines the folder structure of the backend project.
It covers the backend workspace layout and the single-crate layout.
The backend is a separate Rust project from the frontend.
It communicates with the frontend only through REST and sockets.
For the names of files and types, see [[NAMING_CONVENTIONS]].
For the architecture, see [[ARCHITECTURE]].

## Repository Boundary

The repository has two top-level projects.

```
alice_voice/
├── guidelines/                # these documents
├── backend/                   # project 1: Rust daemon
└── frontend/                  # project 2: Rust UI (separate manifest)
```

The backend has its own `Cargo.toml` and its own `Cargo.lock`.
The frontend has its own `Cargo.toml` and its own `Cargo.lock`.
There is no root `Cargo.toml` that contains both projects.
A backend crate never imports a frontend crate and vice versa.
The boundary is REST and sockets only.
See [[../GUIDELINES]] for the boundary rules.

## Backend Workspace Layout

Use a Cargo workspace inside `backend/` with one crate per domain.
Each crate has one responsibility.
The workspace manifest is `backend/Cargo.toml`.

```
backend/
├── Cargo.toml                 # backend workspace manifest
├── Cargo.lock
├── crates/
│   ├── alice-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs         # public exports only
│   │       ├── error.rs       # CoreError and Result
│   │       ├── config.rs      # typed configuration
│   │       ├── event.rs       # SystemEvent enum
│   │       ├── intent.rs      # IntentId, IntentDefinition, ResolvedIntent
│   │       ├── types.rs       # shared primitives
│   │       └── dto.rs         # REST and socket DTOs (serialized)
│   ├── alice-audio/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── capture.rs     # microphone capture
│   │       ├── wake.rs        # wake word detector
│   │       ├── buffer.rs      # ring buffer
│   │       └── session.rs     # listening session
│   ├── alice-transcribe/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs      # Transcriber trait
│   │       ├── fast.rs        # fast transcriber
│   │       ├── slow.rs        # slow transcriber
│   │       └── pipeline.rs    # merge fast and slow results
│   ├── alice-intent/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── database.rs    # IntentDatabase trait and impl
│   │       ├── resolver.rs    # intent resolver
│   │       └── matcher.rs     # text matching logic
│   ├── alice-plugin-host/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── plugin.rs      # Plugin trait
│   │       ├── registry.rs    # plugin registry
│   │       └── isolation.rs   # isolation and timeout
│   ├── alice-executor/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── execute.rs     # execution logic
│   │       └── context.rs     # ExecutionContext
│   └── alice-daemon/
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs        # binary entry, minimal
│       │   ├── lifecycle.rs   # state machine and wiring
│       │   ├── supervisor.rs  # task supervision
│       │   └── server/
│       │       ├── mod.rs     # REST + socket server mount
│       │       ├── rest.rs    # REST handlers
│       │       ├── socket.rs  # WebSocket event push
│       │       ├── routes.rs  # route table
│       │       └── dto.rs     # HTTP request/response types
│       └── benches/
├── plugins/
│   ├── example-plugin/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── ...
├── config/
│   ├── example.toml
│   └── openapi.yaml           # REST and socket contract (optional but recommended)
└── tests/
    └── integration/
```

Rules:

- One domain lives in one crate.
  Do not mix audio and intent in one crate.
- One module in a crate handles one concept.
  See [[CODE_STRUCTURE]] for single responsibility.
- The barrel file `lib.rs` exports the public API only.
  It does not hold logic.
- Keep the daemon binary thin.
  Put logic in `lifecycle.rs` and `supervisor.rs`.
- Put all network code in `alice-daemon/src/server/`.
  No other crate opens a port or a socket.
- Keep the DTOs that cross the network in `alice-core/src/dto.rs` or `alice-daemon/src/server/dto.rs`.
  They are the contract with the frontend.
- Keep tests next to the code or in `backend/tests/` for integration tests.

## Single-Crate Backend Layout

If the backend uses one crate, mirror the crate structure with modules.
The single crate is `backend/src/` with `backend/Cargo.toml`.

```
backend/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── lib.rs
    ├── core/
    │   ├── mod.rs
    │   ├── error.rs
    │   ├── config.rs
    │   ├── event.rs
    │   ├── intent.rs
    │   └── dto.rs
    ├── audio/
    │   ├── mod.rs
    │   ├── capture.rs
    │   ├── wake.rs
    │   ├── buffer.rs
    │   └── session.rs
    ├── transcribe/
    │   ├── mod.rs
    │   ├── traits.rs
    │   ├── fast.rs
    │   ├── slow.rs
    │   └── pipeline.rs
    ├── intent/
    │   ├── mod.rs
    │   ├── database.rs
    │   ├── resolver.rs
    │   └── matcher.rs
    ├── plugin_host/
    │   ├── mod.rs
    │   ├── plugin.rs
    │   ├── registry.rs
    │   └── isolation.rs
    ├── executor/
    │   ├── mod.rs
    │   ├── execute.rs
    │   └── context.rs
    └── server/
        ├── mod.rs
        ├── rest.rs
        ├── socket.rs
        ├── routes.rs
        └── dto.rs
```

Do not put all code in one file.
Each `mod.rs` exports the public symbols of the module only.
Keep the server modules grouped together.

## Module Rules

- One module defines one trait or one struct plus its helpers.
- A module file has less than 300 lines.
  If the file grows, split the module into submodules.
- Group related files in a folder.
  Use `mod.rs` or the folder name as the module root.
- Keep `types.rs` and `error.rs` small.
  They hold only shared definitions.

## Public Exports

Each crate exposes a clear public API through `lib.rs`.

```rust
// backend/crates/alice-audio/src/lib.rs
pub mod buffer;
pub mod capture;
pub mod session;
pub mod wake;

pub use buffer::RingBuffer;
pub use capture::AudioCapture;
pub use session::ListeningSession;
pub use wake::WakeDetector;
```

- Do not export internal helpers.
- Do not use `pub use` for test-only symbols.
- Use `pub(crate)` for symbols that only the daemon needs.

## Resource and Config Files

- Put model files outside the crate.
  Reference them through config paths.
- Put backend configuration examples in `backend/config/example.toml`.
- Put the REST and socket contract in `backend/config/openapi.yaml` when you use OpenAPI.
  The frontend uses this file to generate types.
- Put test fixtures in `backend/crates/<crate>/tests/fixtures/` or `backend/tests/fixtures/`.
- Do not commit large model binaries.
  Document how to fetch them.

## Import Rules

- A backend crate imports `alice-core` for shared types.
- A backend crate does not import `alice-daemon` except the daemon itself.
- A backend crate does not import any frontend crate.
- A plugin crate imports `alice-core` and `alice-plugin-host`.
- Use absolute imports inside a crate (`crate::audio::capture`).
- Use workspace imports for external crates.
  Define versions in `backend/Cargo.toml` workspace dependencies.

See [[../GUIDELINES]] for the dependency and boundary graph.
