/**
 * The conversation list route.
 *
 * The loader reads every stored conversation. The default component renders
 * the page with the rows.
 */
import type { DocumentHead } from '@builder.io/qwik-city';

import type { ConversationsPageData } from '~/components/pages/conversations-page';

import type { ConversationListDto } from '~/types/dto';

import { component$ } from '@builder.io/qwik';
import { routeLoader$ } from '@builder.io/qwik-city';

import { ConversationsPage } from '~/components/pages/conversations-page';

import { backendGet } from '~/lib/backend-client';

import { toConversationRow } from '~/utils/conversation';

/** The path of the conversation list endpoint. */
const CONVERSATIONS_PATH = '/api/v1/conversations';

/** Read every stored conversation. */
export const useConversations = routeLoader$(
  async (): Promise<ConversationsPageData> => {
    try {
      const list = await backendGet<ConversationListDto>(CONVERSATIONS_PATH);
      return { conversations: list.items.map(toConversationRow) };
    } catch {
      return { conversations: [] };
    }
  },
);

export default component$(() => {
  const list = useConversations();

  return <ConversationsPage conversations={list.value.conversations} />;
});

export const head: DocumentHead = {
  title: 'Conversations · Alice Voice',
  meta: [
    {
      name: 'description',
      content: 'Every message history the Alice Voice daemon stored.',
    },
  ],
};
