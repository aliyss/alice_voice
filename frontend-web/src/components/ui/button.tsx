/**
 * `Button` triggers one action.
 *
 * Use `solid` once per view for the primary action. Use `outline` for a
 * secondary action and `quiet` for a HUD control.
 */
import type { QRL } from '@builder.io/qwik';

import { Slot, component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The emphasis of the button. */
export type ButtonVariant = 'solid' | 'outline' | 'quiet';

/** The height and the padding of the button. */
export type ButtonSize = 'sm' | 'md' | 'lg';

const VARIANTS: Record<ButtonVariant, string> = {
  solid:
    'bg-ds-accent text-ds-text-inverse hover:bg-ds-accent-strong active:bg-ds-accent-deep',
  outline:
    'border border-ds-line text-ds-text hover:border-ds-line-strong hover:bg-ds-surface active:bg-ds-surface-raised',
  quiet:
    'text-ds-text-muted hover:bg-ds-surface hover:text-ds-text active:bg-ds-surface-raised',
};

const SIZES: Record<ButtonSize, string> = {
  sm: 'h-7 px-3 text-[10px]',
  md: 'h-9 px-4 text-[11px]',
  lg: 'h-11 px-6 text-[12px]',
};

/** The props of `Button`. */
export interface ButtonProps {
  /** The emphasis. It defaults to `outline`. */
  variant?: ButtonVariant;
  /** The size. It defaults to `md`. */
  size?: ButtonSize;
  /** The native button type. It defaults to `button`. */
  type?: 'button' | 'submit';
  /** Block the interaction and dim the button. */
  disabled?: boolean;
  /** Stretch the button across the container. */
  fullWidth?: boolean;
  /** The label for a screen reader. Use it for an icon-only button. */
  ariaLabel?: string;
  /** The action. It runs in the browser. */
  onClick$?: QRL<() => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Button = component$<ButtonProps>((props) => {
  const classes = joinClassNames(
    'inline-flex items-center justify-center gap-2 rounded-ds-sm font-ds-hud uppercase tracking-[0.16em] transition-colors duration-150 ease-out disabled:pointer-events-none disabled:opacity-40',
    VARIANTS[props.variant ?? 'outline'],
    SIZES[props.size ?? 'md'],
    props.fullWidth ? 'w-full' : undefined,
    props.class,
  );

  return (
    <button
      type={props.type ?? 'button'}
      class={classes}
      disabled={props.disabled}
      aria-label={props.ariaLabel}
      onClick$={props.onClick$}
    >
      <Slot />
    </button>
  );
});
