/**
 * `ContentSwitcher` swaps between alternate views of the same content.
 *
 * The values of a switch hold the same thing seen another way — the same
 * stage read by different readers, the same model on a different device,
 * the same feature on or off. One value is shown in place of another, so
 * the values are alternatives and never stand side by side.
 *
 * The values are joined: one strip, one outline, and the chosen value
 * fills its share of it. The first and the last value keep the rounded
 * corners of the strip and the values between them are square, so the
 * strip reads as one control rather than as separate buttons. The strip is
 * as wide as its values, because a switch of two or three values does not
 * have to fill the surface it stands in.
 *
 * Use `Tabs` when the views do not replace one another: the parts of one
 * form, the groups of one catalog. Tabs name content that all lives in the
 * same context and stands beside its neighbours.
 *
 * It is a radio group: one value is chosen, the arrow keys move between
 * the values, and a screen reader announces the chosen one.
 */
import type { QRL } from '@builder.io/qwik';

import { $, component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The height and the padding of the switch. */
export type ContentSwitcherSize = 'sm' | 'md';

/** One value the switch offers. */
export interface ContentSwitcherOption {
  /** The value the switch reports. */
  value: string;
  /** The words the switch shows. Keep them to one or two. */
  label: string;
  /** Dim one value without hiding it. */
  disabled?: boolean;
}

const SIZES: Record<ContentSwitcherSize, string> = {
  sm: 'h-7 px-2.5 text-[10px]',
  md: 'h-8 px-3 text-[11px]',
};

/** The props of `ContentSwitcher`. */
export interface ContentSwitcherProps {
  /** The label for a screen reader. It names the setting. */
  ariaLabel: string;
  /** The value that is chosen. */
  value: string;
  /** The values the switch offers, in the order it shows them. */
  options: ContentSwitcherOption[];
  /** The height. It defaults to `md`. */
  size?: ContentSwitcherSize;
  /** True while a save runs or the value cannot be stored. */
  disabled?: boolean;
  /** Report the value the user chose. */
  onPick$: QRL<(value: string) => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const ContentSwitcher = component$<ContentSwitcherProps>((props) => {
  // The arrow keys walk the values, which is what a radio group answers
  // to, so the switch is reachable without a pointer.
  const handleKey = $((event: KeyboardEvent) => {
    const enabled = props.options.filter((option) => !option.disabled);
    if (enabled.length === 0) {
      return;
    }
    const current = Math.max(
      0,
      enabled.findIndex((option) => option.value === props.value),
    );
    let index: number | null = null;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') {
      index = (current + 1) % enabled.length;
    } else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') {
      index = (current - 1 + enabled.length) % enabled.length;
    } else if (event.key === 'Home') {
      index = 0;
    } else if (event.key === 'End') {
      index = enabled.length - 1;
    }
    if (index === null) {
      return;
    }
    // The arrow keys would scroll the panel under the switch, so the key
    // stops where it is read. The handler is not asynchronous, so the call
    // runs in the same tick as the event.
    // eslint-disable-next-line qwik/no-async-prevent-default
    event.preventDefault();
    props.onPick$(enabled[index].value);
  });

  return (
    <div
      role="radiogroup"
      aria-label={props.ariaLabel}
      aria-disabled={props.disabled}
      class={joinClassNames(
        // `self-start` keeps the strip as wide as its values: a column would
        // otherwise stretch it across the surface.
        'inline-flex max-w-full items-stretch self-start rounded-ds-sm border border-ds-line',
        props.disabled ? 'opacity-40' : null,
        props.class,
      )}
    >
      {props.options.map((option, position) => {
        const chosen = option.value === props.value;
        const blocked = props.disabled || option.disabled;
        // The strip carries the rounding, so only its ends are rounded and
        // the values between them keep square corners: the filled value sits
        // in the strip rather than standing beside it.
        const edge =
          position === 0
            ? 'rounded-l-ds-sm'
            : position === props.options.length - 1
              ? 'rounded-r-ds-sm'
              : 'rounded-none';

        return (
          <button
            key={option.value}
            type="button"
            role="radio"
            aria-checked={chosen}
            tabIndex={chosen ? 0 : -1}
            disabled={blocked}
            class={joinClassNames(
              'inline-flex items-center justify-center font-ds-hud tracking-[0.16em] uppercase transition-colors duration-150 ease-out disabled:pointer-events-none',
              edge,
              position === 0 ? null : 'border-l border-ds-line',
              SIZES[props.size ?? 'md'],
              chosen
                ? 'bg-ds-accent text-ds-text-inverse'
                : 'text-ds-text-muted hover:bg-ds-surface hover:text-ds-text',
            )}
            onClick$={$(() => props.onPick$(option.value))}
            onKeyDown$={handleKey}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
});
