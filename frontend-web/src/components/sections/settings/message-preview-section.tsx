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
 *
 * A sentence keeps its own log, because a run of all of them is a run of
 * many answers: pressing a sentence shows the log of that sentence, and a
 * run of all of them ends on the last one it read. The log reads in two
 * ways, and the user picks the way: the reading says what the daemon would
 * do, and the technical log names every value it reported, field by field,
 * with one sentence about what each of them means.
 */
import type { QRL } from '@builder.io/qwik';

import type { ResolverPreviewDto } from '~/types/dto';

import type { ResolverPreviewResult } from '~/api/resolver';

import type { PreviewBlock, PreviewReading } from '~/utils/preview-log';

import { $, component$, useSignal } from '@builder.io/qwik';

import { Alert } from '~/components/ui/alert';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { ContentSwitcher } from '~/components/ui/content-switcher';
import { FieldLabel } from '~/components/ui/field-label';
import { InfoHint } from '~/components/ui/info-hint';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

import { joinClassNames } from '~/utils/class-names';
import { debugPreview, readPreview } from '~/utils/preview-log';

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

/** What the panel knows about one sentence. */
interface SentenceState {
  /** True while the daemon reads the sentence. */
  reading: boolean;
  /** The route the daemon answered with, or null. */
  result: ResolverPreviewDto | null;
  /** The failure of the read, or null. */
  message: string | null;
}

/** How the panel reads the log of one sentence. */
type LogMode = 'text' | 'debug';

/** The two ways to read a log, in the order the switch shows them. */
const LOG_MODES: { value: LogMode; label: string }[] = [
  { value: 'text', label: 'Text' },
  { value: 'debug', label: 'Debug' },
];

/** The note of the switch between the two readings of a log. */
const LOG_MODE_HINT =
  'Text reads the answer of the daemon in the words of a user. Debug names every value the daemon reported, field by field, so a user sees which stage read what and how long it took.';

/** The state of a sentence the panel never read. */
function unread(): SentenceState {
  return { reading: false, result: null, message: null };
}

export const MessagePreviewSection = component$<MessagePreviewSectionProps>(
  (props) => {
    /** The sentence the field beside the list would add. */
    const draft = useSignal('');
    /** The sentence that is being read right now, or null. */
    const playing = useSignal<string | null>(null);
    /** How many sentences a run of all of them has read, or null. */
    const progress = useSignal<{ done: number; total: number } | null>(null);
    /** The sentence whose log the panel shows, or null. */
    const shown = useSignal<string | null>(null);
    /** What the panel knows about each sentence, by the sentence. */
    const states = useSignal<Record<string, SentenceState>>({});
    /** How the panel reads the log of the shown sentence. */
    const mode = useSignal<LogMode>('text');
    /** The place of the sentence the user edits, or null. */
    const editing = useSignal<number | null>(null);
    /** The text of the sentence while the user edits it. */
    const editDraft = useSignal('');

    const stateOf = (sentence: string): SentenceState =>
      states.value[sentence] ?? unread();

    const addSentence = $(() => {
      const text = draft.value.trim();
      if (text.length === 0 || props.sentences.includes(text)) {
        return;
      }
      props.onSentences$([...props.sentences, text]);
      draft.value = '';
    });

    /** Forget what the panel knows about one sentence. */
    const forget = $((sentence: string) => {
      const next = { ...states.value };
      delete next[sentence];
      states.value = next;
    });

    const removeSentence = $((sentence: string) => {
      props.onSentences$(props.sentences.filter((held) => held !== sentence));
      forget(sentence);
      if (shown.value === sentence) {
        shown.value = null;
        props.onRoute$(null);
      }
    });

    /** Show the log of one sentence, and the route it took in the flow. */
    const show = $((sentence: string) => {
      const held = states.value;
      shown.value = sentence;
      props.onRoute$(sentence in held ? held[sentence].result : null);
    });

    const startEdit = $((index: number) => {
      editing.value = index;
      editDraft.value = props.sentences[index];
    });

    const cancelEdit = $(() => {
      editing.value = null;
    });

    /**
     * Keep the sentence the user wrote.
     *
     * A sentence the user emptied or wrote twice leaves the list as it was,
     * because the list holds each test of the pipeline once. The log of the
     * text the user replaced belongs to a sentence that is gone, so it is
     * forgotten rather than shown beside the new one.
     */
    const saveEdit = $((index: number) => {
      const text = editDraft.value.trim();
      const before = props.sentences[index];
      editing.value = null;
      if (
        text.length === 0 ||
        text === before ||
        props.sentences.includes(text)
      ) {
        return;
      }
      props.onSentences$(
        props.sentences.map((held, at) => (at === index ? text : held)),
      );
      forget(before);
      if (shown.value === before) {
        shown.value = text;
      }
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
      shown.value = text;
      states.value = {
        ...states.value,
        [text]: { reading: true, result: null, message: null },
      };
      const result = await props.onPreview$(text);
      playing.value = null;
      if (result.failed) {
        states.value = {
          ...states.value,
          [text]: { reading: false, result: null, message: result.message },
        };
        return false;
      }
      states.value = {
        ...states.value,
        [text]: { reading: false, result: result.data, message: null },
      };
      await props.onRoute$(result.data);
      return true;
    });

    const playOne = $((text: string) => play(text));

    const playAll = $(async () => {
      const list = [...props.sentences];
      if (list.length === 0) {
        return;
      }
      for (let index = 0; index < list.length; index += 1) {
        progress.value = { done: index, total: list.length };
        await play(list[index]);
      }
      progress.value = { done: list.length, total: list.length };
      playing.value = null;
    });

    const busy = playing.value !== null;
    const canPlay = !props.disabled && !busy && props.sentences.length > 0;
    const shownState = shown.value ? stateOf(shown.value) : null;
    const reading: PreviewReading | null = shownState?.result
      ? readPreview(shownState.result)
      : null;
    const blocks: PreviewBlock[] = shownState?.result
      ? debugPreview(shownState.result)
      : [];

    return (
      <Stack gap="md">
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
            {props.sentences.map((sentence, index) => {
              const state = stateOf(sentence);
              const open = shown.value === sentence;
              return (
                <Stack
                  key={sentence}
                  gap="xs"
                  class={joinClassNames(
                    'rounded-ds-sm border bg-ds-surface-sunken px-3 py-2',
                    open ? 'border-ds-accent-strong' : 'border-ds-line',
                  )}
                >
                  <Stack
                    direction="row"
                    gap="sm"
                    align="center"
                    justify="between"
                  >
                    {/* The sentence is the pick: pressing it shows the log
                        of that sentence, so a run of all of them is read
                        one answer at a time. */}
                    <button
                      type="button"
                      aria-pressed={open}
                      class="flex min-w-0 flex-1 items-center gap-2 text-left"
                      onClick$={() => show(sentence)}
                      onFocusIn$={() => {
                        if (shown.value !== sentence) {
                          show(sentence);
                        }
                      }}
                    >
                      <Text size="body" block class="min-w-0 truncate">
                        {sentence}
                      </Text>
                      {state.reading ? (
                        <Badge tone="accent" label="Reading" />
                      ) : null}
                      {state.result ? (
                        <Text size="hud" tone="faint" class="shrink-0">
                          {state.result.matched
                            ? (state.result.intent ?? 'an intent')
                            : 'no intent'}
                        </Text>
                      ) : null}
                      {state.message ? (
                        <Badge tone="error" label="Failed" />
                      ) : null}
                    </button>

                    {editing.value === index ? null : (
                      <Stack
                        direction="row"
                        gap="xs"
                        align="center"
                        class="shrink-0"
                      >
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
                          disabled={props.disabled}
                          ariaLabel={`Edit ${sentence}`}
                          onClick$={() => startEdit(index)}
                        >
                          Edit
                        </Button>
                        <Button
                          size="sm"
                          variant="quiet"
                          disabled={props.disabled || busy}
                          ariaLabel={`Remove ${sentence}`}
                          onClick$={() => removeSentence(sentence)}
                        >
                          Remove
                        </Button>
                      </Stack>
                    )}
                  </Stack>

                  {editing.value === index ? (
                    <Stack direction="row" gap="sm" align="end">
                      <Stack gap="xs" class="min-w-0 flex-1">
                        <FieldLabel
                          label="The sentence"
                          hint="A sentence is read the way a turn is read. Changing it forgets the log of the text it replaced."
                        />
                        <TextInput
                          kind="input"
                          surface="field"
                          name="messagePreviewEdit"
                          ariaLabel={`The sentence ${sentence}`}
                          value={editDraft.value}
                          disabled={props.disabled}
                          onInput$={$((event: Event) => {
                            editDraft.value = (
                              event.target as HTMLInputElement
                            ).value;
                          })}
                          onKeyDown$={$((event: KeyboardEvent) => {
                            if (event.key === 'Enter') {
                              saveEdit(index);
                            }
                            if (event.key === 'Escape') {
                              cancelEdit();
                            }
                          })}
                        />
                      </Stack>
                      <Stack
                        direction="row"
                        gap="xs"
                        align="center"
                        class="shrink-0"
                      >
                        <Button
                          size="md"
                          disabled={
                            props.disabled ||
                            editDraft.value.trim().length === 0
                          }
                          ariaLabel={`Keep ${sentence}`}
                          onClick$={() => saveEdit(index)}
                        >
                          Keep
                        </Button>
                        <Button
                          size="md"
                          variant="quiet"
                          ariaLabel={`Cancel ${sentence}`}
                          onClick$={cancelEdit}
                        >
                          Cancel
                        </Button>
                      </Stack>
                    </Stack>
                  ) : null}
                </Stack>
              );
            })}
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
              text="The daemon reads the sentences one after the other, so a run of ten sentences costs ten reads. Every sentence keeps its own log, and the panel ends the run on the one it read last. Nothing runs: a sentence is read the way a turn is read, and no command comes of it."
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

        {shown.value ? (
          <Stack
            gap="sm"
            class="rounded-ds-sm border border-ds-line bg-ds-surface-sunken p-3"
          >
            <Stack
              direction="row"
              gap="sm"
              align="center"
              justify="between"
              wrap
            >
              <Stack gap="none" class="min-w-0">
                <Text size="hud" tone="faint" block>
                  The log of
                </Text>
                <Text size="body" weight="medium" block class="truncate">
                  {shown.value}
                </Text>
              </Stack>
              <Stack direction="row" gap="xs" align="center">
                <ContentSwitcher
                  ariaLabel="How to read the log"
                  value={mode.value}
                  options={LOG_MODES}
                  size="sm"
                  onPick$={$((next: string) => {
                    mode.value = next as LogMode;
                  })}
                />
                <InfoHint
                  label="The two readings of a log"
                  text={LOG_MODE_HINT}
                  side="top"
                  align="right"
                />
              </Stack>
            </Stack>
            {shownState?.message ? (
              <Alert
                tone="error"
                title="The preview failed"
                message={shownState.message}
              />
            ) : null}
            {shownState?.reading && !shownState.result ? (
              <Text size="hud" tone="faint">
                The daemon reads this sentence.
              </Text>
            ) : null}
            {!shownState?.reading &&
            !shownState?.result &&
            !shownState?.message ? (
              <Text size="hud" tone="faint">
                Play this sentence to read the route it takes.
              </Text>
            ) : null}{' '}
            {reading && mode.value === 'text' ? (
              <Stack gap="sm">
                <Stack
                  direction="row"
                  gap="sm"
                  align="center"
                  justify="between"
                >
                  <Badge
                    tone={reading.matched ? 'ok' : 'warn'}
                    label={reading.status}
                  />
                  {reading.confidence ? (
                    <Badge tone="neutral" label={reading.confidence} />
                  ) : null}
                </Stack>

                <Text size="body" weight="medium" block>
                  {reading.answer}
                </Text>

                {reading.reply ? (
                  <Text size="hud" tone="muted" block>
                    {reading.reply}
                  </Text>
                ) : null}

                {reading.entities.length > 0 ? (
                  <Stack gap="none">
                    {reading.entities.map((entity) => (
                      <Text key={entity} size="hud" tone="default" block>
                        {entity}
                      </Text>
                    ))}
                  </Stack>
                ) : null}

                {reading.route.length > 0 ? (
                  <Stack gap="none">
                    {reading.route.map((step) => (
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

                {reading.reason ? (
                  <Text size="hud" tone="faint" block>
                    {reading.reason}
                  </Text>
                ) : null}

                {reading.command ? (
                  <Text size="hud" tone="muted" block class="break-all">
                    {reading.command}
                  </Text>
                ) : null}
              </Stack>
            ) : null}
            {reading && mode.value === 'debug' ? (
              <Stack gap="sm">
                {blocks.map((block, position) => (
                  <Stack key={`${block.title}-${position}`} gap="xs">
                    <Stack direction="row" gap="xs" align="center">
                      <Text size="hud" mono tone={block.tone ?? 'muted'}>
                        {block.title}
                      </Text>
                      <InfoHint
                        label={`About ${block.title}`}
                        text={block.hint}
                        side="top"
                        align="left"
                      />
                    </Stack>
                    <Stack gap="none">
                      {block.fields.map((field, at) => (
                        <Stack
                          key={`${field.label}-${at}`}
                          direction="row"
                          gap="sm"
                          align="start"
                          justify="between"
                          class="border-t border-ds-line py-1 first:border-t-0"
                        >
                          <Stack
                            direction="row"
                            gap="xs"
                            align="center"
                            class="shrink-0"
                          >
                            <Text size="hud" tone="faint">
                              {field.label}
                            </Text>
                            <InfoHint
                              label={`About ${field.label}`}
                              text={field.hint}
                              side="top"
                              align="left"
                            />
                          </Stack>
                          <Text
                            size="hud"
                            mono
                            tone={field.tone ?? 'default'}
                            block
                            class="min-w-0 flex-1 text-right break-all"
                          >
                            {field.value}
                          </Text>
                        </Stack>
                      ))}
                    </Stack>
                  </Stack>
                ))}
              </Stack>
            ) : null}
          </Stack>
        ) : null}
      </Stack>
    );
  },
);
