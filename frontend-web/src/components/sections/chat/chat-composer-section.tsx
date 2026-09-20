/**
 * `ChatComposerSection` sends one message to the daemon.
 *
 * The section owns the draft text. It sends on Enter and starts a new
 * line on Shift and Enter. The page owns the send action, so the section
 * stays free of the REST call.
 *
 * The field takes the caret when the surface loads and again when the
 * answer of the daemon lands, so the next message needs no click.
 */
import type { QRL } from '@builder.io/qwik';

import { $, component$, useSignal } from '@builder.io/qwik';

import { Button } from '~/components/ui/button';
import { Card } from '~/components/ui/card';
import { Spinner } from '~/components/ui/spinner';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

/** The largest height of the growing field, in pixels. */
const MAX_FIELD_HEIGHT = 160;

/** The props of `ChatComposerSection`. */
export interface ChatComposerSectionProps {
  /** True while the daemon answers the last message. */
  pending: boolean;
  /** Send one message. */
  onSend$: QRL<(text: string) => Promise<void>>;
}

export const ChatComposerSection = component$<ChatComposerSectionProps>(
  (props) => {
    const draft = useSignal('');

    const handleInput$ = $((event: Event) => {
      const field = event.target as HTMLTextAreaElement;
      draft.value = field.value;
      field.style.height = 'auto';
      field.style.height = `${Math.min(field.scrollHeight, MAX_FIELD_HEIGHT)}px`;
    });

    const handleSubmit$ = $(async () => {
      const text = draft.value.trim();
      if (text.length === 0 || props.pending) {
        return;
      }
      draft.value = '';
      await props.onSend$(text);
    });

    const handleKeyDown$ = $((event: KeyboardEvent) => {
      if (event.key === 'Enter' && !event.shiftKey) {
        // The keydown attribute suppresses every key, and the field must
        // keep the Shift and Enter case. The handler is not asynchronous,
        // so the call runs in the same tick as the event.
        // eslint-disable-next-line qwik/no-async-prevent-default
        event.preventDefault();
        void handleSubmit$();
      }
    });

    // The body keeps the padding of the chat surface, which is tighter
    // than the default of the panel.
    return (
      <Card label="Prompt" class="w-full shrink-0" bodyClass="p-3">
        <Stack gap="sm">
          <Stack direction="row" gap="sm" align="end">
            <TextInput
              kind="textarea"
              name="message"
              ariaLabel="Message to the daemon"
              placeholder="Ask Alice to do something..."
              value={draft.value}
              disabled={props.pending}
              autoFocus
              class="max-h-40 pt-1.5"
              onInput$={handleInput$}
              onKeyDown$={handleKeyDown$}
            />
            <Button
              variant="solid"
              size="md"
              class="shrink-0"
              disabled={props.pending}
              onClick$={handleSubmit$}
            >
              {props.pending ? (
                <Spinner size="sm" label="Sending" />
              ) : (
                <Text size="micro" tone="inverse">
                  Send
                </Text>
              )}
            </Button>
          </Stack>
          <Text size="micro" tone="faint">
            Enter to send · Shift and Enter for a new line
          </Text>
        </Stack>
      </Card>
    );
  },
);
