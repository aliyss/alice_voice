# Web Code Quality

## Purpose

This document defines the code quality baseline for the web frontend.
It applies to `frontend-web/` (Qwik/TypeScript).
It follows sp-guidelines with Alice Voice specifics for the backend boundary.
See [[../../CODE_QUALITY]] for the shared baseline and [[../../GUIDELINES]] for the three-project boundary.

## ESLint

The file is `eslint.config.js` with flat config built from `typescript-eslint`.

The config has four parts:

- The ignore list
- The recommended rules from `@eslint/js`
- The recommended rules from `typescript-eslint`
- The recommended rules from `eslint-plugin-qwik`

Turn off these rules:

- `@typescript-eslint/no-explicit-any`
- `@typescript-eslint/explicit-module-boundary-types`
- `no-console` (allow `warn` and `error`)

Set these rules:

- `@typescript-eslint/no-unused-vars` is error; ignore names that start with `_`.
- `@typescript-eslint/consistent-type-imports` is warn; prefer type imports.
- `@typescript-eslint/no-unnecessary-condition` is warn.

Copy the `eslint` config from a clean Qwik starter.
Keep the config identical across the web app and do not add backend Node imports.

## Prettier

The file is `prettier.config.js` with `singleQuote: true`, `semi: true`, `tabWidth: 2`, `printWidth: 80`, `trailingComma: 'all'`.

Plugins:

- `@trivago/prettier-plugin-sort-imports` for import sorting
- `prettier-plugin-tailwindcss` for Tailwind class sorting

```
importOrder: [
  '<THIRD_PARTY_TS_TYPES>',
  '<TS_TYPES>^@builder.io/(.*)$',
  '<TS_TYPES>^~/routes/(.*)$',
  '<TS_TYPES>^~/components/(.*)$',
  '<TS_TYPES>^~/types/(.*)$',
  '<TS_TYPES>^~/lib/(.*)$',
  '<TS_TYPES>^[./]',
  '^@builder.io/(.*)$',
  '<THIRD_PARTY_MODULES>',
  '^~/routes/(.*)$',
  '^~/components/(.*)$',
  '^~/types/(.*)$',
  '^~/lib/(.*)$',
  '^[./]',
],
plugins: ['prettier-plugin-tailwindcss', '@trivago/prettier-plugin-sort-imports'],
```

Sort order: type imports first, then value imports; third-party before local; `~/` before relative.
Every web file uses this order.

## Scripts

The web app has these scripts in `frontend-web/package.json`:

- `npm run lint` runs `eslint` on `src/**/*.{ts,tsx}`
- `npm run fmt` runs `prettier --write .`
- `npm run fmt.check` runs `prettier --check .`
- `npm run build` runs `qwik build`
- `npm run build.types` runs `tsc --noEmit`

CI runs `lint`, `fmt.check`, `build.types`, and `build` in `frontend-web/`.
The checks fail on any warning or type error.

## Testing

Write unit tests for pure web logic: utils, formatters, schema parsing, row mapping.
A pure function has no side effects and no backend fetch.

- Put the test next to the code as `<name>.test.ts`.
- Run the tests with `vitest` in `frontend-web/`.

```
npx vitest run
```

A test has three parts: set up input, call the function, check the result.

Do not test the backend in the web suite.
Mock `lib/backend-client.ts` for route and api tests.
Test the real backend only in contract tests that check DTOs against `backend/config/openapi.yaml`.

## DTO Contract Check

The web DTOs in `src/types/dto.ts` must match `backend/config/openapi.yaml`.
Add a contract check that generates or diffs the types.
Fail CI when the contract drifts.

## Single Responsibility Check

- One file has one concept; the file name matches the concept.
- One component renders one section.
- One `server$` function calls one backend REST endpoint.
- No page imports `fetch` directly; only `lib/backend-client.ts` does.
- No component holds two domains.

If a file fails the check, split it before you merge.
