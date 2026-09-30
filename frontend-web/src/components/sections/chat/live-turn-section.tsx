/**
 * `LiveTurnSection` shows the turn the daemon handles right now.
 *
 * The daemon streams every stage of a turn over the socket: the step the
 * resolver is on, the answer of the model, the intent it chose and the
 * engine that chose it, the values of the entities of that intent, the
 * command it started, and the output of that command. The section shows
 * those stages while they happen, so the surface reports the work and not
 * only the result.
 *
 * The timing matters most where the turn is slow. A stage that reads a
 * model takes seconds, and the names of the stages that ran are what turn
 * a long wait into a measurement. The card therefore says that the daemon
 * works and nothing more, and keeps the name and the time of every step
 * behind a hover: the row of a running turn stays quiet, and a reader who
 * asks where the seconds went finds the answer.
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

import { component$, useSignal, useVisibleTask$ } from '@builder.io/qwik';

import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { MetadataRow } from '~/components/ui/metadata-row';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { Tooltip } from '~/components/ui/tooltip';

import {
  formatConfidence,
  formatDuration,
  formatEngine,
  formatEntity,
  formatLiveStep,
} from '~/utils/chat';
import { stageDurations } from '~/utils/status';

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
  // The read of the turn. A turn that already reached its command is past
  // every step of the read, so the row of the read gives way to the command
  // and its output, which carry the wait from there.
  const steps = props.live.stages;
  const reading = !props.live.command;
  const step = steps.at(-1) ?? null;
  // The words of the wait on screen. One stable phase rather than the name
  // of every step: a name that changes under the pointer is a flicker, and
  // the step the daemon is on is what the hover is for.
  const phase = !step
    ? 'Reading the intent…'
    : step.stage === 'answer'
      ? 'Answering…'
      : step.stage === 'script'
        ? 'Writing a script…'
        : 'Reading the intent…';
  // The time of every step, as the hover reads it. The last step of a turn
  // that still runs is measured down to now, and the clock below ticks
  // while the turn runs, so the list keeps moving with it.
  const durations = stageDurations(steps, Date.now());
  const breakdown =
    steps.length === 0
      ? null
      : steps
          .map(
            (entry, index) =>
              `${formatLiveStep(entry.stage)}  ${formatDuration(durations[index])}`,
          )
          .join('\n');
  const elapsed = useSignal(0);

  // The clock of the live turn. The daemon reports the start of the turn
  // with the queued event, so the section counts the wait from it and
  // ticks while the turn runs. A reader therefore tells a slow read from a
  // slow command while it still matters.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ track, cleanup }) => {
    const startedAt = track(() => props.live.startedAt);
    const started = startedAt ? Date.parse(startedAt) : Number.NaN;
    const tick = () => {
      elapsed.value = Number.isNaN(started) ? 0 : Date.now() - started;
    };
    tick();
    const timer = setInterval(tick, 100);
    cleanup(() => clearInterval(timer));
  });

  return (
    <Card label="Live" class="w-full shrink-0" bodyClass="p-0">
      <Stack gap="none" class="divide-y divide-ds-line">
        {props.live.startedAt ? (
          <MetadataRow label="Time" class="px-4 py-2.5">
            <Text size="hud" tone="default">
              {formatDuration(elapsed.value)}
            </Text>
            <Text size="hud" tone="faint">
              from the accepted message
            </Text>
          </MetadataRow>
        ) : null}

        {/* One spinner reads the wait of the read, in the place the single
            label of a wait stood before. The name and the time of every
            step rest behind a hover on it, so the row says that the daemon
            works and a reader who asks where the seconds went finds the
            answer. */}
        {reading ? (
          <MetadataRow label="Step" class="px-4 py-2.5">
            <Tooltip
              text={breakdown}
              ariaLabel="The time of every step of the turn"
              side="top"
              align="left"
            >
              {/* The ring and the words of the wait share one row, so the
                  ring sits on the line of the text and keeps a gap from
                  it. The row of the card cannot space them: the note wraps
                  the two of them as one child. */}
              <Stack direction="row" gap="sm" align="center">
                <Spinner size="sm" label={phase} />
                <Text size="hud" tone={step ? 'default' : 'faint'}>
                  {phase}
                </Text>
              </Stack>
            </Tooltip>
          </MetadataRow>
        ) : null}

        {answer.length > 0 ? (
          <MetadataRow label="Thinking" class="px-4 py-2.5">
            <Box class="flex max-h-20 w-full items-end overflow-hidden">
              <Text size="hud" tone="muted" block class="break-all">
                {answer}
              </Text>
            </Box>
          </MetadataRow>
        ) : null}

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
