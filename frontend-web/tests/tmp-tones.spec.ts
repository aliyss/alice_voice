import { expect, test } from '@playwright/test';

const ALICE_BASE = `http://127.0.0.1:${process.env.ALICE_PORT ?? 8790}`;
const RUN = Date.now().toString(36);

test('the route of a sentence paints the flow', async ({ page }) => {
  // The route of a turn belongs to the layered router, so the test stores
  // the engine the settings page draws before it reads a sentence.
  await page.request.put(`${ALICE_BASE}/api/v1/settings`, {
    data: { resolverBackend: 'router' },
  });
  const created = await page.request.post(`${ALICE_BASE}/api/v1/intents`, {
    data: {
      name: `tone weather ${RUN}`,
      description: 'The weather of a city.',
      command: 'echo sun',
      entities: [],
    },
  });
  const id = (await created.json()).id as string;

  try {
    await page.goto('/settings/');
    await expect(page.getByText('Link live')).toBeVisible({ timeout: 10_000 });
    const card = page.locator('section').filter({ hasText: 'Layered router' });
    await card.getByRole('button', { name: /The message/ }).click();
    const panel = page.getByRole('region', {
      name: 'The values of one step of the layered router',
    });
    await panel
      .getByRole('textbox', { name: 'One sentence to try' })
      .fill('what is the weather');
    await panel.getByRole('button', { name: 'Add', exact: true }).click();
    await panel
      .getByRole('button', { name: 'Play what is the weather' })
      .click();
    await expect(panel.getByText(/Would run|No intent/)).toBeVisible({
      timeout: 20_000,
    });

    const style = async (name: RegExp) => {
      const block = card.getByRole('button', { name });
      return {
        route: await block.getAttribute('data-route'),
        border: await block.evaluate(
          (element) => getComputedStyle(element).borderTopColor,
        ),
      };
    };

    console.log('message', await style(/The message/));
    console.log('deterministic', await style(/Deterministic pass/));
    console.log('retrieval', await style(/Retrieval/));
    console.log('refused', await style(/No intent matched/));

    const edges = await card
      .locator('[data-route]')
      .evaluateAll((elements) =>
        elements.map((element) => [
          element.getAttribute('data-route'),
          getComputedStyle(element).color,
        ]),
      );
    console.log('edges', edges);

    await panel
      .getByRole('button', { name: 'Remove what is the weather' })
      .click();
  } finally {
    await page.request.delete(`${ALICE_BASE}/api/v1/intents/${id}`);
  }
});
