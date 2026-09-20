# Desktop Code Structure

## Purpose

This document defines the structure of desktop frontend code.
It covers single responsibility and the order inside a file.
It applies to all Rust UI code in `frontend-desktop/`.
See [[NAMING_CONVENTIONS]] for the names.
See [[FOLDER_STRUCTURE]] for the folders.
See [[../OVERVIEW]] for the desktop vs web split.

## Single Responsibility

One unit of code does one thing.
Use this rule for pages, views, sections, components, and files.

- A page loads data and composes views.
  It has no primitive markup except layout.
- A view renders one screen or one complete panel.
  It receives data as props and emits actions through callbacks.
  It does not call the bridge directly.
- A section renders one block of a page.
  It has its own props interface.
- A partial is a small reusable piece with no business logic.
- A primitive is a design system element with no domain logic.
- A file holds one component or one hook plus its helpers.

If a component does two things, split it.
Split a large view into sections.
Split a large section into partials.
Split a file with two concepts into two files.

The name of the component describes what it renders.
If the name needs two nouns with `and`, split the component.

## Order Inside a Component File

A component file has this order.

1. Module doc comment with `//!`.
2. Imports, grouped and sorted.
3. Props and data types.
4. Component function.
   Inside the function, keep this order:
   a. Props destructuring and derived variables.
   b. State hooks (`use_signal`, `use_store`, `use_context`, navigation hooks).
   c. Helper functions (plain functions).
   d. Handler closures (wrappers around callbacks or bridge calls).
   e. Effect hooks (`use_effect`, `use_resource`).
   f. Return value with the view markup.

Keep the order the same in every file.
A reader finds a value at the same place in every file.

Example (Dioxus style, adapt to Tauri host):

```rust
//! Status view that shows idle, listening, and transcript.

use dioxus::prelude::*;

use crate::components::primitives::{Badge, Card};
use crate::utils::format::format_confidence;

use super::listening_indicator::ListeningIndicator;

// Types

#[derive(Props, Clone, PartialEq)]
pub struct StatusViewProps {
    pub status: AppStatus,
    pub transcript: Option<String>,
    pub on_toggle_listening: EventHandler<()>,
}

// Component

pub fn StatusView(props: StatusViewProps) -> Element {
    // a. Derived
    let is_listening = props.status == AppStatus::Listening;

    // b. State (none for pure view; page owns state)

    // c. Helpers
    fn confidence_label(value: f32) -> String {
        format_confidence(value)
    }

    // d. Handlers
    let handle_toggle = move |_| props.on_toggle_listening.call(());

    // e. Effects (none for pure view)

    // f. Render
    rsx! {
        Card {
            ListeningIndicator { active: is_listening }
            Badge { label: confidence_label(props.status.confidence()) }
        }
    }
}
```

A pure view has no state hook.
A page holds the state and passes it as props.

## Order Inside a Page File

A page file has this order.

1. Imports.
2. Page props and page data types (if any).
3. Page component with:
   a. State hooks and bridge subscriptions.
   b. Data loading with `use_resource` or equivalent.
   c. Handler definitions that wrap bridge commands.
   d. Render that composes views and sections.

A page file does not hold bridge logic beyond handlers.
Put the REST and socket functions in `crate::bridge::rest` and `crate::bridge::socket`.
The page handler calls the bridge functions.

Example:

```rust
//! Home page: status, transcript, and recent history.

use dioxus::prelude::*;

use crate::bridge::rest::get_status;
use crate::bridge::socket::subscribe_events;
use crate::state::app_state::use_app_state;
use crate::views::status::status_view::StatusView;

pub fn HomePage() -> Element {
    let mut app_state = use_app_state();
    let status = use_resource(|| async { get_status(&client).await });

    let handle_toggle = move |_| {
        // call bridge, update state
    };

    rsx! {
        StatusView {
            status: status.read().clone().unwrap_or_default(),
            on_toggle_listening: handle_toggle,
        }
    }
}
```

## Order Inside a Helper File

A helper file has this order.

1. Imports.
2. Type definitions (input types before output types).
3. Constants.
4. Private helper functions.
5. Public functions.

A function body has this order.

1. Validate the input.
2. Get or transform the data.
3. Return the result.

Keep the steps separate.
Do not mix validation and transformation.

## Callback Pattern

A view triggers an action through a callback prop.
The callback prop starts with `on_` and has type `EventHandler` or `Callback`.

Do not pass a bridge function directly to a child.
Wrap it in a handler in the parent.

- The view defines `on_save: EventHandler<SettingsInput>` in its props.
- The page defines `handle_save` and passes it to the view.
- The view calls `props.on_save.call(data)`.

This pattern keeps the view independent from the host.
The view works with Tauri and with Dioxus.

## State and Bridge Separation

- The bridge folder owns all network access.
  A component never imports `reqwest`, `tungstenite`, or `tauri::invoke` directly.
  Only `crate::bridge` imports the REST and socket clients.
- The state folder owns the shared state.
  A view reads the state, it does not create it.
- The page loads the snapshot through REST and subscribes to the socket, then writes to the state.
  The view reads the state or receives it as props.
  See [[STATE]].
- No view or state file imports from `backend/`.
  The frontend is a separate project.
  The DTOs live in `crate::bridge::types`.

## File Size

A file has less than 300 lines.
A component with more than 200 lines needs a split into sections.
Split the file when the render grows or when helpers cover a second concept.

## Visibility

- Use `pub` for pages, views, and primitives that the app imports.
- Use `pub(crate)` for internal helpers that only the frontend needs.
- Keep helper functions private when they serve one component.

## Comments

- Add `///` doc comments to every public component and public function.
- Add `//!` module comment to every file.
- Use STE for all doc comments.
- Do not comment the markup.
  The markup should be readable without comments.
