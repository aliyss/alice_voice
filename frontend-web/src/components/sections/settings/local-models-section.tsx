/**
 * `LocalModelsSection` installs the built in models of the router.
 *
 * The router reads vectors and, when its decision stage reranks, reads how
 * well a message fits a candidate. Both reads can come from the model
 * server instead, so this section shows only the roles the current
 * settings read.
 *
 * A built in model is a file on disk and not a service, so every role
 * lists its models with the state of each file. The download runs in the
 * backend and the page polls its progress, because a model is a hundred
 * megabytes. The sentence about a model waits in the hint of its row, so
 * the list reads as a list.
 */
import type { QRL } from '@builder.io/qwik';

import type { LocalDevice, LocalStoreDto } from '~/types/dto';

import { $, component$ } from '@builder.io/qwik';

import { ModelRow } from '~/components/sections/settings/model-row-partial';
import { Alert } from '~/components/ui/alert';
import { Button } from '~/components/ui/button';
import { PanelGroup } from '~/components/ui/panel-group';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { formatSize } from '~/utils/size';

/** What one built in model of the router reads. */
export type LocalModelRole = 'embeddings' | 'reranker';

/** The devices the settings page offers, in the order it shows them. */
const DEVICES: { value: LocalDevice; label: string }[] = [
  { value: 'auto', label: 'Auto' },
  { value: 'cpu', label: 'CPU' },
  { value: 'cuda', label: 'CUDA' },
];

/** The name of each role as the section shows it. */
const ROLE_LABELS: Record<LocalModelRole, string> = {
  embeddings: 'Embedding model',
  reranker: 'Reranker',
};

/** The sentence that explains what each role reads. */
const ROLE_NOTES: Record<LocalModelRole, string> = {
  embeddings:
    'Turns a message and an intent into vectors, so `launch the browser` reaches `open application` although the two share no word.',
  reranker:
    'Reads a message and one candidate together and reports how well the two fit, which is surer than a score of two separate vectors.',
};

/**
 * The sentence about the device of a built in model.
 *
 * A build without CUDA and a machine without a usable card read the same
 * way to the model, so the hint distinguishes the two before a reader
 * wonders why the graphics card does nothing.
 */
function deviceNote(props: LocalModelsSectionProps): string {
  if (props.devices.includes(props.device) || props.device === 'auto') {
    return `This build runs on ${props.devices.join(' and ')}. Auto picks the best one.`;
  }
  return props.cudaBuild
    ? 'No CUDA device is usable right now, so the model would run on the processor.'
    : 'This build carries no CUDA support. Build the daemon with --features gliner-cuda to run on the graphics card.';
}

/** The props of `LocalModelsSection`. */
export interface LocalModelsSectionProps {
  /** The state of the built in models, or null while it loads. */
  store: LocalStoreDto | null;
  /** The roles the current settings read, in the order the section shows them. */
  roles: LocalModelRole[];
  /** The model the settings read for each role. */
  chosen: Record<LocalModelRole, string>;
  /** The device the settings ask for. */
  device: LocalDevice;
  /** The devices this build and this machine offer, best first. */
  devices: string[];
  /** Whether this build carries CUDA support. */
  cudaBuild: boolean;
  /** True while a save runs. */
  pending: boolean;
  /** True while a download or a remove request is in flight. */
  installing: boolean;
  /**
   * Draw the device row. It is on by default. A flow that renders one
   * instance per stage turns it off on every instance but the first, so the
   * shared device has one place on screen.
   */
  showDevice?: boolean;
  /** Report the model the user chose for one role. */
  onChoose$: QRL<(role: LocalModelRole, id: string) => void>;
  /** Report the device the user chose. */
  onDevice$: QRL<(device: LocalDevice) => void>;
  /** Download one built in model. */
  onDownload$: QRL<(id: string) => void>;
  /** Remove the files of one built in model. */
  onRemove$: QRL<(id: string) => void>;
}

export const LocalModelsSection = component$<LocalModelsSectionProps>(
  (props) => {
    const store = props.store;
    const download = store?.download ?? null;
    const downloading = Boolean(download && !download.done && !download.error);
    const busy = props.pending || props.installing || downloading;

    return (
      <Stack gap="md">
        {props.roles.map((role) => {
          const models = (store?.models ?? []).filter(
            (entry) => entry.role === role,
          );
          const selected = props.chosen[role];
          const chosenName =
            models.find((entry) => entry.id === selected)?.name ?? selected;
          const installed = models.some(
            (entry) => entry.id === selected && entry.installed,
          );

          return (
            <PanelGroup
              key={role}
              title={ROLE_LABELS[role]}
              hint={ROLE_NOTES[role]}
            >
              <Stack gap="sm">
                {models.map((entry) => (
                  <ModelRow
                    key={entry.id}
                    name={entry.name}
                    note={entry.note}
                    sizeBytes={entry.sizeBytes}
                    installed={entry.installed}
                    chosen={entry.id === selected}
                    busy={busy}
                    disabled={props.pending}
                    downloading={downloading && download?.model === entry.id}
                    onChoose$={$(() => props.onChoose$(role, entry.id))}
                    onDownload$={$(() => props.onDownload$(entry.id))}
                    onRemove$={$(() => props.onRemove$(entry.id))}
                  />
                ))}

                {!installed ? (
                  <Alert
                    tone="warn"
                    title="The selected model is not on disk"
                    message={`${chosenName} has to be on disk before the router can read it. Download it above, or point this stage at the model server, or choose a reader that needs no model.`}
                  />
                ) : null}
              </Stack>
            </PanelGroup>
          );
        })}

        {props.showDevice === false ? null : (
          <PanelGroup title="Device" hint={deviceNote(props)}>
            <Stack direction="row" gap="sm">
              {DEVICES.map((entry) => (
                <Button
                  key={entry.value}
                  variant={props.device === entry.value ? 'solid' : 'outline'}
                  size="sm"
                  disabled={props.pending}
                  onClick$={$(() => props.onDevice$(entry.value))}
                >
                  {entry.label}
                </Button>
              ))}
            </Stack>
          </PanelGroup>
        )}

        {download ? (
          <Stack gap="xs">
            {download.error ? (
              <Alert
                tone="error"
                title="The download failed"
                message={download.error}
              />
            ) : download.done ? (
              <Text size="micro" tone="faint">
                {download.model} is ready.
              </Text>
            ) : (
              <Stack direction="row" gap="sm" align="center">
                <Spinner size="sm" label="The model is downloading" />
                <Text size="micro" tone="faint">
                  {download.model}: {formatSize(download.receivedBytes)}
                  {download.totalBytes
                    ? ` of ${formatSize(download.totalBytes)} (${Math.floor(
                        (download.receivedBytes / download.totalBytes) * 100,
                      )}%)`
                    : ''}
                </Text>
              </Stack>
            )}
          </Stack>
        ) : null}

        <Text size="hud" tone="faint" class="break-all">
          {store?.modelsDir ?? 'the model directory of the router'}
        </Text>
      </Stack>
    );
  },
);
