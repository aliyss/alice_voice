/**
 * `ConversationsPage` is the conversation surface of the daemon.
 *
 * The page shows every stored conversation and offers the start of a new
 * one. The route file reads the list and passes it as props.
 */
import type { ConversationRow } from '~/utils/conversation';

import { component$ } from '@builder.io/qwik';

import { ConversationListSection } from '~/components/sections/conversations/conversation-list-section';
import { Link } from '~/components/ui/link';
import { PageHeader } from '~/components/ui/page-header';
import { Stack } from '~/components/ui/stack';

/** The props of `ConversationsPage`. */
export interface ConversationsPageProps {
  /** The conversations, most recently updated first. */
  conversations: ConversationRow[];
}

/** The loader data of the page. The page has no callback props. */
export type ConversationsPageData = ConversationsPageProps;

export const ConversationsPage = component$<ConversationsPageProps>((props) => {
  return (
    <Stack gap="lg" class="mx-auto w-full max-w-3xl px-8 py-6">
      <PageHeader
        title="Conversations"
        description="Every message history the daemon stored, most recent first."
      >
        <Link href="/?new=1" tone="accent">
          New conversation
        </Link>
      </PageHeader>
      <ConversationListSection rows={props.conversations} />
    </Stack>
  );
});
