/**
 * `MessagePreviewSection` tries sentences against the resolver.
 *
 * The block of the message in the flow of the router holds this: the user
 * writes the sentences they really say, plays one, or plays all of them.
 * The daemon reads a sentence the way a turn would and runs nothing, so a
 * user reads the route before a turn depends on it.
 *
 * The sentences are the tests of the pipeline, so the daemon stores them
 * with the settings: a user who tuned the router against ten sentences
 * finds them again after a reload, and a sentence that never met an intent
 * is still there to fix. A read stores nothing else: no turn, no message,
 * and no command ever comes of a sentence the user plays.
 */
import type { QRL } from '@builder.io/qwik';

import type { ResolverPreviewDto } from '~/types/dto';

import type { ResolverPreviewResult } from '~/api/resolver';

import { $, component$, useSignal } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { InfoHint } from '~/components/ui/info-hint';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

import { formatConfidence, formatEntity, formatRouteStep } from '~/utils/chat';

/** The props of `MessagePreviewSection`. */
export interface MessagePreviewSectionProps {
  /** The sentences the daemon stores, in the order the user wrote them. */
  sentences: string[];
  /** True while a save runs or the settings cannot be stored. */
  disabled: boolean;
  /**
   * Read one sentence without running anything. The page defines the
   * handle around the `server$` function.
   */
  onPreview$: QRL<(text: string) => Promise<ResolverPreviewResult>>;
  /** Store the list of sentences, without waiting for an answer. */
  onSentences$: QRL<(next: string[]) => void>;
  /** Report the route of the sentence that was read, or null when none was. */
  onRoute$: QRL<(preview: ResolverPreviewDto | null) => void>;
}

export const MessagePreviewSection = component$<MessagePreviewSectionProps>(
  (props) => {
    /** The sentence the field beside the list would add. */
    const draft = useSignal('');
    /** The sentence that is being read right now, or null. */
    const playing = useSignal<string | null>(null);
    /** How many sentences a run of all of them has read, or null. */
    const progress = useSignal<{ done: number; total: number } | null>(null);
    /** The route of the sentence that was read last, or null. */
    const answer = useSignal<ResolverPreviewDto | null>(null);
    /** The failure of the last read, or null. */
    const error = useSignal<string | null>(null);

    const addSentence = $(() => {
      const text = draft.value.trim();
      if (text.length === 0 || props.sentences.includes(text)) {
        return;
      }
      props.onSentences$([...props.sentences, text]);
      draft.value = '';
    });

    const removeSentence = $((index: number) => {
      props.onSentences$(props.sentences.filter((_, at) => at !== index));
    });

    /**
     * Read one sentence and report the route it took.
     *
     * A failure of one sentence does not stop a run of all of them: the
     * sentence is reported and the next one is read, so one sentence the
     * daemon cannot read does not hide the ones behind it.
     */
    const play = $(async (text: string): Promise<boolean> => {
      playing.value = text;
      const result = await props.onPreview$(text);
      playing.value = null;
      if (result.failed) {
        error.value = result.message;
        return false;
      }
      error.value = null;
      answer.value = result.data;
      await props.onRoute$(result.data);
      return true;
    });

    const playOne = $((text: string) => play(text));

    const playAll = $(async () => {
      const list = [...props.sentences];
      if (list.length === 0) {
        return;
      }
      error.value = null;
      for (let index = 0; index < list.length; index += 1) {
        progress.value = { done: index, total: list.length };
        await play(list[index]);
      }
      progress.value = { done: list.length, total: list.length };
      playing.value = null;
    });

    const busy = playing.value !== null;
    const canPlay = !props.disabled && !busy && props.sentences.length > 0;

    return (
      <Stack gap="md">
        {error.value ? (
          <Alert
            tone="error"
            title="The preview failed"
            message={error.value}
          />
        ) : null}

        <Stack direction="row" gap="sm" align="end">
          <Stack gap="xs" class="min-w-0 flex-1">
            <Text size="hud" tone="faint">
              One sentence
            </Text>
            <TextInput
              kind="input"
              surface="field"
              name="messagePreviewSentence"
              ariaLabel="One sentence to try"
              placeholder="open firefox"
              value={draft.value}
              disabled={props.disabled}
              onInput$={$((event: Event) => {
                draft.value = (event.target as HTMLInputElement).value;
              })}
              // The field stands in no form, so Enter adds the sentence
              // and nothing else happens.
              onKeyDown$={$((event: KeyboardEvent) => {
                if (event.key === 'Enter') {
                  addSentence();
                }
              })}
            />
          </Stack>
          <Button
            size="md"
            disabled={props.disabled || draft.value.trim().length === 0}
            onClick$={addSentence}
          >
            Add
          </Button>
        </Stack>

        {props.sentences.length > 0 ? (
          <Stack gap="xs">
            {props.sentences.map((sentence, index) => (
              <Stack
                key={sentence}
                direction="row"
                gap="sm"
                align="center"
                justify="between"
                class="rounded-ds-sm border border-ds-line bg-ds-surface-sunken px-3 py-2"
              >
                <Stack
                  direction="row"
                  gap="sm"
                  align="center"
                  class="min-w-0 flex-1"
                >
                  <Text size="body" block class="min-w-0 flex-1 truncate">
                    {sentence}
                  </Text>
                  {playing.value === sentence ? (
                    <Badge tone="accent" label="Reading" />
                  ) : null}
                </Stack>
                <Stack direction="row" gap="xs" align="center" class="shrink-0">
                  <Button
                    size="sm"
                    variant="quiet"
                    disabled={props.disabled || busy}
                    ariaLabel={`Play ${sentence}`}
                    onClick$={() => playOne(sentence)}
                  >
                    Play
                  </Button>
                  <Button
                    size="sm"
                    variant="quiet"
                    disabled={props.disabled || busy}
                    ariaLabel={`Remove ${sentence}`}
                    onClick$={() => removeSentence(index)}
                  >
                    Remove
                  </Button>
                </Stack>
              </Stack>
            ))}
          </Stack>
        ) : (
          <Text size="hud" tone="faint">
            Add the sentences you really say, one at a time, and play them
            against the resolver. No turn and no command comes of a play.
          </Text>
        )}

        <Stack direction="row" gap="sm" align="center" justify="between" wrap>
          <Stack direction="row" gap="xs" align="center">
            <Button
              variant="outline"
              size="sm"
              disabled={!canPlay}
              onClick$={playAll}
            >
              {progress.value && busy
                ? `Reading ${progress.value.done + 1} of ${progress.value.total}`
                : 'Play all'}
            </Button>
            <InfoHint
              label="Playing every sentence"
              text="The daemon reads the sentences one after the other, so a run of ten sentences costs ten reads. The flow keeps the route of the sentence that was read last. Nothing runs: a sentence is read the way a turn is read, and no command comes of it."
              side="top"
              align="left"
            />
          </Stack>
          <Text size="hud" tone="faint">
            {props.sentences.length === 1
              ? '1 sentence'
              : `${props.sentences.length} sentences`}
          </Text>
        </Stack>

        {answer.value ? (
          <Stack
            gap="sm"
            class="rounded-ds-sm border border-ds-line bg-ds-surface-sunken p-3"
          >
            <Stack direction="row" gap="sm" align="center" justify="between">
              <Text size="hud" tone="faint">
                {answer.value.matched ? 'Would run' : 'No intent'}
              </Text>
              {answer.value.confidence !== null ? (
                <Badge
                  tone="neutral"
                  label={formatConfidence(answer.value.confidence) ?? ''}
                />
              ) : null}
            </Stack>
            <Text size="body" weight="medium" block>
              {answer.value.intent ?? answer.value.reply}
            </Text>
            {answer.value.reply && answer.value.intent ? (
              <Text size="hud" tone="muted" block>
                {answer.value.reply}
              </Text>
            ) : null}

            {(answer.value.meta?.entities ?? []).length > 0 ? (
              <Stack gap="none">
                {(answer.value.meta?.entities ?? []).map((entity) => (
                  <Text key={entity.name} size="hud" tone="default" block>
                    {formatEntity(entity)}
                  </Text>
                ))}
              </Stack>
            ) : null}

            {(answer.value.meta?.route?.steps ?? []).length > 0 ? (
              <Stack gap="xs">
                {(answer.value.meta?.route?.steps ?? [])
                  .map(formatRouteStep)
                  .map((step) => (
                    <Stack key={step.stage} gap="none">
                      <Text size="hud" tone="muted" block>
                        {`${step.stage}  ${step.reader}  ${step.outcome}  ${step.duration}`}
                      </Text>
                      {step.detail ? (
                        <Text size="hud" tone="faint" block>
                          {step.detail}
                        </Text>
                      ) : null}
                    </Stack>
                  ))}
              </Stack>
            ) : null}

            {answer.value.meta?.route?.reason ? (
              <Text size="hud" tone="faint" block>
                {answer.value.meta.route.reason}
              </Text>
            ) : null}

            {answer.value.meta?.command ? (
              <Text size="hud" tone="muted" block class="break-all">
                {answer.value.meta.command}
              </Text>
            ) : null}
          </Stack>
        ) : null}
      </Stack>
    );
  },
);
