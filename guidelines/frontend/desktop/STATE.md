# Desktop State

## Purpose

This document defines how the frontend manages state.
It covers local state, global state, and synchronization with the backend.
It applies to the separate frontend project in `frontend/` with any host (Tauri or Dioxus).
The frontend synchronizes with the backend only through REST and sockets.
See [[FOLDER_STRUCTURE]] for the folders and [[../GUIDELINES]] for the boundary.

## State Kinds

Use the lightest state that fits the need.

| State kind | Scope | Use |
| --- | --- | --- |
| Local signal or store | One component | Form input, toggle, open/closed, draft text |
| Page resource | One page | Data that the page loads from the bridge |
| Global state or context | Many components | App status, listening state, settings, history |

Do not put page data in global state.
Do not put global config in local state.

## Local State

Use local state for UI that belongs to one component.

- Use `use_signal` for a single value.
- Use `use_store` or a struct with signals for a group of related values.
- Do not lift local state to the global store unless two pages need it.

Example:

```rust
let mut is_open = use_signal(|| false);
let mut draft = use_signal(|| String::new());
```

A form has its own state.
The form state lives in the form component.
Do not use global state for a form draft.

## Global State

Put shared state in `src/state`.
One file holds one domain.
Each file exports a context id or store and a hook to read it.

```
state/
  app_state.rs    # status: Idle, Listening, Transcribing, Resolving, Executing
  settings.rs     # user settings and config
  history.rs      # recent transcripts and executions
```

Rules:

- Define the state types from the API contract, not from backend source code.
  Copy or generate the DTOs into `frontend/src/bridge/types.rs` and use them in the state store.
  See [[FOLDER_STRUCTURE]] for the bridge types.
- Keep the global state serializable.
  Do not put a future, a channel, or a non-serializable handle in the state.
  Keep the socket handle in `bridge/socket.rs`, not in the state.
- Mount the provider once at the app root (`app.rs`).
  Feed it with the initial value from REST (`GET /api/status`) or with defaults.
- A component reads the state with the hook (`use_app_state()`).
  The hook returns a read handle; writes go through explicit setters.

Example:

```rust
// state/app_state.rs
#[derive(Clone, PartialEq)]
pub struct AppState {
    pub status: AppStatus, // from bridge/types.rs, matches backend DTO
    pub transcript_interim: Option<String>,
    pub transcript_final: Option<String>,
    pub last_intent: Option<IntentId>,
}

pub fn use_app_state() -> Signal<AppState> {
    use_context::<Signal<AppState>>()
}
```

## When to Use a Context

Use a context when many components read the same value.
Do not use a context for data that one page loads.

| Use a context | Do not use a context |
| --- | --- |
| App status and listening state | Page data from `get_status` (pass as props) |
| User settings and plugin enable flags | One-off fetch result |
| History that the sidebar and home page share | Form draft |
| Theme or locale | Non-serializable bridge handle |

## Bridge Synchronization

The backend is the source of truth for status and transcripts.
The frontend mirrors the backend through REST and sockets.

- REST gives the current state on demand.
- Sockets push state changes as they happen.

Flow:

1. On launch, the app loads the snapshot through REST.
   Call `GET /api/status` and `GET /api/config` from `bridge/rest.rs`.
2. Then the app opens one WebSocket to `WS /api/events` through `bridge/socket.rs`.
3. The socket handler writes each event to the global state.
4. The views read the global state and render.

```rust
// bridge/socket.rs
pub async fn subscribe_events(client: &ApiClient, state: Signal<AppState>) {
    let mut stream = client.subscribe_events().await; // WS /api/events
    while let Some(event) = stream.next().await {
        match event {
            SystemEvent::ListeningStarted => state.write().status = AppStatus::Listening,
            SystemEvent::TranscriptInterim { text } => state.write().transcript_interim = Some(text),
            SystemEvent::TranscriptFinal { text } => state.write().transcript_final = Some(text),
            _ => {}
        }
    }
}
```

Rules:

- Subscribe to the socket once at the app or page root.
  Do not subscribe in every view.
- Load the initial snapshot with REST before you subscribe to the socket.
  Handle the gap between REST snapshot and first socket event.
- Write to the state only through the socket handler or through a REST result.
- Do not poll the backend in a loop.
  Use the socket stream; use REST only for request-response or for retry after disconnect.
- Reconnect the socket with backoff when the connection drops.
  Reload the snapshot with REST after reconnect to catch missed events.

## Page Data Loading

Load page data through REST with a resource that the page owns.

```rust
let status = use_resource(|| async {
    rest::get_status(&client).await.unwrap_or_default() // GET /api/status
});
```

- The page passes the loaded data to the view as props.
- The view does not call REST or open a socket for shared data; it reads the global state.
- The page retries on error and shows an empty state or an error partial.
  See [[STYLING]] for feedback elements.
- Use REST for queries that need the latest value (history, plugins).
  Use the socket for pushes that the backend emits (transcript, intent).

## State Updates

Update the state in one place.
The handler in the page calls REST and then updates the state.
The backend may also push the new state through the socket; handle both.

```rust
let handle_set_sensitivity = move |value: f32| {
    let mut state = state;
    let client = client.clone();
    async move {
        match rest::set_config(&client, ConfigInput { wake_sensitivity: value }).await {
            Ok(_) => state.write().settings.wake_sensitivity = value, // REST PUT /api/config
            Err(e) => state.write().error = Some(e.to_string()),
        }
    }
};
```

- Do not update the backend and the state in two separate steps without error handling.
- The socket may echo the change after REST succeeds.
  Make the state update idempotent so duplicate events do not break the UI.
- Show the error in the UI when the REST call fails.
  Do not silently ignore the failure.

## Persistence

Store settings that survive restart in the backend.
The backend owns persistence.
The frontend does not write files.

- The frontend sends `PUT /api/config` through REST.
- The backend validates the input and writes the config file.
- The frontend reloads with `GET /api/config` on next launch or after reconnect.

Do not write config files from the frontend directly.
Do not use host storage for settings that the backend owns.

## Performance

- Keep global state small.
  Do not put large transcript history in the global state; keep the last N items.
- Derive computed values in the view, not in the store.
  Use a plain function: `fn is_idle(state: &AppState) -> bool`.
- Avoid cloning large state in every view.
  Pass references or slices when the framework permits it.
