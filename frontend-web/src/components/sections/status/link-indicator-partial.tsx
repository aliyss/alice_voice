/**
 * `LinkIndicator` shows whether the event stream is open.
 */
import { component$ } from '@builder.io/qwik';

import { Badge } from '~/components/ui/badge';

/** The props of `LinkIndicator`. */
export interface LinkIndicatorProps {
  /** True while the socket is open. */
  connected: boolean;
}

export const LinkIndicator = component$<LinkIndicatorProps>((props) => {
  return (
    <Badge
      tone={props.connected ? 'ok' : 'error'}
      label={props.connected ? 'Link live' : 'Link down'}
      pulse={props.connected}
    />
  );
});
