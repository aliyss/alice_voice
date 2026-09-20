# Web Code Structure

## Purpose

This document defines the structure of web frontend code.
It covers single responsibility and the order inside a file.
It applies to all TypeScript and Qwik code in `frontend-web/`.
It follows sp-guidelines with Alice Voice specifics for REST and sockets.
See [[NAMING_CONVENTIONS]] for the names.
See [[FOLDER_STRUCTURE]] for the folders.

## Import Convention

One import statement per module.
Do not split the imports of one module into several statements.
Import all the value symbols of a module in one statement.
The prettier plugin sorts the statements.

The type imports stay separate from the value imports.
Use one `import type` statement for the types of a module.
Use one `import` statement for the values of a module.
Do not mix a type and a value in one import statement.

Use this form:

```
import type { HistoryRow, HistoryPageProps } from '~/components/pages/history-page';
import { HistoryPage } from '~/components/pages/history-page';
```

Do not use the inline type syntax in a value import.
The wrong form is `import { HistoryPage, type HistoryPageProps }`.

## Single Responsibility

One unit of code does one thing.
Use this rule for components, functions, and files.

- A component renders one section or one form.
- A function does one action.
- A `server$` function does one action or one coherent group of actions and then calls backend REST.
- A lib module handles one helper (backend fetch or socket).
- A file holds one concept.

If a unit does two things, split it.
Split a large component into sections.
Split a large function into helpers.
Split a file with two concepts into two files.

The name of the unit describes what it does.
If the name needs two verbs, split the unit.

Examples:

- `status-hero-section.tsx` shows the daemon status only.
- `listening-indicator-partial.tsx` shows the listening animation only.
- `api/history.ts` calls history REST only.
- `lib/socket-client.ts` manages the socket only.

## Order Inside a Page Component File

A page component has all the page-specific logic.
The page component lives in `src/components/pages`.
The page component receives the loader data as props.
See [[FOLDER_STRUCTURE]] for the pages folder.

A page component file has this order:

1. The imports, sorted by the prettier plugin
2. The type definitions (row types and page props type)
3. The component variables (const derived from props)
4. The hooks for state and navigation (`useSignal`, `useStore`, `useLocation`)
5. The component functions (plain functions)
6. The component functions that are handlers (`const handleClick = $(() => {})`)
   The component receives the action handles as callback props.
   The consumer defines the handles. See section Callback Handle Pattern.
7. The effect hooks (`useTask$`, `useVisibleTask$`)
8. The return statement with the JSX

Keep the order the same in every component.

## Order Inside a Route Index File

A route index file is thin.
It has only the loaders and the default component.
See [[ROUTING]] for the index file rule.

A route index file has this order:

1. The imports, sorted by the prettier plugin
2. The `routeLoader$` functions that fetch from backend REST through `lib/backend-client.ts`
3. The default component that defines the handles and renders the page component

A route index file has no page logic, no valibot schema, and no `server$` function.
The file has handles that wrap `server$` functions with `$()`.
Put the page logic in the page component.
Put the schema in `src/schemas`.
Put the `server$` functions in `src/api`.

## Order Inside a Function File

A function file (`src/api/*.ts` or `src/utils/*.ts`) has this order:

1. The imports
2. The type definitions (input types before output types)
3. The private helper functions
4. The public functions

A function body has this order:

1. Validate the input with valibot if required
2. Call the backend REST or get the data if required (through `lib/backend-client.ts`)
3. Transform the data
4. Return the result

Keep the steps separate.
Do not mix validation and data access.

## Order Inside a File

A file has this order:

1. The imports
2. The type definitions
3. The constants
4. The implementation
5. The export

An exported symbol is easy to find.
A constant lives near the place that uses it.

## Callback Handle Pattern

A component triggers an action through a callback prop.
The callback prop has the `$` suffix.
The action runs through `server$` which then calls backend REST.
This pattern applies everywhere: route file, section, or form.

Do not pass a `server$` function directly to a callback prop.
Wrap the `server$` function in a handle first.

A handle is a QRL. The name has the `handle` prefix.
A handle wraps the `server$` function in `$()`.

```
const handleSave = $((data: ConfigInput) => updateConfig(data));
```

Pass the handle to the callback prop:

```
<SettingsPage onSave$={handleSave} />
```

Type the callback prop as `QRL`:

```
export interface SettingsPageProps {
  onSave$: QRL<(data: ConfigInput) => Promise<UpdateConfigResult>>;
}
```

Call the callback prop inside the component:

```
result.value = await props.onSave$(data);
```

Define the handle inside the consumer (route file or page wrapper).
Do not define a handle inside a shared section.

Separate the data from the callbacks.
A loader returns the data only.
Name the loader return type `<Domain>PageData`.
Derive the type from the props type with `Omit`:

```
export type HistoryPageData = Omit<HistoryPageProps, 'onLoadMore$'>;
```

The route file renders the page:
The route file passes the loader data as explicit props and the handles as callback props.
Do not spread the loader data onto the view.

```
export default component$(() => {
  const history = useHistory();
  const handleLoadMore = $((cursor: string) => loadMoreHistory(cursor));
  return <HistoryPage items={history.value.items} onLoadMore$={handleLoadMore} />;
});
```

See [[NAMING_CONVENTIONS]] for the handle names.
See [[ROUTING]] for the route file rule.

## Backend Fetch Inside server$

A `server$` function in `frontend-web` never calls a database.
It calls the backend REST.

```
export const updateConfig = server$(async (input: ConfigInput) => {
  const parsed = parse(configSchema, input);
  const result = await backendFetch<ConfigDto>('/api/v1/config', { method: 'PUT', body: parsed });
  return { failed: false, data: result };
});
```

The `server$` lives in `src/api/`.
The route loader lives in `src/routes/*/index.tsx`.
Both use `lib/backend-client.ts`.

## File Size

A Qwik component file has less than 300 lines.
A `server$` file has less than 200 lines.
If a file grows, split the sections into `src/components/sections/`.

## Visibility

- Export only the page component and its props types from a page file.
- Keep helper functions private unless another file needs them.
- Do not export internal constants from a page file.
