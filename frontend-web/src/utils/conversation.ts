/**
 * The view model of the conversation list.
 *
 * A DTO carries the raw backend fields. A row carries the fields the list
 * renders, so the view never formats a date.
 */
import type { ConversationDto } from '~/types/dto';

import { formatClock } from '~/utils/chat';

/** One rendered conversation of the list. */
export interface ConversationRow {
  /** The stable conversation identifier. */
  id: string;
  /** The title the daemon generated from the first message. */
  title: string;
  /** The local day of the last change as `YYYY-MM-DD`. */
  updatedDay: string;
  /** The local time of the last change as `HH:MM:SS`. */
  updatedClock: string;
}

/** The placeholder of an unreadable date. */
const UNKNOWN_DAY = '----------';

/** The title of a conversation that has no readable title. */
const UNTITLED_CONVERSATION = 'Untitled conversation';

/** Pad one number to two digits. */
function pad(value: number): string {
  return String(value).padStart(2, '0');
}

/** Format a timestamp as the local day `YYYY-MM-DD`. */
export function formatDay(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return UNKNOWN_DAY;
  }
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** Read the title of a conversation, or a placeholder when it is empty. */
export function toTitle(title: string): string {
  const trimmed = title.trim();
  return trimmed.length > 0 ? trimmed : UNTITLED_CONVERSATION;
}

/** Map one conversation DTO to one list row. */
export function toConversationRow(dto: ConversationDto): ConversationRow {
  return {
    id: dto.id,
    title: toTitle(dto.title),
    updatedDay: formatDay(dto.updatedAt),
    updatedClock: formatClock(dto.updatedAt),
  };
}
