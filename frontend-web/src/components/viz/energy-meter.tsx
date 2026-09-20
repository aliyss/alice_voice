/**
 * `EnergyMeter` shows how busy the daemon is.
 *
 * The bars follow the energy of the daemon state, not the microphone.
 * The daemon does not stream an audio level over the socket yet, so the
 * component must not pretend that it does.
 *
 * The bar heights come from a fixed pattern so the server render and the
 * browser render agree.
 */
import { component$ } from '@builder.io/qwik';

import { joinClassNames } from '~/utils/class-names';

/** The relative height of each bar. The pattern is fixed, not random. */
const BAR_WEIGHTS = [0.4, 0.68, 0.92, 0.55, 0.8, 1, 0.72, 0.46, 0.88, 0.62];

/** The theme tone of the meter. */
export type MeterTone = 'accent' | 'muted';

const TONES: Record<MeterTone, string> = {
  accent: 'bg-ds-accent',
  muted: 'bg-ds-text-faint',
};

/** The props of `EnergyMeter`. */
export interface EnergyMeterProps {
  /** The energy between 0 and 1. */
  energy: number;
  /** The tone. It defaults to `accent`. */
  tone?: MeterTone;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const EnergyMeter = component$<EnergyMeterProps>((props) => {
  const energy = Math.min(1, Math.max(0, props.energy));
  const tone = TONES[props.tone ?? 'accent'];

  return (
    <div
      class={joinClassNames('flex h-4 items-end gap-[3px]', props.class)}
      role="presentation"
      aria-hidden="true"
    >
      {BAR_WEIGHTS.map((weight) => (
        <span
          key={weight}
          class={joinClassNames(
            'w-[2px] rounded-ds-full transition-[height] duration-500 ease-out',
            tone,
          )}
          style={{
            height: `${Math.max(8, Math.round(weight * energy * 100))}%`,
          }}
        />
      ))}
    </div>
  );
});
