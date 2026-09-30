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

import type { ConversationResult } from '~/api/chat';
import type { ScriptResult } from '~/api/scripts';

import type { SendMessageInput } from '~/schemas/chat';

import type { ChatRow } from '~/utils/chat';
import type { AuraState, ProposedScript } from '~/utils/status';

import {
  $,
  component$,
  useContext,
  useSignal,
  useTask$,
} from '@builder.io/qwik';

import { AuraStageSection } from '~/components/sections/chat/aura-stage-section';
import { ChatComposerSection } from '~/components/sections/chat/chat-composer-section';
import { ChatTranscriptSection } from '~/components/sections/chat/chat-transcript-section';
import { ConversationHeaderSection } from '~/components/sections/chat/conversation-header-section';
import { LiveTurnSection } from '~/components/sections/chat/live-turn-section';
import { ScriptApprovalSection } from '~/components/sections/chat/script-approval-section';
import { Box } from '~/components/ui/box';
import { Stack } from '~/components/ui/stack';

import { appStatusContext } from '~/context/app-status.context';

import {
  formatClock,
  formatMemoryFact,
  toChatRow,
  toPendingRow,
} from '~/utils/chat';
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
  /** Read the stored script of a turn, so a reload still shows it. */
  onReadScript$: QRL<(id: string) => Promise<ScriptResult>>;
  /** Approve one script and let the daemon run it. */
  onApproveScript$: QRL<(id: string) => Promise<ScriptResult>>;
  /** Deny one script, so the daemon never runs it. */
  onDenyScript$: QRL<(id: string) => Promise<ScriptResult>>;
  /** Read the conversation again, so a stored turn joins the transcript. */
  onReloadConversation$: QRL<(id: string) => Promise<ConversationResult>>;
}

/** The loader data of the page. The callback props are excluded. */
export type ChatPageData = Omit<
  ChatPageProps,
  | 'onSend$'
  | 'onReadScript$'
  | 'onApproveScript$'
  | 'onDenyScript$'
  | 'onReloadConversation$'
>;

/**
 * Pick the script the surface still has to ask about, or null.
 *
 * The script of the live turn is the one the socket just reported, and the
 * stored script is the one a reload read back. A decision hides both.
 */
function pickProposal(
  live: ProposedScript | null,
  loaded: ProposedScript | null,
  decided: string | null,
): ProposedScript | null {
  if (live && live.id !== decided) {
    return live;
  }
  return loaded && loaded.id !== decided ? loaded : null;
}

export const ChatPage = component$<ChatPageProps>((props) => {
  const status = useContext(appStatusContext);
  const rows = useSignal<ChatRow[]>(props.messages.map(toChatRow));
  const conversation = useSignal<ConversationDto | null>(props.conversation);
  const pending = useSignal(false);
  const failure = useSignal<string | null>(null);
  const notice = useSignal<string | null>(null);
  const lastText = useSignal('');
  // The context switch of the retried message, so a retry reads the turn
  // the way the user asked for it the first time.
  const lastContext = useSignal(true);
  // The decision about a script the model wrote. The daemon owns the
  // decision, and these signals fold its answer into the surface.
  const scriptPending = useSignal(false);
  const scriptError = useSignal<string | null>(null);
  const decided = useSignal<string | null>(null);
  const loaded = useSignal<ProposedScript | null>(null);
  // The message the transcript already marked, so the mark of one turn is
  // folded into the rows once.
  const marked = useSignal<string | null>(null);

  const proposal = pickProposal(
    status.value.live.script,
    loaded.value,
    decided.value,
  );
  // The live turn stays on screen after the turn ends while the daemon
  // runs an approved script, so the output of the script reads back.
  const showsLive =
    pending.value || proposal !== null || status.value.state === 'Executing';

  // The memory reads a turn after the daemon answered it, so the mark of
  // the turn arrives on its own. The page folds it into the row the memory
  // learned from, rather than reading the whole conversation again: the
  // event carries the facts, and a reader who scrolled up keeps the place.
  useTask$(({ track }) => {
    const learned = track(() => status.value.live.memory);
    if (!learned || marked.value === learned.messageId) {
      return;
    }
    // The mark is written before the rows change, so the second run of
    // this task stops on the guard above instead of marking twice.
    marked.value = learned.messageId;
    rows.value = rows.value.map((row) =>
      row.id === learned.messageId
        ? {
            ...row,
            memory: { facts: learned.facts.map(formatMemoryFact) },
          }
        : row,
    );
  });

  // A reload still shows a script the user has not decided about, so the
  // latest turn the daemon stored is read back from the store.
  useTask$(async () => {
    const scriptId = props.messages
      .map((message) => message.meta?.scriptId ?? null)
      .reverse()
      .find((id) => id !== null);
    if (!scriptId) {
      return;
    }
    const result = await props.onReadScript$(scriptId);
    if (result.failed || result.data.status !== 'pending') {
      return;
    }
    loaded.value = {
      id: result.data.id,
      summary: result.data.summary,
      script: result.data.script,
      destructiveness: result.data.destructiveness,
    };
  });

  const auraState: AuraState = pending.value
    ? 'resolving'
    : toAuraState(status.value);

  const title = conversation.value ? toTitle(conversation.value.title) : null;
  const updatedLabel = conversation.value
    ? `${formatDay(conversation.value.updatedAt)}  ·  ${formatClock(conversation.value.updatedAt)}`
    : null;

  const handleSend = $(async (text: string, context: boolean) => {
    const optimistic = toPendingRow(text, new Date());
    lastText.value = text;
    lastContext.value = context;
    failure.value = null;
    notice.value = null;
    pending.value = true;
    rows.value = [...rows.value, optimistic];

    const result = await props.onSend$({
      text,
      conversationId: conversation.value?.id ?? null,
      context,
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

  const handleApprove = $(async () => {
    const live = status.value.live.script;
    const current =
      live && live.id !== decided.value ? live : (loaded.value ?? null);
    if (!current || current.id === decided.value) {
      return;
    }
    scriptPending.value = true;
    scriptError.value = null;
    const result = await props.onApproveScript$(current.id);
    scriptPending.value = false;
    if (result.failed) {
      scriptError.value = result.message;
      return;
    }
    decided.value = current.id;
    // The daemon stored the run of the script, so the transcript reads the
    // script and its output without a reload of the page.
    if (conversation.value) {
      const refreshed = await props.onReloadConversation$(
        conversation.value.id,
      );
      if (!refreshed.failed) {
        conversation.value = refreshed.data.conversation;
        rows.value = refreshed.data.messages.map(toChatRow);
      }
    }
  });

  const handleDeny = $(async () => {
    const live = status.value.live.script;
    const current =
      live && live.id !== decided.value ? live : (loaded.value ?? null);
    if (!current || current.id === decided.value) {
      return;
    }
    scriptPending.value = true;
    scriptError.value = null;
    const result = await props.onDenyScript$(current.id);
    scriptPending.value = false;
    if (result.failed) {
      scriptError.value = result.message;
      return;
    }
    decided.value = current.id;
  });

  const handleRetry = $(async () => {
    const text = lastText.value;
    if (text.length === 0) {
      return;
    }
    await handleSend(text, lastContext.value);
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
          {showsLive ? <LiveTurnSection live={status.value.live} /> : null}
          {proposal ? (
            <ScriptApprovalSection
              script={proposal}
              pending={scriptPending.value}
              error={scriptError.value}
              onApprove$={handleApprove}
              onDeny$={handleDeny}
            />
          ) : null}
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
