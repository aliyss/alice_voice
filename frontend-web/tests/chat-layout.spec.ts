/**
 * E2e flow for the layout of the chat surface.
 *
 * The suite drives a real browser against a live backend and frontend. It
 * covers the shape of the surface: the transcript panel sits beside the
 * stage on the right, the composer sits at the bottom of the stage, and
 * the transcript keeps the newest turn in view unless the reader scrolled
 * up on purpose.
 */
import type { Page } from '@playwright/test';

import { expect, test } from '@playwright/test';

/** The width of the desktop window the layout is measured in. */
const WIDTH = 1280;

/** The height of the desktop window the layout is measured in. */
const HEIGHT = 800;

/** How many turns the suite sends to overflow the transcript region. */
const TURNS = 6;

/** A unique suffix of this run. */
const RUN = Date.now().toString(36);

/** The transcript card of the chat surface. */
function transcript(page: Page) {
  return page.locator('section').filter({ hasText: 'Transcript' });
}

/** The scroll region of the transcript. */
function scroller(page: Page) {
  return page.getByRole('region', { name: 'Conversation transcript' });
}

/** The assistant turns of the transcript. */
function replies(page: Page) {
  return transcript(page).getByRole('article', { name: 'Alice message' });
}

/** The field that sends one message. */
function composer(page: Page) {
  return page.getByRole('textbox', { name: 'Message to the daemon' });
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

/** Send one message and wait until the daemon answered it. */
async function send(page: Page, text: string, turns: number): Promise<void> {
  await composer(page).fill(text);
  await page.getByRole('button', { name: 'Send' }).click();
  await expect(replies(page)).toHaveCount(turns);
}

/** The distance from the end of the scroll region, in pixels. */
function endDistance(page: Page): Promise<number> {
  return scroller(page).evaluate(
    (element) => element.scrollHeight - element.scrollTop - element.clientHeight,
  );
}

test.describe.serial('chat layout', () => {
  test.use({ viewport: { width: WIDTH, height: HEIGHT } });

  test('puts the transcript on the right and the composer at the bottom', async ({
    page,
  }) => {
    await page.goto('/?new=1');
    await waitForShell(page);

    const panel = await transcript(page).boundingBox();
    const field = await composer(page).boundingBox();
    expect(panel).not.toBeNull();
    expect(field).not.toBeNull();
    if (!panel || !field) {
      return;
    }

    // The panel takes the right half of the window, and the stage keeps
    // the rest. The composer ends the stage, centered under the aura.
    expect(panel.x).toBeGreaterThan(WIDTH / 2);
    expect(panel.x + panel.width).toBeLessThanOrEqual(WIDTH);
    expect(field.y).toBeGreaterThan(HEIGHT / 2);
    expect(field.x + field.width).toBeLessThan(panel.x);
    expect(Math.abs(field.x + field.width / 2 - panel.x / 2)).toBeLessThan(120);

    // The aura is the largest thing on the stage, and it keeps that place
    // when the conversation starts.
    const aura = page.locator('canvas');
    const before = await aura.boundingBox();
    expect(before).not.toBeNull();
    expect(before?.width ?? 0).toBeGreaterThan(300);

    await send(page, `aura size ${RUN}`, 1);
    await expect
      .poll(async () => (await aura.boundingBox())?.width ?? 0)
      .toBeGreaterThan(300);
  });

  test('puts the caret in the prompt on load and after a turn', async ({
    page,
  }) => {
    await page.goto('/?new=1');
    await waitForShell(page);

    // The surface is ready for a message without a click.
    await expect(composer(page)).toBeFocused();

    // The field is blocked while the daemon answers, and the caret comes
    // back to it, so the next message needs no click either.
    await send(page, `caret ${RUN}`, 1);
    await expect(composer(page)).toBeFocused();
  });

  test('keeps the newest turn in view and holds the place of a reader', async ({
    page,
  }) => {
    await page.goto('/?new=1');
    await waitForShell(page);

    // Enough turns to make the region scroll.
    for (let index = 0; index < TURNS; index += 1) {
      await send(page, `layout turn ${index} ${RUN}`, index + 1);
    }

    // The region follows the newest turn.
    await expect
      .poll(() => endDistance(page), { timeout: 10_000 })
      .toBeLessThan(8);
    expect(await scroller(page).evaluate((element) => element.scrollTop)).toBeGreaterThan(0);

    // A reader who scrolls up keeps their place when the next turn lands.
    await scroller(page).evaluate((element) => {
      element.scrollTop = 0;
    });
    await send(page, `layout turn held ${RUN}`, TURNS + 1);

    expect(await scroller(page).evaluate((element) => element.scrollTop)).toBeLessThan(8);
  });
});
