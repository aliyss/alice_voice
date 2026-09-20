/**
 * Prettier configuration for the Alice Voice web frontend.
 * The import order keeps type imports before value imports and `~` before
 * relative paths.
 * @see https://prettier.io/docs/configuration
 * @type {import('prettier').Config}
 */

const config = {
  singleQuote: true,
  semi: true,
  tabWidth: 2,
  printWidth: 80,
  trailingComma: 'all',
  plugins: [
    'prettier-plugin-tailwindcss',
    '@trivago/prettier-plugin-sort-imports',
  ],
  importOrder: [
    '<THIRD_PARTY_TS_TYPES>',
    '<TS_TYPES>^@builder.io/(.*)$',
    '<TS_TYPES>^~/routes/(.*)$',
    '<TS_TYPES>^~/components/(.*)$',
    '<TS_TYPES>^~/types/(.*)$',
    '<TS_TYPES>^~/lib/(.*)$',
    '<TS_TYPES>^~/api/(.*)$',
    '<TS_TYPES>^~/schemas/(.*)$',
    '<TS_TYPES>^~/context/(.*)$',
    '<TS_TYPES>^~/utils/(.*)$',
    '<TS_TYPES>^[./]',
    '^@builder.io/(.*)$',
    '<THIRD_PARTY_MODULES>',
    '^~/routes/(.*)$',
    '^~/components/(.*)$',
    '^~/types/(.*)$',
    '^~/lib/(.*)$',
    '^~/api/(.*)$',
    '^~/schemas/(.*)$',
    '^~/context/(.*)$',
    '^~/utils/(.*)$',
    '^[./]',
  ],
  importOrderSeparation: true,
  importOrderSortSpecifiers: true,
};

export default config;
