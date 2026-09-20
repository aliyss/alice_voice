# Backend Conventions

## Purpose

This document defines the conventions that the backend uses.
It covers data types, errors, async, configuration, and logging.
Follow these rules for all backend changes.
See [[ARCHITECTURE]] for the pipeline structure.

## Language and Edition

Use Rust stable and the current edition.
Use workspace dependencies in `backend/Cargo.toml`.
Do not duplicate version numbers in backend crate manifests.
The frontend keeps its own dependencies in `frontend/Cargo.toml`.

## Data Types

Use the correct type for each kind of value.

| Kind of value | Type |
| --- | --- |
| Timestamp from audio | `std::time::Instant` for monotonic, `chrono::DateTime<Utc>` for wall time |
| Duration | `std::time::Duration` |
| Audio samples | `&[f32]` at 16 kHz mono, normalized to -1.0 to 1.0 |
| Transcript text | `String`, trimmed, non-empty when valid |
| Confidence | `f32` in range 0.0 to 1.0 |
| Identifiers | `String` or newtype `struct IntentId(String)` |
| Money or precise decimal | `rust_decimal::Decimal` when needed, never `f32` for money |
| Optional value | `Option<T>` |
| Collection of intents | `Vec<IntentDefinition>` |

Create a newtype for identifiers.

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct IntentId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginId(pub String);
```

Do not use raw `String` for two different identifier kinds in one function.

## Error Handling

Use `thiserror` for library errors and `anyhow` only at the binary boundary.
Define one error enum per crate and map it to `CoreError` through `From`.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TranscribeError {
    #[error("model missing at {path}")]
    ModelMissing { path: String },
    #[error("transcribe failed: {reason}")]
    Failed { reason: String },
}
```

Rules:

- Do not use `unwrap` or `expect` in production code.
  Use `unwrap` only in tests.
- Do not use `panic!` for expected failures.
- Return `Result<T, CoreError>` from public functions.
- Map internal errors with `map_err` and provide context.
- Log the error with `tracing::error!` before you return it when you handle it at the boundary.
- Do not swallow errors silently.
  Return them or log them.

## Async and Concurrency

Use `tokio` as the async runtime.
One runtime lives in the daemon.
Do not create multiple runtimes.

Rules:

- Mark traits as `Send + Sync` when they cross tasks.
- Use `tokio::sync::mpsc` for one-to-one work and `broadcast` for events.
- Use `CancellationToken` or `tokio_util::sync::CancellationToken` for shutdown.
- Use `tokio::select!` for timeouts and cancellation.
- Do not block the async thread with CPU work.
  Use `tokio::task::spawn_blocking` for model inference.
- Keep the audio capture task separate from transcribe tasks.
  Do not let slow transcription block audio capture.

Example timeout:

```rust
tokio::select! {
    result = slow_transcribe(audio) => handle_result(result),
    _ = sleep(Duration::from_secs(5)) => handle_timeout(),
    _ = cancel_token.cancelled() => handle_cancel(),
}
```

## API Contract

The backend defines the contract for REST and sockets.
The contract includes all DTOs, status codes, and socket messages.

- Define DTOs in `backend/crates/alice-core/src/dto.rs` or `backend/crates/alice-daemon/src/server/dto.rs`.
  Derive `Serialize` and `Deserialize` with `serde`.
- Use `snake_case` field names and `PascalCase` DTO names.
- Document every DTO with `///` that states the unit and the range.
- Export an OpenAPI file at `backend/config/openapi.yaml` when possible.
  The OpenAPI file is the canonical contract for the frontend.
- Version the API path such as `/api/v1/status`.
  Keep old paths until the frontend migrates.

Example DTO:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStatusDto {
    /// Current daemon state: Idle, Listening, Transcribing, Resolving, Executing
    pub state: String,
    /// Last interim transcript, if any
    pub transcript_interim: Option<String>,
}
```

Socket messages use the same DTOs.
The backend pushes `SystemEvent` as JSON through `WS /api/events`.
The frontend deserializes them into `bridge/types.rs`.

## Configuration

Define typed config in `alice-core`.
No crate reads `env` or files directly except `alice-core`.

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CoreConfig {
    pub wake: WakeConfig,
    pub transcribe: TranscribeConfig,
    pub intent: IntentConfig,
    pub executor: ExecutorConfig,
    pub server: ServerConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WakeConfig {
    pub sensitivity: f32,
    pub model_path: PathBuf,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// REST and socket listen address, example: 127.0.0.1:8765
    pub listen_addr: String,
}
```

Rules:

- Provide defaults for all fields.
- Validate the config at startup and fail fast with a clear error.
- Load the config once and share it with `Arc<CoreConfig>`.
- Use `config` crate or `serde` with `toml`.
- Document each field with a comment that states the unit and the range.
- Keep server address and ports in config.
  Do not hard-code them.

## Logging and Tracing

Use `tracing` for all logs.
Do not use `println!` in library code.

Rules:

- Use `tracing::info!` for state transitions.
- Use `tracing::debug!` for detailed pipeline steps.
- Use `tracing::warn!` for recoverable issues.
- Use `tracing::error!` for failures that need attention.
- Add `#[tracing::instrument]` to public async functions that perform work.
- Include `intent_id`, `plugin_id`, and `duration_ms` as fields when you log execution.
- Do not log raw audio or sensitive user data.

## Resource and Memory

- Use a ring buffer with a fixed capacity for audio.
  The capacity is `sample_rate * max_secs`.
- Release audio buffers after transcription.
  Do not hold buffers across Idle.
- Limit the number of concurrent transcribe tasks to one fast and one slow task.
- Measure Idle memory and document the budget in the config.

## Ownership and Borrowing

- Pass large buffers by reference (`&[f32]`) or `Arc<Vec<f32>>`.
  Do not clone buffers unless you need ownership on another task.
- Return owned data when the caller needs to keep it.
- Use `Arc` for shared config and shared registries.
- Use `&self` for read methods and `&mut self` only when the method changes state.

## Testing Conventions

- Put unit tests in the same file with `#[cfg(test)] mod tests`.
- Name the test file `*.test.rs` only for integration tests in `tests/`.
- Use a fake database or fake plugin for intent tests.
- Use `proptest` or fixtures for audio edge cases.
- Do not test with real model files in CI by default.
  Gate model tests with a feature flag.

## Safety and FFI

- Prefer safe Rust.
- Wrap unsafe FFI calls in a safe abstraction.
  The safe wrapper validates inputs and handles errors.
- Document safety invariants with `// SAFETY:` comments.
