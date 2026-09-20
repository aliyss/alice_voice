# Web Naming Conventions

## Purpose

This document defines the names that the web frontend uses.
It applies to files, components, functions, routes, contexts, and schemas in `frontend-web/`.
Follow these rules for all new code.
Use the same name for the same concept everywhere.
Do not use two names for the same concept.
See [[FOLDER_STRUCTURE]] for the folders.

## File Names

A file name is kebab-case.
Use lowercase letters and hyphens.
The file name ends with the role of the file.

| Role | End of file name | Example |
| --- | --- | --- |
| A complete page | `-page.tsx` | `home-page.tsx` |
| A section of a page | `-section.tsx` | `status-hero-section.tsx` |
| A form | `-form.tsx` | `settings-form.tsx` |
| A page-level view | `-view.tsx` | `plugin-list-view.tsx` |
| A small reusable piece | `-partial.tsx` | `empty-state-partial.tsx` |
| A chart | `-graph.tsx` | `history-graph.tsx` |
| A validation schema | `.ts` (domain) | `config.ts` |
| A Qwik context | `.context.ts` | `app.context.ts` |

A page component lives in `src/components/pages`.
A view and a partial live in `src/components`.
See [[FOLDER_STRUCTURE]] for the full layout.

An API `server$` file in `src/api` is kebab-case and matches the domain: `settings.ts`, `history.ts`.

## Component Names

A component name is PascalCase.
The name describes the concept, not the file.

- `HomePage`
- `StatusHeroSection`
- `ListeningIndicator`
- `PluginListView`
- `SettingsForm`

A page component ends with `Page`.
A section component ends with `Section`.
A form component ends with `Form`.

## Function Names

A function name is camelCase.
The first word is a verb.
The name has the form verb + subject + qualifier.

| Group | Verbs | Use |
| --- | --- | --- |
| Retrieve | `get`, `find` | Read from backend REST |
| Change | `create`, `update`, `delete`, `set` | Write to backend REST |
| Transform | `convert`, `parse`, `format` | Change the form of data |

Examples: `getStatus`, `listHistory`, `updateConfig`, `setPluginEnabled`, `formatTranscript`.

Do not use `run` or `do` in a name.
Use one verb for one action.

## Schema Names

A schema file lives in `src/schemas`.
The schema constant is camelCase and ends with `Schema`.
The update variant ends with `UpdateSchema`.
The input type is PascalCase and ends with `Input`.

- `config.ts` exports `configSchema` and `ConfigInput`
- `plugin.ts` exports `pluginToggleSchema` and `PluginToggleInput`

The `server$` file imports the schema and the input type.
See [[CODE_STRUCTURE]] for the validation order.

## server$ Function Names

A `server$` function changes or loads data through backend REST.
The name starts with the action verb and has no `$` suffix.

- `getStatus`
- `updateConfig`
- `listPlugins`
- `listHistory`

Put a `server$` function in `src/api`.
The file name matches the domain.
See [[FOLDER_STRUCTURE]] for the api folder.

## Callback Prop and Handle Names

A component triggers an action through a callback prop.
The callback prop name is `on` + Action + `$`.

- `onSave$`
- `onToggle$`
- `onLoadMore$`

Type the callback prop as `QRL`.
See [[CODE_STRUCTURE]] for the callback handle pattern.

The consumer passes a handle to the callback prop.
A handle name is `handle` + Action.

- `handleSave`
- `handleToggle`
- `handleLoadMore`

A handle is a `$()` wrapper around a `server$` function.

A loader for a page returns the data only.
The loader return type is `<Domain>PageData`.
The type omits the callback props.

## Loader Names

A `routeLoader$` has the `use` prefix: `useStatus`, `useHistory`.
It returns the data for the page.
See [[ROUTING]] for the loader convention.

## Type Names

A type name is PascalCase.

- `Row` for one row in a list: `HistoryRow`, `PluginRow`
- `Item` for one entry in a chart: `HistoryItem`
- DTO keeps the domain name: `AppStatusDto`, `ConfigDto`

Row types and props live in the page or section file.
Shared DTO types live in `src/types/dto.ts`.
The shared DTOs come from the backend contract.
Do not rename the DTO fields without a contract change.

## Route Folder Names

A route folder is lowercase with hyphens.
A dynamic segment uses square brackets `[id]`.
A group uses parentheses `(app)`.
See [[ROUTING]] for the route structure.

## Context Names

A context file is `<name>.context.ts`.
The context id is lowercase: `appContext`, `settingsContext`.
See [[STATE]] for the context structure.

## Package Names

The web package name is `frontend-web`.
Keep the backend prefix `alice-` for the backend only.
A Qwik app that shares the repo can use the name `alice-web`.
