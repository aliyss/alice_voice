/**
 * `Select` reads one value out of a known list.
 *
 * It renders a native `select`, so the keyboard and the picker of the
 * platform work without extra code. The caller owns the value and the
 * change action, so the primitive stays stateless.
 */
import type { QRL } from '@builder.io/qwik';

import { component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** One choice of the list. */
export interface SelectOption {
  /** The stored value. */
  value: string;
  /** The text the user reads. */
  label: string;
}

/** The props of `Select`. */
export interface SelectProps {
  /** The field name for the browser. */
  name?: string;
  /** The current value. It has to match one option. */
  value: string;
  /** The choices, in the order the field shows them. */
  options: SelectOption[];
  /** Block the interaction. */
  disabled?: boolean;
  /** The label for a screen reader. */
  ariaLabel: string;
  /** Run on every change. */
  onChange$?: QRL<(event: Event) => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

const FIELDS =
  'w-full rounded-ds-sm border border-ds-line bg-ds-surface-raised px-3 py-2 font-ds-body text-[14px] leading-[1.6] text-ds-text outline-none transition-colors duration-150 ease-out focus:border-ds-accent-strong disabled:cursor-not-allowed disabled:opacity-40';

export const Select = component$<SelectProps>((props) => {
  return (
    <select
      name={props.name}
      value={props.value}
      disabled={props.disabled}
      aria-label={props.ariaLabel}
      class={joinClassNames(FIELDS, 'cursor-pointer', props.class)}
      onChange$={(event: Event) => props.onChange$?.(event)}
    >
      {props.options.map((option) => (
        // The value of a select is the value of its selected option, so
        // the option that matches the stored value carries the mark. The
        // value on the select itself is an attribute and selects nothing.
        <option
          key={option.value}
          value={option.value}
          selected={option.value === props.value}
        >
          {option.label}
        </option>
      ))}
    </select>
  );
});
