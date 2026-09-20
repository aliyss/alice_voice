/**
 * E2e flow for the queue toggle and the conversations.
 *
 * The suite drives a real browser against a live backend and frontend.
 * The conversation tests run on a scratch database (see
 * playwright.config.ts), and every run uses a unique message text, so a
 * repeated run never collides with an older conversation.
 */
import type { Page } from '@playwright/test';

import { expect, test } from '@playwright/test';

/** A unique suffix of this run. */
const RUN = Date.now().toString(36);
const FIRST_TEXT = `e2e start ${RUN}`;
const SECOND_TEXT = `e2e second ${RUN}`;
const THIRD_TEXT = `e2e third ${RUN}`;
const NOT_STORED_NOTICE =
  'The queue is off. The daemon did not store this turn.';

/** Set the queue toggle to the wanted value. */
async function setQueueEnabled(page: Page, enabled: boolean): Promise<void> {
  await page.goto('/settings/?section=queue');
  const toggle = page.getByRole('button', { name: /Turn off|Turn on/ });
  await expect(toggle).toBeVisible();

  const wanted = enabled ? 'Turn off' : 'Turn on';
  if ((await toggle.textContent())?.trim() !== wanted) {
    await toggle.click();
    await expect(toggle).toHaveText(wanted);
  }
}

/**
 * Wait for the surface to answer, so a send cannot outrun it.
 *
 * The shell paints on the server with a closed link and opens the socket
 * once the browser resumes it. Typing before that point reaches a field
 * with no handler and a stream with no reader, so every send waits for
 * the link. See `lib/socket-client.ts`.
 */
async function waitForShell(page: Page): Promise<void> {
  await expect(page.getByText('Link live')).toBeVisible({ timeout: 10_000 });
}

/** Send one message on the chat surface of the current page. */
async function sendOnPage(page: Page, text: string): Promise<void> {
  const composer = page.getByRole('textbox', { name: 'Message to the daemon' });
  await expect(composer).toBeVisible();
  await waitForShell(page);
  await composer.fill(text);
  await page.getByRole('button', { name: 'Send' }).click();
}

/** Open `path` and send one message. */
async function sendMessage(
  page: Page,
  path: string,
  text: string,
): Promise<void> {
  await page.goto(path);
  await sendOnPage(page, text);
}

/** The transcript card of the chat surface. */
function transcript(page: Page) {
  return page.locator('section').filter({ hasText: 'Transcript' });
}

/**
 * The assistant turns of the transcript.
 *
 * The reply text depends on the intent the resolver chooses, so the tests
 * assert that the daemon answered instead of asserting its words.
 */
function replies(page: Page) {
  return transcript(page).getByRole('article', { name: 'Alice message' });
}

test.describe('queue toggle', () => {
  test('flips the queue and persists it after reload', async ({ page }) => {
    // 1. Open settings and read the initial toggle.
    await page.goto('/settings/?section=queue');
    const toggle = page.getByRole('button', { name: /Turn off|Turn on/ });
    await expect(toggle).toBeVisible();

    const before = (await toggle.textContent())?.trim();
    expect(before).toMatch(/Turn (on|off)/);

    // 2. Click and wait for the toggle and the badge to flip. The badge is
    //    scoped to the queue card and matched exactly, because "off" also
    //    appears in the body copy of the card.
    await toggle.click();
    const after = before === 'Turn off' ? 'Turn on' : 'Turn off';
    await expect(toggle).toHaveText(after);

    const queueCard = page
      .locator('section')
      .filter({ hasText: 'Message queue' });
    const badge = queueCard.getByText(after === 'Turn on' ? 'Off' : 'On', {
      exact: true,
    });
    await expect(badge).toBeVisible();

    // 3. Reload and verify the toggle persisted (backend app_setting).
    await page.reload();
    const toggleAfterReload = page.getByRole('button', {
      name: /Turn off|Turn on/,
    });
    await expect(toggleAfterReload).toHaveText(after);

    // 4. Restore to enabled so the next run starts from a known state.
    if (after === 'Turn on') {
      await toggleAfterReload.click();
      await expect(toggleAfterReload).toHaveText('Turn off');
    }
  });
});

test.describe('conversations', () => {
  test('the first message starts a conversation with a generated title', async ({
    page,
  }) => {
    await setQueueEnabled(page, true);
    await sendMessage(page, '/?new=1', FIRST_TEXT);

    await expect(replies(page).last()).toBeVisible({ timeout: 10_000 });

    // The title comes from the first message of the conversation.
    await expect(
      page.getByRole('heading', { name: FIRST_TEXT }),
    ).toBeVisible();

    // The conversation joins the list.
    await page.goto('/conversations');
    await expect(
      page.getByRole('link', { name: new RegExp(FIRST_TEXT) }),
    ).toBeVisible();
  });

  test('a conversation keeps its full history when it continues', async ({
    page,
  }) => {
    await setQueueEnabled(page, true);
    await sendMessage(page, '/?new=1', SECOND_TEXT);
    await expect(replies(page).last()).toBeVisible({ timeout: 10_000 });

    // 1. Open the conversation from the list.
    await page.goto('/conversations');
    await page
      .getByRole('link', { name: new RegExp(SECOND_TEXT) })
      .first()
      .click();
    await expect(page).toHaveURL(/\/conversations\/[0-9a-f-]{36}\/?$/);

    // 2. The full history of the conversation is there.
    await expect(
      transcript(page).getByText(SECOND_TEXT, { exact: true }),
    ).toBeVisible();
    await expect(replies(page)).toHaveCount(1);

    // 3. A new turn joins the same conversation. The daemon answers it, so
    //    the transcript holds a second reply and not only the pending row.
    await sendOnPage(page, THIRD_TEXT);
    await expect(
      transcript(page).getByText(THIRD_TEXT, { exact: true }),
    ).toBeVisible({ timeout: 10_000 });
    await expect(replies(page)).toHaveCount(2, { timeout: 10_000 });

    // 4. The history survives a reload.
    await page.reload();
    await expect(
      transcript(page).getByText(SECOND_TEXT, { exact: true }),
    ).toBeVisible();
    await expect(
      transcript(page).getByText(THIRD_TEXT, { exact: true }),
    ).toBeVisible();
  });

  test('the daemon answers without a store when the queue is off', async ({
    page,
  }) => {
    await setQueueEnabled(page, false);
    await sendMessage(page, '/?new=1', `e2e ephemeral ${RUN}`);

    await expect(replies(page).last()).toBeVisible({ timeout: 10_000 });

    // Nothing was stored, so the surface keeps its placeholder title and
    // the page says why.
    await expect(
      page.getByRole('heading', { name: 'New conversation' }),
    ).toBeVisible();
    await expect(
      page.getByText(NOT_STORED_NOTICE, { exact: true }),
    ).toBeVisible();

    // Restore the queue for the next run.
    await setQueueEnabled(page, true);
  });
});
