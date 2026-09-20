/**
 * `TextInput` reads one free text value.
 *
 * It renders an `input` or a `textarea`. The caller owns the value and
 * the change action, so the primitive stays stateless.
 *
 * Pass `autoFocus` for a field the user is about to type in: it takes the
 * caret when it appears and again every time it comes back from the
 * blocked state, so a field that is blocked while the daemon answers is
 * ready for the next message without a click.
 */
import type { QRL } from '@builder.io/qwik';

import { component$, useSignal, useVisibleTask$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The element the primitive renders. */
export type TextInputKind = 'input' | 'textarea';

/** The surface the field sits on. */
export type TextInputSurface = 'plain' | 'field';

const FIELDS =
  'w-full bg-transparent font-ds-body text-[14px] leading-[1.6] text-ds-text outline-none placeholder:text-ds-text-faint disabled:opacity-40';

/** A field draws its own glass box. A plain field uses the parent box. */
const SURFACES: Record<TextInputSurface, string> = {
  plain: '',
  field:
    'rounded-ds-sm border border-ds-line bg-ds-surface-raised px-3 py-2 transition-colors duration-150 ease-out focus:border-ds-accent-strong',
};

/** The props of `TextInput`. */
export interface TextInputProps {
  /** The element. It defaults to `input`. */
  kind?: TextInputKind;
  /** The surface. It defaults to `plain`. */
  surface?: TextInputSurface;
  /** The field name for the browser. */
  name?: string;
  /** The current value. */
  value: string;
  /** The placeholder in the user language. */
  placeholder?: string;
  /** Block the interaction. */
  disabled?: boolean;
  /** Take the caret when the field appears and when it is unblocked. */
  autoFocus?: boolean;
  /** The label for a screen reader. */
  ariaLabel: string;
  /** Run on every change. */
  onInput$?: QRL<(event: Event) => void>;
  /** Run on a key press. */
  onKeyDown$?: QRL<(event: KeyboardEvent) => void>;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const TextInput = component$<TextInputProps>((props) => {
  const field = useSignal<HTMLElement>();
  const classes = joinClassNames(
    FIELDS,
    SURFACES[props.surface ?? 'plain'],
    'resize-none',
    props.class,
  );

  // The caret lives in the DOM, so the work is browser only. The task runs
  // once with the field on screen and again whenever the field switches
  // between blocked and ready.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ track }) => {
    const ready = track(() => props.autoFocus && !props.disabled);
    if (ready) {
      field.value?.focus();
    }
  });

  if (props.kind === 'textarea') {
    return (
      <textarea
        ref={field}
        name={props.name}
        class={classes}
        rows={1}
        value={props.value}
        placeholder={props.placeholder}
        disabled={props.disabled}
        aria-label={props.ariaLabel}
        onInput$={props.onInput$}
        onKeyDown$={props.onKeyDown$}
      />
    );
  }

  return (
    <input
      ref={field}
      type="text"
      name={props.name}
      class={classes}
      value={props.value}
      placeholder={props.placeholder}
      disabled={props.disabled}
      aria-label={props.ariaLabel}
      onInput$={props.onInput$}
      onKeyDown$={props.onKeyDown$}
    />
  );
});
