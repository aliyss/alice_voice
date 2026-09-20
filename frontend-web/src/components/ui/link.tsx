/**
 * `Link` navigates to another route.
 *
 * It wraps the router link of Qwik City, so the navigation stays in the
 * client. The link is always underlined on hover and always visible on
 * focus, because a HUD hides its chrome too easily.
 */
import { Slot, component$ } from '@builder.io/qwik';
import { Link as RouterLink } from '@builder.io/qwik-city';

import { joinClassNames } from '~/utils/class-names';

/** The emphasis of the link. */
export type LinkTone = 'default' | 'muted' | 'accent';

const TONES: Record<LinkTone, string> = {
  default:
    'text-ds-text hover:text-ds-accent-strong decoration-ds-line-strong hover:decoration-ds-accent-strong',
  muted:
    'text-ds-text-muted hover:text-ds-text decoration-ds-line hover:decoration-ds-line-strong',
  accent:
    'text-ds-accent-strong hover:text-ds-accent decoration-ds-accent-strong hover:decoration-ds-accent',
};

/** The props of `Link`. */
export interface LinkProps {
  /** The target route. */
  href: string;
  /** The emphasis. It defaults to `default`. */
  tone?: LinkTone;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Link = component$<LinkProps>((props) => {
  const classes = joinClassNames(
    'rounded-ds-sm font-ds-hud text-[10px] tracking-[0.22em] uppercase underline decoration-1 underline-offset-4 transition-colors duration-150 ease-out',
    TONES[props.tone ?? 'default'],
    props.class,
  );

  return (
    <RouterLink href={props.href} class={classes}>
      <Slot />
    </RouterLink>
  );
});
