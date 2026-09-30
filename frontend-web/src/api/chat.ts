/**
 * The chat endpoint of the daemon.
 *
 * A `server$` function runs on the server and calls the backend REST API.
 * The browser never calls the daemon directly, so the API host stays
 * private and the client keeps one integration point.
 */
import type { ApiResult } from '~/types/bridge';
import type { ChatReplyDto, ConversationDetailDto } from '~/types/dto';

import type { SendMessageInput } from '~/schemas/chat';

import { server$ } from '@builder.io/qwik-city';

import { safeParse } from 'valibot';

import { BackendError, backendGet, backendPost } from '~/lib/backend-client';

import { sendMessageSchema } from '~/schemas/chat';

/** The path of the chat endpoint. */
const CHAT_PATH = '/api/v1/chat';

/** The path of one conversation. */
const CONVERSATION_PATH = '/api/v1/conversations';

/**
 * The time the client waits for one handled message, in milliseconds.
 *
 * A turn is not one round trip: the daemon reads the intent from the
 * local model and then runs the command of that intent. Both have a
 * timeout of their own on the daemon, so this is their sum with room.
 */
const CHAT_TIMEOUT_MS = 120_000;

/** The reply of the send message function. */
export type SendMessageResult = ApiResult<ChatReplyDto>;

/** The reply of the read conversation function. */
export type ConversationResult = ApiResult<ConversationDetailDto>;

/** Turn a thrown error into a message for the user. */
function toFailureMessage(error: unknown): string {
  if (error instanceof BackendError) {
    return error.message;
  }
  return 'The message did not reach the daemon.';
}

/**
 * Send one message into a conversation and return the stored turn.
 *
 * The daemon starts a conversation when the input carries no identifier.
 * The result carries the failure instead of throwing, so the composer can
 * show the design system alert component.
 */
export const sendMessage = server$(
  async (input: SendMessageInput): Promise<SendMessageResult> => {
    const parsed = safeParse(sendMessageSchema, input);
    if (!parsed.success) {
      return { failed: true, message: parsed.issues[0].message };
    }

    try {
      const reply = await backendPost<ChatReplyDto>(CHAT_PATH, parsed.output, {
        timeoutMs: CHAT_TIMEOUT_MS,
      });
      return { failed: false, data: reply };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);

/**
 * Read one conversation with its full message history.
 *
 * The chat page reads a conversation again after a script runs, so the
 * turn the daemon stored joins the transcript without a reload of the
 * page.
 */
export const getConversation = server$(
  async (id: string): Promise<ConversationResult> => {
    try {
      const detail = await backendGet<ConversationDetailDto>(
        `${CONVERSATION_PATH}/${id}`,
      );
      return { failed: false, data: detail };
    } catch (error) {
      return { failed: true, message: toFailureMessage(error) };
    }
  },
);
