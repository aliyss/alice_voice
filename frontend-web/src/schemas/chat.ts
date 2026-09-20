/**
 * The validation schema of the chat input.
 *
 * A `server$` function parses the input with this schema before it calls
 * the backend REST API.
 */
import * as v from 'valibot';

/** The largest message the composer accepts. */
const MAX_MESSAGE_LENGTH = 4000;

/** The shape of a conversation identifier. */
const CONVERSATION_ID_PATTERN =
  /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

/** One chat message with a text body and its conversation. */
export const sendMessageSchema = v.object({
  text: v.pipe(
    v.string('Type a message first.'),
    v.trim(),
    v.minLength(1, 'Type a message first.'),
    v.maxLength(MAX_MESSAGE_LENGTH, 'The message is too long.'),
  ),
  conversationId: v.nullable(
    v.pipe(
      v.string(),
      v.regex(CONVERSATION_ID_PATTERN, 'The conversation is not valid.'),
    ),
  ),
});

/** The input of the send message function. */
export type SendMessageInput = v.InferInput<typeof sendMessageSchema>;
