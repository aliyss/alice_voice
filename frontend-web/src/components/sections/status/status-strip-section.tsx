/**
 * `StatusStripSection` is the top HUD bar of the shell.
 *
 * It shows the daemon state, the version, the link state, and the busy
 * meter. It also carries the backdrop switch, which is the one control
 * of the shell.
 */
import type { BadgeTone } from '~/components/ui/badge';

import type { AuraState } from '~/utils/status';

import { $, component$, useContext } from '@builder.io/qwik';
import { useLocation } from '@builder.io/qwik-city';

import { LinkIndicator } from '~/components/sections/status/link-indicator-partial';
import { Badge } from '~/components/ui/badge';
import { Button } from '~/components/ui/button';
import { Link } from '~/components/ui/link';
import { Stack } from '~/components/ui/stack';
import { Text } from '~/components/ui/text';
import { EnergyMeter } from '~/components/viz/energy-meter';

import { appStatusContext } from '~/context/app-status.context';
import { displayContext } from '~/context/display.context';

import { toAuraState, toEnergy, toStateLabel } from '~/utils/status';

/** The badge tone of each aura state. */
const STATE_TONES: Record<AuraState, BadgeTone> = {
  offline: 'neutral',
  idle: 'neutral',
  listening: 'accent',
  transcribing: 'warn',
  resolving: 'warn',
  executing: 'ok',
  success: 'ok',
  error: 'error',
};

export const StatusStripSection = component$(() => {
  const status = useContext(appStatusContext);
  const backdrop = useContext(displayContext);
  const location = useLocation();

  // The strip carries the shell navigation.
  const isCatalog = location.url.pathname.startsWith('/ui');
  const isSettings = location.url.pathname.startsWith('/settings');
  const isConversations = location.url.pathname.startsWith('/conversations');

  const auraState = toAuraState(status.value);
  const isBusy = auraState !== 'idle' && auraState !== 'offline';

  const handleToggleBackdrop$ = $(() => {
    backdrop.value = backdrop.value === 'transparent' ? 'solid' : 'transparent';
  });

  return (
    <Stack
      direction="row"
      gap="md"
      align="center"
      justify="between"
      wrap
      role="banner"
      ariaLabel="Daemon status"
      class="shrink-0 border-b border-ds-line px-8 py-3"
    >
      <Stack direction="row" gap="md" align="center">
        <Text size="micro" tone="default" weight="medium">
          Alice
        </Text>
        <Text size="micro" tone="faint">
          {`v${status.value.version}`}
        </Text>
        <Badge
          tone={STATE_TONES[auraState]}
          label={toStateLabel(status.value)}
          pulse={isBusy}
        />
        <Link href={isConversations ? '/' : '/conversations'} tone="muted">
          {isConversations ? 'Conversation' : 'Conversations'}
        </Link>
        <Link href={isCatalog ? '/' : '/ui'} tone="muted">
          {isCatalog ? 'Conversation' : 'Design system'}
        </Link>
        <Link href={isSettings ? '/' : '/settings'} tone="muted">
          {isSettings ? 'Conversation' : 'Settings'}
        </Link>
      </Stack>

      <Stack direction="row" gap="lg" align="center">
        <EnergyMeter
          energy={toEnergy(status.value)}
          tone={status.value.connected ? 'accent' : 'muted'}
        />
        <LinkIndicator connected={status.value.connected} />
        <Button
          size="sm"
          variant="quiet"
          onClick$={handleToggleBackdrop$}
          ariaLabel="Switch the backdrop between transparent and solid"
        >
          {backdrop.value === 'transparent'
            ? 'Backdrop glass'
            : 'Backdrop solid'}
        </Button>
      </Stack>
    </Stack>
  );
});
