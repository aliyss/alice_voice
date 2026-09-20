/**
 * Playwright e2e config for the web frontend.
 *
 * One config, one responsibility: run the queue toggle flow against a live
 * backend and frontend. The webServer starts the backend and the frontend
 * preview build. Use --project=chromium for local dev.
 *
 * The flow writes real rows and flips app_setting, so it must never touch the
 * live database. It runs on a throwaway sqlite file by default, and every test
 * restores what it changed, so a repeated run starts where the last one did.
 * For a Postgres run, start the isolated test database first:
 *
 *   docker compose --env-file docker/.env -f docker/compose.test.yml up -d
 *   DATABASE_URL=postgres://alice:alice@127.0.0.1:5435/alice_e2e npx playwright test
 *
 * See docker/README.md and frontend-web/README.md.
 */
import { defineConfig, devices } from '@playwright/test';

// The e2e daemon listens on its own port. The dev daemon listens on 8788
// (see docker/.env), so the suite never reaches a daemon that points at the
// live database.
const ALICE_PORT = Number(process.env.ALICE_PORT ?? 8790);
const WEB_PORT = Number(process.env.WEB_PORT ?? 3000);
// The fake model server of the resolver. A real model is slow, needs a
// GPU, and answers differently on every run, so the suite answers itself.
const FAKE_LLAMA_PORT = Number(process.env.FAKE_LLAMA_PORT ?? 8792);
const BASE_URL =
  process.env.PLAYWRIGHT_BASE_URL ?? `http://127.0.0.1:${WEB_PORT}`;

/** The name of the live database. The suite refuses to write to it. */
const LIVE_DATABASE_NAME = 'alice';

/** The throwaway sqlite file used when DATABASE_URL is not set. */
const DEFAULT_DATABASE_URL = `sqlite:${process.cwd()}/../.tmp/e2e.db?mode=rwc`;

/**
 * Resolve the database under test and refuse the live one.
 *
 * The suite stores messages and toggles app_setting, so a run against the
 * live database leaves test rows in the real conversation. A Postgres URL is
 * accepted only when it names a database other than the live one. Use the
 * isolated `alice_e2e` from docker/compose.test.yml.
 */
function resolveDatabaseUrl(): string {
  const url = process.env.DATABASE_URL ?? DEFAULT_DATABASE_URL;
  if (!url.startsWith('postgres')) {
    return url;
  }

  const name = new URL(url).pathname.replace(/^\//, '');
  if (name === LIVE_DATABASE_NAME) {
    throw new Error(
      `Refusing to run the e2e suite against the live database "${LIVE_DATABASE_NAME}". ` +
        'Start the isolated test database and point at that instead:\n' +
        '  docker compose --env-file docker/.env -f docker/compose.test.yml up -d\n' +
        '  DATABASE_URL=postgres://alice:alice@127.0.0.1:5435/alice_e2e npm run test:e2e',
    );
  }
  return url;
}

const DATABASE_URL = resolveDatabaseUrl();

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: [['list'], ['html', { open: 'never' }]],
  timeout: 30_000,
  expect: { timeout: 10_000 },
  use: {
    baseURL: BASE_URL,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: [
    {
      // Backend: sqlite file in .tmp so e2e is self-contained (no Docker needed).
      // Override with `DATABASE_URL=postgres://...` to test against Docker DB.
      command: `cargo run -p alice-daemon --manifest-path backend/Cargo.toml`,
      url: `http://127.0.0.1:${ALICE_PORT}/api/health`,
      // Always start a private daemon. A reused dev daemon ignores the
      // DATABASE_URL below, so the suite would write to the live database.
      reuseExistingServer: false,
      // The daemon compiles before it listens. A cold `cargo run` (empty
      // target/ on a fresh CI cache) takes minutes, so the readiness wait
      // must cover the build. Pre-build with `cargo build -p alice-daemon`
      // in CI to keep this fast.
      timeout: 300_000,
      env: {
        DATABASE_URL,
        ALICE_LISTEN_ADDR: `0.0.0.0:${ALICE_PORT}`,
        ALICE_CORS_ORIGIN: BASE_URL,
        ALICE_RESOLVER_BASE_URL: `http://127.0.0.1:${FAKE_LLAMA_PORT}/v1`,
        ALICE_RESOLVER_MODEL: 'fake-4b',
        RUST_LOG: 'info',
      },
      cwd: '..',
    },
    {
      // A stand-in for the llama.cpp server of the intent resolver, so the
      // suite reads the intent of a message without a GPU or a download.
      command: 'node tests/fake-llama.mjs',
      url: `http://127.0.0.1:${FAKE_LLAMA_PORT}/health`,
      reuseExistingServer: false,
      timeout: 30_000,
      env: { FAKE_LLAMA_PORT: String(FAKE_LLAMA_PORT) },
    },
    {
      // Frontend preview — build preview entry then serve. For dev HMR use `npm run dev` manually.
      command: `npx qwik build preview && npx vite preview --host 0.0.0.0 --port ${WEB_PORT}`,
      url: `${BASE_URL}/settings/`,
      // Start a private preview build, so the suite never serves a stale one.
      reuseExistingServer: false,
      // Covers a cold preview build (client + SSR entry) before the server listens.
      timeout: 180_000,
      env: {
        PUBLIC_ALICE_API_URL: `http://127.0.0.1:${ALICE_PORT}`,
      },
    },
  ],
});
