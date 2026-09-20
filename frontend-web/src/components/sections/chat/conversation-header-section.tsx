/**
 * `ConversationHeaderSection` names the conversation of the chat surface.
 *
 * It shows the title the daemon generated from the first message and the
 * time of the last stored turn. A surface without a conversation shows the
 * placeholder title instead.
 */
import { component$ } from '@builder.io/qwik';

import { PageHeader } from '~/components/ui/page-header';

/** The props of `ConversationHeaderSection`. */
export interface ConversationHeaderSectionProps {
  /** The title of the conversation, or null when it has none yet. */
  title: string | null;
  /** The local day and time of the last change, or null. */
  updatedLabel: string | null;
}

export const ConversationHeaderSection =
  component$<ConversationHeaderSectionProps>((props) => {
    return (
      <PageHeader
        title={props.title ?? 'New conversation'}
        description="The daemon keeps one message history for each conversation."
        meta={props.updatedLabel ?? undefined}
      />
    );
  });
