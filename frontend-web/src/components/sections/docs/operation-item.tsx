/**
 * `OperationItem` is one endpoint of the API reference.
 *
 * The row is collapsable and shows the method, the path, the parameters,
 * the body, the responses, the curl snippet and the Try it out flow. One
 * component renders one endpoint, so the docs page stays under the size
 * limit.
 */
import type { OpEntry } from '~/utils/docs';

import { $, component$, useSignal } from '@builder.io/qwik';

import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Button } from '~/components/ui/button';
import { CodeEditor } from '~/components/ui/code-editor';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { TextInput } from '~/components/ui/text-input';

import { exampleFor, methodLabel, methodTone, schemaLabel } from '~/utils/docs';

interface OperationItemProps {
  /** The entry to render. */
  entry: OpEntry;
  /** The base URL the Try it out flow calls. */
  serverUrl: string;
}

export const OperationItem = component$<OperationItemProps>((props) => {
  const open = useSignal(false);
  const tryIt = useSignal(false);
  const executing = useSignal(false);
  const bodyValue = useSignal('');
  const paramValues = useSignal<Record<string, string>>({});
  const response = useSignal<null | {
    status: number;
    statusText: string;
    body: string;
    headers: Record<string, string>;
    error?: string;
  }>(null);

  const { path, method, op } = props.entry;
  const hasBody = !!op.requestBody;
  const exampleBody = exampleFor(
    Object.values(op.requestBody?.content ?? {})[0]?.schema,
  );
  const paramList = op.parameters ?? [];
  const entryPath = path;
  const entryMethod = method;
  const serverBase = props.serverUrl;

  const toggleOpen$ = $(() => {
    open.value = !open.value;
  });

  const toggleTry$ = $(() => {
    tryIt.value = !tryIt.value;
    if (tryIt.value && hasBody) bodyValue.value = exampleBody;
    if (tryIt.value) open.value = true;
  });

  const handleExecute$ = $(async () => {
    executing.value = true;
    response.value = null;
    try {
      let url = serverBase + entryPath;
      for (const p of paramList) {
        if (p.in === 'path') {
          const v = paramValues.value[p.name] ?? '';
          url = url.replace(`{${p.name}}`, encodeURIComponent(v));
        }
      }
      const qs = new URLSearchParams();
      for (const p of paramList) {
        if (p.in === 'query') {
          const v = paramValues.value[p.name];
          if (v) qs.set(p.name, v);
        }
      }
      if (qs.toString()) url += `?${qs.toString()}`;
      const headers: Record<string, string> = {};
      if (hasBody) headers['content-type'] = 'application/json';
      const res = await fetch(url, {
        method: entryMethod.toUpperCase(),
        headers,
        body: hasBody && bodyValue.value ? bodyValue.value : undefined,
      });
      const raw = await res.text();
      let pretty = raw;
      try {
        pretty = JSON.stringify(JSON.parse(raw), null, 2);
      } catch {
        // keep raw when not JSON
      }
      response.value = {
        status: res.status,
        statusText: res.statusText,
        body: pretty,
        headers: Object.fromEntries(res.headers.entries()),
      };
    } catch (e) {
      response.value = {
        status: 0,
        statusText: 'fetch failed',
        body: String(e),
        headers: {},
        error: String(e),
      };
    }
    executing.value = false;
  });

  const curl = (() => {
    let url = serverBase + entryPath;
    for (const p of paramList) {
      if (p.in === 'path')
        url = url.replace(
          `{${p.name}}`,
          paramValues.value[p.name] || `:${p.name}`,
        );
    }
    const qs = new URLSearchParams();
    for (const p of paramList)
      if (p.in === 'query' && paramValues.value[p.name])
        qs.set(p.name, paramValues.value[p.name]!);
    if (qs.toString()) url += `?${qs}`;
    const body =
      hasBody && bodyValue.value
        ? ` -H 'content-type: application/json' -d '${bodyValue.value.replace(/'/g, "'\\''").slice(0, 20000)}'`
        : '';
    return `curl -X ${entryMethod.toUpperCase()} '${url}'${body}`;
  })();

  return (
    <Box class="border-b border-ds-line-ultra last:border-b-0">
      <Box class="flex w-full items-center gap-2 px-3 py-2">
        <button
          type="button"
          aria-expanded={open.value}
          onClick$={toggleOpen$}
          class="flex min-w-0 flex-1 items-center gap-2 overflow-hidden text-left"
        >
          <Badge
            tone={methodTone(entryMethod)}
            label={methodLabel(entryMethod)}
          />
          <Text
            size="micro"
            mono
            weight="medium"
            class="min-w-0 flex-1 truncate text-left"
          >
            {entryPath}
          </Text>
          {op.summary ? (
            <Text
              size="micro"
              tone="muted"
              class="hidden min-w-0 flex-1 truncate text-left lg:block"
            >
              {op.summary}
            </Text>
          ) : null}
        </button>
        <Button
          variant="quiet"
          size="sm"
          onClick$={toggleTry$}
          class="shrink-0"
        >
          {tryIt.value ? 'Cancel' : 'Try it out'}
        </Button>
        <button
          type="button"
          aria-expanded={open.value}
          onClick$={toggleOpen$}
          class="flex size-6 shrink-0 items-center justify-center rounded-ds-sm hover:bg-ds-surface"
          aria-label={open.value ? 'Collapse' : 'Expand'}
        >
          <span
            aria-hidden="true"
            class={`block size-1.5 shrink-0 border-r border-b border-current text-ds-text-faint transition-transform duration-150 ${open.value ? 'rotate-45' : '-rotate-45'}`}
          />
        </button>
      </Box>

      {open.value ? (
        <Box class="border-t border-ds-line-ultra bg-ds-surface/20 p-4">
          <Stack gap="sm">
            {op.summary ? (
              <Text size="body" weight="medium" block>
                {op.summary}
              </Text>
            ) : null}
            {op.description ? (
              <Text size="micro" tone="muted" block class="whitespace-pre-wrap">
                {op.description}
              </Text>
            ) : null}
            {op.operationId ? (
              <Text size="micro" mono tone="faint" block>
                operationId: {op.operationId}
              </Text>
            ) : null}

            {paramList.length > 0 ? (
              <Box class="overflow-hidden rounded-ds-sm border border-ds-line-ultra">
                <Box class="border-b border-ds-line-ultra bg-ds-surface-sunken px-3 py-2">
                  <Text size="micro" tone="faint">
                    Parameters
                  </Text>
                </Box>
                <table class="w-full border-collapse text-left">
                  <thead>
                    <tr class="border-b border-ds-line-ultra bg-ds-surface">
                      <th class="px-3 py-1.5">
                        <Text size="micro" tone="faint" weight="medium">
                          Name
                        </Text>
                      </th>
                      <th class="px-3 py-1.5">
                        <Text size="micro" tone="faint" weight="medium">
                          In
                        </Text>
                      </th>
                      <th class="px-3 py-1.5">
                        <Text size="micro" tone="faint" weight="medium">
                          Type
                        </Text>
                      </th>
                      <th class="px-3 py-1.5">
                        <Text size="micro" tone="faint" weight="medium">
                          Required
                        </Text>
                      </th>
                    </tr>
                  </thead>
                  <tbody>
                    {paramList.map((param) => {
                      const paramName = param.name;
                      const paramDesc = param.description;
                      const paramIn = param.in;
                      const paramRequired = param.required;
                      const paramSchema = param.schema;
                      return (
                        <tr
                          key={`${paramName}:${paramIn}`}
                          class="border-b border-ds-line-ultra last:border-b-0"
                        >
                          <td class="px-3 py-2">
                            <Text size="micro" mono>
                              {paramName}
                            </Text>
                            {paramDesc ? (
                              <Text size="micro" tone="faint" block>
                                {paramDesc}
                              </Text>
                            ) : null}
                            {tryIt.value ? (
                              <Box class="mt-1">
                                <TextInput
                                  surface="field"
                                  ariaLabel={paramName}
                                  placeholder={paramName}
                                  value={paramValues.value[paramName] ?? ''}
                                  onInput$={$((e: Event) => {
                                    const next = (e.target as HTMLInputElement)
                                      .value;
                                    paramValues.value = {
                                      ...paramValues.value,
                                      [paramName]: next,
                                    };
                                  })}
                                />
                              </Box>
                            ) : null}
                          </td>
                          <td class="px-3 py-2">
                            <Badge tone="neutral" label={paramIn} />
                          </td>
                          <td class="px-3 py-2">
                            <Text size="micro" mono tone="muted">
                              {schemaLabel(paramSchema)}
                            </Text>
                          </td>
                          <td class="px-3 py-2">
                            <Text
                              size="micro"
                              tone={paramRequired ? 'accent' : 'faint'}
                            >
                              {paramRequired ? 'yes' : '—'}
                            </Text>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </Box>
            ) : null}

            {op.requestBody ? (
              <Box class="overflow-hidden rounded-ds-sm border border-ds-line-ultra">
                <Box class="flex items-center gap-2 border-b border-ds-line-ultra bg-ds-surface-sunken px-3 py-2">
                  <Text size="micro" tone="faint">
                    Body
                  </Text>
                  {op.requestBody.required ? (
                    <Badge tone="warn" label="required" />
                  ) : null}
                  <Text size="micro" mono tone="faint" class="ml-auto">
                    {Object.keys(op.requestBody.content ?? {}).join(', ') ||
                      'application/json'}{' '}
                    →{' '}
                    {schemaLabel(
                      Object.values(op.requestBody.content ?? {})[0]?.schema,
                    )}
                  </Text>
                </Box>
                <Box class="p-3">
                  {!tryIt.value ? (
                    <CodeEditor code={exampleBody} language="json" readOnly />
                  ) : (
                    <CodeEditor
                      code={bodyValue.value}
                      language="json"
                      onChange$={$((v: string) => {
                        bodyValue.value = v;
                      })}
                    />
                  )}
                </Box>
              </Box>
            ) : null}

            {op.responses ? (
              <Box class="overflow-hidden rounded-ds-sm border border-ds-line-ultra">
                <Box class="border-b border-ds-line-ultra bg-ds-surface-sunken px-3 py-2">
                  <Text size="micro" tone="faint">
                    Responses
                  </Text>
                </Box>
                <Stack gap="none">
                  {Object.entries(op.responses).map(([code, res]) => {
                    const firstMedia = Object.values(res.content ?? {})[0] as
                      | { schema?: { $ref?: string; type?: unknown } }
                      | undefined;
                    const ex = firstMedia && firstMedia.schema ? exampleFor(firstMedia.schema) : null;
                    return (
                      <Box
                        key={code}
                        class="border-b border-ds-line-ultra last:border-b-0 p-3"
                      >
                        <Stack gap="xs">
                          <Stack direction="row" gap="sm" align="center" wrap>
                            <Badge
                              tone={
                                code.startsWith('2')
                                  ? 'ok'
                                  : code.startsWith('4')
                                    ? 'warn'
                                    : code.startsWith('5')
                                      ? 'error'
                                      : 'neutral'
                              }
                              label={code}
                            />
                            <Text size="micro" tone="muted" class="flex-1">
                              {res.description}
                            </Text>
                            {res.content ? (
                              <Text size="micro" mono tone="faint">
                                {Object.entries(res.content)
                                  .map(
                                    ([mime, media]) =>
                                      `${mime} → ${schemaLabel(media.schema)}`,
                                  )
                                  .join(' · ')}
                              </Text>
                            ) : null}
                          </Stack>
                          {ex ? (
                            <CodeEditor code={ex} language="json" readOnly />
                          ) : null}
                        </Stack>
                      </Box>
                    );
                  })}
                </Stack>
              </Box>
            ) : null}

            <Box class="flex flex-wrap items-center gap-2">
              {tryIt.value ? (
                <Button
                  variant="solid"
                  size="sm"
                  disabled={executing.value}
                  onClick$={handleExecute$}
                >
                  {executing.value ? 'Executing…' : 'Execute'}
                </Button>
              ) : null}
              <Box class="flex-1">
                <CodeEditor code={curl} language="bash" readOnly />
              </Box>
            </Box>

            {response.value ? (
              <Box class="overflow-hidden rounded-ds-sm border border-ds-line-ultra">
                <Box class="border-b border-ds-line-ultra bg-ds-surface-sunken px-3 py-2">
                  <Stack direction="row" gap="sm" align="center">
                    <Text size="micro" tone="faint">
                      Server response
                    </Text>
                    <Badge
                      tone={
                        String(response.value.status).startsWith('2')
                          ? 'ok'
                          : 'error'
                      }
                      label={`${response.value.status} ${response.value.statusText}`}
                    />
                  </Stack>
                </Box>
                <Box class="p-3">
                  <CodeEditor
                    code={response.value.body.slice(0, 20000)}
                    language="json"
                    readOnly
                  />
                </Box>
              </Box>
            ) : null}
          </Stack>
        </Box>
      ) : null}
    </Box>
  );
});
