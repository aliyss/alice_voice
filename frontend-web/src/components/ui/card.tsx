/**
 * `Card` is the glass panel of the design system.
 *
 * It is a translucent surface with a hairline border, a soft blur, and
 * four corner ticks. The interface runs over a transparent backdrop, so
 * the panel never paints an opaque color.
 *
 * Use `label` for the HUD caption of the panel.
 *
 * A panel can be one pick: `pickable` lays a press over the whole panel,
 * so a list of panels is a list of choices and a reader picks a row by
 * pressing it. The content stays readable and a control inside the panel
 * keeps its own press by taking a place of its own (`relative z-10`), so
 * an action such as a delete never picks the row it stands in. The picked
 * panel carries its state in the border, which takes the accent color, so
 * a list shows which one is chosen without a label of its own.
 */
import type { QRL } from '@builder.io/qwik';

import { Slot, component$ } from '@builder.io/qwik';

import { Text } from '~/components/ui/text';

import { joinClassNames } from '~/utils/class-names';

/** The fill of the panel. */
export type CardTone = 'default' | 'sunken' | 'clear';

const TONES: Record<CardTone, string> = {
  default: 'bg-ds-surface',
  sunken: 'bg-ds-surface-sunken',
  clear: 'bg-transparent',
};

/** The corner tick positions of the HUD frame. */
const CORNERS = [
  'left-1 top-1 border-l border-t',
  'right-1 top-1 border-r border-t',
  'bottom-1 left-1 border-b border-l',
  'bottom-1 right-1 border-b border-r',
];

/** The props of `Card`. */
export interface CardProps {
  /** The HUD caption at the top of the panel. */
  label?: string;
  /** The ARIA role of the panel. It defaults to a region of the page. */
  role?: string;
  /** The label for a screen reader. It names the panel. */
  ariaLabel?: string;
  /** The fill. It defaults to `default`. */
  tone?: CardTone;
  /** Draw the corner ticks. They are on by default. */
  frame?: boolean;
  /** Extra utility classes for the panel itself. */
  class?: string;
  /** Extra utility classes for the content area. It defaults to 24px padding. */
  bodyClass?: string;
  /** Make the whole panel one pick. */
  pickable?: boolean;
  /** Whether the panel is the picked one. */
  picked?: boolean;
  /** The label of the pick for a screen reader. It defaults to `label`. */
  pickLabel?: string;
  /** Report the press on a pickable panel. */
  onPick$?: QRL<() => void>;
}

export const Card = component$<CardProps>((props) => {
  const classes = joinClassNames(
    'relative rounded-ds-md border backdrop-blur-xl',
    TONES[props.tone ?? 'default'],
    // The picked panel takes the accent over the hairline. One of the two
    // colors is written, so the accent cannot lose to the order of the
    // stylesheet.
    props.picked ? 'border-ds-accent-strong' : 'border-ds-line',
    props.class,
  );

  return (
    <section role={props.role} aria-label={props.ariaLabel} class={classes}>
      {props.frame === false
        ? null
        : CORNERS.map((corner) => (
            <span
              key={corner}
              aria-hidden="true"
              class={joinClassNames(
                'pointer-events-none absolute size-3 border-ds-line-strong',
                corner,
              )}
            />
          ))}

      {props.label ? (
        <div class="border-b border-ds-line px-6 py-2">
          <Text size="micro" tone="faint">
            {props.label}
          </Text>
        </div>
      ) : null}

      <div class={joinClassNames('p-6', props.bodyClass)}>
        <Slot />
      </div>

      {props.pickable ? (
        <button
          type="button"
          aria-label={props.pickLabel ?? props.label}
          aria-pressed={props.picked}
          onClick$={props.onPick$}
          class="absolute inset-0 z-0 cursor-pointer rounded-ds-md"
        />
      ) : null}
    </section>
  );
});
