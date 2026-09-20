# Backend Naming Conventions

## Purpose

This document defines the names that the backend uses.
It applies to files, modules, types, functions, and constants.
Follow these rules for all new code.
Use the same name for the same concept everywhere.
Do not use two names for the same concept.

## File and Module Names

A file name is snake_case.
The name uses lowercase letters and underscores.
The name describes the concept of the file.
The file holds one concept.

Use these patterns.

| Role | File name | Example |
| --- | --- | --- |
| Trait definition | trait name in snake_case | `transcriber.rs` for Transcriber |
| Struct implementation | struct name in snake_case | `wake_detector.rs` |
| Module root | mod.rs | `audio/mod.rs` |
| Binary entry | main.rs | `alice-daemon/src/main.rs` |
| Library root | lib.rs | `alice-core/src/lib.rs` |

One file exports one primary type or one primary trait.
Helper types for that concept live in the same file when they are small.
When helpers grow, move them to a submodule.

## Crate Names

A crate name is kebab-case with the `alice-` prefix.

- alice-core
- alice-audio
- alice-transcribe
- alice-intent
- alice-plugin-host
- alice-executor
- alice-daemon

Use the same prefix for plugins: `alice-plugin-*`.
Keep the name short and descriptive.

## Type Names

A type name is PascalCase.

| Kind | Pattern | Example |
| --- | --- | --- |
| Struct | Noun | `WakeDetector`, `ListeningSession` |
| Enum | Noun | `SystemEvent`, `StopReason` |
| Trait | Verb or capability | `Transcriber`, `IntentDatabase`, `Plugin` |
| Error | Noun + Error | `CoreError`, `TranscribeError` |

A DTO or event type keeps the domain name.

- `IntentDefinition`
- `ResolvedIntent`
- `TranscriptResult`
- `ExecutionContext`

Do not add a `T` prefix or a `Type` suffix.
Do not use abbreviations except well known ones (Id, Cpu).

## Function Names

A function name is snake_case.
The first word is a verb.
The verb describes the action.
The name has the form verb + subject + qualifier.

Use these verbs.

| Group | Verbs | Use |
| --- | --- | --- |
| Retrieve | get, find, list | Read data |
| Change | create, update, delete, insert, remove | Write data |
| Check | is, has, should | Boolean check |
| Transform | parse, format, convert, normalize, resolve | Change the form of data |
| Control | start, stop, spawn, cancel, handle | Lifecycle |
| Domain | detect, transcribe, resolve, execute | Domain action |

Examples:

- `find_intent_by_text`
- `get_candidates_for_text`
- `create_listening_session`
- `is_wake_detected`
- `parse_intent_text`
- `resolve_intent`
- `execute_intent`
- `start_capture`
- `stop_capture`
- `spawn_transcribe_task`

A predicate returns `bool` and starts with `is_`, `has_`, or `should_`.

Use `new` for constructors and `with_` for builders.

```rust
impl WakeDetector {
    pub fn new(config: WakeConfig) -> Self { ... }
    pub fn with_sensitivity(mut self, value: f32) -> Self { ... }
}
```

## Constant and Static Names

A constant is SCREAMING_SNAKE_CASE.

- `DEFAULT_WAKE_THRESHOLD`
- `MAX_AUDIO_BUFFER_SECS`
- `FAST_TRANSCRIBE_TIMEOUT_MS`

A static is also SCREAMING_SNAKE_CASE.
A global config instance uses the same rule when it is immutable.

## Variable Names

A variable is snake_case.
The name is descriptive but short.

- `wake_detector`
- `audio_buffer`
- `transcript_text`
- `intent_id`

Do not use single-letter names except for short closures or loop indices.
Use `tx` and `rx` only for channels when the context is clear.
Prefer `event_tx` and `event_rx`.

## Enum Variant Names

A variant is PascalCase.
The variant describes one case.

```rust
pub enum StopReason {
    SilenceTimeout,
    UserStopped,
    Cancelled,
    Error(String),
}
```

Do not add the enum name as a prefix to the variant.

## Error Variant Names

An error variant describes the failure cause.

```rust
pub enum CoreError {
    AudioCaptureFailed(String),
    WakeModelMissing { path: String },
    TranscribeFailed { reason: String },
    IntentNotFound { text: String },
    PluginTimeout { plugin: PluginId },
}
```

Use `Failed`, `Missing`, `NotFound`, and `Timeout` as suffixes.
Do not use `Error` as a variant suffix.

## Trait and Method Names

A trait name is a capability.

- `Transcriber` with `transcribe_chunk` and `finalize`
- `IntentDatabase` with `find_candidates` and `get_intent`
- `Plugin` with `execute`

A method name follows the function verb rules.
A trait method does not use `get_` when it clearly gets data; use the noun.

```rust
trait IntentDatabase {
    fn candidates(&self, text: &str) -> Vec<IntentCandidate>;
    fn intent(&self, id: &IntentId) -> Option<IntentDefinition>;
}
```

Keep the trait small.
A trait has one responsibility.

## Feature and Config Names

A config key is snake_case in TOML and maps to snake_case fields in Rust.

- `wake_sensitivity`
- `listen_timeout_secs`
- `slow_transcriber_model_path`

A feature flag is kebab-case in `Cargo.toml`.

- `wake-detection`
- `slow-correction`

## Test Names

A test function describes the case in snake_case.

```rust
#[test]
fn resolve_intent_returns_best_match_when_confidence_is_high() { }

#[test]
fn wake_detector_ignores_noise_below_threshold() { }
```

Use `test_` prefix only when needed for grouping.
Prefer descriptive sentences without `test_`.

## Generic Names

A generic parameter is PascalCase with one letter or a descriptive name.

- `T`, `E` for short generics
- `Item`, `Ctx` for clear generics when bounds explain the role

Do not use `_T` or `TItem`.
