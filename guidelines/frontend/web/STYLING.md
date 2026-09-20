# Web Styling

## Purpose

This document defines how the web frontend styles its UI.
It covers the design system, tokens, and the CSS rules.
It applies to `frontend-web/` with Qwik.
It follows sp-guidelines with minimal changes.
See [[FOLDER_STRUCTURE]] for the component folders.

## Design System

The design system is the single source of truth for the web look and behavior.
It defines layout, spacing, interaction states, and component rules.
Follow it exactly for every page.

The tokens and primitives live in `frontend-web/src/components/ui` or in `src/styles/tokens.css`.
The web design system may share tokens with the desktop through copy.
See [[../OVERVIEW]] for the sharing strategy.

Principles:

- Calm: one surface, one hierarchy, one accent color.
- Hierarchy: size, weight, and color show what matters.
- Density: dense for data, airy for reading.
- Consistency: a control looks the same on every page.
- Restraint: few accents, one blue accent for primary actions.
- Accessibility: focus is always visible, a state change is always visible.

## Tokens and Colors

The tokens use the `ds-` prefix.
Examples: `bg-ds-blue-accent`, `text-ds-white`, `bg-ds-surface`.

All colors come from the design system.
A color is always a token.
The web app does not define a color in a page.
A component does not use a hex code, an rgb value, or a raw color utility.

Use the tone prop for semantic colors when the primitive supports it.
Examples: `tone="primary"`, `tone="neutral"`.

When a color is missing, add a token to the design system.
The change applies to the web app after a rebuild.
Do not add a raw color as a one-off.

The tokens live in `src/styles/tokens.css`:

```
:root {
  --ds-color-bg: #f5f5f5;
  --ds-color-surface: #ffffff;
  --ds-color-accent: #2563eb;
  --ds-radius-sm: 3px;
  --ds-space-md: 16px;
}
```

Keep the web tokens aligned with the desktop tokens when you share the design.

## Primitives and JSX Rule

Almost all JSX comes from the design system.
A raw HTML element is the exception.

Use the primitives for structure.
Primitives are buttons, tables, inputs, cards, and layout components.
A view composes the primitives.
Examples: `AppLayout`, `Sidebar`, `Header`, `PageHeader`, `Button`, `Card`, `TextInput`.

A new element type goes to the design system.
Do not write it in a page.
Check the exports of the UI library first.

Use `Text` for text and `Box`, `Stack`, `Grid` for layout.
Do not use a raw `div` for layout except inside a primitive.

The allowed exceptions for raw elements are:

- The document shell in `src/root.tsx` (`head`, `body`, `meta`, `link`, `title`)
- The Qwik City infrastructure (`QwikCityProvider`, `RouterOutlet`, `Slot`, `Form`)
- The context providers
- The router head component

When you need an element outside this list, add it to the design system.

### Choosing a control

Two controls can look alike and are not: a tab names content, a content switcher swaps a value.

- `Tabs` names the parts of one context that do not replace one another:
  the parts of a form, the groups of a catalog.
  Each tab brings its own content, so a part can be read on its own.
- `ContentSwitcher` swaps between alternate views of the same content:
  the same stage read by another reader, the same model on another device.
  One value stands in place of another, so the values are alternatives and
  they are joined into one strip rather than standing as separate buttons.

Neither has to fill the surface: a strip and a tab row are as wide as the values they name.

Keep the sentence about a value behind an `InfoHint` next to the name (`FieldLabel`) instead of beside the control, and keep the values of one question together in a `PanelGroup`.
A narrow panel then shows the values rather than the prose around them.

A note that floats over a surface is placed by `src/utils/note-placement.ts`.
It opens to the side the caller asks for, takes the other side when that one has no room, and shifts along the surface until it fits.
Both `InfoHint` and `Tooltip` read that rule, so a note in a scrolling panel is never cut off.

## Utility Classes

Use Tailwind utility classes for layout and spacing.
Use them for margins, padding, gap, and alignment.
Tailwind sorts the class names with the prettier plugin.
See [[CODE_QUALITY]] for the prettier baseline.

The sort order follows the prettier Tailwind plugin.
Do not add raw colors with utilities.
The color comes from the primitive or the token.

## CSS Rule

The web app has one CSS file.
The file is `src/global.css`.
It imports `src/styles/tokens.css`.
Do not create a CSS file for a component.
Do not add CSS classes to a component file.
Use the utility classes and the design tokens.

The rule applies to sections and pages too.
They use the UI primitives and the utilities.
They have no CSS file.

## Page Composition

Follow the same composition for every page.

- The page has a page header with title, description, and primary action.
- The page content has `32px` outer padding.
- The sections use `16px` vertical rhythm unless a rule says otherwise.
- The cards use `24px` interior padding.
- At most one primary solid button per page, at the rightmost position.

This rule keeps every page consistent.

## Interaction States

Each control shows default, hover, pressed, focus, and disabled.
The primitive defines the states.
The page does not redefine them.

- Focus is always visible.
- Hover is subtle.
- Pressed is darker than hover.
- Disabled has reduced opacity and no interaction.

## Feedback and Status

Use `Alert` for a backend REST error.
Use `Badge` for a status: Idle, Listening, or Error.
Use `Spinner` for a pending loader or `server$` call.
Use `EmptyState` for an empty history.
Show a validation error next to the field.

The feedback text uses the user language.
See [[../OVERVIEW]] for the language rule.

## Host Agnosticism

The web primitives run only on the web.
The desktop primitives run only on the desktop.
They share tokens and visual rules but not code by default.
See [[../OVERVIEW]] for the URL sharing option.
Do not import desktop Rust components in the web.
