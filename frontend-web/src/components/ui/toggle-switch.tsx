/**
 * `ToggleSwitch` turns one setting on or off.
 *
 * The control is a button with the switch role, so a screen reader
 * announces the state and the space key flips it. The track carries the
 * color and the knob moves along it, so the state is visible without
 * reading the label.
 *
 * Use it for a value that is on or off and the words of the value are the
 * same both ways. Use `ContentSwitcher` for a value that takes one of
 * several alternatives, where each alternative has a name of its own.
 */
import type { QRL } from '@builder.io/qwik';

import { $, component$ } from '@builder.io/qwik';

import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The props of `ToggleSwitch`. */
export interface ToggleSwitchProps {
  /** The label shown beside the track. It names the setting. */
  label: string;
  /**
   * One sentence about what the switch changes. It stays out of the layout
   * and appears as the native tooltip of the control.
   */
  hint?: string;
  /** Whether the switch is on. */
  checked: boolean;
  /** True while a save runs or the value cannot be stored. */
  disabled?: boolean;
  /** Report the state the user chose. */
  onChange$: QRL<(next: boolean) => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const ToggleSwitch = component$<ToggleSwitchProps>((props) => {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={props.checked}
      aria-label={props.label}
      title={props.hint}
      disabled={props.disabled}
      onClick$={$(() => props.onChange$(!props.checked))}
      class={joinClassNames(
        'group inline-flex items-center gap-2 rounded-ds-sm disabled:pointer-events-none disabled:opacity-40',
        props.class,
      )}
    >
      <span
        aria-hidden="true"
        class={joinClassNames(
          'relative inline-flex h-4 w-7 shrink-0 items-center rounded-ds-full border transition-colors duration-150 ease-out',
          props.checked
            ? 'border-ds-accent bg-ds-accent'
            : 'border-ds-line bg-ds-surface-sunken group-hover:border-ds-line-strong',
        )}
      >
        <span
          class={joinClassNames(
            'pointer-events-none absolute size-3 rounded-ds-full bg-ds-text transition-transform duration-150 ease-out',
            props.checked ? 'translate-x-3.5' : 'translate-x-0.5',
          )}
        />
      </span>
      <Text size="hud" tone={props.checked ? 'default' : 'muted'}>
        {props.label}
      </Text>
    </button>
  );
});
