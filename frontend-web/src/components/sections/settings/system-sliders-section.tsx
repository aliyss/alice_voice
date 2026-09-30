/**
 * `SystemSlidersSection` shows the host profile and two linked sliders.
 *
 * The sliders control the same `quality` value where 0 is fastest and 100
 * is best. Speed is `100 - quality`. Every value between the minimum
 * successful settings and the maximum quality settings is a linear
 * interpolation. The backend recommends the quality that fits the machine.
 */
import type { QRL } from '@builder.io/qwik';

import type { SettingsDto, SystemGpuDto, SystemProfileDto } from '~/types/dto';

import { $, component$, useSignal, useTask$ } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { InfoHint } from '~/components/ui/info-hint';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { getSystemProfile } from '~/api/system';

import { presetForQuality } from '~/utils/preset';
import { formatSize } from '~/utils/size';

/** The props of `SystemSlidersSection`. */
export interface SystemSlidersSectionProps {
  /** Current stored settings, used as defaults for the preset. */
  settings: SettingsDto;
  /** Current quality 0..100, 0 is fastest. */
  quality: number;
  /** Whether the current router does not match the preset for the quality. */
  isCustom: boolean;
  /** Apply the router preset for the chosen quality. */
  onQualityChange$: QRL<(quality: number) => void>;
  /** True while the settings save runs. */
  disabled?: boolean;
}

/**
 * Read one graphics device as a line of the facts.
 *
 * The memory of a device decides which built in model fits on it, so the
 * line names the total and how much of it is in use. An integrated device
 * holds no memory of its own and draws on the memory of the system, so it
 * reads the total of the host rather than a number of zero. A machine that
 * reports no memory at all says so rather than showing a zero.
 */
function gpuFact(gpu: SystemGpuDto, systemMemoryBytes: number): string {
  const memory = gpu.sharedMemory
    ? `shared system memory · ${formatSize(systemMemoryBytes)} RAM`
    : gpu.memoryTotalBytes === null
      ? 'memory not reported'
      : `${formatSize(gpu.memoryTotalBytes)} VRAM${
          gpu.memoryUsedBytes === null
            ? ''
            : ` · ${formatSize(gpu.memoryUsedBytes)} used`
        }`;
  const driver = gpu.driverVersion ? ` · driver ${gpu.driverVersion}` : '';
  return `${gpu.name} · ${memory}${driver}`;
}

/** One row of the system facts. */
function FactRow(props: { label: string; value: string }) {
  return (
    <Box class="flex items-baseline justify-between gap-2 py-1">
      <Text size="micro" tone="faint">
        {props.label}
      </Text>
      <Text size="micro" weight="medium" class="text-right">
        {props.value}
      </Text>
    </Box>
  );
}

export const SystemSlidersSection = component$<SystemSlidersSectionProps>(
  (props) => {
    const profile = useSignal<SystemProfileDto | null>(null);
    const error = useSignal<string | null>(null);
    // Local drag state keeps focus while the thumb is held. The parent
    // holds the committed quality; we mirror it here and only push on
    // input so the preview below stays live without remounting the
    // range element.
    const dragQuality = useSignal(props.quality);

    // Keep local drag in sync when the stored quality changes externally
    // (after a save or when switching to recommended).
    useTask$(({ track }) => {
      track(() => props.quality);
      dragQuality.value = props.quality;
    });

    // Load the profile once on the browser.
    useTask$(async ({ track }) => {
      track(() => true);
      const result = await getSystemProfile();
      if (result.failed) {
        error.value = result.message;
        return;
      }
      profile.value = result.data;
    });

    const speed = 100 - dragQuality.value;

    // The graphics devices read the memory of the host for an integrated
    // device, so both values are read once and handed to the row.
    const gpus = profile.value?.devices.gpus ?? [];
    const systemMemoryBytes = profile.value?.memory.totalBytes ?? 0;
    const recommendation = profile.value?.recommendation ?? null;
    const preview = (() => {
      try {
        const p = presetForQuality(dragQuality.value, props.settings);
        return {
          retrieve: p.routerRetrieve,
          decide: p.routerDecide,
          extract: p.routerExtract,
          topK: p.routerTopK,
          floor: p.routerFloor,
          margin: p.routerMargin,
          lexicalWeight: p.routerLexicalWeight,
          denseWeight: p.routerDenseWeight,
          embedSource: p.routerEmbedSource,
          embedModel:
            p.routerEmbedSource === 'local'
              ? p.routerEmbedLocalModel
              : p.routerEmbedModel,
          listMatch: p.routerListMatch,
          listFloor: p.routerListFloor,
          phraseGate: p.routerPhraseGate,
          fastPath: p.routerFastPath,
        };
      } catch {
        return null;
      }
    })();

    return (
      <Card>
        <Stack gap="md">
          <Stack direction="row" gap="xs" align="center">
            <Text size="body" weight="medium">
              Performance
            </Text>
            <InfoHint
              label="About performance"
              text="Two linked sliders choose the trade-off between response speed and response quality. Both control the same quality value where 0 is fastest and 100 is best. Speed is 100 − quality. Every value in between is a linear interpolation between the minimum successful settings and the maximum quality settings. The default is the lerp position that fits the PC."
            />
          </Stack>

          {error.value ? (
            <Alert
              tone="error"
              title="System profile unavailable"
              message={error.value}
            />
          ) : null}

          {profile.value ? (
            <Box class="rounded-ds-sm border border-ds-line bg-ds-surface-raised p-3">
              <Stack gap="xs">
                <Stack
                  direction="row"
                  gap="sm"
                  align="center"
                  justify="between"
                >
                  <Text size="micro" weight="medium">
                    PC: {profile.value.os.name ?? 'Unknown'}{' '}
                    {profile.value.os.version ?? ''} · {profile.value.cpu.brand}
                  </Text>
                  <Badge
                    tone="neutral"
                    label={`${profile.value.cpu.logicalCores} cores · ${formatSize(profile.value.memory.totalBytes)} RAM`}
                  />
                </Stack>
                <FactRow
                  label="CPU"
                  value={`${profile.value.cpu.logicalCores} logical · ${profile.value.cpu.physicalCores ?? '-'} physical · ${profile.value.cpu.frequencyMhz} MHz`}
                />
                <FactRow
                  label="Memory"
                  value={`${formatSize(profile.value.memory.totalBytes)} total · ${formatSize(profile.value.memory.availableBytes)} free`}
                />
                <FactRow
                  label="Devices"
                  value={`${profile.value.devices.cudaAvailable ? 'CUDA' : 'CPU'} · ${profile.value.devices.availableDevices.join(', ')}`}
                />
                {gpus.map((gpu) => (
                  <FactRow
                    key={gpu.name}
                    label={`GPU · ${gpu.vendor}`}
                    value={gpuFact(gpu, systemMemoryBytes)}
                  />
                ))}
                {gpus.length === 0 ? (
                  <FactRow
                    label="GPU"
                    value="no graphics device reported by this machine"
                  />
                ) : null}
                <FactRow
                  label="Resolver"
                  value={`${profile.value.resolver.intentCount} intents · ${profile.value.resolver.labelCount} labels · ${profile.value.resolver.llamaReachable ? 'server up' : 'server down'}`}
                />
                {recommendation ? (
                  <Box class="mt-2 rounded-ds-sm bg-ds-surface p-2">
                    <Text size="micro" weight="medium">
                      Recommended: {recommendation.quality} quality /{' '}
                      {recommendation.speed} speed — {recommendation.reason}
                    </Text>
                    <Stack gap="xs" class="mt-1">
                      {recommendation.factors.slice(0, 6).map((factor) => (
                        <Text key={factor} size="micro" tone="faint">
                          · {factor}
                        </Text>
                      ))}
                    </Stack>
                    <Box class="mt-2 flex justify-end">
                      <Button
                        variant="quiet"
                        size="sm"
                        disabled={
                          props.disabled ||
                          (!props.isCustom &&
                            dragQuality.value === recommendation.quality)
                        }
                        onClick$={() => {
                          const q = recommendation.quality;
                          dragQuality.value = q;
                          void props.onQualityChange$(q);
                        }}
                      >
                        Use recommended
                      </Button>
                    </Box>
                  </Box>
                ) : null}
              </Stack>
            </Box>
          ) : (
            <Text size="micro" tone="faint">
              Loading system profile…
            </Text>
          )}

          {props.isCustom ? (
            <Alert
              tone="warn"
              title="Custom flow"
              message="Flow was edited manually. Moving a slider will overwrite the custom flow with the preset for that quality."
            />
          ) : null}

          <Stack gap="sm">
            <Stack gap="xs">
              <Box class="flex items-center justify-between">
                <Text size="micro" weight="medium">
                  Response quality
                </Text>
                <Badge
                  tone={props.isCustom ? 'warn' : 'accent'}
                  label={String(dragQuality.value)}
                />
              </Box>
              <Box class="relative flex items-center">
                {recommendation ? (
                  <Box
                    ariaHidden
                    class="pointer-events-none absolute top-1/2 h-4 w-1 -translate-y-1/2 rounded-full bg-ds-accent/40"
                    style={{ left: `calc(${recommendation.quality}% - 2px)` }}
                  />
                ) : null}
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={dragQuality.value}
                  disabled={props.disabled}
                  aria-label="Response quality"
                  class={
                    props.isCustom
                      ? 'h-2 w-full cursor-pointer appearance-none rounded-full bg-ds-line accent-ds-warn'
                      : 'h-2 w-full cursor-pointer appearance-none rounded-full bg-ds-line accent-ds-accent'
                  }
                  onInput$={$((event: Event) => {
                    const next = Number(
                      (event.target as HTMLInputElement).value,
                    );
                    dragQuality.value = next;
                    void props.onQualityChange$(next);
                  })}
                />
              </Box>
              <Box class="flex justify-between">
                <Text size="micro" tone="faint">
                  Fastest
                </Text>
                <Text size="micro" tone="faint">
                  Best
                </Text>
              </Box>
            </Stack>

            <Stack gap="xs">
              <Box class="flex items-center justify-between">
                <Text size="micro" weight="medium">
                  Response speed
                </Text>
                <Badge
                  tone={props.isCustom ? 'warn' : 'accent'}
                  label={String(speed)}
                />
              </Box>
              <Box class="relative flex items-center">
                {recommendation ? (
                  <Box
                    ariaHidden
                    class="pointer-events-none absolute top-1/2 h-4 w-1 -translate-y-1/2 rounded-full bg-ds-accent/40"
                    style={{ left: `calc(${recommendation.speed}% - 2px)` }}
                  />
                ) : null}
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={speed}
                  disabled={props.disabled}
                  aria-label="Response speed"
                  class={
                    props.isCustom
                      ? 'h-2 w-full cursor-pointer appearance-none rounded-full bg-ds-line accent-ds-warn'
                      : 'h-2 w-full cursor-pointer appearance-none rounded-full bg-ds-line accent-ds-accent'
                  }
                  onInput$={$((event: Event) => {
                    const next = Number(
                      (event.target as HTMLInputElement).value,
                    );
                    const q = 100 - next;
                    dragQuality.value = q;
                    void props.onQualityChange$(q);
                  })}
                />
              </Box>
              <Box class="flex justify-between">
                <Text size="micro" tone="faint">
                  Slowest
                </Text>
                <Text size="micro" tone="faint">
                  Fastest
                </Text>
              </Box>
            </Stack>

            {preview ? (
              <Box class="rounded-ds-sm border border-ds-line bg-ds-surface-raised p-3">
                <Stack gap="xs">
                  <Stack direction="row" gap="xs" align="center">
                    <Text size="micro" weight="medium">
                      What will change
                    </Text>
                    <InfoHint
                      label="About what will change"
                      text="The sliders are a lerp between the minimum successful settings (lexical/score/lists) and the maximum quality settings (hybrid/generative/generative). Moving a slider live-updates the Flow tab and the values below. Save to persist."
                    />
                  </Stack>
                  <FactRow label="Retrieval" value={preview.retrieve} />
                  <FactRow label="Decision" value={preview.decide} />
                  <FactRow label="Extraction" value={preview.extract} />
                  <FactRow label="Short list" value={String(preview.topK)} />
                  <FactRow
                    label="Floor / Margin"
                    value={`${preview.floor.toFixed(2)} / ${preview.margin.toFixed(2)}`}
                  />
                  <FactRow
                    label="Weights"
                    value={`lex ${preview.lexicalWeight.toFixed(1)} / dense ${preview.denseWeight.toFixed(1)}`}
                  />
                  <FactRow label="Embeddings" value={preview.embedSource} />
                  <FactRow
                    label="List match"
                    value={`${preview.listMatch} · floor ${preview.listFloor.toFixed(2)}`}
                  />
                  <FactRow
                    label="Phrase gate / Fast path"
                    value={`${preview.phraseGate ? 'on' : 'off'} / ${preview.fastPath ? 'on' : 'off'}`}
                  />
                  <FactRow label="Embed model" value={preview.embedModel} />
                </Stack>
              </Box>
            ) : (
              <Text size="micro" tone="faint">
                Loading preview…
              </Text>
            )}

            {props.disabled ? (
              <Text size="micro" tone="faint">
                Saving…
              </Text>
            ) : null}

            {profile.value?.presets.length ? (
              <Box class="flex flex-wrap gap-1 pt-1">
                {profile.value.presets.map((preset) => (
                  <Badge
                    key={preset.quality}
                    tone={
                      !props.isCustom && dragQuality.value === preset.quality
                        ? 'ok'
                        : 'neutral'
                    }
                    label={`${preset.label} ${preset.quality}`}
                  />
                ))}
              </Box>
            ) : null}
          </Stack>
        </Stack>
      </Card>
    );
  },
);
