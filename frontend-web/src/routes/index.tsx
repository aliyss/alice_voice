/**
 * The home route.
 *
 * The loader reads the most recent conversation with its full history, so
 * the surface continues where the user stopped. The query `new=1` starts
 * an empty surface instead, and the first message creates the conversation.
 * The default component defines the send handle and renders the page.
 */
import type { DocumentHead } from '@builder.io/qwik-city';

import type { ChatPageData } from '~/components/pages/chat-page';

import type { ConversationDetailDto, ConversationListDto } from '~/types/dto';

import type { SendMessageInput } from '~/schemas/chat';

import { $, component$ } from '@builder.io/qwik';
import { routeLoader$ } from '@builder.io/qwik-city';

import { ChatPage } from '~/components/pages/chat-page';

import { backendGet } from '~/lib/backend-client';

import { sendMessage } from '~/api/chat';

/** The path of the conversation list. The list is ordered, so one item is the latest. */
const LATEST_CONVERSATION_PATH = '/api/v1/conversations?limit=1';

/** The surface of a user who has no conversation yet. */
const EMPTY_SURFACE: ChatPageData = { conversation: null, messages: [] };

/** Read the most recent conversation, or an empty surface. */
export const useChatSurface = routeLoader$(
  async (event): Promise<ChatPageData> => {
    if (event.url.searchParams.get('new') === '1') {
      return EMPTY_SURFACE;
    }

    try {
      const list = await backendGet<ConversationListDto>(
        LATEST_CONVERSATION_PATH,
      );
      const latest = list.items.at(0);
      if (!latest) {
        return EMPTY_SURFACE;
      }
      const detail = await backendGet<ConversationDetailDto>(
        `/api/v1/conversations/${latest.id}`,
      );
      return { conversation: detail.conversation, messages: detail.messages };
    } catch {
      return EMPTY_SURFACE;
    }
  },
);

export default component$(() => {
  const surface = useChatSurface();
  const handleSend = $((input: SendMessageInput) => sendMessage(input));

  return (
    <ChatPage
      conversation={surface.value.conversation}
      messages={surface.value.messages}
      onSend$={handleSend}
    />
  );
});

export const head: DocumentHead = {
  title: 'Alice Voice',
  meta: [
    {
      name: 'description',
      content: 'The HUD of the Alice Voice listening service.',
    },
  ],
};
