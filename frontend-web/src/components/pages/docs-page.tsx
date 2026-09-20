/**
 * `DocsPage` is the API reference of the daemon.
 *
 * It reads the live OpenAPI spec and renders it with the HUD tokens.
 * Tags are collapsable groups and each operation is its own collapsable
 * row with Try it out. The page delegates one operation to
 * `OperationItem` so the file stays under the size limit.
 */
import type { OpenApiDto } from '~/types/openapi';

import { component$ } from '@builder.io/qwik';

import { OperationItem } from '~/components/sections/docs/operation-item';
import { Badge } from '~/components/ui/badge';
import { Box } from '~/components/ui/box';
import { Card } from '~/components/ui/card';
import { CodeEditor } from '~/components/ui/code-editor';
import { Disclosure } from '~/components/ui/disclosure';
import { PageHeader } from '~/components/ui/page-header';
import { ScrollArea } from '~/components/ui/scroll-area';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

import { groupByTag, schemaLabel } from '~/utils/docs';

/** The props of `DocsPage`. */
export interface DocsPageProps {
  /** The live spec, or null when the daemon does not answer. */
  spec: OpenApiDto | null;
  /** The error the loader read, or null. */
  error?: string | null;
}

export const DocsPage = component$<DocsPageProps>((props) => {
  if (!props.spec) {
    return (
      <ScrollArea ariaLabel="API reference" class="flex-1">
        <Stack gap="lg" class="mx-auto w-full max-w-5xl px-8 py-6">
          <PageHeader
            title="API reference"
            description="REST and socket contract owned by the backend."
            meta="offline"
          />
          <Card
            label="The daemon does not answer"
            frame={false}
            class="!border-ds-line-faint"
          >
            <Stack gap="sm">
              <Text size="body" tone="muted" block>
                The docs page reads the live spec from{' '}
                <span class="font-ds-hud text-ds-text">
                  GET /api/docs/openapi.json
                </span>
                . The daemon did not answer.
              </Text>
              {props.error ? (
                <Box class="rounded-ds-sm border border-ds-line-ultra bg-ds-surface-sunken px-4 py-3">
                  <Text size="micro" mono tone="faint" block>
                    {props.error}
                  </Text>
                </Box>
              ) : null}
            </Stack>
          </Card>
        </Stack>
      </ScrollArea>
    );
  }

  const spec = props.spec;
  const grouped = groupByTag(spec.paths);
  const schemaCount = Object.keys(spec.components?.schemas ?? {}).length;
  const endpointCount = grouped.reduce((acc, [, ops]) => acc + ops.length, 0);
  const currentBase =
    (import.meta.env.PUBLIC_ALICE_API_URL as string | undefined) ||
    spec.servers?.[0]?.url ||
    'http://127.0.0.1:8787';
  const serverUrl = currentBase.replace(/\/$/, '');

  return (
    <ScrollArea ariaLabel="API reference" class="flex-1">
      <Stack gap="lg" class="mx-auto w-full max-w-5xl px-8 py-6">
        <PageHeader
          title="API reference"
          description={
            spec.info.description ??
            'REST and socket contract owned by the backend.'
          }
          meta={`${spec.info.title} · ${spec.info.version}`}
        />

        <Card label="Service" frame={false} class="!border-ds-line-faint">
          <Stack gap="sm">
            <Stack direction="row" gap="sm" align="center" wrap>
              <Badge tone="accent" label={spec.openapi} />
              <Text size="micro" tone="muted" mono>
                {spec.info.title}
              </Text>
              <Text size="micro" tone="accent" mono>
                {serverUrl} — current
              </Text>
              {(spec.servers ?? [])
                .filter((s) => s.url !== serverUrl)
                .map((server) => (
                  <Text key={server.url} size="micro" tone="faint" mono>
                    {server.url} — {server.description}
                  </Text>
                ))}
            </Stack>
            <Stack direction="row" gap="sm" wrap>
              <Badge tone="neutral" label={`${endpointCount} endpoints`} />
              <Badge tone="neutral" label={`${grouped.length} tags`} />
              <Badge tone="neutral" label={`${schemaCount} schemas`} />
              <Badge tone="neutral" label={spec.info.version} />
            </Stack>
            <Box class="rounded-ds-sm border border-ds-line-ultra bg-ds-surface-sunken px-4 py-3">
              <Text size="micro" mono tone="faint" block class="break-all">
                GET {serverUrl}/api/docs/openapi.json
              </Text>
              <Text size="micro" mono tone="faint" block class="break-all">
                GET {serverUrl}/api/docs/openapi.yaml
              </Text>
              <Text size="micro" tone="faint" block class="mt-2">
                Spec generated from Rust handlers and DTOs. Generate the
                committed file with{' '}
                <span class="font-ds-hud text-ds-text">
                  cargo run -p alice-daemon --bin gen_openapi
                </span>
                .
              </Text>
            </Box>
          </Stack>
        </Card>

        <Stack gap="md">
          {grouped.map(([tag, ops]) => (
            <Card
              key={tag}
              frame={false}
              class="!border-ds-line-faint"
              bodyClass="p-0"
            >
              <Disclosure
                label={tag}
                summary={`${ops.length} endpoints`}
                defaultOpen
              >
                <Stack gap="none">
                  {ops.map((entry) => (
                    <OperationItem
                      key={`${entry.method}:${entry.path}`}
                      entry={entry}
                      serverUrl={serverUrl}
                    />
                  ))}
                </Stack>
              </Disclosure>
            </Card>
          ))}
        </Stack>

        {spec.components?.schemas ? (
          <Card label="Schemas" frame={false} class="!border-ds-line-faint">
            <Stack gap="sm">
              <Text size="micro" tone="faint" block>
                DTOs from <span class="font-ds-hud">alice-core/src/dto.rs</span>
                . Swagger shows the same list as “Schemas”.
              </Text>
              {Object.entries(spec.components.schemas)
                .sort(([a], [b]) => a.localeCompare(b))
                .map(([name, schema]) => (
                  <Disclosure
                    key={name}
                    label={name}
                    summary={
                      schema.description?.slice(0, 80) ??
                      schema.type?.toString() ??
                      ''
                    }
                  >
                    <Box class="overflow-hidden rounded-ds-sm border border-ds-line-ultra">
                      <Box class="border-b border-ds-line-ultra bg-ds-surface-sunken px-3 py-2">
                        <Stack direction="row" gap="sm" align="center" wrap>
                          <Text size="micro" mono>
                            {name}
                          </Text>
                          {schema.enum ? (
                            <Badge
                              tone="accent"
                              label={`${schema.enum.length} values`}
                            />
                          ) : null}
                          {schema.type ? (
                            <Badge
                              tone="neutral"
                              label={
                                Array.isArray(schema.type)
                                  ? schema.type.join('|')
                                  : schema.type
                              }
                            />
                          ) : null}
                        </Stack>
                      </Box>
                      <Box class="p-3">
                        {schema.description ? (
                          <Text
                            size="micro"
                            tone="muted"
                            block
                            class="mb-3 whitespace-pre-wrap"
                          >
                            {schema.description}
                          </Text>
                        ) : null}
                        {schema.properties ? (
                          <table class="w-full border-collapse text-left">
                            <thead>
                              <tr class="border-b border-ds-line-ultra bg-ds-surface">
                                <th class="px-2 py-1">
                                  <Text size="micro" tone="faint">
                                    Field
                                  </Text>
                                </th>
                                <th class="px-2 py-1">
                                  <Text size="micro" tone="faint">
                                    Type
                                  </Text>
                                </th>
                                <th class="px-2 py-1">
                                  <Text size="micro" tone="faint">
                                    Required
                                  </Text>
                                </th>
                              </tr>
                            </thead>
                            <tbody>
                              {Object.entries(schema.properties).map(
                                ([field, prop]) => (
                                  <tr
                                    key={field}
                                    class="border-b border-ds-line-ultra last:border-b-0"
                                  >
                                    <td class="px-2 py-1">
                                      <Text size="micro" mono>
                                        {field}
                                      </Text>
                                      {prop.description ? (
                                        <Text size="micro" tone="faint" block>
                                          {prop.description}
                                        </Text>
                                      ) : null}
                                    </td>
                                    <td class="px-2 py-1">
                                      <Text size="micro" mono tone="muted">
                                        {schemaLabel(prop)}
                                        {prop.format ? ` · ${prop.format}` : ''}
                                        {prop.enum
                                          ? ` · ${prop.enum.join(' | ')}`
                                          : ''}
                                      </Text>
                                    </td>
                                    <td class="px-2 py-1">
                                      <Text
                                        size="micro"
                                        tone={
                                          schema.required?.includes(field)
                                            ? 'accent'
                                            : 'faint'
                                        }
                                      >
                                        {schema.required?.includes(field)
                                          ? 'yes'
                                          : '—'}
                                      </Text>
                                    </td>
                                  </tr>
                                ),
                              )}
                            </tbody>
                          </table>
                        ) : null}
                        {schema.enum ? (
                          <Box class="mt-3 flex flex-wrap gap-1">
                            {schema.enum.map((v) => (
                              <Badge key={v} tone="neutral" label={v} />
                            ))}
                          </Box>
                        ) : null}
                        <Box class="mt-3">
                          <CodeEditor
                            code={JSON.stringify(schema, null, 2).slice(
                              0,
                              20000,
                            )}
                            language="json"
                            readOnly
                          />
                        </Box>
                      </Box>
                    </Box>
                  </Disclosure>
                ))}
            </Stack>
          </Card>
        ) : null}
      </Stack>
    </ScrollArea>
  );
});
