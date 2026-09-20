/**
 * `BadgesSection` shows every tone of `Badge`.
 *
 * The badge carries a machine value, so the sample uses the same kind of
 * short upper case label the application uses.
 */
import type { BadgeTone } from '~/components/ui/badge';
import type { LinkTone } from '~/components/ui/link';

import { component$ } from '@builder.io/qwik';

import { ShowcaseBlock } from '~/components/sections/ui/showcase-block-partial';
import { Badge } from '~/components/ui/badge';
import { Card } from '~/components/ui/card';
import { Link } from '~/components/ui/link';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';

/** The tones of `Badge`. */
const TONES: BadgeTone[] = ['neutral', 'accent', 'ok', 'warn', 'error'];

/** The tones of `Link`. */
const LINK_TONES: LinkTone[] = ['default', 'muted', 'accent'];

export const BadgesSection = component$(() => {
  return (
    <Card label="Badge and Link">
      <Stack gap="lg">
        <ShowcaseBlock title="Badge tone" note="the dot blinks on a live state">
          {TONES.map((tone) => (
            <Badge key={tone} tone={tone} label={tone} />
          ))}
        </ShowcaseBlock>

        <ShowcaseBlock title="Badge pulse">
          <Badge tone="ok" label="Link live" pulse />
          <Badge tone="ok" label="Link live" />
          <Badge tone="error" label="Link down" />
        </ShowcaseBlock>

        <ShowcaseBlock title="Link tone">
          {LINK_TONES.map((tone) => (
            <Link key={tone} href="/ui" tone={tone}>
              {tone}
            </Link>
          ))}
          <Text size="body" tone="muted">
            The link stays underlined so it never looks like plain text.
          </Text>
        </ShowcaseBlock>
      </Stack>
    </Card>
  );
});
