import type { ConversationDto } from '~/types/dto';

import { describe, expect, it } from 'vitest';

import { formatDay, toConversationRow, toTitle } from '~/utils/conversation';

/** Build one conversation DTO with a readable timestamp. */
function conversation(
  overrides: Partial<ConversationDto> = {},
): ConversationDto {
  return {
    id: '11111111-2222-3333-4444-555555555555',
    title: 'Turn on the light',
    createdAt: '2026-09-19T10:00:00.000Z',
    updatedAt: '2026-09-19T10:00:00.000Z',
    ...overrides,
  };
}

describe('formatDay', () => {
  it('formats a timestamp as a padded local day', () => {
    expect(formatDay('2026-01-05T12:00:00.000Z')).toBe('2026-01-05');
  });

  it('returns a placeholder for an unreadable timestamp', () => {
    expect(formatDay('not a date')).toBe('----------');
  });
});

describe('toTitle', () => {
  it('keeps a readable title', () => {
    expect(toTitle('  Set a timer ')).toBe('Set a timer');
  });

  it('names a conversation that has no title', () => {
    expect(toTitle('   ')).toBe('Untitled conversation');
  });
});

describe('toConversationRow', () => {
  it('maps the DTO fields to the row fields', () => {
    const row = toConversationRow(conversation());

    expect(row.id).toBe('11111111-2222-3333-4444-555555555555');
    expect(row.title).toBe('Turn on the light');
    expect(row.updatedDay).toBe(formatDay('2026-09-19T10:00:00.000Z'));
    expect(row.updatedClock).toMatch(/^\d{2}:\d{2}:\d{2}$/);
  });
});
