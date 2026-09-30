/**
 * `ChatTranscriptSection` holds the conversation.
 *
 * It is a glass panel with one scrolling region, so the panel scrolls on
 * its own and the stage beside it stays still. The region keeps the newest
 * turn in view while the reader is at the end, and leaves the place of a
 * reader who scrolled up. A failure shows above the list, so the retry
 * action stays reachable.
 *
 * The section owns the preference of how a route reads, because every turn
 * of the transcript reads its route the same way. The preference is stored
 * in the browser, so a reader who asks for the debug reading keeps it over
 * a reload, and the switch of one turn moves every turn with it.
 */
import type { QRL } from '@builder.io/qwik';

import type { ChatRow } from '~/utils/chat';

import { $, component$ } from '@builder.io/qwik';

import { ChatMessagePartial } from '~/components/sections/chat/chat-message-partial';
import { Alert } from '~/components/ui/alert';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { EmptyState } from '~/components/ui/empty-state';
import { ScrollArea } from '~/components/ui/scroll-area';

import { ROUTE_DEBUG_PREFERENCE, useStoredFlag } from '~/utils/preference';

/** The props of `ChatTranscriptSection`. */
export interface ChatTranscriptSectionProps {
  /** The turns of the conversation, newest last. */
  rows: ChatRow[];
  /** The failure of the last send action, or null. */
  error: string | null;
  /** A note about the last turn, or null. */
  notice?: string | null;
  /** Repeat the last send action. */
  onRetry$?: QRL<() => void>;
}

export const ChatTranscriptSection = component$<ChatTranscriptSectionProps>(
  (props) => {
    const debug = useStoredFlag(ROUTE_DEBUG_PREFERENCE, false);

    const handleDebug$ = $((next: boolean) => {
      debug.value = next;
    });

    return (
      <Card
        label="Transcript"
        class="flex min-h-[220px] w-full flex-1 flex-col"
        bodyClass="flex min-h-0 flex-1 flex-col gap-3 !p-0"
      >
        {props.error ? (
          <Box class="px-4 pt-3">
            <Alert
              tone="error"
              title="Send failed"
              message={props.error}
              onRetry$={props.onRetry$}
            />
          </Box>
        ) : null}

        {props.notice ? (
          <Box class="px-4 pt-3">
            <Alert tone="info" title="Not stored" message={props.notice} />
          </Box>
        ) : null}

        <ScrollArea
          ariaLabel="Conversation transcript"
          stickToEnd
          stickKey={props.rows.length}
          class="flex flex-1 flex-col gap-3 px-4 py-3"
        >
          {props.rows.length === 0 ? (
            <EmptyState
              title="No turns yet"
              description="The conversation with the daemon appears here. Type a message or use the wake word."
            />
          ) : (
            props.rows.map((row) => (
              <ChatMessagePartial
                key={row.id}
                row={row}
                debug={debug.value}
                onDebug$={handleDebug$}
              />
            ))
          )}
        </ScrollArea>
      </Card>
    );
  },
);
