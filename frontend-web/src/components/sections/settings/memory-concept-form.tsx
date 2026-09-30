/**
 * `MemoryConceptForm` edits one concept of the long term memory.
 *
 * The form owns the draft values: the name, the body, and the facts that
 * are true now. A new fact is added to the draft and written on save. A
 * fact that is already true has its value edited, and a save closes the
 * older value rather than deleting it, because the memory is dated.
 *
 * A fact is retired on its own, so a word the daemon learned wrongly
 * leaves the current memory without erasing the history. The replaced
 * facts stay under the editable ones, so a reader sees what a value was
 * before it changed.
 *
 * A concept a reader opened twice can move into one of the others, so the
 * memory settles the two concepts it holds for one thing. The merge asks
 * for a choice rather than doing it at once, because it is the one change
 * here that cannot be undone by typing the old value back.
 */
import type { QRL } from '@builder.io/qwik';

import type {
  MemoryFactDto,
  MemoryNodeDto,
  MemoryWriteRequestDto,
} from '~/types/dto';

import { $, component$, useSignal, useTask$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { FieldLabel } from '~/components/ui/field-label';
import { PanelGroup } from '~/components/ui/panel-group';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

/** One fact of the form. A stored fact carries the identifier it came from. */
interface FactDraft {
  /** The key of the row in the list. */
  key: string;
  /** The identifier of the stored fact, or null for a fact just written. */
  id: string | null;
  /** The relation the fact names. */
  relation: string;
  /** The value the fact carries. */
  value: string;
  /** When the memory last saw the fact, and how often it saw it again. */
  note: string;
}

/** Build the drafts of the facts that are true now. */
function toDrafts(concept: MemoryNodeDto | null): FactDraft[] {
  if (!concept) {
    return [];
  }
  return concept.facts
    .filter((fact) => fact.current)
    .map((fact) => ({
      key: fact.id,
      id: fact.id,
      relation: fact.relation,
      value: fact.value,
      note: factNote(fact),
    }));
}

/**
 * One line about how often and when the memory saw a fact.
 *
 * A fact a later turn taught again is worth more than one nothing ever
 * repeated, so the reader sees which of them the daemon keeps finding.
 */
function factNote(fact: MemoryFactDto): string {
  const seen = `seen ${day(fact.lastConfirmedAt)}`;
  if (fact.confirmations <= 0) {
    return seen;
  }
  const times =
    fact.confirmations === 1 ? 'once more' : `${fact.confirmations} more times`;
  return `${seen} \u00b7 taught ${times}`;
}

/** Read the facts a newer fact replaced. */
function replaced(concept: MemoryNodeDto | null): MemoryFactDto[] {
  if (!concept) {
    return [];
  }
  return concept.facts.filter((fact) => !fact.current);
}

/** Show the day of one timestamp, or `now` when it is open. */
function day(value: string | null): string {
  return value ? value.slice(0, 10) : 'now';
}

/** The props of `MemoryConceptForm`. */
export interface MemoryConceptFormProps {
  /**
   * The stored concept the form edits, or null.
   *
   * The page draws the form while a concept is picked, and the proposal
   * of the values is read again when the pick closes, so the form reads a
   * concept that may be gone and writes nothing for it.
   */
  concept: MemoryNodeDto | null;
  /** The other concepts of the memory, so this one can move into one. */
  others: MemoryNodeDto[];
  /** True while a save, a retire, or a delete is in flight. */
  pending: boolean;
  /** The last failure message, or null. */
  error: string | null;
  /** Save the concept. The page defines the handle. */
  onSave$: QRL<(input: MemoryWriteRequestDto) => void>;
  /** Retire one stored fact. The page defines the handle. */
  onRetireFact$: QRL<(factId: string) => void>;
  /** Move this concept into another one. The page defines the handle. */
  onMerge$: QRL<(into: string) => void>;
  /** Delete the whole concept. The page defines the handle. */
  onDelete$: QRL<() => void>;
}

export const MemoryConceptForm = component$<MemoryConceptFormProps>((props) => {
  const title = useSignal(props.concept?.title ?? '');
  const body = useSignal(props.concept?.body ?? '');
  const facts = useSignal<FactDraft[]>(toDrafts(props.concept));
  const newRelation = useSignal('');
  const newValue = useSignal('');
  const nextKey = useSignal(0);
  const merging = useSignal(false);

  // A save or a retire answers with the stored concept, so the drafts
  // follow it when it changes instead of keeping a stale fact list. A
  // concept that is gone leaves the form empty rather than stale.
  useTask$(({ track }) => {
    const concept = track(() => props.concept);
    title.value = concept?.title ?? '';
    body.value = concept?.body ?? '';
    facts.value = toDrafts(concept);
    merging.value = false;
  });

  const handleAdd = $(() => {
    const relation = newRelation.value.trim().toLowerCase();
    const value = newValue.value.trim();
    if (!relation || !value) {
      return;
    }
    nextKey.value += 1;
    facts.value = [
      ...facts.value,
      {
        key: `new-${nextKey.value}`,
        id: null,
        relation,
        value,
        note: 'not stored yet',
      },
    ];
    newRelation.value = '';
    newValue.value = '';
  });

  const handleRemove = $((key: string) => {
    facts.value = facts.value.filter((draft) => draft.key !== key);
  });

  const handleValue = $((key: string, value: string) => {
    facts.value = facts.value.map((draft) =>
      draft.key === key ? { ...draft, value } : draft,
    );
  });

  const handleSave = $(() => {
    if (!props.concept) {
      return;
    }
    props.onSave$({
      key: props.concept.key,
      title: title.value.trim(),
      body: body.value.trim(),
      facts: facts.value
        .map((draft) => ({
          relation: draft.relation.trim().toLowerCase(),
          value: draft.value.trim(),
        }))
        .filter((fact) => fact.relation.length > 0 && fact.value.length > 0),
    });
  });

  const replacedFacts = replaced(props.concept);

  return (
    <Stack gap="md">
      <PanelGroup
        title="The concept"
        hint="The key names the concept and never changes. The name and the body are what a reader sees."
      >
        <Stack gap="sm">
          <FieldLabel label="Key" hint="The identifier of the concept." />
          <TextInput
            kind="input"
            surface="field"
            name="memory-key"
            ariaLabel="Key of the concept"
            value={props.concept?.key ?? ''}
            disabled
          />
          <FieldLabel
            label="Name"
            hint="The words the memory shows for the concept."
          />
          <TextInput
            kind="input"
            surface="field"
            name="memory-title"
            ariaLabel="Name of the concept"
            value={title.value}
            disabled={props.pending}
            onInput$={$((event: Event) => {
              title.value = (event.target as HTMLInputElement).value;
            })}
          />
          <FieldLabel
            label="Body"
            hint="The plain text of the concept. What the turn learned beyond the facts lives here."
          />
          <TextInput
            kind="textarea"
            surface="field"
            name="memory-body"
            ariaLabel="Body of the concept"
            value={body.value}
            disabled={props.pending}
            class="min-h-20"
            onInput$={$((event: Event) => {
              body.value = (event.target as HTMLTextAreaElement).value;
            })}
          />
        </Stack>
      </PanelGroup>

      <PanelGroup
        title="The facts"
        hint="A fact is one relation and one value. A saved value closes the older one, and a value retired on its own leaves the current memory without erasing the history."
      >
        <Stack gap="sm">
          {facts.value.length === 0 ? (
            <Text size="hud" tone="faint" block>
              The concept carries no fact yet.
            </Text>
          ) : null}

          {facts.value.map((draft) => (
            <Stack key={draft.key} direction="row" gap="sm" align="end">
              {draft.id ? (
                <Stack gap="xs" class="w-28 shrink-0">
                  <FieldLabel label="Relation" />
                  <Badge tone="neutral" label={draft.relation} />
                </Stack>
              ) : (
                <Stack gap="xs" class="w-28 shrink-0">
                  <FieldLabel label="Relation" />
                  <TextInput
                    kind="input"
                    surface="field"
                    name={`relation-${draft.key}`}
                    ariaLabel="Relation of the fact"
                    placeholder="lives_in"
                    value={draft.relation}
                    disabled={props.pending}
                    onInput$={$((event: Event) => {
                      const next = (event.target as HTMLInputElement).value;
                      facts.value = facts.value.map((entry) =>
                        entry.key === draft.key
                          ? { ...entry, relation: next }
                          : entry,
                      );
                    })}
                  />
                </Stack>
              )}
              <Stack gap="xs" class="min-w-0 flex-1">
                <FieldLabel label="Value" />
                <TextInput
                  kind="input"
                  surface="field"
                  name={`value-${draft.key}`}
                  ariaLabel="Value of the fact"
                  value={draft.value}
                  disabled={props.pending}
                  onInput$={$((event: Event) => {
                    handleValue(
                      draft.key,
                      (event.target as HTMLInputElement).value,
                    );
                  })}
                />
                {draft.id ? (
                  <Text size="micro" tone="faint">
                    {draft.note}
                  </Text>
                ) : null}
              </Stack>
              <Button
                variant="outline"
                size="md"
                class="shrink-0"
                disabled={props.pending}
                onClick$={
                  draft.id
                    ? $(() => props.onRetireFact$(draft.id as string))
                    : $(() => handleRemove(draft.key))
                }
              >
                {draft.id ? 'Forget' : 'Remove'}
              </Button>
            </Stack>
          ))}

          <Stack
            direction="row"
            gap="sm"
            align="end"
            class="border-t border-ds-line pt-3"
          >
            <Stack gap="xs" class="w-28 shrink-0">
              <FieldLabel label="New relation" />
              <TextInput
                kind="input"
                surface="field"
                name="memory-new-relation"
                ariaLabel="Relation of the new fact"
                placeholder="prefers"
                value={newRelation.value}
                disabled={props.pending}
                onInput$={$((event: Event) => {
                  newRelation.value = (event.target as HTMLInputElement).value;
                })}
              />
            </Stack>
            <Stack gap="xs" class="min-w-0 flex-1">
              <FieldLabel label="New value" />
              <TextInput
                kind="input"
                surface="field"
                name="memory-new-value"
                ariaLabel="Value of the new fact"
                placeholder="Neovim"
                value={newValue.value}
                disabled={props.pending}
                onInput$={$((event: Event) => {
                  newValue.value = (event.target as HTMLInputElement).value;
                })}
              />
            </Stack>
            <Button
              variant="outline"
              size="md"
              class="shrink-0"
              disabled={props.pending}
              onClick$={handleAdd}
            >
              Add fact
            </Button>
          </Stack>
        </Stack>
      </PanelGroup>

      {replacedFacts.length > 0 ? (
        <PanelGroup
          title="Replaced"
          hint="The values a newer value closed. They stay readable, so the memory answers a question about the past."
        >
          <Stack gap="xs">
            {replacedFacts.map((fact) => (
              <Stack key={fact.id} direction="row" gap="sm" align="center">
                <Badge tone="neutral" label={fact.relation} />
                <Text size="hud" tone="faint">
                  {`${fact.value} \u00b7 until ${day(fact.invalidAt)}`}
                </Text>
              </Stack>
            ))}
          </Stack>
        </PanelGroup>
      ) : null}

      {props.others.length > 0 ? (
        <PanelGroup
          title="Merge"
          hint="Two concepts that name one thing can become one. The facts of this concept move into the concept you pick, its key becomes a name of it, a shared relation settles by the newer value, and this concept goes."
        >
          <Stack gap="sm">
            {merging.value ? (
              <Stack direction="row" gap="sm" align="center" wrap>
                {props.others.slice(0, 8).map((other) => (
                  <Button
                    key={other.id}
                    variant="outline"
                    size="md"
                    disabled={props.pending}
                    onClick$={$(() => props.onMerge$(other.id))}
                  >{`Into ${other.title}`}</Button>
                ))}
                <Button
                  variant="quiet"
                  size="md"
                  disabled={props.pending}
                  onClick$={$(() => {
                    merging.value = false;
                  })}
                >
                  Cancel
                </Button>
              </Stack>
            ) : (
              <Button
                variant="outline"
                size="md"
                class="self-start"
                disabled={props.pending}
                onClick$={$(() => {
                  merging.value = true;
                })}
              >
                Merge into another concept
              </Button>
            )}
          </Stack>
        </PanelGroup>
      ) : null}

      {props.error ? (
        <Alert tone="error" title="Change failed" message={props.error} />
      ) : null}

      <Stack direction="row" gap="sm" align="center" wrap>
        <Button
          variant="solid"
          size="md"
          disabled={props.pending}
          onClick$={handleSave}
        >
          {props.pending ? 'Saving...' : 'Save concept'}
        </Button>
        <Button
          variant="outline"
          size="md"
          disabled={props.pending}
          onClick$={props.onDelete$}
        >
          Delete concept
        </Button>
      </Stack>
    </Stack>
  );
});
