/**
 * Pure helpers for the docs page.
 *
 * The docs page reads the live OpenAPI spec and groups it by tag. The
 * helpers below have no side effects and no backend fetch.
 */
import type { OpenApiOperationDto } from '~/types/openapi';

/** One operation with its path and method. */
export interface OpEntry {
  /** The path, for example `/api/v1/chat`. */
  path: string;
  /** The HTTP method in lower case. */
  method: string;
  /** The operation. */
  op: OpenApiOperationDto;
}

const METHOD_TONE: Record<
  string,
  'ok' | 'accent' | 'warn' | 'error' | 'neutral'
> = {
  get: 'ok',
  post: 'accent',
  put: 'warn',
  delete: 'error',
  patch: 'neutral',
};

const METHOD_LABEL: Record<string, string> = {
  get: 'GET',
  post: 'POST',
  put: 'PUT',
  delete: 'DELETE',
  patch: 'PATCH',
};

/** The tone for a method badge. */
export function methodTone(
  method: string,
): 'ok' | 'accent' | 'warn' | 'error' | 'neutral' {
  return METHOD_TONE[method] ?? 'neutral';
}

/** The label for a method badge. */
export function methodLabel(method: string): string {
  return METHOD_LABEL[method] ?? method.toUpperCase();
}

/** Remove the `#/components/schemas/` prefix from a `$ref`. */
export function formatRef(ref?: string): string {
  if (!ref) return '';
  return ref.replace('#/components/schemas/', '');
}

/** The short type label for a schema. */
export function schemaLabel(schema?: {
  $ref?: string;
  type?: string | string[];
}): string {
  if (!schema) return '—';
  if (schema.$ref) return formatRef(schema.$ref);
  if (Array.isArray(schema.type)) return schema.type.join(' | ');
  if (typeof schema.type === 'string') return schema.type;
  return 'object';
}

/** Map a raw tag from the spec to the display name. */
export function prettyTag(tag: string): string {
  if (tag.includes('::')) {
    if (tag.includes('conversations')) return 'conversations';
    if (tag.includes('intents')) return 'intents';
    if (tag.includes('settings')) return 'settings';
    if (tag.includes('resolver')) return 'resolver';
    if (tag.includes('rest')) return 'system';
    return 'system';
  }
  return tag;
}

/** Group paths by tag. */
export function groupByTag(
  paths: Record<string, Record<string, OpenApiOperationDto>>,
): Array<[string, OpEntry[]]> {
  const map = new Map<string, OpEntry[]>();
  for (const [path, methods] of Object.entries(paths)) {
    for (const [method, op] of Object.entries(methods)) {
      const rawTags =
        op.tags && op.tags.length > 0
          ? op.tags
          : [path.split('/')[3] ?? 'default'];
      for (const raw of rawTags) {
        const tag = prettyTag(raw);
        const list = map.get(tag) ?? [];
        list.push({ path, method, op });
        map.set(tag, list);
      }
    }
  }
  const entries = Array.from(map.entries());
  entries.sort(([a], [b]) => a.localeCompare(b));
  for (const [, ops] of entries) {
    ops.sort(
      (a, b) =>
        a.path.localeCompare(b.path) || a.method.localeCompare(b.method),
    );
  }
  return entries;
}

/** Example JSON for a schema. */
export function exampleFor(schema?: { $ref?: string; type?: unknown }): string {
  if (!schema) return '{}';
  if (schema.$ref) {
    const name = formatRef(schema.$ref);
    const hardcoded: Record<string, unknown> = {
      ChatRequestDto: { text: 'hello', conversationId: null },
      ChatReplyDto: {
        stored: true,
        conversation: {
          id: '00000000-0000-0000-0000-000000000000',
          title: 'hello',
          createdAt: '2024-01-01T00:00:00.000Z',
          updatedAt: '2024-01-01T00:00:00.000Z',
        },
        user: {
          id: '00000000-0000-0000-0000-000000000000',
          conversationId: '00000000-0000-0000-0000-000000000000',
          role: 'user',
          text: 'hello',
          createdAt: '2024-01-01T00:00:00.000Z',
          intentId: null,
          intentName: null,
          confidence: null,
          meta: null,
        },
        reply: {
          id: '00000000-0000-0000-0000-000000000000',
          conversationId: '00000000-0000-0000-0000-000000000000',
          role: 'assistant',
          text: 'hi there',
          createdAt: '2024-01-01T00:00:00.000Z',
          intentId: null,
          intentName: null,
          confidence: null,
          meta: null,
        },
      },
      HealthDto: { status: 'ok', version: '0.1.0' },
      StatusDto: {
        state: 'Idle',
        since: '2024-01-01T00:00:00.000Z',
        version: '0.1.0',
      },
      ConversationListDto: {
        items: [
          {
            id: '00000000-0000-0000-0000-000000000000',
            title: 'hello',
            createdAt: '2024-01-01T00:00:00.000Z',
            updatedAt: '2024-01-01T00:00:00.000Z',
          },
        ],
      },
      IntentWriteDto: {
        name: 'my intent',
        description: 'does a thing',
        command: 'echo hi',
        entities: [],
        examples: [],
      },
      SettingsUpdateDto: { queueEnabled: true },
      ResolverPreviewRequestDto: { text: 'open firefox' },
      ScriptPreviewWriteDto: { script: 'echo hi' },
    };
    const found = hardcoded[name];
    if (found) return JSON.stringify(found, null, 2);
    return JSON.stringify({}, null, 2);
  }
  return JSON.stringify({}, null, 2);
}
