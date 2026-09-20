/**
 * `IntentFormSection` writes one intent.
 *
 * The section owns the draft values, including the entities, the values of
 * the closed ones, the script of a script entity, and the phrases a user
 * may say. The page owns the save action, so the section stays free of the
 * REST call.
 *
 * The form is one column: what the intent is, what it runs, and the
 * entities it reads under them. Each entity is a card of its own, and the
 * sentence about a value waits in the hint of its name, so the panel shows
 * what a reader writes rather than the prose around it.
 *
 * An entity is open, closed, or script. A closed entity carries the values
 * the resolver may choose from, the form takes them as one comma separated
 * text, and a script entity carries the command the daemon runs for its
 * live list. An entity is required or optional: the daemon asks the user
 * for a required value it could not read, and runs the command of an
 * optional one without the value.
 */
import type { QRL } from '@builder.io/qwik';

import type {
  EntityDraft,
  ScriptPreview,
} from '~/components/sections/settings/entity-card-partial';

import type { IntentDto } from '~/types/dto';

import type { ScriptPreviewResult } from '~/api/intents';

import type { IntentInput } from '~/schemas/intent';

import { $, component$, useSignal } from '@builder.io/qwik';

import { EntityCard } from '~/components/sections/settings/entity-card-partial';
import { Alert } from '~/components/ui/alert';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { FieldLabel } from '~/components/ui/field-label';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

/** The props of `IntentFormSection`. */
export interface IntentFormSectionProps {
  /** The intent to write, or null to write a new one. */
  intent: IntentDto | null;
  /**
   * Draw the frame and the name of the form. It is on by default. A form
   * inside a panel carries neither, because the panel has both.
   */
  frame?: boolean;
  /** True while the save request is in flight. */
  pending: boolean;
  /** The last failure message, or null. */
  error: string | null;
  /** Save the intent. The page defines the handle. */
  onSave$: QRL<(input: IntentInput) => void>;
  /** Run one script and return the values it answers with. */
  onPreviewScript$: QRL<(script: string) => Promise<ScriptPreviewResult>>;
  /** Close the form without a write. */
  onCancel$: QRL<() => void>;
}

/** Read the stored entities of an intent as form drafts. */
function toDrafts(intent: IntentDto | null): EntityDraft[] {
  if (!intent) {
    return [];
  }
  return intent.entities.map((entity) => ({
    key: entity.id,
    name: entity.name,
    kind: entity.kind,
    values: entity.values.join(', '),
    script: entity.script ?? '',
    required: entity.required,
  }));
}

/** Split a comma separated text into the values it holds. */
function toValues(text: string): string[] {
  return text
    .split(',')
    .map((value) => value.trim())
    .filter((value) => value.length > 0);
}

/** Split a text into the phrases it holds, one per line. */
function toExamples(text: string): string[] {
  return text
    .split('\n')
    .map((phrase) => phrase.trim())
    .filter((phrase) => phrase.length > 0);
}

export const IntentFormSection = component$<IntentFormSectionProps>((props) => {
  const name = useSignal(props.intent?.name ?? '');
  const description = useSignal(props.intent?.description ?? '');
  const command = useSignal(props.intent?.command ?? '');
  const examples = useSignal((props.intent?.examples ?? []).join('\n'));
  const entities = useSignal<EntityDraft[]>(toDrafts(props.intent));
  const nextKey = useSignal(props.intent?.entities.length ?? 0);
  const preview = useSignal<ScriptPreview | null>(null);

  const handleAdd = $(() => {
    const key = `entity-${nextKey.value}`;
    nextKey.value += 1;
    entities.value = [
      ...entities.value,
      {
        key,
        name: '',
        kind: 'open',
        values: '',
        script: '',
        required: true,
      },
    ];
  });

  const handleRemove = $((key: string) => {
    entities.value = entities.value.filter((entity) => entity.key !== key);
    if (preview.value?.key === key) {
      preview.value = null;
    }
  });

  const handleChange = $((key: string, change: Partial<EntityDraft>) => {
    entities.value = entities.value.map((entity) =>
      entity.key === key ? { ...entity, ...change } : entity,
    );
  });

  const handlePreview = $(async (key: string, script: string) => {
    preview.value = { key, pending: true, result: null, error: null };
    const result = await props.onPreviewScript$(script);
    // The answer may arrive after the writer changed the row, so the
    // preview checks that it still belongs to the row it started from.
    const current = entities.value.find((entity) => entity.key === key)?.script;
    if (current !== script) {
      preview.value = null;
      return;
    }
    preview.value = result.failed
      ? { key, pending: false, result: null, error: result.message }
      : { key, pending: false, result: result.data, error: null };
  });

  const handleSave = $(() => {
    props.onSave$({
      name: name.value.trim(),
      description: description.value.trim(),
      command: command.value.trim(),
      entities: entities.value.map((entity) => ({
        name: entity.name.trim(),
        kind: entity.kind,
        values: entity.kind === 'closed' ? toValues(entity.values) : [],
        script: entity.kind === 'script' ? entity.script.trim() : '',
        required: entity.required,
      })),
      examples: toExamples(examples.value),
    });
  });

  const body = (
    <Stack gap="md">
      <Stack gap="xs">
        <FieldLabel
          label="Name"
          hint="The resolver reads the name, so give it the words a user would say."
        />
        <TextInput
          kind="input"
          surface="field"
          name="intentName"
          ariaLabel="Name of the intent"
          placeholder="get weather"
          value={name.value}
          disabled={props.pending}
          onInput$={$((event: Event) => {
            name.value = (event.target as HTMLInputElement).value;
          })}
        />
      </Stack>

      <Stack gap="xs">
        <FieldLabel
          label="Description"
          hint="One sentence about what the intent does. The router reads it next to the name."
        />
        <TextInput
          kind="input"
          surface="field"
          name="intentDescription"
          ariaLabel="Description of the intent"
          placeholder="Look up the weather for one city."
          value={description.value}
          disabled={props.pending}
          onInput$={$((event: Event) => {
            description.value = (event.target as HTMLInputElement).value;
          })}
        />
      </Stack>

      <Stack gap="xs">
        <FieldLabel
          label="Command"
          hint="The daemon runs the command through `sh -c` when the resolver chooses this intent. It has a timeout and a size limit. Name an entity with `{name}` and the daemon runs the command with the value it read for that entity."
        />
        <TextInput
          kind="textarea"
          surface="field"
          name="intentCommand"
          ariaLabel="Shell command of the intent"
          placeholder="curl wttr.in/Berlin"
          value={command.value}
          disabled={props.pending}
          class="min-h-20 font-ds-hud"
          onInput$={$((event: Event) => {
            command.value = (event.target as HTMLTextAreaElement).value;
          })}
        />
      </Stack>

      <Stack gap="xs">
        <FieldLabel
          label="Phrases"
          hint="One phrase per line. The resolver reads them next to the name, so a phrase that names the intent helps it choose."
        />
        <TextInput
          kind="textarea"
          surface="field"
          name="intentExamples"
          ariaLabel="Phrases a user may say for this intent"
          placeholder={'launch firefox\nopen obs\nstart discord'}
          value={examples.value}
          disabled={props.pending}
          class="min-h-20"
          onInput$={$((event: Event) => {
            examples.value = (event.target as HTMLTextAreaElement).value;
          })}
        />
      </Stack>

      {/* The entities of the intent stand under its values: each one is a
          card of its own, so the form reads as one list of what the intent
          is and what it needs. */}
      <Stack gap="sm" class="border-t border-ds-line pt-4">
        <Stack direction="row" gap="md" align="center" justify="between" wrap>
          <FieldLabel
            label="Entities"
            hint="An entity is one value the intent needs out of the message, for example the city of a weather command."
          />
          <Button
            size="sm"
            variant="outline"
            disabled={props.pending}
            onClick$={handleAdd}
          >
            Add entity
          </Button>
        </Stack>

        {entities.value.length === 0 ? (
          <Text size="hud" tone="faint" block>
            An intent reads nothing out of the message yet.
          </Text>
        ) : null}

        {entities.value.map((entity, index) => (
          <EntityCard
            key={entity.key}
            entity={entity}
            position={index + 1}
            pending={props.pending}
            preview={preview.value?.key === entity.key ? preview.value : null}
            onChange$={$((change: Partial<EntityDraft>) => {
              handleChange(entity.key, change);
            })}
            onRemove$={$(() => {
              handleRemove(entity.key);
            })}
            onPreview$={$((script: string) => {
              handlePreview(entity.key, script);
            })}
          />
        ))}
      </Stack>

      {props.error ? (
        <Alert tone="error" title="Save failed" message={props.error} />
      ) : null}

      <Stack direction="row" gap="sm" align="center">
        <Button
          variant="solid"
          size="md"
          disabled={props.pending}
          onClick$={handleSave}
        >
          {props.pending ? 'Saving...' : 'Save intent'}
        </Button>
        <Button
          variant="outline"
          size="md"
          disabled={props.pending}
          onClick$={props.onCancel$}
        >
          Cancel
        </Button>
      </Stack>
    </Stack>
  );

  // A panel carries the frame and the name of the form, so a form inside a
  // panel draws no frame of its own.
  if (props.frame === false) {
    return body;
  }

  return (
    <Card
      label={props.intent ? `Edit ${props.intent.name}` : 'New intent'}
      class="w-full"
    >
      {body}
    </Card>
  );
});
