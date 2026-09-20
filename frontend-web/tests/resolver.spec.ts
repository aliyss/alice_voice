/**
 * E2e flow for the resolver settings.
 *
 * The suite drives a real browser against a live backend and frontend. It
 * covers the layered router: the flow it draws, the values of the stage a
 * reader picks, the canvas that pans and zooms, and the field of a place
 * that does not answer. The router is the engine of the surface, so the
 * suite never switches engines.
 */
import type { Page } from '@playwright/test';

import { expect, test } from '@playwright/test';

/** The base URL of the daemon under test. */
const ALICE_BASE = `http://127.0.0.1:${process.env.ALICE_PORT ?? 8790}`;

/** A unique suffix of this run. */
const RUN = Date.now().toString(36);

/**
 * The card that holds the flow of the resolver.
 *
 * The values of a stage take a panel of their own, so the card is the
 * surface that draws the flow and nothing else.
 */
function resolverCard(page: Page) {
  return page.locator('section').filter({ hasText: 'Layered router' });
}

/** The graph of the flow. */
function flowGraph(page: Page) {
  return resolverCard(page).getByRole('group', {
    name: 'The stages of the layered router',
  });
}

/**
 * The panel that holds the values of the step a reader picked.
 *
 * The panel is a surface of its own beside the card of the resolver, so the
 * values of a step are not read out of the card.
 */
function stepPanel(page: Page) {
  return page.getByRole('region', {
    name: 'The values of one step of the layered router',
  });
}

/** Save the resolver form and wait for the daemon to answer. */
async function save(page: Page): Promise<void> {
  await stepPanel(page)
    .getByRole('button', { name: 'Save resolver' })
    .click();
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
}

/** Pick one stage of the flow. */
async function pickStage(page: Page, stage: string): Promise<void> {
  await resolverCard(page)
    .getByRole('button', { name: new RegExp(stage) })
    .click();
}

/** Pick one reader of the stage the panel shows. */
async function pickReader(page: Page, option: string): Promise<void> {
  // A reader is one value out of a few, so the panel shows the values as
  // a radio group rather than as buttons.
  await stepPanel(page)
    .getByRole('radio', { name: option, exact: true })
    .click();
}

/**
 * Wait for the surface to resume, so a field cannot outrun its handler.
 *
 * The shell paints on the server with a closed link and opens the socket
 * once the browser resumes it. A field the test types into before that
 * point keeps the value of the server and sends the stored one instead.
 */
async function waitForShell(page: Page): Promise<void> {
  await expect(page.getByText('Link live')).toBeVisible({ timeout: 10_000 });
}

test.describe.serial('layered router', () => {
  test('draws the pipeline and opens on the stage it names', async ({
    page,
  }) => {
    await page.goto('/settings/');
    await waitForShell(page);
    const card = resolverCard(page);

    // The card draws the flow and no engine switch stands beside it: the
    // layered router is the engine of the surface.
    await expect(card.getByText('Layered router')).toBeVisible();
    await expect(card.getByText('Engine', { exact: true })).toHaveCount(0);
    await expect(
      card.getByRole('button', { name: 'llama.cpp', exact: true }),
    ).toHaveCount(0);
    await expect(
      card.getByRole('button', { name: 'GLiNER', exact: true }),
    ).toHaveCount(0);

    // The flow is a graph: one block per stage, one block per turn that
    // leaves the chain, joined by the value that moves along a link.
    for (const block of [
      'The message',
      'Deterministic pass',
      'Retrieval',
      'Decision',
      'Extraction',
      'The run',
      'No intent matched',
      'Asks for the value',
    ]) {
      await expect(
        card.getByRole('button', { name: new RegExp(block) }),
      ).toBeVisible();
    }
    await expect(card.getByText('one message', { exact: true })).toBeVisible();
    await expect(card.getByText('the values', { exact: true })).toBeVisible();
    await expect(card.getByText(/short list of \d+/)).toBeVisible();

    // The flow opens on the retrieval stage, so its readers are the values
    // the panel beside the card shows, and it needs no download to run.
    await expect(
      stepPanel(page).getByRole('radio', { name: 'Words', exact: true }),
    ).toBeVisible();
    await expect(
      stepPanel(page).getByRole('radio', { name: 'Embeddings', exact: true }),
    ).toBeVisible();

    // The save of the resolver takes the footer of the panel, so it stands
    // at the end of the values rather than inside the card.
    await expect(
      stepPanel(page).getByRole('button', { name: 'Save resolver' }),
    ).toBeVisible();
    await expect(
      card.getByRole('button', { name: 'Save resolver' }),
    ).toHaveCount(0);
  });

  test('reads the intent in layerable stages and persists them', async ({
    page,
  }) => {
    await page.goto('/settings/');
    await waitForShell(page);
    const card = resolverCard(page);

    // The graph is a canvas rather than a scroll region. The first view
    // fits the whole graph into the card, a drag moves it without picking
    // the block under the pointer, and the fit control brings it back.
    const graph = flowGraph(page);
    const canvas = graph.locator('> div').first();
    const view = () =>
      canvas.evaluate((element) => getComputedStyle(element).transform);
    const fitted = await view();
    await expect
      .poll(() =>
        graph.evaluate(
          (element) => element.scrollWidth <= element.clientWidth + 1,
        ),
      )
      .toBe(true);

    const box = await graph.boundingBox();
    expect(box).not.toBeNull();
    const originX = (box?.x ?? 0) + 40;
    const originY = (box?.y ?? 0) + 40;
    await page.mouse.move(originX, originY);
    await page.mouse.down();
    await page.mouse.move(originX - 90, originY - 60, { steps: 6 });
    await page.mouse.up();
    await expect.poll(view).not.toBe(fitted);
    await expect(
      card.getByRole('button', { name: /Retrieval/ }),
    ).toHaveAttribute('aria-pressed', 'true');

    await graph.getByRole('button', { name: 'Fit the graph' }).click();
    await expect.poll(view).toBe(fitted);

    // A generative decision stage reads the model server, so the graph
    // wires the server block to the decision block and the values of the
    // stage hold the chat model.
    await pickStage(page, 'Decision');
    await pickReader(page, 'Model');
    await expect(card.getByText('chat', { exact: true }).first()).toBeVisible();
    await expect(
      stepPanel(page).getByRole('combobox', {
        name: 'Model of the llama.cpp server',
      }),
    ).toBeVisible();

    // A spans extraction stage reads the built in GLiNER model, so the
    // graph wires that block to the extraction block and the values of the
    // stage hold the model on disk, the device, and the threshold.
    await pickStage(page, 'Extraction');
    await pickReader(page, 'Spans');
    await expect(card.getByText('spans', { exact: true })).toBeVisible();
    await expect(
      stepPanel(page).getByRole('textbox', {
        name: 'Smallest probability a GLiNER label needs to count',
      }),
    ).toBeVisible();
    await expect(
      stepPanel(page).getByText('GLiNER small v2.1', { exact: true }),
    ).toBeVisible();
    await expect(
      stepPanel(page).getByRole('radio', { name: 'Auto' }),
    ).toBeVisible();
    await expect(
      stepPanel(page).getByRole('radio', { name: 'CPU' }),
    ).toBeVisible();

    // The reranker replaces the chat model on the decision stage, so the
    // server link leaves the graph with the reader that used it.
    await pickStage(page, 'Decision');
    await pickReader(page, 'Reranker');
    await expect(
      stepPanel(page).getByRole('combobox', {
        name: 'Model of the llama.cpp server',
      }),
    ).toHaveCount(0);
    await save(page);

    const stored = await page.request.get(`${ALICE_BASE}/api/v1/settings`);
    const saved = await stored.json();
    expect(saved.resolverBackend).toBe('router');
    expect(saved.routerDecide).toBe('rerank');
    expect(saved.routerExtract).toBe('spans');

    // The choice survives a reload, graph and all.
    await page.reload();
    await waitForShell(page);
    await expect(resolverCard(page).getByText('Layered router')).toBeVisible();
    await expect(
      resolverCard(page).getByRole('button', { name: /Deterministic pass/ }),
    ).toBeVisible();

    // Restore the stages the daemon ships with, so the next run starts
    // where this one did.
    await pickStage(page, 'Decision');
    await pickReader(page, 'Model');
    await pickStage(page, 'Extraction');
    await pickReader(page, 'Lists');
    await save(page);
  });

  test('disables a field whose place does not answer', async ({ page }) => {
    await page.goto('/settings/');
    await waitForShell(page);

    // The address of the server stays editable: it is a database setting,
    // and the user has to be able to point the daemon somewhere else.
    await pickStage(page, 'Decision');
    await pickReader(page, 'Model');
    const address = stepPanel(page).getByRole('textbox', {
      name: 'Address of the llama.cpp server',
    });
    const before = await address.inputValue();
    await expect(address).toBeEnabled();

    // Point the daemon at an address where nothing answers.
    await address.fill('http://127.0.0.1:9/v1');
    await save(page);
    await page.reload();
    await waitForShell(page);

    const panel = stepPanel(page);
    await pickStage(page, 'Decision');

    // The model becomes a disabled field that shows the stored value.
    const stored = await page.request.get(`${ALICE_BASE}/api/v1/settings`);
    const model = (await stored.json()).resolverModel as string;
    const field = panel.getByRole('textbox', {
      name: 'Model name of the llama.cpp server',
    });
    await expect(field).toBeDisabled();
    await expect(field).toHaveValue(model);
    await expect(panel.getByText(/shows the stored name/)).toBeVisible();

    // Restore the address, so the next run starts where it did.
    await address.fill(before);
    await save(page);
    await expect(
      panel.getByRole('combobox', { name: 'Model of the llama.cpp server' }),
    ).toBeVisible();
  });

  test('reads a sentence the user tries and lights its route', async ({
    page,
  }) => {
    // A preview reads a message the way a turn does, so the test needs an
    // intent of the catalog the fake model server chooses.
    const created = await page.request.post(`${ALICE_BASE}/api/v1/intents`, {
      data: {
        name: `e2e weather ${RUN}`,
        description: 'The weather of a city.',
        command: 'echo sun',
        entities: [],
      },
    });
    expect(created.ok()).toBeTruthy();
    const id = (await created.json()).id as string;

    try {
      await page.goto('/settings/');
      await waitForShell(page);
      const card = resolverCard(page);

      // The block of the message holds the sentences the user really
      // says, because a sentence is where a route starts.
      await pickStage(page, 'The message');
      const panel = stepPanel(page);
      await panel
        .getByRole('textbox', { name: 'One sentence to try' })
        .fill('what is the weather');
      await panel.getByRole('button', { name: 'Add', exact: true }).click();
      await expect(
        panel.getByRole('button', { name: 'Play what is the weather' }),
      ).toBeVisible();

      // The sentences are the tests of the pipeline, so the daemon stores
      // them: a user who tuned the router against a sentence finds it
      // again after a reload. The write is sent beside the click, so the
      // test reads until it lands. The sentence itself stored no turn.
      const storedSentences = async (): Promise<unknown> => {
        const stored = await page.request.get(`${ALICE_BASE}/api/v1/settings`);
        return (await stored.json()).previewSentences;
      };
      await expect.poll(storedSentences).toContain('what is the weather');
      await page.reload();
      await waitForShell(page);
      await pickStage(page, 'The message');
      await expect(
        stepPanel(page).getByRole('button', {
          name: 'Play what is the weather',
        }),
      ).toBeVisible();

      // The daemon reads the sentence with the stored settings and runs
      // nothing, so the panel names the intent it would run and the route
      // the message took.
      await stepPanel(page)
        .getByRole('button', { name: 'Play what is the weather' })
        .click();
      await expect(
        stepPanel(page).getByText('Would run', { exact: true }),
      ).toBeVisible({ timeout: 20_000 });
      await expect(
        stepPanel(page).getByText(/e2e weather/).first(),
      ).toBeVisible();
      await expect(
        stepPanel(page).getByText(/deterministic pass/),
      ).toBeVisible();

      // The flow keeps the route of the sentence that was read: every
      // stage the message did not meet steps back, and every stage it met
      // takes the tone of how it ended. The links a place feeds the chain
      // are part of the route too, so a dashed link is colored with the
      // stage it answers.
      const refused = card.getByRole('button', { name: /No intent matched/ });
      await expect(refused).toHaveClass(/opacity-40/);
      const message = card.getByRole('button', { name: /The message/ });
      await expect(message).toHaveAttribute('data-route', 'ok');
      const step = card.getByRole('button', { name: /Deterministic pass/ });
      await expect(step).toHaveAttribute('data-route', 'ok');
      await expect(
        card.locator('[data-edge="message-deterministic"]'),
      ).toHaveAttribute('data-route', 'ok');
      await card.getByRole('button', { name: 'Show every stage' }).click();
      await expect(refused).not.toHaveClass(/opacity-40/);
      await expect(refused).not.toHaveAttribute('data-route', /.*/);

      // The log of one sentence reads in two ways: the reading names what
      // the daemon would do, and the technical log names every value it
      // reported, each with the sentence about what it means.
      await expect(stepPanel(page).getByText('The log of')).toBeVisible();
      await stepPanel(page).getByRole('radio', { name: 'Debug' }).click();
      await expect(
        stepPanel(page).getByText('The answer', { exact: true }).first(),
      ).toBeVisible();
      await expect(
        stepPanel(page).getByText('How it ended', { exact: true }).first(),
      ).toBeVisible();
      await stepPanel(page).getByRole('radio', { name: 'Text' }).click();
      await expect(
        stepPanel(page).getByText('The answer', { exact: true }),
      ).toHaveCount(0);

      // A stored sentence can be edited in place, and the edit is stored
      // where the sentence was.
      await stepPanel(page)
        .getByRole('button', { name: 'Edit what is the weather' })
        .click();
      await stepPanel(page)
        .getByRole('textbox', { name: 'The sentence what is the weather' })
        .fill('what is the weather today');
      await stepPanel(page)
        .getByRole('button', { name: 'Keep what is the weather' })
        .click();
      await expect(
        stepPanel(page).getByRole('button', {
          name: 'Play what is the weather today',
        }),
      ).toBeVisible();
      await expect.poll(storedSentences).toContain('what is the weather today');
      await expect.poll(storedSentences).not.toContain('what is the weather');

      // The list is the user's, so the test leaves it as it found it.
      await stepPanel(page)
        .getByRole('button', { name: 'Remove what is the weather today' })
        .click();
      await expect(
        stepPanel(page).getByRole('button', {
          name: 'Play what is the weather today',
        }),
      ).toHaveCount(0);
      await expect
        .poll(storedSentences)
        .not.toContain('what is the weather today');
    } finally {
      await page.request.delete(`${ALICE_BASE}/api/v1/intents/${id}`);
    }
  });

  test('warns when the intent configuration has many labels', async ({
    page,
  }) => {
    // One label per intent, per entity, and per value of a closed entity.
    // Eight intents with two entities each add more labels than a GLiNER
    // model reads comfortably.
    const created: string[] = [];
    for (let index = 0; index < 8; index += 1) {
      const response = await page.request.post(`${ALICE_BASE}/api/v1/intents`, {
        data: {
          name: `e2e label ${RUN} ${index}`,
          description: 'A label of the budget test.',
          command: `echo label ${index}`,
          entities: [
            { name: `first ${index}`, kind: 'open', values: [] },
            { name: `second ${index}`, kind: 'open', values: [] },
          ],
        },
      });
      expect(response.ok()).toBeTruthy();
      created.push((await response.json()).id);
    }

    try {
      await page.goto('/settings/');
      // The warning belongs to the configuration of GLiNER, so it shows
      // where GLiNER is read: the stage that reads spans with it.
      await pickStage(page, 'Extraction');
      await pickReader(page, 'Spans');
      // The daemon reports the budget of the stored settings, so the test
      // stores the reader before it reads the warning.
      await save(page);
      await expect(
        stepPanel(page).getByText('Many labels', { exact: true }),
      ).toBeVisible();

      const status = await page.request.get(`${ALICE_BASE}/api/v1/resolver`);
      const budget = (await status.json()).budget;
      expect(budget.labels).toBeGreaterThan(budget.softLimit);
      expect(budget.warning).toBeTruthy();
    } finally {
      for (const id of created) {
        await page.request.delete(`${ALICE_BASE}/api/v1/intents/${id}`);
      }
    }
  });
});
