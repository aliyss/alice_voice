# Code Quality Baseline

## Purpose

This document defines the code quality baseline for the Rust parts of Alice Voice.
It applies to `backend/` and `frontend-desktop/`.
Both projects use the same Rust formatting and linting rules.
They have separate manifests but the same baseline.
The web frontend has its own baseline in [[frontend/web/CODE_QUALITY]].
See [[GUIDELINES]] for the definition of done and the three-project boundary.

## Formatting

Use `rustfmt` for all Rust code.
Do not edit the style manually.
The formatter is the authority.

Rules:

- Run `cargo fmt --check` in CI for each Rust project.
  The check fails when the code is not formatted.
- Run `cargo fmt` before you commit in each Rust project.
- Keep one `rustfmt.toml` per Rust project at `backend/rustfmt.toml` and `frontend-desktop/rustfmt.toml`.
  Keep the files identical.
  Both projects use the same style.
- Use `printWidth` 100, `tabWidth` 4, and `edition` 2021 or later.
- The import order follows [[backend/CODE_STRUCTURE]] and [[frontend/desktop/CODE_STRUCTURE]].
  `rustfmt` sorts the imports; do not sort them by hand.

No crate inside a project has its own formatting config.
Change the `rustfmt.toml` in both Rust projects when you need a change.

## Linting

Use `clippy` for all Rust code.
The lint level is strict.

Run these commands in each Rust project and fix all warnings.

```sh
# backend
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace

# frontend-desktop
cargo clippy --all-targets --all-features -- -D warnings
cargo check
```

Rules:

- Do not suppress a clippy warning with `allow` without a comment.
  The comment states the reason and the scope.
- Keep `pedantic` warnings as `warn` and `nursery` as `allow`.
  Set the level in `lints` table in `backend/Cargo.toml` and in `frontend-desktop/Cargo.toml`.

```toml
[workspace.lints.clippy]
pedantic = "warn"
nursery = "allow"
unwrap_used = "deny"
expect_used = "warn"
```

- `unwrap_used` is `deny` in library code.
  Use `expect` only in tests or at startup with a clear message.
- Fix `unwrap_used` by returning `Result` or by handling the `Option`.

## Type Safety

- Do not use `unsafe` without a `SAFETY` comment.
- Do not use `any` or untyped maps.
  Define a typed struct or enum.
- Define newtypes for identifiers.
  See [[backend/CONVENTIONS]].
- Use `Result<T, CoreError>` for public functions.
  See [[backend/CONVENTIONS]].

## Testing

Write a unit test for pure logic.
Examples: intent matching, text normalization, confidence ranking, pipeline merging.
A pure function has no side effects and no audio access.

Rules:

- Put the test next to the code with `#[cfg(test)]`.
- Name the test to describe the case in snake_case.
- Run the tests per Rust project: `cargo test --workspace` in `backend/` and `cargo test` in `frontend-desktop/`.
- Keep tests fast.
  Mark slow tests with `#[ignore]` and run them with `cargo test -- --ignored`.

Structure of a test:

1. Set up the input.
2. Call the function.
3. Check the result.

Do not test generated code or large model inference in the default suite.
Gate model tests with a feature flag.

Example:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_intent_returns_best_match_when_confidence_is_high() {
        let db = fake_database();
        let resolver = Resolver::new(db);
        let result = resolver.resolve("set timer for five minutes").unwrap();
        assert_eq!(result.intent_id.0, "set_timer");
        assert!(result.confidence > 0.8);
    }
}
```

Integration tests live in `tests/` at the crate root or at `backend/tests/` and `frontend-desktop/tests/`.
They test the daemon lifecycle and the REST/socket boundary.
Use contract tests that verify the frontend DTOs (`frontend-desktop/src/bridge/types.rs` and `frontend-web/src/types/dto.ts`) against `backend/config/openapi.yaml`.

## Documentation

- Add `///` doc comments to every public item.
- Add `//!` module comments to every file.
- Write the doc comments in STE.
- Keep the example in the doc comment runnable when possible.

Check docs per Rust project with `cargo doc --workspace --no-deps` in `backend/` and `cargo doc --no-deps` in `frontend-desktop/`.

## Scripts and CI

Both Rust projects use the same scripts.
Define them per project in `.cargo/config.toml` or as `Makefile.toml` tasks.

```sh
# backend
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps

# frontend-desktop
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc --no-deps
```

CI runs these checks for all three projects on every commit.

- `fmt` must pass in `backend/` and `frontend-desktop/` (rustfmt) and in `frontend-web/` (prettier).
- `clippy` and `eslint` must have no warnings.
- `test` must pass in all projects.
- `doc` must build in Rust projects and `tsc --noEmit` must pass in web.
- The API contract check must pass: `frontend-desktop/src/bridge/types.rs` and `frontend-web/src/types/dto.ts` must match `backend/config/openapi.yaml`.
  See [[frontend/web/CODE_QUALITY]] for the web checks.

Add a pre-commit hook that runs `cargo fmt` and `cargo clippy` in the changed Rust project or `prettier` and `eslint` in the web project.
The hook does not replace CI; CI is the authority.

## Single Responsibility Check

During code review, check these items.

- One file has one concept.
  The file name matches the concept.
- One function has one action.
  The name has one verb.
- One module has one domain.
  The domain does not leak to another module.
- One crate or one Qwik route has one layer or one domain.
  See [[backend/ARCHITECTURE]], [[frontend/desktop/FOLDER_STRUCTURE]], and [[frontend/web/FOLDER_STRUCTURE]].
- Group code by domain, not by type.
  There is no `utils` bag for unrelated helpers.

If a file or function fails the check, split it before you merge.

## Audits

- Measure Idle CPU and memory after each release.
  Keep Idle near zero.
- Measure wake detection latency and transcribe latency.
  Document the numbers in the release notes.
- Review dependency licenses with `cargo deny`.
- Review security advisories with `cargo audit`.

## Language Rule

All documentation prose uses STE.
The prose has short sentences, approved words, and the active voice.
It has no contractions and no Latin abbreviations.
The rule does not apply to code or command syntax.
See [[GUIDELINES]] for the language rule.
