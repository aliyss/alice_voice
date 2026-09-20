/**
 * The motion of the aura.
 *
 * A state does not cut from one look to the next. The aura is a field of
 * light, and a field that snapped from a crawl to a race would read as a
 * video that skipped a frame, so every parameter of the look is eased here
 * and the shader only ever sees a look in motion. `success` is a state like
 * any other: it eases in when a turn completes, is held for its moment, and
 * eases back out.
 *
 * The functions are pure: the caller keeps the motion and passes it back
 * with the frame time.
 */
import type { AuraLook, AuraPulse } from '~/utils/aura';

/**
 * How long the aura takes to carry one look into the next, in seconds.
 *
 * The catalog on `/ui` holds a state for five seconds and lets the aura take
 * a second to move to the next one, and the shell moves at the same pace, so
 * one state becoming another always takes about the same time.
 */
export const AURA_TRANSITION = 1;

/**
 * How hard the aura eases toward the look of its state.
 *
 * The ease is exponential, so it never quite arrives. Four time constants is
 * the point at which the difference left is under a fiftieth of the look,
 * which is what makes the transition last `AURA_TRANSITION` and not more.
 */
const EASE_RATE = 4 / AURA_TRANSITION;

/** The longest frame the motion accepts, in seconds. */
const MAX_FRAME = 0.1;

/** The parameters of the look that the motion carries and eases. */
const EASED = [
  'speed',
  'amplitude',
  'frequency',
  'scale',
  'blur',
  'shift',
  'gain',
  'saturation',
] as const;

/** One eased parameter of the motion. */
type EasedKey = (typeof EASED)[number];

/** The motion of the aura at one moment. */
export type AuraMotion = Record<EasedKey, number> & {
  /** The time the breath clock has run, in seconds. */
  breath: number;
};

/** The motion of an aura that has just been created. */
export function createAuraMotion(look: AuraLook): AuraMotion {
  const eased = {} as Record<EasedKey, number>;
  for (const key of EASED) {
    eased[key] = look[key];
  }
  return { ...eased, breath: 0 };
}

/**
 * Advance the motion by one frame.
 *
 * @param motion The motion of the previous frame.
 * @param look The look of the state.
 * @param delta The frame time in seconds. A long frame is clamped, so a tab
 *   that was hidden does not come back to a field that has already settled.
 */
export function stepAuraMotion(
  motion: AuraMotion,
  look: AuraLook,
  delta: number,
): AuraMotion {
  const step = Math.min(Math.max(delta, 0), MAX_FRAME);
  const amount = Math.min(step * EASE_RATE, 1);
  const next: AuraMotion = { ...motion, breath: motion.breath + step };
  for (const key of EASED) {
    next[key] = motion[key] + (look[key] - motion[key]) * amount;
  }
  return next;
}

/**
 * The brightness of the breath at one moment, as a multiplier around one.
 *
 * A state with a steady light comes back as exactly one, so a caller can
 * always multiply its brightness by this.
 */
export function auraPulse(motion: AuraMotion, pulse: AuraPulse): number {
  if (pulse.depth <= 0 || pulse.period <= 0) {
    return 1;
  }
  return (
    1 + pulse.depth * Math.sin((motion.breath / pulse.period) * Math.PI * 2)
  );
}
