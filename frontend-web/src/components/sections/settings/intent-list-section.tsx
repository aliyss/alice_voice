/**
 * `IntentListSection` holds the intent configuration.
 *
 * One row is one intent: the name the resolver chooses from, the command
 * the daemon runs, and the entities the intent reads. The row is the pick
 * itself, so a reader presses an intent to edit it rather than reaching for
 * an action button, and the delete keeps its own press beside it. The
 * section reports the actions through callback props, so it stays free of
 * the REST call.
 */
import type { QRL } from '@builder.io/qwik';

import type { IntentDto, IntentEntityDto } from '~/types/dto';

import { $, component$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { EmptyState } from '~/components/ui/empty-state';
import { InfoHint } from '~/components/ui/info-hint';
import { ScrollArea } from '~/components/ui/scroll-area';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The props of `IntentListSection`. */
export interface IntentListSectionProps {
  /** The stored intents, in name order. */
  intents: IntentDto[];
  /** True while a write request is in flight. */
  pending: boolean;
  /**
   * True when an intent cannot be written.
   *
   * An intent is a database setting, so a database that does not answer
   * makes every write useless.
   */
  disabled: boolean;
  /** Why the intents cannot be written, or null. */
  reason: string | null;
  /** The intent the form edits, or null. */
  editingId: string | null;
  /**
   * Open the form for one intent, or for a new one when the id is null.
   * The id of the intent a reader picked, or null for a new one.
   */
  onEdit$: QRL<(id: string | null) => void>;
  /** Delete one intent. */
  onDelete$: QRL<(id: string) => void>;
  /** Add the example intents the catalog does not hold yet. */
  onAddExamples$: QRL<() => void>;
}

/** Describe one entity of an intent. */
function describeEntity(entity: IntentEntityDto): string {
  const kind =
    entity.kind === 'closed'
      ? `${entity.name}: ${entity.values.join(', ')}`
      : entity.kind === 'script'
        ? `${entity.name}: script`
        : `${entity.name}: open`;

  // An optional entity is the one the resolver may leave out, so it is
  // the one worth naming here. A required entity is the rule.
  return entity.required ? kind : `${kind} (optional)`;
}

export const IntentListSection = component$<IntentListSectionProps>((props) => {
  return (
    <Card
      class="flex h-full min-h-0 w-full flex-col"
      bodyClass="flex min-h-0 flex-1 flex-col"
    >
      <Stack gap="md" class="min-h-0 flex-1">
        <Stack
          direction="row"
          gap="md"
          align="center"
          justify="between"
          wrap
          class="shrink-0"
        >
          <Stack direction="row" gap="xs" align="center" class="min-w-0">
            <Text size="body" weight="medium">
              Catalog
            </Text>
            <InfoHint
              label="About the catalog"
              text="The resolver chooses among these intents and the daemon runs the command of the one it chooses. Add examples writes the intents a fresh installation starts with."
              align="left"
            />
          </Stack>
          <Stack direction="row" gap="xs" align="center">
            <Button
              variant="quiet"
              size="md"
              disabled={props.pending || props.disabled}
              onClick$={props.onAddExamples$}
            >
              Add examples
            </Button>
            <Button
              variant="outline"
              size="md"
              disabled={props.pending || props.disabled}
              onClick$={() => props.onEdit$(null)}
            >
              New intent
            </Button>
          </Stack>
        </Stack>

        {props.disabled && props.reason ? (
          <Alert tone="warn" title="Cannot be changed" message={props.reason} />
        ) : null}

        {props.intents.length === 0 ? (
          <EmptyState
            title="No intents yet"
            description="Add an intent and the daemon reads the messages that ask for it, then runs its command."
          />
        ) : (
          // The list of intents owns the rest of the section and scrolls
          // inside it, so a long catalog stays readable next to the editor
          // of the intent the reader picked.
          <ScrollArea ariaLabel="Intents" class="min-h-0 flex-1">
            <Stack gap="sm">
              {props.intents.map((intent) => (
                <Card
                  key={intent.id}
                  // One row is one intent. The row is the pick, so a reader
                  // presses an intent to edit it, and the name is its label,
                  // so a reader and a test reach the row of one intent.
                  role="article"
                  ariaLabel={intent.name}
                  tone="sunken"
                  frame={false}
                  pickable={!props.pending && !props.disabled}
                  picked={props.editingId === intent.id}
                  pickLabel={`Edit ${intent.name}`}
                  bodyClass="flex flex-col gap-2 p-4"
                  onPick$={$(() => props.onEdit$(intent.id))}
                >
                  <Stack
                    direction="row"
                    gap="sm"
                    align="center"
                    justify="between"
                    wrap
                  >
                    <Text size="body" weight="medium">
                      {intent.name}
                    </Text>
                    <Button
                      size="sm"
                      variant="quiet"
                      class="relative z-10"
                      disabled={props.pending || props.disabled}
                      onClick$={() => props.onDelete$(intent.id)}
                    >
                      Delete
                    </Button>
                  </Stack>

                  {intent.description ? (
                    <Text size="body" tone="muted" block>
                      {intent.description}
                    </Text>
                  ) : null}

                  <Text size="hud" tone="faint" block class="break-all">
                    {intent.command}
                  </Text>

                  {intent.entities.length > 0 ? (
                    <Stack direction="row" gap="xs" wrap>
                      {intent.entities.map((entity) => (
                        <Badge
                          key={entity.id}
                          tone="neutral"
                          label={describeEntity(entity)}
                        />
                      ))}
                    </Stack>
                  ) : null}
                </Card>
              ))}
            </Stack>
          </ScrollArea>
        )}
      </Stack>
    </Card>
  );
});
