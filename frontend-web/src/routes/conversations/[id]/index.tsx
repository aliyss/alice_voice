/**
 * The conversation detail route.
 *
 * The loader reads one conversation with its full history. A missing
 * conversation answers with the not found page. The default component
 * defines the send handle and renders the chat page.
 */
import type { DocumentHead } from '@builder.io/qwik-city';

import type { ChatPageData } from '~/components/pages/chat-page';

import type { ConversationDetailDto } from '~/types/dto';

import type { SendMessageInput } from '~/schemas/chat';

import { $, component$ } from '@builder.io/qwik';
import { routeLoader$ } from '@builder.io/qwik-city';

import { ChatPage } from '~/components/pages/chat-page';

import { backendGet } from '~/lib/backend-client';

import { getConversation, sendMessage } from '~/api/chat';
import { approveScript, denyScript, getScript } from '~/api/scripts';

/** Read one conversation with its full message history. */
export const useConversation = routeLoader$(
  async (event): Promise<ChatPageData> => {
    const id = event.params.id;
    try {
      const detail = await backendGet<ConversationDetailDto>(
        `/api/v1/conversations/${id}`,
      );
      return { conversation: detail.conversation, messages: detail.messages };
    } catch {
      throw event.error(404, 'Conversation not found');
    }
  },
);

export default component$(() => {
  const conversation = useConversation();
  const handleSend = $((input: SendMessageInput) => sendMessage(input));
  const handleReadScript = $((id: string) => getScript(id));
  const handleApproveScript = $((id: string) => approveScript(id));
  const handleDenyScript = $((id: string) => denyScript(id));
  const handleReloadConversation = $((id: string) => getConversation(id));

  return (
    <ChatPage
      conversation={conversation.value.conversation}
      messages={conversation.value.messages}
      onSend$={handleSend}
      onReadScript$={handleReadScript}
      onApproveScript$={handleApproveScript}
      onDenyScript$={handleDenyScript}
      onReloadConversation$={handleReloadConversation}
    />
  );
});

export const head: DocumentHead = {
  title: 'Conversation · Alice Voice',
  meta: [
    {
      name: 'description',
      content: 'One stored conversation of the Alice Voice daemon.',
    },
  ],
};
