/**
 * `ConversationListSection` holds the conversation list.
 *
 * One row is one conversation. A row links to the chat surface of that
 * conversation, so the list is the door to every stored history.
 */
import type { ConversationRow } from '~/utils/conversation';

import { component$ } from '@builder.io/qwik';
import { Link } from '@builder.io/qwik-city';

import { Card } from '~/components/ui/card';
import { EmptyState } from '~/components/ui/empty-state';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The props of `ConversationListSection`. */
export interface ConversationListSectionProps {
  /** The conversations, most recently updated first. */
  rows: ConversationRow[];
}

export const ConversationListSection = component$<ConversationListSectionProps>(
  (props) => {
    return (
      <Card label="Conversations" class="w-full">
        {props.rows.length === 0 ? (
          <EmptyState
            title="No conversations yet"
            description="Send a message on the conversation page and the daemon starts one here."
          />
        ) : (
          <Stack gap="sm">
            {props.rows.map((row) => (
              <Link
                key={row.id}
                href={`/conversations/${row.id}`}
                class="flex w-full flex-wrap items-center justify-between gap-2 rounded-ds-sm border border-ds-line bg-ds-surface px-4 py-3 no-underline transition-colors duration-150 ease-out hover:border-ds-accent-strong hover:bg-ds-surface-raised"
              >
                <Text size="body" tone="default" weight="medium">
                  {row.title}
                </Text>
                <Text size="micro" tone="faint">
                  {`${row.updatedDay}  ·  ${row.updatedClock}`}
                </Text>
              </Link>
            ))}
          </Stack>
        )}
      </Card>
    );
  },
);
