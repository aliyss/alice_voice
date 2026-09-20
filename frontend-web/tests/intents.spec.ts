/**
 * E2e flow for the intent configuration and the handled turn.
 *
 * The suite drives a real browser against a live backend and frontend.
 * The resolver points at `tests/fake-llama.mjs`, so the daemon reads the
 * intent of a message without a GPU and answers the same way every run.
 * Every run uses unique text, so a repeated run never collides with a row
 * of an older one.
 */
import type { Page } from '@playwright/test';

import { expect, test } from '@playwright/test';

/** The base URL of the daemon under test. */
const ALICE_BASE = `http://127.0.0.1:${process.env.ALICE_PORT ?? 8790}`;

/** A unique suffix of this run. */
const RUN = Date.now().toString(36);

/** The name of the intent the suite configures. */
const INTENT_NAME = 'e2e weather';

/**
 * The command of the intent, and the text it writes.
 *
 * The command waits before it answers, so the stages of the turn stay on
 * screen long enough for the suite to read them. The daemon publishes the
 * resolved intent and the command only after the model finished, and a
 * command that returns at once leaves those stages visible for a blink.
 */
const FIRST_COMMAND = `sleep 1 && echo "e2e report ${RUN}"`;
const FIRST_OUTPUT = `e2e report ${RUN}`;

/** The command the edit test stores, and the text it writes. */
const SECOND_COMMAND = `echo "e2e updated ${RUN}"`;
const SECOND_OUTPUT = `e2e updated ${RUN}`;

/** The intent whose command names an entity. */
const TEMPLATE_NAME = 'e2e templated';
const TEMPLATE_COMMAND = 'sleep 1 && echo "city={city}"';
/**
 * What the daemon runs for that intent.
 *
 * The command names the entity `city`, so the daemon puts the value it
 * read into the command. The fake model server answers `fake-city`, so
 * the rendered command is the same on every run.
 */
const TEMPLATE_RENDERED = 'sleep 1 && echo "city=fake-city"';
const TEMPLATE_OUTPUT = 'city=fake-city';

/** The entity of that intent with the value the fake model server reads. */
const TEMPLATE_ENTITY = 'city = fake-city';

/** The intent whose entity the daemon cannot read. */
const REQUIRED_NAME = 'e2e required';

/** The command of that intent, which names the entity twice over. */
const REQUIRED_COMMAND = 'sleep 1 && echo "list={applications}"';

/** The script of a list longer than the labels the resolver offers. */
const REQUIRED_SCRIPT = "seq 1 30 | sed 's/^/app-/'";

/** The number of values that script answers with, as the preview shows it. */
const LIST_VALUES = '30 values';

/** The reply that asks the user for the value of the required entity. */
const REQUIRED_ASK = 'To run e2e required I need a value for applications.';

/**
 * What the command writes when the entity has no value.
 *
 * The daemon removes the placeholder of an optional entity it read no
 * value for, so the command runs with an empty value instead of a brace
 * group. A required entity stops the turn instead, so the command never
 * writes this line before the entity is optional.
 */
const LIST_OUTPUT = 'list=';

/** The intent whose list the words of a message read. */
const WORDS_NAME = 'e2e list';

/** The command of that intent, which names the entity of the list. */
const WORDS_COMMAND = 'echo "launched={applications}"';

/** The script of a list longer than the labels the resolver offers. */
const WORDS_SCRIPT = "seq 1 30 | sed 's/^/app-/'";

/**
 * The entry of the list the message names.
 *
 * The entry is written with a dash and the message says the words of it,
 * so the matcher reads the entry by its words.
 */
const WORDS_ENTRY = 'app-7';

/** What the command writes when the words of the message read the entry. */
const WORDS_OUTPUT = `launched=${WORDS_ENTRY}`;

/** The intent the unmatched turn finds in the catalog. */
const UNMATCHED_NAME = 'e2e unmatched';

/** The message the fake model server answers with `none of these`. */
const UNMATCHED_TEXT = `nothing fits ${RUN}`;

/** The reply the daemon sends when no intent fits the message. */
const UNMATCHED_REPLY = 'I could not match that to an intent.';

/** The transcript card of the chat surface. */
function transcript(page: Page) {
  return page.locator('section').filter({ hasText: 'Transcript' });
}

/**
 * The intent card of the settings surface.
 *
 * The card carries no caption of its own, because the section names it, so
 * the row of actions inside it is the anchor.
 */
function intentCard(page: Page) {
  return page.locator('section').filter({ hasText: 'Add examples' });
}

/**
 * The row of one intent.
 *
 * The daemon seeds the example intent of a fresh catalog, so the suite
 * never assumes that the intent it wrote is the only one on the page.
 */
function intentRow(page: Page, name: string) {
  return intentCard(page).getByRole('article', { name, exact: true });
}

/** The card that shows the stages of the running turn. */
function liveCard(page: Page) {
  return page.locator('section').filter({ hasText: /^Live/ });
}

/** The assistant turns of the transcript. */
function replies(page: Page) {
  return transcript(page).getByRole('article', { name: 'Alice message' });
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
  const composer = page.getByRole('textbox', {
    name: 'Message to the daemon',
  });
  await expect(composer).toBeVisible();
  await waitForShell(page);
  await composer.fill(text);
  await page.getByRole('button', { name: 'Send' }).click();
}

test.describe.serial('intent handling', () => {
  test('configures an intent with a closed entity', async ({ page }) => {
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await page.getByRole('button', { name: 'New intent' }).click();

    await page
      .getByRole('textbox', { name: 'Name of the intent' })
      .fill(INTENT_NAME);
    await page
      .getByRole('textbox', { name: 'Description of the intent' })
      .fill('Read the weather of one city.');
    await page
      .getByRole('textbox', { name: 'Shell command of the intent' })
      .fill(FIRST_COMMAND);

    await page.getByRole('button', { name: 'Add entity' }).click();
    await page.getByRole('textbox', { name: 'Name of the entity' }).fill('when');
    await page.getByRole('radio', { name: 'Closed' }).click();
    await page
      .getByRole('textbox', { name: 'Values of the closed entity' })
      .fill('today, tomorrow');

    await page.getByRole('button', { name: 'Save intent' }).click();

    // The stored intent joins the list with its command and its entity.
    const list = intentCard(page);
    await expect(list.getByText(INTENT_NAME, { exact: true })).toBeVisible();
    await expect(list.getByText(FIRST_COMMAND, { exact: true })).toBeVisible();
    await expect(
      list.getByText('when: today, tomorrow', { exact: true }),
    ).toBeVisible();
  });

  test('resolves the intent and streams the stages of the turn', async ({
    page,
  }) => {
    await useModelDecision(page);
    await page.goto('/?new=1');
    await sendOnPage(page, `what is the weather ${RUN}`);

    // The stages of the turn arrive while the daemon works. The fake model
    // server spreads its answer over time, so the stages are readable.
    const live = liveCard(page);
    await expect(live.getByText('Thinking', { exact: true })).toBeVisible();
    await expect(live.getByText(INTENT_NAME, { exact: true })).toBeVisible();
    await expect(live.getByText('Command', { exact: true })).toBeVisible();
    await expect(live.getByText(FIRST_COMMAND, { exact: true })).toBeVisible();

    // The turn lands in the transcript with the output of the command and
    // the name of the intent the resolver chose.
    const reply = replies(page).last();
    await expect(reply).toContainText(FIRST_OUTPUT);
    await expect(reply).toContainText(INTENT_NAME);
  });

  test('edits the command and the next turn uses the new one', async ({
    page,
  }) => {
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    const list = intentCard(page);
    // The row is the pick of the intent, so a reader presses the intent
    // rather than an action button beside it.
    await intentRow(page, INTENT_NAME)
      .getByRole('button', { name: /^Edit / })
      .click();

    const command = page.getByRole('textbox', {
      name: 'Shell command of the intent',
    });
    await expect(command).toHaveValue(FIRST_COMMAND);
    await command.fill(SECOND_COMMAND);
    await page.getByRole('button', { name: 'Save intent' }).click();
    await expect(list.getByText(SECOND_COMMAND, { exact: true })).toBeVisible();

    await page.goto('/?new=1');
    await sendOnPage(page, `weather again ${RUN}`);
    await expect(replies(page).last()).toContainText(SECOND_OUTPUT);
  });

  test('deletes the intent', async ({ page }) => {
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    const list = intentCard(page);
    await intentRow(page, INTENT_NAME)
      .getByRole('button', { name: 'Delete' })
      .click();

    await expect(list.getByText(INTENT_NAME, { exact: true })).toHaveCount(0);
  });

  test('puts the value of an entity into the command', async ({ page }) => {
    // 1. Configure an intent whose command names an entity.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await page.getByRole('button', { name: 'New intent' }).click();
    await page
      .getByRole('textbox', { name: 'Name of the intent' })
      .fill(TEMPLATE_NAME);
    await page
      .getByRole('textbox', { name: 'Description of the intent' })
      .fill('Read the weather of one city.');
    await page
      .getByRole('textbox', { name: 'Shell command of the intent' })
      .fill(TEMPLATE_COMMAND);
    await page.getByRole('button', { name: 'Add entity' }).click();
    await page.getByRole('textbox', { name: 'Name of the entity' }).fill('city');
    await page.getByRole('button', { name: 'Save intent' }).click();

    const list = intentCard(page);
    await expect(list.getByText(TEMPLATE_NAME, { exact: true })).toBeVisible();
    // The daemon seeds an example intent that also reads a city, so the
    // test reads the entities of its own intent and not of the catalog.
    await expect(
      intentRow(page, TEMPLATE_NAME).getByText('city: open', { exact: true }),
    ).toBeVisible();

    // 2. Send a message. The daemon reads the value of the entity out of
    //    it and runs the command with the value in place of the
    //    placeholder, so the placeholder never reaches the shell.
    await useModelDecision(page);
    await page.goto('/?new=1');
    await sendOnPage(page, `what is the weather in ${RUN}`);
    await expect(
      liveCard(page).getByText(TEMPLATE_RENDERED, { exact: true }),
    ).toBeVisible();

    // The live card names the engine of each step and the value the
    // resolver read for the entity of the intent.
    await expect(liveCard(page).getByText('Entities', { exact: true })).toBeVisible();
    // The value carries the reader that read it, so the row names the
    // value and the reader that found it rather than the value alone.
    await expect(
      liveCard(page).getByText(new RegExp(`^${TEMPLATE_ENTITY}`)),
    ).toBeVisible();
    await expect(
      liveCard(page).getByText('chosen by Router', { exact: true }),
    ).toBeVisible();

    // The stored turn keeps the same metadata. The header of the turn
    // carries the time, the length of the run waits in a tooltip on the
    // time, and the rest waits behind the toggle.
    const reply = replies(page).last();
    await expect(reply).toContainText(TEMPLATE_OUTPUT);
    await expect(reply.getByRole('tooltip')).toHaveCount(0);
    await expect(reply.getByText('Resolver')).toBeHidden();

    await reply.getByText(/^\d{2}:\d{2}:\d{2}$/).hover();
    await expect(reply.getByRole('tooltip')).toBeVisible();
    await expect(reply.getByRole('tooltip')).toContainText(/\d+ ms|\d\.\d s/);

    await reply.getByRole('button', { name: 'Metadata' }).click();
    await expect(reply.getByText('Resolver')).toBeVisible();
    // The router names the candidates it weighed, so the name of the intent
    // also stands in that list and the row itself is read exactly.
    await expect(reply.getByText(TEMPLATE_NAME, { exact: true })).toBeVisible();
    await expect(reply.getByText(TEMPLATE_ENTITY)).toBeVisible();
    // The route the message took is stored with the turn and read back
    // stage by stage, so the transcript explains how the daemon read it.
    await expect(reply.getByText('Route', { exact: true })).toBeVisible();
    await expect(reply.getByText(/ranking of the catalog/)).toBeVisible();
    await expect(reply.getByText('exit 0')).toBeVisible();

    // 3. Remove the intent, so the next suite starts with no intent.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await intentRow(page, TEMPLATE_NAME)
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect(
      intentCard(page).getByText(TEMPLATE_NAME, { exact: true }),
    ).toHaveCount(0);
  });
});

test.describe.serial('the entities an intent needs', () => {
  test('asks for a required value and runs without an optional one', async ({
    page,
  }) => {
    // 1. Configure an intent whose command names a script entity. The
    //    script answers more values than the resolver offers as choices,
    //    so the daemon reads the words of the value and matches them
    //    against the list. The fake model server answers
    //    `fake-applications`, which names no entry of the list.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await page.getByRole('button', { name: 'New intent' }).click();
    await page
      .getByRole('textbox', { name: 'Name of the intent' })
      .fill(REQUIRED_NAME);
    await page
      .getByRole('textbox', { name: 'Description of the intent' })
      .fill('Open an application of this machine.');
    await page
      .getByRole('textbox', { name: 'Shell command of the intent' })
      .fill(REQUIRED_COMMAND);

    await page.getByRole('button', { name: 'Add entity' }).click();
    await page
      .getByRole('textbox', { name: 'Name of the entity' })
      .fill('applications');
    await page.getByRole('radio', { name: 'Script' }).click();
    await page
      .getByRole('textbox', { name: 'Script of the entity' })
      .fill(REQUIRED_SCRIPT);

    // The settings page runs the script, so the writer reads the list.
    await page.getByRole('button', { name: 'Preview' }).click();
    await expect(page.getByText(LIST_VALUES, { exact: true })).toBeVisible();

    await page.getByRole('button', { name: 'Save intent' }).click();
    const list = intentCard(page);
    await expect(list.getByText(REQUIRED_NAME, { exact: true })).toBeVisible();
    await expect(
      intentRow(page, REQUIRED_NAME).getByText('applications: script', {
        exact: true,
      }),
    ).toBeVisible();

    // 2. A required entity the daemon cannot read stops the turn and asks
    //    the user for the value, so a command that misses a value never
    //    reaches the shell.
    await useModelDecision(page);
    await page.goto('/?new=1');
    await sendOnPage(page, `open something ${RUN}`);
    const asked = replies(page).last();
    await expect(asked).toContainText(REQUIRED_ASK);
    await expect(asked).not.toContainText(LIST_OUTPUT);

    // 3. The same entity as optional: the command runs without it, and the
    //    placeholder of the entity leaves no trace in the command.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await intentRow(page, REQUIRED_NAME)
      .getByRole('button', { name: /^Edit / })
      .click();
    await page.getByRole('radio', { name: 'Optional' }).click();
    await page.getByRole('button', { name: 'Save intent' }).click();
    await expect(
      intentRow(page, REQUIRED_NAME).getByText(
        'applications: script (optional)',
        { exact: true },
      ),
    ).toBeVisible();

    await page.goto('/?new=1');
    await sendOnPage(page, `open something else ${RUN}`);
    await expect(replies(page).last()).toContainText(LIST_OUTPUT);

    // 4. Remove the intent, so the next suite starts with no intent.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await intentRow(page, REQUIRED_NAME)
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect(
      intentCard(page).getByText(REQUIRED_NAME, { exact: true }),
    ).toHaveCount(0);
  });
});

test.describe.serial('the words of a message', () => {
  test('read the value of a list the engine could not read', async ({
    page,
  }) => {
    // 1. Configure an intent whose command names a script entity. The
    //    script answers more values than the resolver offers as choices,
    //    so the daemon reads the words of the value against the list. The
    //    fake model server answers `fake-applications`, which names no
    //    entry, so only the words of the message can read the value.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await page.getByRole('button', { name: 'New intent' }).click();
    await page
      .getByRole('textbox', { name: 'Name of the intent' })
      .fill(WORDS_NAME);
    await page
      .getByRole('textbox', { name: 'Description of the intent' })
      .fill('Open an application of this machine.');
    await page
      .getByRole('textbox', { name: 'Shell command of the intent' })
      .fill(WORDS_COMMAND);
    // The router ranks the catalog before a model reads it, so the intent
    // needs the words of the message in its own phrases to reach the short
    // list the model decides over.
    await page
      .getByRole('textbox', { name: 'Phrases a user may say for this intent' })
      .fill(`launch ${WORDS_ENTRY}`);

    await page.getByRole('button', { name: 'Add entity' }).click();
    await page
      .getByRole('textbox', { name: 'Name of the entity' })
      .fill('applications');
    await page.getByRole('radio', { name: 'Script' }).click();
    await page
      .getByRole('textbox', { name: 'Script of the entity' })
      .fill(WORDS_SCRIPT);
    await page.getByRole('button', { name: 'Save intent' }).click();
    await expect(
      intentRow(page, WORDS_NAME).getByText('applications: script', {
        exact: true,
      }),
    ).toBeVisible();

    // 2. The message names one entry of the list, and the command runs with
    //    the entry the list spells, not with the words of the message.
    await useModelDecision(page);
    await page.goto('/?new=1');
    await sendOnPage(page, `launch ${WORDS_ENTRY}`);
    await expect(replies(page).last()).toContainText(WORDS_OUTPUT);

    // 3. Remove the intent, so the next suite starts with no intent.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await intentRow(page, WORDS_NAME)
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect(
      intentCard(page).getByText(WORDS_NAME, { exact: true }),
    ).toHaveCount(0);
  });
});

test.describe.serial('a turn with no intent', () => {
  test('reports how the resolver read a message it chose no intent for', async ({
    page,
  }) => {
    // 1. An intent has to be in the catalog, or the resolver never reads
    //    the message and the turn has nothing to report.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await page.getByRole('button', { name: 'New intent' }).click();
    await page
      .getByRole('textbox', { name: 'Name of the intent' })
      .fill(UNMATCHED_NAME);
    await page
      .getByRole('textbox', { name: 'Description of the intent' })
      .fill('Read the weather of one city.');
    await page
      .getByRole('textbox', { name: 'Shell command of the intent' })
      .fill('echo unreachable');
    await page.getByRole('button', { name: 'Save intent' }).click();
    await expect(
      intentCard(page).getByText(UNMATCHED_NAME, { exact: true }),
    ).toBeVisible();

    const stored = await page.request.get(`${ALICE_BASE}/api/v1/settings`);
    const { resolverModel } = await stored.json();

    // 2. Send a message the model reads and answers with `none of these`.
    await useModelDecision(page);
    await page.goto('/?new=1');
    await sendOnPage(page, UNMATCHED_TEXT);

    const reply = replies(page).last();
    await expect(reply).toContainText(UNMATCHED_REPLY);
    await expect(reply.getByRole('tooltip')).toHaveCount(0);

    // 3. The turn reports the read it did, even on a missing intent: the
    //    table names the engine, and the tooltip of that row names the
    //    model the engine ran.
    await reply.getByRole('button', { name: 'Metadata' }).click();
    await expect(reply.getByText('no intent matched')).toBeVisible();
    await expect(reply.getByText('Resolver')).toBeVisible();

    // The turn reports the engine the daemon ran, which is the layered
    // router, and the tooltip of the row names the model of the stage that
    // decided.
    const resolver = reply.getByText('Router');
    await expect(resolver).toBeVisible();
    await resolver.hover();
    await expect(reply.getByRole('tooltip')).toContainText(resolverModel);
    await expect(reply.getByText('exit 0')).toHaveCount(0);

    // 4. Remove the intent, so the next suite starts with no intent.
    await page.goto('/settings/?section=intents');
    await waitForShell(page);
    await intentRow(page, UNMATCHED_NAME)
      .getByRole('button', { name: 'Delete' })
      .click();
    await expect(
      intentCard(page).getByText(UNMATCHED_NAME, { exact: true }),
    ).toHaveCount(0);
  });
});

/**
 * Open the values of the decision stage on the reader that holds the chat
 * model.
 *
 * The model of the llama.cpp server belongs to the stage that reads it, so
 * the fields of the server live in the panel of that stage rather than in
 * the card of the flow.
 */
async function chatModelStage(page: Page): Promise<void> {
  await page
    .locator('section')
    .filter({ hasText: 'Layered router' })
    .getByRole('button', { name: /Decision/ })
    .click();
  await page
    .getByRole('region', {
      name: 'The values of one step of the layered router',
    })
    .getByRole('radio', { name: 'Model', exact: true })
    .click();
}

/**
 * Point the decision stage at the model server and store it.
 *
 * The suite reads the intent of a message with the fake model server, so a
 * turn that has to name an intent needs a decision stage that asks the
 * model. The stage is a stored setting, so the test writes it rather than
 * depending on the run before it.
 */
async function useModelDecision(page: Page): Promise<void> {
  await page.goto('/settings/');
  await waitForShell(page);
  await chatModelStage(page);
  await page
    .getByRole('region', {
      name: 'The values of one step of the layered router',
    })
    .getByRole('button', { name: 'Save resolver' })
    .click();
  await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  const stored = await page.request.get(`${ALICE_BASE}/api/v1/settings`);
  expect((await stored.json()).routerDecide).toBe('generative');
}

test.describe('resolver settings', () => {
  test('offers the models of the server and persists the choice', async ({
    page,
  }) => {
    await page.goto('/settings/');
    await waitForShell(page);
    // The chat model belongs to the stage that reads it, so the test opens
    // the decision stage on the server reader first.
    await chatModelStage(page);
    // The model is a dropdown, because the daemon maps the list from the
    // llama.cpp server instead of asking the user for a name.
    const model = page.getByRole('combobox', {
      name: 'Model of the llama.cpp server',
    });
    const before = await model.inputValue();
    expect(before.length).toBeGreaterThan(0);
    await expect(model.locator('option')).toHaveCount(2);

    await model.selectOption('fake-small');
    await page.getByRole('button', { name: 'Save resolver' }).click();
    // Wait for the save to land before the reload reads the stored value.
    await expect(page.getByText('Saved', { exact: true })).toBeVisible();

    // The daemon stored the value, not only the page.
    const stored = await page.request.get(`${ALICE_BASE}/api/v1/settings`);
    expect((await stored.json()).resolverModel).toBe('fake-small');

    await page.reload();
    await waitForShell(page);
    // The panel opens on the first stage again, so the test walks back to
    // the stage that holds the chat model.
    await chatModelStage(page);
    const reloaded = page.getByRole('combobox', {
      name: 'Model of the llama.cpp server',
    });
    await expect(reloaded).toHaveValue('fake-small');

    // Restore the configured value, so the next run starts where it did.
    await reloaded.selectOption(before);
    await page.getByRole('button', { name: 'Save resolver' }).click();
    await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  });

  test('refuses an address that is not a web address', async ({ page }) => {
    await page.goto('/settings/');
    await waitForShell(page);
    await chatModelStage(page);
    const address = page.getByRole('textbox', {
      name: 'Address of the llama.cpp server',
    });
    const before = await address.inputValue();

    await address.fill('127.0.0.1:8012');
    await page.getByRole('button', { name: 'Save resolver' }).click();
    await expect(page.getByText('Save failed', { exact: true })).toBeVisible();

    // Restore the address, so the resolver stays reachable for later runs.
    await address.fill(before);
    await page.getByRole('button', { name: 'Save resolver' }).click();
    await expect(page.getByText('Saved', { exact: true })).toBeVisible();
  });
});
