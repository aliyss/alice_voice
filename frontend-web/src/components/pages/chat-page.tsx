/**
 * `ChatPage` is the conversation surface of the daemon.
 *
 * The page holds the transcript of one conversation and the pending flag.
 * The daemon state comes from the app status context, so the aura reacts to
 * the live socket stream.
 *
 * The stage sits on the left: the aura takes the room it needs, the
 * composer waits at the bottom, and the stages of the running turn sit
 * right above the composer, so the work of the daemon reads as the answer
 * that is on its way. The transcript panel sits on the right and scrolls on
 * its own, so the newest turn never moves the aura. A narrow window stacks
 * the panel above the stage.
 *
 * The page receives the send action as a callback prop. The route file
 * defines the handle that wraps the `server$` function.
 */
import type { QRL } from '@builder.io/qwik';

import type { ApiResult } from '~/types/bridge';
import type {
  ChatMessageDto,
  ChatReplyDto,
  ConversationDto,
} from '~/types/dto';

import type { SendMessageInput } from '~/schemas/chat';

import type { ChatRow } from '~/utils/chat';
import type { AuraState } from '~/utils/status';

import { $, component$, useContext, useSignal } from '@builder.io/qwik';

import { AuraStageSection } from '~/components/sections/chat/aura-stage-section';
import { ChatComposerSection } from '~/components/sections/chat/chat-composer-section';
import { ChatTranscriptSection } from '~/components/sections/chat/chat-transcript-section';
import { ConversationHeaderSection } from '~/components/sections/chat/conversation-header-section';
import { LiveTurnSection } from '~/components/sections/chat/live-turn-section';
import { Box } from '~/components/ui/box';
import { Stack } from '~/components/ui/stack';

import { appStatusContext } from '~/context/app-status.context';

import { formatClock, toChatRow, toPendingRow } from '~/utils/chat';
import { formatDay, toTitle } from '~/utils/conversation';
import { toAuraState } from '~/utils/status';

/** The notice the page shows after a turn the daemon did not store. */
const NOT_STORED_NOTICE =
  'The queue is off. The daemon did not store this turn.';

/** The props of `ChatPage`. */
export interface ChatPageProps {
  /** The conversation of the surface, or null when it has none yet. */
  conversation: ConversationDto | null;
  /** The stored turns of the conversation, newest last. */
  messages: ChatMessageDto[];
  /**
   * Send one message into the conversation. The route file defines the
   * handle and passes it as this callback prop.
   */
  onSend$: QRL<(input: SendMessageInput) => Promise<ApiResult<ChatReplyDto>>>;
}

/** The loader data of the page. The callback props are excluded. */
export type ChatPageData = Omit<ChatPageProps, 'onSend$'>;

export const ChatPage = component$<ChatPageProps>((props) => {
  const status = useContext(appStatusContext);
  const rows = useSignal<ChatRow[]>(props.messages.map(toChatRow));
  const conversation = useSignal<ConversationDto | null>(props.conversation);
  const pending = useSignal(false);
  const failure = useSignal<string | null>(null);
  const notice = useSignal<string | null>(null);
  const lastText = useSignal('');

  const auraState: AuraState = pending.value
    ? 'resolving'
    : toAuraState(status.value);

  const title = conversation.value ? toTitle(conversation.value.title) : null;
  const updatedLabel = conversation.value
    ? `${formatDay(conversation.value.updatedAt)}  ·  ${formatClock(conversation.value.updatedAt)}`
    : null;

  const handleSend = $(async (text: string) => {
    const optimistic = toPendingRow(text, new Date());
    lastText.value = text;
    failure.value = null;
    notice.value = null;
    pending.value = true;
    rows.value = [...rows.value, optimistic];

    const result = await props.onSend$({
      text,
      conversationId: conversation.value?.id ?? null,
    });

    pending.value = false;

    if (result.failed) {
      rows.value = rows.value.filter((row) => row.id !== optimistic.id);
      failure.value = result.message;
      return;
    }

    // The first message of a surface starts the conversation.
    if (result.data.conversation) {
      conversation.value = result.data.conversation;
    }
    if (!result.data.stored) {
      notice.value = NOT_STORED_NOTICE;
    }

    rows.value = [
      ...rows.value.filter((row) => row.id !== optimistic.id),
      toChatRow(result.data.user),
      toChatRow(result.data.reply),
    ];
  });

  const handleRetry = $(async () => {
    const text = lastText.value;
    if (text.length === 0) {
      return;
    }
    await handleSend(text);
  });

  return (
    <Box class="flex min-h-0 flex-1 flex-col-reverse gap-3 p-3 lg:flex-row lg:p-4">
      {/* The stage: the aura in the middle, the composer at the bottom. */}
      <Stack
        gap="md"
        align="center"
        class="min-h-0 flex-1 overflow-y-auto"
        ariaLabel="Stage"
      >
        <AuraStageSection
          state={auraState}
          turns={status.value.turns}
          outcome={status.value.outcome}
        />
        {/* The stages of the running turn sit right above the field. */}
        <Stack gap="sm" class="w-full max-w-xl">
          {pending.value ? <LiveTurnSection live={status.value.live} /> : null}
          <ChatComposerSection pending={pending.value} onSend$={handleSend} />
        </Stack>
      </Stack>

      {/* The transcript panel: its own scroll region on the right. */}
      <Stack
        gap="sm"
        class="min-h-0 w-full flex-1 lg:w-[380px] lg:flex-none xl:w-[440px]"
      >
        <ConversationHeaderSection title={title} updatedLabel={updatedLabel} />
        <ChatTranscriptSection
          rows={rows.value}
          error={failure.value}
          notice={notice.value}
          onRetry$={lastText.value.length > 0 ? handleRetry : undefined}
        />
      </Stack>
    </Box>
  );
});
