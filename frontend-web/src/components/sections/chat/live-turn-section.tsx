/**
 * `LiveTurnSection` shows the turn the daemon handles right now.
 *
 * The daemon streams every stage of a turn over the socket: the answer of
 * the resolver, the intent it chose and the engine that chose it, the
 * values of the entities of that intent, the command it started, and the
 * output of that command. The section shows those stages while they
 * happen, so the surface reports the work and not only the result.
 *
 * The page decides when the section is on screen. It shows the section
 * while a turn is pending, right above the composer, so the work of the
 * daemon reads as the answer that is on its way.
 *
 * The model behind each engine of the turn waits in a tooltip on the step
 * that names the engine, so the card names who read the turn and what it
 * ran without a row of its own.
 */
import type { LiveTurnState } from '~/utils/status';

import { component$ } from '@builder.io/qwik';

import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { MetadataRow } from '~/components/ui/metadata-row';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { Tooltip } from '~/components/ui/tooltip';

import { formatConfidence, formatEngine, formatEntity } from '~/utils/chat';

/** The largest part of the model answer the section shows. */
const VISIBLE_ANSWER_CHARS = 400;

/** The largest number of output lines the section shows. */
const VISIBLE_OUTPUT_LINES = 6;

/** The props of `LiveTurnSection`. */
export interface LiveTurnSectionProps {
  /** The stages of the turn that runs right now. */
  live: LiveTurnState;
}

export const LiveTurnSection = component$<LiveTurnSectionProps>((props) => {
  const answer = props.live.thinking.trim().slice(-VISIBLE_ANSWER_CHARS);
  const confidence = formatConfidence(props.live.confidence);
  const intentEngine = formatEngine(props.live.intentEngine);
  const valueEngine = formatEngine(props.live.valueEngine);
  const output = props.live.output.slice(-VISIBLE_OUTPUT_LINES);
  const isRunning = Boolean(props.live.command) && output.length === 0;

  return (
    <Card label="Live" class="w-full shrink-0" bodyClass="p-0">
      <Stack gap="none" class="divide-y divide-ds-line">
        <MetadataRow label="Thinking" class="px-4 py-2.5">
          {answer.length === 0 ? (
            <Spinner size="sm" label="Waiting for the resolver" />
          ) : null}
          {answer.length === 0 ? (
            <Text size="hud" tone="faint">
              Reading the intent...
            </Text>
          ) : (
            <Box class="flex max-h-20 w-full items-end overflow-hidden">
              <Text size="hud" tone="muted" block class="break-all">
                {answer}
              </Text>
            </Box>
          )}
        </MetadataRow>

        <MetadataRow label="Intent" class="px-4 py-2.5">
          {props.live.intent ? (
            <Text size="body" tone="default">
              {props.live.intent}
            </Text>
          ) : (
            <Text size="hud" tone="faint">
              Waiting
            </Text>
          )}
          {props.live.intent && confidence ? (
            <Badge tone="neutral" label={confidence} />
          ) : null}
          {props.live.intent && intentEngine ? (
            <Tooltip text={props.live.intentModel}>
              <Text size="hud" tone="muted">
                {`chosen by ${intentEngine}`}
              </Text>
            </Tooltip>
          ) : null}
        </MetadataRow>

        {props.live.entities.length > 0 ? (
          <MetadataRow label="Entities" class="px-4 py-2.5">
            {props.live.entities.map((entity) => (
              <Text key={entity.name} size="hud" tone="default">
                {formatEntity(entity)}
              </Text>
            ))}
            {valueEngine ? (
              <Tooltip text={props.live.valueModel}>
                <Text size="hud" tone="muted">
                  {`read by ${valueEngine}`}
                </Text>
              </Tooltip>
            ) : null}
          </MetadataRow>
        ) : null}

        {props.live.command ? (
          <MetadataRow label="Command" align="start" class="px-4 py-2.5">
            <Text size="body" mono tone="muted" block class="break-all">
              {props.live.command}
            </Text>
          </MetadataRow>
        ) : null}

        {output.length > 0 || isRunning ? (
          <MetadataRow label="Output" align="start" class="px-4 py-2.5">
            {isRunning ? (
              <Spinner size="sm" label="Running the command" />
            ) : null}
            {output.map((line, index) => (
              <Text
                key={`${index}-${line}`}
                size="hud"
                tone="faint"
                block
                class="break-all"
              >
                {line}
              </Text>
            ))}
          </MetadataRow>
        ) : null}
      </Stack>
    </Card>
  );
});
