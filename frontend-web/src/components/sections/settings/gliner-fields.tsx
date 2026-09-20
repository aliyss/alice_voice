/**
 * `GlinerFields` reads the built in GLiNER model of the resolver.
 *
 * GLiNER is the reader that finds the exact span of a label, so a stage
 * that reads spans owns these fields: the model on disk, the device it
 * runs on, and the smallest probability a label needs. A stage that reads
 * its values another way hides them, because a model that is not read
 * never needs a file.
 *
 * The list of models is one row per model, and the sentences about them
 * wait in the hints of those rows, so a reader sees the choice rather than
 * the explanation.
 */
import type { QRL } from '@builder.io/qwik';

import type {
  GlinerDevice,
  GlinerStoreDto,
  LabelBudgetDto,
  ResolverBackend,
} from '~/types/dto';

import { $, component$ } from '@builder.io/qwik';

import { ModelRow } from '~/components/sections/settings/model-row-partial';
import { Alert } from '~/components/ui/alert';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { InfoHint } from '~/components/ui/info-hint';
import { PanelGroup } from '~/components/ui/panel-group';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

/** The devices the settings page offers, in the order it shows them. */
const DEVICES: { value: GlinerDevice; label: string }[] = [
  { value: 'auto', label: 'Auto' },
  { value: 'cpu', label: 'CPU' },
  { value: 'cuda', label: 'CUDA' },
];

/** The props of `GlinerFields`. */
export interface GlinerFieldsProps {
  /** The state of the built in models. */
  gliner: GlinerStoreDto;
  /** The id of the chosen model. */
  model: string;
  /** The name of the chosen model, or its id when the daemon does not know it. */
  chosenName: string;
  /** The device the settings ask for. */
  device: GlinerDevice;
  /** The smallest probability a label needs. */
  threshold: string;
  /** True when the value can be stored. */
  canStore: boolean;
  /** True while a save request is in flight. */
  pending: boolean;
  /** True while a download or a remove request is in flight. */
  installing: boolean;
  /** The engine the settings chose, for the wording of the warning. */
  backend: ResolverBackend;
  /**
   * The label budget of the catalog, or null. The budget says how much of a
   * GLiNER model the intent configuration asks for, so the warning belongs
   * to the configuration of the model and shows only where the model is
   * read.
   */
  budget?: LabelBudgetDto | null;
  /** Report the model the user chose. */
  onChoose$: QRL<(id: string) => void>;
  /** Report the device the user chose. */
  onDevice$: QRL<(device: GlinerDevice) => void>;
  /** Report the smallest probability the user typed. */
  onThreshold$: QRL<(value: string) => void>;
  /** Download one model. */
  onDownload$: QRL<(id: string) => void>;
  /** Remove the files of one model. */
  onRemove$: QRL<(id: string) => void>;
}

export const GlinerFields = component$<GlinerFieldsProps>((props) => {
  const download = props.gliner.download ?? null;
  const downloading = Boolean(download && !download.done && !download.error);
  const busy = props.installing || downloading;
  const deviceNote =
    props.gliner.devices.includes(props.device) || props.device === 'auto'
      ? `This build runs on ${props.gliner.devices.join(' and ')}. Auto picks the best one.`
      : props.gliner.cudaBuild
        ? 'No CUDA device is usable right now, so the model would run on the processor.'
        : 'This build carries no CUDA support. Build the daemon with --features gliner-cuda to run on the graphics card.';

  return (
    <Stack gap="md">
      {!props.gliner.installed ? (
        <Alert
          tone="error"
          title="The selected model is not installed"
          message={
            props.backend === 'gliner'
              ? `${props.chosenName} has to be on disk before the daemon can read a message. Download it below, or switch the engine to llama.cpp.`
              : `${props.chosenName} has to be on disk before GLiNER can read the entity values. Until then the daemon reads them with the language model. Download it below for the full setup.`
          }
        />
      ) : null}

      <PanelGroup
        title="Model"
        hint="A model reads an intent on its own label and on the labels of the entities that intent needs, so a smaller model is faster and a larger one reads more."
      >
        <Stack gap="sm">
          {props.gliner.models.map((entry) => (
            <ModelRow
              key={entry.id}
              name={entry.name}
              note={entry.note}
              sizeBytes={entry.sizeBytes}
              installed={entry.installed}
              chosen={entry.id === props.model}
              busy={busy}
              disabled={props.pending || !props.canStore}
              downloading={downloading && download?.model === entry.id}
              onChoose$={$(() => props.onChoose$(entry.id))}
              onDownload$={$(() => props.onDownload$(entry.id))}
              onRemove$={$(() => props.onRemove$(entry.id))}
            />
          ))}
        </Stack>
      </PanelGroup>

      <PanelGroup title="Device" hint={deviceNote}>
        <ContentSwitcher
          ariaLabel="Device of the built in model"
          value={props.device}
          options={DEVICES.map((entry) => ({
            value: entry.value,
            label: entry.label,
          }))}
          disabled={props.pending || !props.canStore}
          onPick$={$((value: string) => {
            props.onDevice$(value as GlinerDevice);
          })}
        />
      </PanelGroup>

      <PanelGroup
        title="Smallest probability a label needs to count"
        hint="A low value reads more labels and a high value reads more surely. A label the model reads below this value leaves its entity without a value."
      >
        <TextInput
          kind="input"
          surface="field"
          name="glinerThreshold"
          ariaLabel="Smallest probability a GLiNER label needs to count"
          placeholder="0.3"
          value={props.threshold}
          disabled={props.pending || !props.canStore}
          onInput$={$((event: Event) => {
            props.onThreshold$((event.target as HTMLInputElement).value);
          })}
        />
      </PanelGroup>

      {props.budget ? (
        <PanelGroup title="Label budget">
          <Text size="body" tone="muted">
            {props.budget.intents} intents add up to {props.budget.labels}{' '}
            labels. This model reads {props.budget.softLimit} comfortably and
            stays usable to {props.budget.hardLimit}.
          </Text>
          {props.budget.warning ? (
            <Alert
              tone="warn"
              title="Many labels"
              message={props.budget.warning}
            />
          ) : null}
        </PanelGroup>
      ) : null}

      <Stack direction="row" gap="xs" align="center">
        <Text size="hud" tone="faint" class="break-all">
          {props.gliner.modelsDir}
        </Text>
        <InfoHint
          label="About the model directory"
          text="The daemon reads the models of this directory and downloads new ones into it."
          side="top"
        />
      </Stack>
    </Stack>
  );
});
