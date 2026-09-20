/**
 * `ModelRow` shows one model of a list.
 *
 * A list of models answers three questions at once: which one the settings
 * read, whether its file is on disk, and what it costs. The row is the
 * pick itself, so a reader chooses a model by pressing it and the chosen
 * row carries the accent outline rather than a button of its own. The name
 * takes the first line with the actions beside it, and the state and the
 * size take the second line as small text, so the row keeps one hierarchy
 * however narrow it becomes.
 *
 * The sentence about a model waits in the hint of its name, so a list of
 * six models reads as a list rather than as six paragraphs.
 */
import type { QRL } from '@builder.io/qwik';

import { component$ } from '@builder.io/qwik';

import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { InfoHint } from '~/components/ui/info-hint';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { formatSize } from '~/utils/size';

/** The props of `ModelRow`. */
export interface ModelRowProps {
  /** The name of the model. */
  name: string;
  /** One sentence about what the model reads. */
  note: string;
  /** The size of the file. */
  sizeBytes: number;
  /** True when the file is on disk. */
  installed: boolean;
  /** True when the settings read this model. */
  chosen: boolean;
  /** True while a download or a remove request is in flight. */
  busy: boolean;
  /** True while a save runs or the value cannot be stored. */
  disabled: boolean;
  /** True while the file of this row is downloading. */
  downloading?: boolean;
  /** Report the model the user chose. */
  onChoose$: QRL<() => void>;
  /** Download the file of this model. */
  onDownload$: QRL<() => void>;
  /** Remove the file of this model. */
  onRemove$: QRL<() => void>;
}

export const ModelRow = component$<ModelRowProps>((props) => {
  return (
    <Card
      tone="sunken"
      frame={false}
      pickable={!props.disabled}
      picked={props.chosen}
      pickLabel={`Use ${props.name}`}
      ariaLabel={props.name}
      bodyClass="flex flex-col gap-1 p-3"
      onPick$={props.onChoose$}
    >
      <Stack direction="row" gap="sm" align="center" justify="between" wrap>
        <Stack direction="row" gap="xs" align="center" class="min-w-0">
          <Text size="body" weight={props.chosen ? 'medium' : 'regular'}>
            {props.name}
          </Text>
          {/* The row is one pick, so the hint takes a place of its own:
              the press of the row would otherwise swallow its hover. */}
          <InfoHint
            label={`About ${props.name}`}
            text={`${props.note} It takes ${formatSize(props.sizeBytes)} on disk.`}
            class="relative z-10"
          />
        </Stack>

        <Stack
          direction="row"
          gap="xs"
          align="center"
          class="relative z-10 shrink-0"
        >
          {props.installed ? (
            <Button
              variant="quiet"
              size="sm"
              disabled={props.busy}
              onClick$={props.onRemove$}
            >
              Remove
            </Button>
          ) : (
            <Button
              variant="outline"
              size="sm"
              disabled={props.busy}
              onClick$={props.onDownload$}
            >
              Download
            </Button>
          )}
        </Stack>
      </Stack>

      {/* The state and the cost share the second line: the state is small
          and colored, so it reads without a badge that pushes the name
          aside. */}
      <Stack direction="row" gap="xs" align="center" wrap>
        <Text size="hud" tone={props.installed ? 'ok' : 'faint'}>
          {props.installed ? 'On disk' : 'Not on disk'}
        </Text>
        <Text size="hud" tone="faint">
          ·
        </Text>
        <Text size="hud" tone="faint">
          {formatSize(props.sizeBytes)}
        </Text>
        {props.downloading ? (
          <Stack direction="row" gap="xs" align="center">
            <Spinner size="sm" label={`${props.name} is downloading`} />
            <Text size="hud" tone="faint">
              Downloading...
            </Text>
          </Stack>
        ) : null}
      </Stack>
    </Card>
  );
});
