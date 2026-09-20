# Desktop Naming Conventions

## Purpose

This document defines the names that the frontend uses.
It applies to files, components, functions, state, and IPC commands.
Follow these rules for all new code.
Use the same name for the same concept everywhere.
Do not use two names for the same concept.

## File Names

A file name is kebab-case.
The name uses lowercase letters and hyphens.
The file name ends with the role of the file when the file holds a UI concept.

Use these roles.

| Role | End of file name | Example |
| --- | --- | --- |
| A complete page | -page.rs | `home-page.rs` (or `home.rs` in pages folder) |
| A page-level view | -view.rs | `status-view.rs` |
| A section of a page | -section.rs | `wake-settings-section.rs` |
| A form | -form.rs | `settings-form.rs` |
| A small reusable piece | -partial.rs | `empty-state-partial.rs` (or in partials/) |
| A layout | -layout.rs | `app-layout.rs` |
| A state store | .rs (state name) | `app_state.rs` |
| A bridge command | .rs (domain) | `commands.rs` |
| A validation schema | .rs (domain) | `settings.rs` |

A file holds one concept.
Do not put two views in one file.

## Component Names

A component name is PascalCase.
The name describes the concept, not the file.

Use these patterns.

| Kind | Pattern | Example |
| --- | --- | --- |
| Page | <Domain>Page | `HomePage`, `SettingsPage` |
| View | <Domain>View | `StatusView`, `TranscriptView` |
| Section | <Domain>Section | `WakeSettingsSection`, `PluginListSection` |
| Form | <Domain>Form | `SettingsForm` |
| Partial | <Domain>Part or descriptive | `EmptyState`, `ListeningIndicator` |
| Primitive | descriptive noun | `Button`, `Card`, `TextInput` |

A page component ends with `Page`.
A view component ends with `View`.
A section component ends with `Section`.
A form component ends with `Form`.

## Function Names

A function name is snake_case.
The first word is a verb.
The name has the form verb + subject + qualifier.

Use these verbs.

| Group | Verbs | Use |
| --- | --- | --- |
| Retrieve | get, find, list | Read data |
| Change | create, update, delete, set | Write data |
| Transform | format, parse, convert, normalize | Change the form of data |
| Control | handle, on, show, hide | UI handlers |
| Bridge | fetch, send, subscribe | IPC |

Examples:

- `get_status` (bridge command)
- `set_wake_sensitivity`
- `format_confidence`
- `parse_transcript`
- `handle_save`
- `handle_microphone_toggle`

An event handler that the parent passes as a callback has the `on_` prefix and `$` is not used in Rust.

- `on_save`
- `on_toggle`
- `on_transcript`

The consumer defines the handler and passes it as a prop.
The handler wraps the bridge call.

```rust
let handle_save = move |data: SettingsInput| {
    let data = data.clone();
    async move { set_config(data).await }
};
```

## Props and State Names

A props struct ends with `Props`.

- `StatusViewProps`
- `TranscriptViewProps`
- `WakeSettingsSectionProps`

A DTO that the bridge returns omits the callback fields.

- `StatusViewData` is `StatusViewProps` without the callbacks.
  Derive it with `Omit` or define it explicitly.

A signal or store field uses the concept name.

- `is_listening`
- `transcript_text`
- `confidence`

A boolean field starts with `is_`, `has_`, or `should_`.

## State Names

A state file is `snake_case.rs`.
The context or store id has a lowercase name.

- `app_state` for the app status
- `settings_state` for the settings
- `history_state` for the history

The provider component ends with `Provider` when a context is used.

- `AppStateProvider`
- `SettingsProvider`

The hook to read the state starts with `use_`.

- `use_app_state()`
- `use_settings()`

## IPC Command Names

A bridge command is snake_case and starts with a verb.
The name describes the backend action.

- `get_status`
- `get_config`
- `set_config`
- `list_plugins`
- `enable_plugin`
- `disable_plugin`
- `subscribe_events`

The command returns a typed result.
Do not return untyped JSON.

```rust
pub async fn get_status() -> Result<AppStatus, BridgeError> { ... }
pub async fn set_config(input: ConfigInput) -> Result<(), BridgeError> { ... }
```

Event names match the backend `SystemEvent` variants in snake_case.

- `wake_detected`
- `transcript_interim`
- `intent_resolved`

## Type Names

A type name is PascalCase.

| Kind | Pattern | Example |
| --- | --- | --- |
| Props | <Domain>Props | `StatusViewProps` |
| Data | <Domain>Data | `StatusViewData` |
| Row or Item | <Domain>Row | `PluginRow`, `HistoryItem` |
| DTO | descriptive | `AppStatus`, `TranscriptDto` |
| Error | <Domain>Error | `BridgeError` |

A `Row` holds one entry in a list.
An `Item` holds one entry in a chart or timeline.
Keep the same suffix for the same shape everywhere.

## Route and Page Names

If the framework uses file-based routing, a route folder is kebab-case.
A dynamic segment uses `[param]`.
A group uses `(group)`.

```
pages/
  home.rs       # /
  settings.rs   # /settings
  plugins/
    mod.rs
    detail.rs   # /plugins/:id
```

The page component name matches the route: `HomePage` for `/` and `SettingsPage` for `/settings`.

## Style Token Names

A token name is kebab-case with the `ds-` prefix.

- `--ds-color-accent`
- `--ds-space-md`
- `--ds-radius-sm`

See [[STYLING]] for the token rules.

## Constant Names

A constant is SCREAMING_SNAKE_CASE.

- `DEFAULT_LISTEN_TIMEOUT_SECS`
- `MAX_HISTORY_ITEMS`

## Test Names

A test name describes the case in snake_case.

```rust
#[test]
fn status_view_shows_listening_indicator_when_listening() { }

#[test]
fn format_confidence_returns_percent_with_one_decimal() { }
```
