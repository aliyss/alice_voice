/**
 * `LibrarianSection` shows what the daemon remembers about the user.
 *
 * The section is the explorer of the long term memory: the concepts the
 * daemon holds, the facts of each one, and the panel that changes them. A
 * reader searches the memory, picks a concept in the cloud or in the list,
 * and edits what the daemon learned.
 *
 * The values of the memory (the reader, the address, the model) are set
 * from the flow of the resolver, because the memory belongs to the branch
 * that no intent matches. This section reads them and never writes them:
 * the state of the store and the linter stay here, so a reader sees what
 * the memory holds next to what it holds it in.
 */
import type { QRL } from '@builder.io/qwik';

import type {
  MemoryConceptOutcome,
  MemoryDeleteOutcome,
  MemorySearchOutcome,
} from '~/components/sections/settings/memory-search-section';

import type {
  LibrarianStatusDto,
  MemoryFactDto,
  MemoryNodeDto,
  MemoryWriteRequestDto,
} from '~/types/dto';

import { $, component$, useSignal } from '@builder.io/qwik';

import { MemorySearchSection } from '~/components/sections/settings/memory-search-section';
import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { MetadataRow } from '~/components/ui/metadata-row';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** What one linter run answered. */
export interface LintOutcome {
  /** Whether the run failed. */
  failed: boolean;
  /** Why it failed, or null. */
  message?: string;
  /** Nodes two keys name, so the memory holds one thing twice. */
  duplicates: MemoryNodeDto[];
  /** Nodes that no relation reaches. */
  orphans: MemoryNodeDto[];
  /** Relations a newer relation replaced. */
  closed: MemoryFactDto[];
}

/** The props of `LibrarianSection`. */
export interface LibrarianSectionProps {
  /** The concepts the memory holds, newest first, or empty. */
  concepts: MemoryNodeDto[];
  /** The state of the librarian, or null while it loads. */
  status: LibrarianStatusDto | null;
  /** True while a request is in flight. */
  pending: boolean;
  /** True when the database does not answer. */
  blocked: boolean;
  /** Why a write cannot take effect, or null. */
  blockedReason: string | null;
  /** Search the memory. The page defines the handle. */
  onSearch$: QRL<(text: string) => Promise<MemorySearchOutcome>>;
  /** Save one concept of the memory. The page defines the handle. */
  onSaveConcept$: QRL<
    (input: MemoryWriteRequestDto) => Promise<MemoryConceptOutcome>
  >;
  /** Retire one fact of a concept. The page defines the handle. */
  onRetireFact$: QRL<(factId: string) => Promise<MemoryConceptOutcome>>;
  /** Move one concept into another. The page defines the handle. */
  onMergeConcept$: QRL<
    (id: string, into: string) => Promise<MemoryConceptOutcome>
  >;
  /** Delete one concept and every fact of it. The page defines the handle. */
  onDeleteConcept$: QRL<(id: string) => Promise<MemoryDeleteOutcome>>;
  /** Read the linter report. The page defines the handle. */
  onLint$: QRL<() => Promise<LintOutcome>>;
  /** Read the state of the librarian again. */
  onRefresh$: QRL<() => void>;
}

export const LibrarianSection = component$<LibrarianSectionProps>((props) => {
  const linting = useSignal(false);
  const lintError = useSignal<string | null>(null);
  const lint = useSignal<LintOutcome | null>(null);

  const handleLint = $(async () => {
    linting.value = true;
    const outcome = await props.onLint$();
    linting.value = false;
    if (outcome.failed) {
      lintError.value = outcome.message ?? 'The linter failed.';
      return;
    }
    lintError.value = null;
    lint.value = outcome;
  });

  return (
    // The memory takes the room under the head and the state of the store
    // takes a column beside it.
    <Box class="flex min-h-0 flex-1 flex-col gap-6 overflow-y-auto lg:flex-row lg:overflow-hidden">
      <Stack
        gap="lg"
        class="w-full shrink-0 lg:min-h-0 lg:w-[clamp(20rem,26vw,30rem)] lg:overflow-y-auto"
      >
        {props.blocked && props.blockedReason ? (
          <Alert
            tone="warn"
            title="Cannot be changed"
            message={props.blockedReason}
          />
        ) : null}

        <Card label="The store">
          <Stack gap="md">
            <Stack
              direction="row"
              gap="sm"
              align="center"
              justify="between"
              wrap
            >
              <Text size="hud" tone="faint">
                The memory is a store beside the conversation store. A reader
                that is down stops the learning, not the memory: the store still
                answers.
              </Text>
              <Button
                variant="outline"
                size="md"
                disabled={props.pending}
                onClick$={props.onRefresh$}
              >
                Read the state
              </Button>
            </Stack>

            {props.status ? (
              <Stack gap="sm">
                <MetadataRow label="Reader" labelWidth="w-28">
                  <Badge
                    tone={props.status.reachable ? 'ok' : 'warn'}
                    label={props.status.reachable ? 'Up' : 'Down'}
                  />
                  <Text size="micro" tone="faint">
                    {props.status.detail ?? props.status.baseUrl}
                  </Text>
                </MetadataRow>
                <MetadataRow label="Keeping" labelWidth="w-28">
                  <Badge
                    tone={props.status.enabled ? 'ok' : 'neutral'}
                    label={props.status.enabled ? 'On' : 'Off'}
                  />
                </MetadataRow>
                <MetadataRow label="Model" labelWidth="w-28">
                  <Text size="hud">{props.status.model}</Text>
                </MetadataRow>
                <MetadataRow label="Concepts" labelWidth="w-28">
                  <Text size="hud">{`${props.status.nodes}`}</Text>
                </MetadataRow>
                <MetadataRow label="Facts" labelWidth="w-28">
                  <Text size="hud">{`${props.status.currentEdges}`}</Text>
                </MetadataRow>
                <MetadataRow label="Replaced" labelWidth="w-28">
                  <Text size="hud">
                    {`${props.status.edges - props.status.currentEdges}`}
                  </Text>
                </MetadataRow>
                <MetadataRow label="Confirmed" labelWidth="w-28">
                  <Text size="hud">{`${props.status.confirmedEdges}`}</Text>
                </MetadataRow>
                <MetadataRow label="Waiting" labelWidth="w-28">
                  <Text size="hud">{`${props.status.pendingEpisodes}`}</Text>
                </MetadataRow>
                <MetadataRow label="Read" labelWidth="w-28">
                  <Text size="hud">{`${props.status.ingestedEpisodes}`}</Text>
                </MetadataRow>
              </Stack>
            ) : (
              <Text size="hud" tone="faint" block>
                The daemon did not answer with the state of the memory.
              </Text>
            )}
          </Stack>
        </Card>

        <Card label="Linter">
          <Stack gap="md">
            <Stack
              direction="row"
              gap="sm"
              align="center"
              justify="between"
              wrap
            >
              <Text size="hud" tone="faint">
                The linter names the concepts two keys name, the concepts
                nothing links to, and the facts a newer fact replaced.
              </Text>
              <Button
                variant="outline"
                size="md"
                disabled={linting.value}
                onClick$={handleLint}
              >
                {linting.value ? 'Reading...' : 'Run the linter'}
              </Button>
            </Stack>

            {lintError.value ? (
              <Alert
                tone="error"
                title="Linter failed"
                message={lintError.value}
              />
            ) : null}

            {lint.value ? (
              <Stack gap="sm">
                <MetadataRow label="Twice" labelWidth="w-28">
                  <Text size="hud">{`${lint.value.duplicates.length}`}</Text>
                </MetadataRow>
                {lint.value.duplicates.map((node) => (
                  <Stack key={node.id} direction="row" gap="sm" align="center">
                    <Badge tone="warn" label={node.key} />
                    <Text size="hud" tone="faint">
                      {`${node.title} \u00b7 two keys name this concept`}
                    </Text>
                  </Stack>
                ))}
                <MetadataRow label="Orphans" labelWidth="w-28">
                  <Text size="hud">{`${lint.value.orphans.length}`}</Text>
                </MetadataRow>
                <MetadataRow label="Replaced" labelWidth="w-28">
                  <Text size="hud">{`${lint.value.closed.length}`}</Text>
                </MetadataRow>
                {lint.value.closed.map((fact) => (
                  <Stack key={fact.id} direction="row" gap="sm" align="center">
                    <Badge tone="neutral" label={fact.relation} />
                    <Text size="hud" tone="faint">
                      {`${fact.value} (until ${fact.invalidAt ?? 'now'})`}
                    </Text>
                  </Stack>
                ))}
              </Stack>
            ) : null}
          </Stack>
        </Card>
      </Stack>

      <MemorySearchSection
        concepts={props.concepts}
        onSearch$={props.onSearch$}
        onSaveConcept$={props.onSaveConcept$}
        onRetireFact$={props.onRetireFact$}
        onMergeConcept$={props.onMergeConcept$}
        onDeleteConcept$={props.onDeleteConcept$}
      />
    </Box>
  );
});
