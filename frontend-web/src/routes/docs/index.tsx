/**
 * The API reference route.
 *
 * The loader reads the live OpenAPI spec from the daemon over REST. The page
 * owns no state of its own: the spec is the source of truth and the
 * frontend is a glass copy of it. The daemon still serves `GET
 * /api/docs/openapi.json` and the swagger UI at `/docs` for a bare fetch.
 */
import type { DocumentHead } from '@builder.io/qwik-city';

import type { OpenApiDto } from '~/types/openapi';

import { component$ } from '@builder.io/qwik';
import { routeLoader$ } from '@builder.io/qwik-city';

import { DocsPage } from '~/components/pages/docs-page';

import { backendGet } from '~/lib/backend-client';

/** The path of the live OpenAPI JSON. */
const OPENAPI_PATH = '/api/docs/openapi.json';

export const useOpenApiSpec = routeLoader$(
  async (): Promise<{ spec: OpenApiDto | null; error: string | null }> => {
    try {
      const spec = await backendGet<OpenApiDto>(OPENAPI_PATH);
      return { spec, error: null };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return { spec: null, error: message };
    }
  },
);

export default component$(() => {
  const data = useOpenApiSpec();

  return <DocsPage spec={data.value.spec} error={data.value.error} />;
});

export const head: DocumentHead = {
  title: 'API reference — Alice Voice',
  meta: [
    {
      name: 'description',
      content:
        'The REST and socket contract of the Alice Voice daemon. Glass copy of the live OpenAPI spec.',
    },
  ],
};
