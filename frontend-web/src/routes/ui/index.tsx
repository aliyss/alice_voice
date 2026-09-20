/**
 * The design system route.
 *
 * The page reads no data, so the file has the default component only and
 * no loader.
 */
import type { DocumentHead } from '@builder.io/qwik-city';

import { component$ } from '@builder.io/qwik';

import { UiCatalogPage } from '~/components/pages/ui-catalog-page';

export default component$(() => {
  return <UiCatalogPage />;
});

export const head: DocumentHead = {
  title: 'Design system · Alice Voice',
  meta: [
    {
      name: 'description',
      content: 'Every primitive of the Alice Voice web frontend.',
    },
  ],
};
