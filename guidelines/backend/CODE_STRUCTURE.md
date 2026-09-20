# Backend Code Structure

## Purpose

This document defines the structure of backend code.
It covers single responsibility and the order inside a file.
It applies to all Rust crates, including alice-core and alice-daemon.
See [[NAMING_CONVENTIONS]] for the names.
See [[FOLDER_STRUCTURE]] for the folders.

## Single Responsibility

One unit of code does one thing.
Use this rule for crates, modules, structs, traits, and files.

- A crate handles one domain (audio, transcribe, intent, plugin).
- A module defines one trait or one struct plus its helpers.
- A struct holds one state.
- A trait has one capability.
- A function does one action.
- A file holds one concept.

If a unit does two things, split it.
Split a large struct into two structs.
Split a large function into helpers.
Split a file with two concepts into two files.

The name of the unit describes what it does.
If the name needs two verbs, split the unit.

Examples:

- `wake.rs` detects the wake word only. It does not capture audio.
- `fast.rs` runs the fast transcriber only. It does not correct the result.
- `resolver.rs` resolves intent only. It does not execute the intent.
- `registry.rs` manages the plugin registry only. It does not run plugins.

## Grouping and Hierarchy

Group code by domain, not by type.
Do not create folders like `utils`, `helpers`, or `common` for mixed code.
Each folder has a clear owner.

```
audio/          # all audio code
  capture.rs
  wake.rs
  buffer.rs
transcribe/     # all transcribe code
  fast.rs
  slow.rs
  pipeline.rs
```

A shared helper that two crates need lives in alice-core.
A helper that one crate needs lives in that crate.
Do not create a crate for helpers only.

Hierarchy rules:

- The parent module exports the public API.
- The child module implements the detail.
- No child imports its sibling through the parent.
  Use direct crate imports: `crate::audio::buffer::RingBuffer`.

## Order Inside a File

A file has this order.

1. Module doc comment with `//!` that describes the purpose.
2. Imports, grouped and sorted.
3. Type definitions (structs, enums, traits).
4. Constants.
5. Private helper functions and private impl blocks.
6. Public impl blocks and public functions.
7. Trait implementations (`impl Trait for Type`).
8. Tests with `#[cfg(test)]`.

Example:

```rust
//! Wake word detection with a small model.
//! This module runs in Idle and emits WakeDetected.

use std::path::PathBuf;
use std::sync::Arc;

use tracing::instrument;

use crate::config::WakeConfig;
use crate::error::{CoreError, Result};

// Types

pub struct WakeDetector { ... }

pub struct WakeEvent { ... }

// Constants

const DEFAULT_THRESHOLD: f32 = 0.7;

// Private helpers

fn normalize_samples(input: &[f32]) -> Vec<f32> { ... }

// Public API

impl WakeDetector {
    pub fn new(config: WakeConfig) -> Result<Self> { ... }

    #[instrument(skip(self, samples))]
    pub fn detect(&mut self, samples: &[f32]) -> Result<Option<WakeEvent>> { ... }
}

// Trait impls

impl Drop for WakeDetector { ... }

#[cfg(test)]
mod tests { ... }
```

Keep the order the same in every file.
A reader finds a value at the same place in every file.

## Import Order

Group imports in this order and sort alphabetically inside each group.

1. `std` imports.
2. External crate imports.
3. Workspace crate imports (`alice-core`, `alice-audio`).
4. Local imports (`crate::`, `self::`, `super::`).

Separate the groups with one blank line.
Use one `use` per module, merge symbols from the same module.

```rust
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::instrument;

use alice_core::config::WakeConfig;
use alice_core::error::{CoreError, Result};

use crate::buffer::RingBuffer;
```

Do not use glob imports (`use foo::*`) except in tests.

## Order Inside a Function

A function body has this order.

1. Validate the input.
2. Get or change the data if required.
3. Transform the data.
4. Return the result.

Keep the steps separate.
Do not mix validation and data access.

```rust
pub fn resolve_intent(&self, text: &str) -> Result<ResolvedIntent> {
    // 1. Validate
    let text = text.trim();
    if text.is_empty() {
        return Err(CoreError::InvalidInput { reason: "text is empty".into() });
    }

    // 2. Get data
    let candidates = self.database.find_candidates(text);

    // 3. Transform
    let best = Self::pick_best(candidates)?;

    // 4. Return
    Ok(best)
}
```

## Trait Definition Order

A trait file has this order.

1. Imports.
2. Trait definition with doc comments for each method.
3. Associated types if any.
4. Helper types.

Keep the trait small (two to five methods).
If a trait grows, split it into two traits.

## Struct and Impl Order

A struct file has this order.

1. Struct definition.
2. `impl` with constructors (`new`, `with_*`).
3. `impl` with core methods in logical order.
4. Private helpers.

```rust
pub struct ListeningSession { ... }

impl ListeningSession {
    pub fn new(config: SessionConfig) -> Self { ... }
    pub fn start(&mut self) -> Result<()> { ... }
    pub fn push_samples(&mut self, samples: &[f32]) -> Result<()> { ... }
    pub fn stop(&mut self) -> Vec<f32> { ... }
}

fn trim_silence(samples: &[f32]) -> &[f32] { ... }
```

## Error and Event File Order

An error file lists variants from most common to most specific.
An event file lists variants in pipeline order (wake, listen, transcribe, intent, execute).

Keep each variant doc comment short and precise.

## Function Size and Splitting

- A function has less than 50 lines.
  If it grows, extract helpers.
- A function has less than four parameters.
  If it needs more, group them in a config struct.
- A function returns one result type.
  Use a struct for multiple return values, not a tuple with more than two elements.

## Visibility

- Use `pub` only for the intended public API.
- Use `pub(crate)` for symbols that the daemon needs but external users do not need.
- Use private visibility for helpers.
- Do not use `pub` to avoid import friction.

## Comments

- Add a doc comment `///` to every public item.
  The comment states what the item does and what it returns.
- Add a `// SAFETY:` comment to every unsafe block.
- Add a `// NOTE:` comment to non-obvious decisions.
- Do not comment obvious code.
- Use STE for all doc comments.

## Module Size

A file has less than 300 lines.
A module with more than three files becomes a folder with `mod.rs`.
Split the module when you add a second responsibility.

Check with `wc -l` or `tokei` when you review.
A large file signals that the module needs a split.
