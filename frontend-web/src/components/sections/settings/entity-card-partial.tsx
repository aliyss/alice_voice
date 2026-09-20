/**
 * `EntityCard` writes one entity of an intent.
 *
 * An entity answers three questions: what it is called, how it takes its
 * value, and what the turn does when the value is missing. The card is a
 * block of its own inside the form, so two entities read as two blocks.
 *
 * The name takes a line of its own and the two switches stand under it, so
 * a long name never squeezes a switch beside it. The sentence about a
 * choice waits in the hint of its name. A closed entity takes its values
 * as one list and a script entity takes the command the daemon runs for
 * its live list, with a preview so the writer reads the list the daemon
 * would.
 *
 * The card is a component of the form and not part of its markup: the
 * values of an entity are the values of the form, and the card reports
 * every change through a callback rather than owning them.
 */
import type { QRL } from '@builder.io/qwik';

import type { EntityKindDto, ScriptPreviewDto } from '~/types/dto';

import { $, component$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { FieldLabel } from '~/components/ui/field-label';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

/** One entity of the form. Its values arrive as one comma separated text. */
export interface EntityDraft {
  /** The stable key of the row. */
  key: string;
  /** The name of the entity. */
  name: string;
  /** How the entity takes its value. */
  kind: EntityKindDto;
  /** The values of a closed entity, comma separated. */
  values: string;
  /** The shell command of a script entity. */
  script: string;
  /** Whether the intent needs a value for this entity. */
  required: boolean;
}

/** The preview of one script, and the entity its writer asked it for. */
export interface ScriptPreview {
  /** The key of the entity the preview belongs to. */
  key: string;
  /** True while the script runs. */
  pending: boolean;
  /** The values the script answered with, or null when it did not answer. */
  result: ScriptPreviewDto | null;
  /** The message of a preview that did not reach the daemon, or null. */
  error: string | null;
}

/** The values of the kind switch, in the order the card shows them. */
const KINDS: { value: EntityKindDto; label: string }[] = [
  { value: 'open', label: 'Open' },
  { value: 'closed', label: 'Closed' },
  { value: 'script', label: 'Script' },
];

/** The values of the switch of a missing value. */
const MISSING: { value: string; label: string }[] = [
  { value: 'required', label: 'Required' },
  { value: 'optional', label: 'Optional' },
];

/** The one line that explains an entity kind. */
const KIND_NOTE: Record<EntityKindDto, string> = {
  open: 'An open entity takes the value the user says. The daemon reads it out of the message and puts it into the command.',
  closed:
    'A closed entity carries the values the resolver may choose from. Type them below as a list, separated by a comma.',
  script:
    'A script entity carries the command the daemon runs for its live list. A short list reaches the resolver as choices, a long one the daemon matches the message against.',
};

/** The one line that explains what a missing value does to the turn. */
const MISSING_NOTE =
  'A required value the resolver could not read stops the turn and asks the user for it. An optional one leaves the command without the value.';

/** The props of `EntityCard`. */
export interface EntityCardProps {
  /** The entity the card writes. */
  entity: EntityDraft;
  /** The place of the entity in the form, counted from one. */
  position: number;
  /** True while the save request is in flight. */
  pending: boolean;
  /** The preview of the script of this entity, or null. */
  preview: ScriptPreview | null;
  /** Report one changed value of the entity. */
  onChange$: QRL<(change: Partial<EntityDraft>) => void>;
  /** Remove the entity from the form. */
  onRemove$: QRL<() => void>;
  /** Run the script of the entity and show the values it answers with. */
  onPreview$: QRL<(script: string) => void>;
}

export const EntityCard = component$<EntityCardProps>((props) => {
  const entity = props.entity;

  return (
    <Stack
      gap="md"
      class="rounded-ds-md border border-ds-line p-3"
      ariaLabel={`Entity ${props.position}`}
      role="group"
    >
      <Stack direction="row" gap="sm" align="center" justify="between">
        <Text size="micro" tone="faint">
          {`Entity ${props.position}`}
        </Text>
        <Button
          size="sm"
          variant="quiet"
          disabled={props.pending}
          onClick$={props.onRemove$}
        >
          Remove
        </Button>
      </Stack>

      {/* The name takes a line of its own, so a long name never squeezes
          the switches beside it. */}
      <Stack gap="xs">
        <FieldLabel
          label="Name"
          hint="The name stands in the command as a placeholder, so `{city}` reads the value of the entity called `city`."
        />
        <TextInput
          kind="input"
          surface="field"
          ariaLabel="Name of the entity"
          placeholder="city"
          value={entity.name}
          disabled={props.pending}
          onInput$={$((event: Event) => {
            props.onChange$({
              name: (event.target as HTMLInputElement).value,
            });
          })}
        />
      </Stack>

      <Stack gap="xs">
        <FieldLabel label="Kind" hint={KIND_NOTE[entity.kind]} />
        <ContentSwitcher
          ariaLabel="Kind of the entity"
          value={entity.kind}
          options={KINDS}
          disabled={props.pending}
          onPick$={$((next: string) => {
            props.onChange$({ kind: next as EntityKindDto });
          })}
        />
      </Stack>

      <Stack gap="xs">
        <FieldLabel label="When the value is missing" hint={MISSING_NOTE} />
        <ContentSwitcher
          ariaLabel="When the value of the entity is missing"
          value={entity.required ? 'required' : 'optional'}
          options={MISSING}
          disabled={props.pending}
          onPick$={$((next: string) => {
            props.onChange$({ required: next === 'required' });
          })}
        />
      </Stack>

      {entity.kind === 'closed' ? (
        <Stack gap="xs">
          <FieldLabel
            label="Values"
            hint="Separate the values with a comma. The resolver picks one of them."
          />
          <TextInput
            kind="input"
            surface="field"
            ariaLabel="Values of the closed entity"
            placeholder="today, tomorrow"
            value={entity.values}
            disabled={props.pending}
            onInput$={$((event: Event) => {
              props.onChange$({
                values: (event.target as HTMLInputElement).value,
              });
            })}
          />
        </Stack>
      ) : entity.kind === 'script' ? (
        <Stack gap="xs">
          <Stack direction="row" gap="sm" align="center" justify="between" wrap>
            <FieldLabel
              label="Script"
              hint="The daemon runs the script with `sh -c` and reads one value per line of its output."
            />
            <Button
              size="sm"
              variant="outline"
              disabled={
                props.pending ||
                entity.script.trim().length === 0 ||
                props.preview?.pending === true
              }
              onClick$={() => props.onPreview$(entity.script.trim())}
            >
              {props.preview?.pending ? 'Running...' : 'Preview'}
            </Button>
          </Stack>
          <TextInput
            kind="textarea"
            surface="field"
            ariaLabel="Script of the entity"
            placeholder="ls /usr/share/applications"
            value={entity.script}
            disabled={props.pending}
            class="min-h-20 font-ds-hud"
            onInput$={$((event: Event) => {
              props.onChange$({
                script: (event.target as HTMLTextAreaElement).value,
              });
            })}
          />

          {props.preview ? (
            <ScriptPreviewBlock preview={props.preview} />
          ) : null}
        </Stack>
      ) : null}
    </Stack>
  );
});

/** The props of `ScriptPreviewBlock`. */
interface ScriptPreviewBlockProps {
  /** The preview to render. */
  preview: ScriptPreview;
}

/** Show what one script answers with. */
const ScriptPreviewBlock = component$<ScriptPreviewBlockProps>((props) => {
  if (props.preview.pending) {
    return (
      <Stack direction="row" gap="xs" align="center">
        <Spinner size="sm" label="The script is running" />
        <Text size="hud" tone="faint">
          Running the script...
        </Text>
      </Stack>
    );
  }

  if (props.preview.error) {
    return (
      <Alert
        tone="error"
        title="The preview failed"
        message={props.preview.error}
      />
    );
  }

  const result = props.preview.result;
  if (!result) {
    return null;
  }

  if (result.error) {
    return (
      <Alert tone="warn" title="The script failed" message={result.error} />
    );
  }

  return (
    <Stack gap="xs">
      <Stack direction="row" gap="xs" align="center" wrap>
        <Badge
          tone={result.values.length > 0 ? 'ok' : 'warn'}
          label={`${result.values.length} value${result.values.length === 1 ? '' : 's'}`}
        />
        <Text size="hud" tone="faint">
          {`${result.durationMs} ms`}
        </Text>
      </Stack>
      {result.values.length > 0 ? (
        <Stack direction="row" gap="xs" wrap>
          {result.values.slice(0, 24).map((value) => (
            <Badge key={value} tone="neutral" label={value} />
          ))}
        </Stack>
      ) : (
        <Text size="hud" tone="faint">
          The script answered with no value.
        </Text>
      )}
      {result.values.length > 24 ? (
        <Text size="hud" tone="faint">
          {`... and ${result.values.length - 24} more`}
        </Text>
      ) : null}
    </Stack>
  );
});
