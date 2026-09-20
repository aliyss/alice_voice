/**
 * The vitest configuration of the web frontend.
 *
 * The test suite covers the pure web logic only. It does not load the
 * Qwik City plugin, because the tests never render a route.
 */
import tsconfigPaths from 'vite-tsconfig-paths';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [tsconfigPaths({ root: '.' })],
  test: {
    include: ['src/**/*.test.ts'],
    environment: 'node',
  },
});
