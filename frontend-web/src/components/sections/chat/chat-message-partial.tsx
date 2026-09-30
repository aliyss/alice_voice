/**
 * `ChatMessagePartial` renders one turn of the transcript.
 *
 * The turn of the user sits at the right edge on the accent veil, so the
 * voice of the user carries the accent. The turn of the daemon sits at the
 * left edge on plain glass, so the two are never confused.
 *
 * The header line keeps the name and the time, and the mark of the memory
 * stands beside the time of the message the memory learned from. The mark
 * is a check, and the facts the turn taught wait in its tooltip, so a
 * reader sees that the daemon kept something and what it kept without
 * leaving the transcript. Everything else the daemon
 * knows about the turn waits behind the `Metadata` toggle of the turn, in
 * one row per field: the label in the HUD hand, the value in the text hand,
 * and a hairline between the rows so the table is easy to read down.
 *
 * A turn the resolver read and chose no intent for still reports the row of
 * the read, so the table names the engine and, on the tooltip of that row,
 * the model that read the message.
 *
 * The route of a turn through the layered router takes a row of its own:
 * one line per stage with the reader it ran and the time it took, and the
 * sentence of the stage under it. A reader therefore sees which stage
 * answered a message and which stage refused it, instead of the stage
 * that happened to answer alone.
 *
 * The route of the turn reads in two ways, and the switch of the row picks
 * between them: one line per stage, or the field by field report the
 * settings page shows for a sentence it tried. The switch belongs to the
 * transcript, so every turn of it reads a route the same way.
 */
import type { QRL } from '@builder.io/qwik';

import type { ChatRow } from '~/utils/chat';

import { $, component$ } from '@builder.io/qwik';

import { Badge } from '~/components/ui/badge';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { Disclosure } from '~/components/ui/disclosure';
import { InfoHint } from '~/components/ui/info-hint';
import { MetadataRow } from '~/components/ui/metadata-row';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { Tooltip } from '~/components/ui/tooltip';
import { RouteLog } from '~/components/viz/route-log';

import { joinClassNames } from '~/utils/class-names';
import {
  ROUTE_LOG_MODES,
  ROUTE_LOG_MODE_HINT,
  debugRoute,
} from '~/utils/preview-log';

/** The props of `ChatMessagePartial`. */
export interface ChatMessagePartialProps {
  /** The turn to render. */
  row: ChatRow;
  /** True when the route of the turn reads field by field. */
  debug: boolean;
  /** Report the reading the user chose for the route. */
  onDebug$: QRL<(next: boolean) => void>;
}

export const ChatMessagePartial = component$<ChatMessagePartialProps>(
  (props) => {
    const isUser = props.row.role === 'user';
    // The name of the intent is the readable one. A message whose intent is
    // gone keeps its identifier, so the row still reports what the daemon did.
    const intent = props.row.intentName ?? props.row.intentId;
    // The metadata of the turn belongs to the answer of the daemon.
    const meta = isUser ? null : props.row.meta;
    const entities = meta ? meta.entities : [];
    const resolver = meta?.resolver ?? '';
    const resolverModels = meta?.resolverModels ?? null;
    const stage = meta?.stage ?? null;
    const candidates = meta?.candidates ?? [];
    const route = meta?.route ?? [];
    const reason = meta?.reason ?? null;
    const resolve = meta?.resolve ?? null;
    const hasDetail = Boolean(
      meta &&
        (intent ||
          resolver ||
          stage ||
          candidates.length > 0 ||
          entities.length > 0 ||
          meta.memorySeed ||
          route.length > 0 ||
          reason ||
          resolve ||
          meta.command ||
          meta.exitCode),
    );

    return (
      <Stack
        direction="row"
        gap="none"
        justify={isUser ? 'end' : 'start'}
        role="article"
        ariaLabel={isUser ? 'Your message' : 'Alice message'}
        class="w-full"
      >
        <Stack
          gap="xs"
          class={joinClassNames(
            // The field of the header needs room, so a short turn keeps a
            // width that carries its time.
            'max-w-[86%] min-w-[11rem] rounded-ds-lg border px-3 py-2 backdrop-blur-md',
            isUser
              ? 'rounded-br-ds-sm border-ds-accent-edge bg-ds-accent-veil'
              : 'rounded-bl-ds-sm border-ds-line bg-ds-surface',
          )}
        >
          <Stack
            direction="row"
            gap="sm"
            align="baseline"
            justify="between"
            class="w-full"
          >
            <Text size="micro" tone={isUser ? 'accent' : 'muted'}>
              {isUser ? 'You' : 'Alice'}
            </Text>
            <Stack direction="row" gap="xs" align="center" class="shrink-0">
              {/* The memory reads a turn after the daemon answered it, so
                  the mark appears on the message the memory learned from
                  once it has read it. */}
              {props.row.memory ? (
                <Tooltip
                  text={props.row.memory.facts.join('\n')}
                  ariaLabel="Saved to the memory"
                >
                  <Text size="hud" tone="ok">
                    ✓
                  </Text>
                </Tooltip>
              ) : null}
              <Tooltip text={meta?.duration ?? null} class="shrink-0">
                <Text size="micro" tone="faint">
                  {props.row.clock}
                </Text>
              </Tooltip>
            </Stack>
          </Stack>

          <Text size="body" block class="whitespace-pre-wrap">
            {props.row.text}
          </Text>

          {meta && hasDetail ? (
            <Disclosure label="Metadata" class="mt-0.5">
              <Stack gap="none" class="w-full divide-y divide-ds-line-faint">
                <MetadataRow label="Intent" labelWidth="w-20" class="py-1.5">
                  {intent ? (
                    <Text size="body" tone="default">
                      {intent}
                    </Text>
                  ) : (
                    <Text size="body" tone="muted">
                      no intent matched
                    </Text>
                  )}
                  {intent && props.row.confidence ? (
                    <Badge tone="neutral" label={props.row.confidence} />
                  ) : null}
                </MetadataRow>
                {resolver ? (
                  <MetadataRow
                    label="Resolver"
                    labelWidth="w-20"
                    class="py-1.5"
                  >
                    {/* The model behind the engines waits here, so the row
                        names who read the turn and what it ran. */}
                    <Tooltip text={resolverModels} side="bottom">
                      <Text size="hud" tone="muted">
                        {resolver}
                      </Text>
                    </Tooltip>
                  </MetadataRow>
                ) : null}
                {stage ? (
                  <MetadataRow label="Stage" labelWidth="w-20" class="py-1.5">
                    {/* The router reads a message in stages and stops at
                        the first stage that can answer, so the row names
                        the one that answered this turn. */}
                    <Text size="hud" tone="muted">
                      {stage}
                    </Text>
                  </MetadataRow>
                ) : null}
                {candidates.length > 0 ? (
                  <MetadataRow
                    label="Candidates"
                    labelWidth="w-20"
                    align="start"
                    class="py-1.5"
                  >
                    <Stack gap="none" class="w-full">
                      {candidates.map((candidate) => (
                        <Text key={candidate} size="hud" tone="muted" block>
                          {candidate}
                        </Text>
                      ))}
                    </Stack>
                  </MetadataRow>
                ) : null}
                {entities.length > 0 ? (
                  <MetadataRow
                    label="Entities"
                    labelWidth="w-20"
                    class="py-1.5"
                  >
                    <Stack gap="none" class="w-full">
                      {entities.map((entity) => (
                        <Text key={entity} size="hud" tone="default" block>
                          {entity}
                        </Text>
                      ))}
                    </Stack>
                  </MetadataRow>
                ) : null}
                {meta.memorySeed ? (
                  <MetadataRow
                    label="Memory"
                    labelWidth="w-20"
                    align="start"
                    class="py-1.5"
                  >
                    {/* What the daemon read from the long term memory
                        before it answered, so a reader sees the facts the
                        answer leaned on rather than only its words. */}
                    <Text
                      size="hud"
                      tone="muted"
                      block
                      class="whitespace-pre-line"
                    >
                      {meta.memorySeed}
                    </Text>
                  </MetadataRow>
                ) : null}
                {route.length > 0 ? (
                  <MetadataRow
                    label="Route"
                    labelWidth="w-20"
                    align="start"
                    class="py-1.5"
                  >
                    {/* Every stage the turn passed, in the order it ran
                        them: which stage answered a message and which one
                        refused it is what the route is for. */}
                    <Stack gap="xs" class="w-full">
                      <Stack direction="row" gap="xs" align="center">
                        <ContentSwitcher
                          ariaLabel="How to read the route"
                          value={props.debug ? 'debug' : 'text'}
                          options={ROUTE_LOG_MODES}
                          size="sm"
                          onPick$={$((next: string) =>
                            props.onDebug$(next === 'debug'),
                          )}
                        />
                        <InfoHint
                          label="The two readings of a route"
                          text={ROUTE_LOG_MODE_HINT}
                          side="top"
                          align="left"
                        />
                      </Stack>

                      {props.debug ? (
                        <RouteLog blocks={debugRoute(props.row.source)} />
                      ) : (
                        route.map((step) => (
                          <Stack key={step.stage} gap="none" class="w-full">
                            <Text size="hud" tone="default" block>
                              {`${step.stage}  ${step.reader}  ${step.outcome}  ${step.duration}`}
                            </Text>
                            {step.detail ? (
                              <Text size="hud" tone="faint" block>
                                {step.detail}
                              </Text>
                            ) : null}
                          </Stack>
                        ))
                      )}
                    </Stack>
                  </MetadataRow>
                ) : null}
                {reason ? (
                  <MetadataRow
                    label="Reason"
                    labelWidth="w-20"
                    align="start"
                    class="py-1.5"
                  >
                    <Text size="hud" tone="muted" block>
                      {reason}
                    </Text>
                  </MetadataRow>
                ) : null}
                {resolve ? (
                  <MetadataRow label="Read" labelWidth="w-20" class="py-1.5">
                    {/* The read runs from the queued message to the command,
                        so a slow turn is told apart from a slow command. */}
                    <Text size="hud" tone="muted">
                      {resolve}
                    </Text>
                  </MetadataRow>
                ) : null}
                {meta.command ? (
                  <MetadataRow
                    label="Command"
                    labelWidth="w-20"
                    align="start"
                    class="py-1.5"
                  >
                    <Text size="hud" tone="muted" block class="break-all">
                      {meta.command}
                    </Text>
                  </MetadataRow>
                ) : null}
                {meta.exitCode ? (
                  <MetadataRow label="Exit" labelWidth="w-20" class="py-1.5">
                    <Text size="hud" tone={meta.exitFailed ? 'error' : 'ok'}>
                      {meta.exitCode}
                    </Text>
                  </MetadataRow>
                ) : null}
              </Stack>
            </Disclosure>
          ) : null}
        </Stack>
      </Stack>
    );
  },
);
