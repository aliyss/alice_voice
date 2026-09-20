# Desktop Styling

## Purpose

This document defines how the frontend styles the UI.
It covers tokens, primitives, and the CSS rules.
It applies to alice-frontend with any host.
The rules keep the result the same whether the host is Tauri or Dioxus.

## Design Tokens

All visual values come from tokens.
A token is a CSS custom property with the `ds-` prefix.
The tokens live in `styles/tokens.css`.

```css
/* styles/tokens.css */
:root {
  --ds-color-bg: #f5f5f5;
  --ds-color-surface: #ffffff;
  --ds-color-accent: #2563eb;
  --ds-color-text: #1a1a1a;
  --ds-color-muted: #6b7280;
  --ds-radius-sm: 3px;
  --ds-radius-md: 6px;
  --ds-space-xs: 4px;
  --ds-space-sm: 8px;
  --ds-space-md: 16px;
  --ds-space-lg: 24px;
  --ds-space-xl: 32px;
  --ds-font-sans: system-ui, sans-serif;
  --ds-text-sm: 12px;
  --ds-text-md: 14px;
  --ds-text-lg: 16px;
}
```

Rules:

- Do not use a raw color, spacing, or radius in a component.
  Use the token.
- When a value is missing, add a token to `tokens.css`.
  A component never defines a new token locally.
- Use semantic names: `accent` for primary actions, `muted` for secondary text.

## Primitives

All markup uses primitives from `components/primitives`.
Primitives are the only elements that emit DOM nodes.

- Layout: `Stack`, `Row`, `Grid`, `Card`, `PageHeader`
- Text: `Text`, `Heading`, `Label`, `Code`
- Controls: `Button`, `IconButton`, `TextInput`, `Select`, `Toggle`, `Slider`
- Feedback: `Badge`, `Spinner`, `Alert`, `EmptyState`
- Overlay: `Modal`, `Popover`, `Tooltip`

Rules:

- A view composes primitives.
  It has no raw `div`, `span`, or `button` except inside a primitive.
- A primitive handles its own interaction states: hover, pressed, focus, disabled.
  A view does not redefine these states.
- Add a new primitive when you need a new element type.
  Do not write raw HTML in a view.

## Utility Classes

Use utility classes for layout and spacing when the primitive needs adjustment.
Use Tailwind or a similar system when available, or use token-based classes.

- Use utilities for `margin`, `padding`, `gap`, and `alignment`.
- Sort class names with the formatter.
  See [[CODE_QUALITY]] for the formatter baseline.

The utility never carries a color.
The color comes from the primitive or the token.

## CSS Files

The frontend has two CSS files.

- `styles/tokens.css` holds the tokens.
- `styles/global.css` imports tokens, resets base styles, and defines the app shell.

Do not create a CSS file per component.
Do not add styles inside a Rust file except for primitives.
A view has no CSS file.

```css
/* styles/global.css */
@import "./tokens.css";

html, body {
  background: var(--ds-color-bg);
  color: var(--ds-color-text);
  font-family: var(--ds-font-sans);
}

.app-shell {
  padding: var(--ds-space-xl);
}
```

## Page Composition

Follow the same composition for every page.

- The page has a `PageHeader` with title, description, and primary action.
- The page content has `32px` outer padding (`--ds-space-xl`).
- The sections use `16px` vertical rhythm (`--ds-space-md`).
- The cards use `24px` inner padding (`--ds-space-lg`).
- At most one primary solid button per page, at the right end of the action group.

This rule makes every page look like it comes from one hand.

## Interaction States

Each control shows four states: default, hover, pressed, focus, and disabled.
The primitive defines the states.
The view does not change them.

- Focus is always visible (outline or ring).
- Hover is subtle; do not use heavy shadows.
- Pressed is darker than hover.
- Disabled has reduced opacity and no interaction.

## Status and Feedback

Use `Alert` for a bridge error.
Use `Badge` for a status (Idle, Listening, Error).
Use `Spinner` for a pending bridge call.
Use `EmptyState` for an empty history.

Show a validation error next to the field.
The error text uses the user language.

## Language of the UI

User-facing text uses the user language.
Code identifiers and developer messages use STE English.
See [[../GUIDELINES]] for the language rule.

## Host Agnosticism

The same primitives run on Tauri and on Dioxus.
Do not import Tauri styles or Dioxus-specific CSS in a view.
Keep host-specific shell styles in `app.rs` or `main.rs`.
The views and sections use only primitives and tokens.

## Accessibility

- An icon-only control carries a label (`aria-label`).
- A control has a visible focus state.
- A form field has a label and an error association.
- An overlay closes on `Escape` and on backdrop click.
- A destructive action goes through a confirmation step.
