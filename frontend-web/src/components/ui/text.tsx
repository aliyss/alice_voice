/**
 * `Text` renders text with one tone, one size, and one weight.
 *
 * It renders a `span` and stays inline. Set `block` to make it a block
 * element. The primitive never changes the text content.
 */
import { Slot, component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The semantic color of the text. */
export type TextTone =
  | 'default'
  | 'muted'
  | 'faint'
  | 'accent'
  | 'ok'
  | 'warn'
  | 'error'
  | 'inverse'
  | 'inherit';

/** The size of the text. */
export type TextSize = 'micro' | 'hud' | 'body' | 'lead' | 'title' | 'display';

/** The weight of the text. */
export type TextWeight = 'regular' | 'medium' | 'semibold';

const TONES: Record<TextTone, string> = {
  default: 'text-ds-text',
  muted: 'text-ds-text-muted',
  faint: 'text-ds-text-faint',
  accent: 'text-ds-accent-strong',
  ok: 'text-ds-ok',
  warn: 'text-ds-warn',
  error: 'text-ds-error',
  inverse: 'text-ds-text-inverse',
  // The parent carries the color, for example a badge or an alert tone.
  inherit: 'text-inherit',
};

const SIZES: Record<TextSize, string> = {
  micro: 'text-[10px] leading-[1.4] uppercase tracking-[0.22em]',
  hud: 'text-[11px] leading-[1.5] tracking-[0.12em]',
  body: 'text-[13px] leading-[1.65]',
  lead: 'text-[15px] leading-[1.7]',
  title: 'text-[20px] leading-[1.3] tracking-[-0.01em]',
  display: 'text-[30px] leading-[1.15] tracking-[-0.02em]',
};

const WEIGHTS: Record<TextWeight, string> = {
  regular: 'font-normal',
  medium: 'font-medium',
  semibold: 'font-semibold',
};

/** The props of `Text`. */
export interface TextProps {
  /** The semantic color. It defaults to `default`. */
  tone?: TextTone;
  /** The size. It defaults to `body`. */
  size?: TextSize;
  /** The weight. It defaults to `regular`. */
  weight?: TextWeight;
  /**
   * Use the display font for the text. The HUD sizes use it by default
   * because they carry machine state.
   */
  mono?: boolean;
  /** Render the text as a block element. */
  block?: boolean;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Text = component$<TextProps>((props) => {
  const size = props.size ?? 'body';
  const isHudSize = size === 'micro' || size === 'hud';
  const classes = joinClassNames(
    TONES[props.tone ?? 'default'],
    SIZES[size],
    WEIGHTS[props.weight ?? 'regular'],
    (props.mono ?? isHudSize) ? 'font-ds-hud' : 'font-ds-body',
    props.block ? 'block' : 'inline',
    props.class,
  );

  return (
    <span class={classes}>
      <Slot />
    </span>
  );
});
