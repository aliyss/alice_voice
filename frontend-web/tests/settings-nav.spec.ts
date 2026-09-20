/**
 * E2e flow for the section bar of the settings surface.
 *
 * The suite drives a real browser against a live backend and frontend. It
 * covers the bar itself: the section a reader picks replaces the section in
 * view, the bar carries the state of every section, and the address opens
 * the surface on the section it names.
 */
import type { Page } from '@playwright/test';

import { expect, test } from '@playwright/test';

/**
 * Wait for the surface to resume, so a pick cannot outrun its handler.
 *
 * The shell paints on the server with a closed link and opens the socket
 * once the browser resumes it. See `lib/socket-client.ts`.
 */
async function waitForShell(page: Page): Promise<void> {
  await expect(page.getByText('Link live')).toBeVisible({ timeout: 10_000 });
}

/** The bar of the settings surface. */
function sectionBar(page: Page) {
  return page.getByRole('navigation', { name: 'Settings sections' });
}

test('moves between the sections from the bar', async ({ page }) => {
  await page.goto('/settings/');
  await waitForShell(page);
  const bar = sectionBar(page);

  // The surface opens on the resolver, so its values are the ones on
  // screen and the bar says which engine holds them.
  await expect(bar.getByRole('button', { name: /Intent resolver/ })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Save resolver' })).toBeVisible();
  await expect(bar.getByRole('button', { name: /Intent resolver/ })).toHaveAttribute(
    'aria-current',
    'true',
  );

  // The bar carries the state of a section, so its badge reads the value
  // the section holds.
  await expect(bar.getByRole('button', { name: /Queue/ })).toContainText(
    /On|Off/,
  );
  await expect(bar.getByRole('button', { name: /Intents/ })).toContainText(
    /\d+/,
  );

  // The queue section replaces the resolver instead of joining it, so the
  // surface shows one section at a time.
  await bar.getByRole('button', { name: /Queue/ }).click();
  await expect(page.getByText('Message queue', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Save resolver' }),
  ).toHaveCount(0);

  // The intents section holds the list and the form of an intent.
  await bar.getByRole('button', { name: /Intents/ }).click();
  await expect(page.getByRole('button', { name: 'New intent' })).toBeVisible();
  await expect(page.getByText('Message queue', { exact: true })).toHaveCount(0);

  // The address opens the surface on the section it names, so a link to a
  // section needs no pick.
  await page.goto('/settings/?section=queue');
  await waitForShell(page);
  await expect(page.getByText('Message queue', { exact: true })).toBeVisible();
  await expect(sectionBar(page).getByRole('button', { name: /Queue/ })).toHaveAttribute(
    'aria-current',
    'true',
  );

  // A name the surface does not know opens the first section rather than
  // an empty surface.
  await page.goto('/settings/?section=nonsense');
  await waitForShell(page);
  await expect(page.getByRole('button', { name: 'Save resolver' })).toBeVisible();
});
