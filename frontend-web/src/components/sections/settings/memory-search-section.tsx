/**
 * `MemorySearchSection` shows the memory of the daemon and edits a concept.
 *
 * The cloud draws what the memory holds and the list reads it, so a reader
 * sees the shape of the memory and the concepts of it in one view. The
 * cloud and the list share one pick, and the pick opens the editor beside
 * them, so a reader reads one concept and changes it without leaving the
 * memory. The page owns the REST calls, so this section owns the list and
 * the drafts alone.
 */
import type { QRL } from '@builder.io/qwik';

import type { MemoryNodeDto, MemoryWriteRequestDto } from '~/types/dto';

import { $, component$, useSignal } from '@builder.io/qwik';

import { MemoryConceptForm } from '~/components/sections/settings/memory-concept-form';
import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { FieldLabel } from '~/components/ui/field-label';
import { SidePanel } from '~/components/ui/side-panel';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';
import { MemoryGraph } from '~/components/viz/memory-graph';

/** What one memory search answered. */
export interface MemorySearchOutcome {
  /** Whether the search failed. */
  failed: boolean;
  /** Why it failed, or null. */
  message?: string;
  /** The nodes the search matched. */
  nodes: MemoryNodeDto[];
}

/** What a write, a retire, or a read of one concept answered. */
export interface MemoryConceptOutcome {
  /** Whether the change failed. */
  failed: boolean;
  /** Why it failed, or null. */
  message?: string;
  /** The stored concept, or null when the change left none. */
  node: MemoryNodeDto | null;
}

/** What a delete of one concept answered. */
export interface MemoryDeleteOutcome {
  /** Whether the delete failed. */
  failed: boolean;
  /** Why it failed, or null. */
  message?: string;
}

/** The props of `MemorySearchSection`. */
export interface MemorySearchSectionProps {
  /**
   * The concepts the memory holds, newest first, as the loader read them.
   * They open the list and the cloud, so a reader sees the memory before
   * searching it.
   */
  concepts: MemoryNodeDto[];
  /** Search the memory. The page defines the handle. */
  onSearch$: QRL<(text: string) => Promise<MemorySearchOutcome>>;
  /** Save one concept. The page defines the handle. */
  onSaveConcept$: QRL<
    (input: MemoryWriteRequestDto) => Promise<MemoryConceptOutcome>
  >;
  /** Retire one fact and return its concept. The page defines the handle. */
  onRetireFact$: QRL<(factId: string) => Promise<MemoryConceptOutcome>>;
  /** Move one concept into another. The page defines the handle. */
  onMergeConcept$: QRL<
    (id: string, into: string) => Promise<MemoryConceptOutcome>
  >;
  /** Delete one concept. The page defines the handle. */
  onDeleteConcept$: QRL<(id: string) => Promise<MemoryDeleteOutcome>>;
}

export const MemorySearchSection = component$<MemorySearchSectionProps>(
  (props) => {
    const searchText = useSignal('');
    const searching = useSignal(false);
    const searchError = useSignal<string | null>(null);
    const results = useSignal<MemoryNodeDto[]>(props.concepts);
    const selected = useSignal<MemoryNodeDto | null>(null);
    const conceptPending = useSignal(false);
    const conceptError = useSignal<string | null>(null);

    const handleSearch = $(async () => {
      searching.value = true;
      const outcome = await props.onSearch$(searchText.value);
      searching.value = false;
      if (outcome.failed) {
        searchError.value = outcome.message ?? 'The search failed.';
        results.value = [];
        return;
      }
      searchError.value = null;
      results.value = outcome.nodes;
    });

    const handlePick = $((node: MemoryNodeDto) => {
      selected.value = node;
      conceptError.value = null;
    });

    const handleClose = $(() => {
      selected.value = null;
      conceptError.value = null;
    });

    // A save and a retire answer with the stored concept, so the panel and
    // the cloud read the new state without a second call.
    const handleSaveConcept = $(async (input: MemoryWriteRequestDto) => {
      conceptPending.value = true;
      conceptError.value = null;
      const outcome = await props.onSaveConcept$(input);
      conceptPending.value = false;
      if (outcome.failed) {
        conceptError.value = outcome.message ?? 'The save failed.';
        return;
      }
      const node = outcome.node;
      if (!node) {
        return;
      }
      selected.value = node;
      results.value = results.value.some((entry) => entry.id === node.id)
        ? results.value.map((entry) => (entry.id === node.id ? node : entry))
        : [node, ...results.value];
    });

    const handleRetireFact = $(async (factId: string) => {
      conceptPending.value = true;
      conceptError.value = null;
      const outcome = await props.onRetireFact$(factId);
      conceptPending.value = false;
      if (outcome.failed) {
        conceptError.value = outcome.message ?? 'The change failed.';
        return;
      }
      const node = outcome.node;
      if (!node) {
        return;
      }
      selected.value = node;
      results.value = results.value.map((entry) =>
        entry.id === node.id ? node : entry,
      );
    });

    // A merge answers with the concept that keeps the facts, so the panel
    // reads the two concepts as one and the one that moved leaves the
    // list.
    const handleMergeConcept = $(async (into: string) => {
      const current = selected.value;
      if (!current) {
        return;
      }
      conceptPending.value = true;
      conceptError.value = null;
      const outcome = await props.onMergeConcept$(current.id, into);
      conceptPending.value = false;
      if (outcome.failed) {
        conceptError.value = outcome.message ?? 'The merge failed.';
        return;
      }
      const node = outcome.node;
      if (!node) {
        return;
      }
      selected.value = node;
      const others = results.value.filter((entry) => entry.id !== current.id);
      results.value = others.some((entry) => entry.id === node.id)
        ? others.map((entry) => (entry.id === node.id ? node : entry))
        : [node, ...others];
    });

    const handleDelete = $(async () => {
      const current = selected.value;
      if (!current) {
        return;
      }
      conceptPending.value = true;
      conceptError.value = null;
      const outcome = await props.onDeleteConcept$(current.id);
      conceptPending.value = false;
      if (outcome.failed) {
        conceptError.value = outcome.message ?? 'The delete failed.';
        return;
      }
      selected.value = null;
      results.value = results.value.filter((node) => node.id !== current.id);
    });

    return (
      <Box class="flex min-h-0 flex-1 flex-col gap-6 lg:flex-row lg:overflow-hidden">
        <Stack
          gap="lg"
          class="min-h-0 w-full min-w-0 flex-1 lg:overflow-y-auto"
        >
          <Card label="The memory">
            <Stack gap="md">
              <Stack
                direction="row"
                gap="sm"
                align="center"
                justify="between"
                wrap
              >
                <Text size="hud" tone="faint">
                  One point for every concept, and one line for every fact that
                  names another concept. Pick a point to read it.
                </Text>
                <Badge tone="neutral" label={`${results.value.length} shown`} />
              </Stack>
              <MemoryGraph
                nodes={results.value}
                selected={selected.value?.id ?? null}
                ariaLabel="The memory of the daemon"
                // The cloud names a concept by its identifier, and the
                // list holds the concept itself.
                onPick$={$((id: string) => {
                  const node = results.value.find((entry) => entry.id === id);
                  if (node) {
                    handlePick(node);
                  }
                })}
              />
            </Stack>
          </Card>

          <Card
            label="Concepts"
            class="flex min-h-0 flex-col"
            bodyClass="flex min-h-0 flex-col"
          >
            <Stack gap="md">
              <Stack direction="row" gap="sm" align="end">
                <Stack gap="xs" class="min-w-0 flex-1">
                  <FieldLabel
                    label="Search"
                    hint="The key, the name, the body, a relation, or a value. An empty search returns the most recently changed concepts."
                  />
                  <TextInput
                    kind="input"
                    surface="field"
                    name="memory-search"
                    ariaLabel="Search the memory"
                    placeholder="Where does the user live?"
                    value={searchText.value}
                    disabled={searching.value}
                    onInput$={$((event: Event) => {
                      searchText.value = (
                        event.target as HTMLInputElement
                      ).value;
                    })}
                    onKeyDown$={$((event: KeyboardEvent) => {
                      if (event.key === 'Enter') {
                        void handleSearch();
                      }
                    })}
                  />
                </Stack>
                <Button
                  variant="outline"
                  size="md"
                  class="shrink-0"
                  disabled={searching.value}
                  onClick$={handleSearch}
                >
                  {searching.value ? 'Searching...' : 'Search'}
                </Button>
              </Stack>

              {searchError.value ? (
                <Alert
                  tone="error"
                  title="Search failed"
                  message={searchError.value}
                />
              ) : null}

              {results.value.length === 0 ? (
                <Text size="hud" tone="faint" block>
                  No concept matches that. Search by a word the daemon would
                  have remembered, or clear the search to read the newest
                  concepts.
                </Text>
              ) : (
                <Stack gap="sm" class="min-h-0 flex-1 overflow-y-auto">
                  {results.value.map((node) => (
                    <Card
                      key={node.id}
                      tone="sunken"
                      frame={false}
                      pickable
                      picked={selected.value?.id === node.id}
                      pickLabel={`Edit ${node.title}`}
                      bodyClass="p-3"
                      onPick$={$(() => handlePick(node))}
                    >
                      <Stack gap="xs">
                        <Stack
                          direction="row"
                          gap="sm"
                          align="center"
                          justify="between"
                        >
                          <Text size="body" weight="medium">
                            {node.title}
                          </Text>
                          <Text size="micro" tone="faint">
                            {node.key}
                          </Text>
                        </Stack>
                        {node.body ? (
                          <Text size="body" tone="muted">
                            {node.body}
                          </Text>
                        ) : null}
                        <Stack direction="row" gap="sm" wrap>
                          {node.facts
                            .filter((fact) => fact.current)
                            .map((fact) => (
                              <Stack
                                key={fact.id}
                                direction="row"
                                gap="xs"
                                align="center"
                              >
                                <Badge tone="neutral" label={fact.relation} />
                                <Text size="hud">{fact.value}</Text>
                              </Stack>
                            ))}
                        </Stack>
                      </Stack>
                    </Card>
                  ))}
                </Stack>
              )}
            </Stack>
          </Card>
        </Stack>

        <SidePanel
          ariaLabel="The editor of one concept"
          title={selected.value?.title ?? 'Concept'}
          note={selected.value?.key ?? null}
          open={selected.value !== null}
          onClose$={handleClose}
        >
          {selected.value ? (
            <MemoryConceptForm
              key={selected.value.id}
              concept={selected.value}
              others={results.value.filter(
                (entry) => entry.id !== selected.value?.id,
              )}
              pending={conceptPending.value}
              error={conceptError.value}
              onSave$={handleSaveConcept}
              onRetireFact$={handleRetireFact}
              onMerge$={handleMergeConcept}
              onDelete$={handleDelete}
            />
          ) : null}
        </SidePanel>
      </Box>
    );
  },
);
