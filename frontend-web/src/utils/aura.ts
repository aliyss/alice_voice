/**
 * The look of the aura.
 *
 * The presence of the daemon is a ring of light, drawn as a field that flows
 * around an open middle: each layer of the field warps the face a little
 * further along the flow and paints a ring where the warped point sits at
 * the radius of the ring, so the layers smear into one another and the ring
 * reads as light moving in water instead of as a circle.
 *
 * The technique is the one a turbulence shader is built on: warp the point,
 * measure its distance to a shape, and paint what the distance says, once
 * per layer. This is our own reading of it in
 * `components/viz/aura-stage.ts`; it is not a port of anybody's source.
 *
 * This file holds what the aura is at every daemon state: which token colors
 * it, how fast the flow runs, how far it carries the field, how fine it is,
 * how wide the ring is, how far each layer is shifted in hue, and how bright
 * the light burns. The color itself is not here. The aura is the color of
 * the app, so the token is read by the component and follows the theme; this
 * table only names the tone of each state.
 */
import type { AuraState } from '~/utils/status';

/** An RGB color. Every channel runs from 0 to 255. */
export type Rgb = [number, number, number];

/**
 * The tone of an aura state: which token of the interface colors it.
 *
 * There are only two. Every state of a working daemon is the accent of the
 * interface, so the aura reads as the same light all the way through a turn,
 * and `attention` is the one state that has to read as a warning.
 */
export type AuraTone = 'accent' | 'attention';

/** The token of the interface, and the fallback, of each tone. */
export const AURA_TONES: Record<AuraTone, { token: string; fallback: Rgb }> = {
  accent: { token: '--ds-color-accent', fallback: [255, 111, 77] },
  attention: { token: '--ds-color-error', fallback: [255, 122, 114] },
};

/**
 * The breath of the light.
 *
 * A quiet aura breathes slowly and shallowly; one that is listening or
 * thinking breathes hard enough to read as a pulse.
 */
export interface AuraPulse {
  /** How far the brightness swings, as a fraction of the brightness itself. */
  depth: number;
  /** The length of one breath, in seconds. */
  period: number;
}

/** How the aura looks and moves at one daemon state. */
export interface AuraLook {
  /** The tone of the state, which decides the color of the light. */
  tone: AuraTone;
  /** How fast the flow runs, as the phase it turns through per second. */
  speed: number;
  /** How far the flow carries a point of the field, at every scale it has. */
  amplitude: number;
  /** How fine the flow is, between 0 and 1. Zero is one broad sweep. */
  frequency: number;
  /** The radius of the ring of light, in the space of the face. */
  scale: number;
  /** The softness of the ring, between 0 and 1. */
  blur: number;
  /** How far each layer is shifted in hue from the one before it, between 0 and 1. */
  shift: number;
  /** The brightness of the light. One is the light at its own strength. */
  gain: number;
  /** How much of its color the light keeps, between 0 and 1. */
  saturation: number;
  /** The breath of the light. */
  pulse: AuraPulse;
}

/** A state with a steady light and no pulse at all. */
const STILL: AuraPulse = { depth: 0, period: 8 };

/** The look of a healthy aura at rest. */
const REST: AuraLook = {
  tone: 'accent',
  speed: 0.5,
  amplitude: 1.1,
  frequency: 0.4,
  scale: 0.5,
  blur: 0.3,
  shift: 0.06,
  gain: 0.85,
  saturation: 1,
  pulse: { depth: 0.05, period: 7 },
};

/** Change one field of a look. */
function setLook(look: AuraLook, change: Partial<AuraLook>): AuraLook {
  return { ...look, ...change };
}

/**
 * The look of every state of the aura.
 *
 * The motion follows the pace of a voice agent: the flow crawls while nothing
 * is happening, quickens when the aura is listening, runs fast while it is
 * working, and races while it is speaking. The two states that are not a
 * healthy aura change the color as well. `offline` drains the light to grey,
 * because a daemon without a link must not look alive, and `error` is the
 * one state that takes the warning tone.
 *
 * `success` is the moment after a turn that ran. It is not a daemon state:
 * the component holds it for `AURA_SUCCESS_HOLD` and lets it fall back to the
 * state of the daemon. Its light is the accent like every other working
 * state, and what makes it read as done is that its ring is wider and its
 * light settles instead of stirring.
 */
export const AURA_BY_STATE: Record<AuraState, AuraLook> = {
  offline: setLook(REST, {
    amplitude: 1.3,
    gain: 0.4,
    saturation: 0.06,
    pulse: STILL,
  }),
  idle: REST,
  listening: setLook(REST, {
    speed: 1,
    amplitude: 1,
    frequency: 0.7,
    gain: 1.75,
    pulse: { depth: 0.16, period: 0.75 },
  }),
  transcribing: setLook(REST, {
    speed: 1.5,
    amplitude: 0.55,
    frequency: 1,
    blur: 0.26,
    gain: 1.5,
    pulse: { depth: 0.5, period: 0.8 },
  }),
  resolving: setLook(REST, {
    speed: 1.5,
    amplitude: 0.55,
    frequency: 1,
    blur: 0.26,
    gain: 1.5,
    pulse: { depth: 0.5, period: 0.8 },
  }),
  executing: setLook(REST, {
    speed: 3.5,
    amplitude: 0.75,
    frequency: 1,
    gain: 1.5,
    pulse: { depth: 0.08, period: 1.3 },
  }),
  success: setLook(REST, {
    speed: 0.9,
    amplitude: 0.9,
    frequency: 0.6,
    blur: 0.34,
    shift: 0.08,
    gain: 1.5,
    pulse: { depth: 0.08, period: 1.6 },
  }),
  error: setLook(REST, {
    tone: 'attention',
    speed: 0.8,
    amplitude: 1.4,
    frequency: 0.5,
    blur: 0.36,
    shift: 0.03,
    gain: 1.1,
    saturation: 0.85,
    pulse: { depth: 0.12, period: 1.5 },
  }),
};

/**
 * How long the aura holds the `success` state, in seconds.
 *
 * Long enough to be seen and short enough to be a moment: the light has
 * settled back into the state of the daemon by the time the eye has moved on.
 */
export const AURA_SUCCESS_HOLD = 1.4;

/**
 * Read an `#rgb` or `#rrggbb` color.
 *
 * @param value The color, as it comes out of a CSS custom property.
 * @returns The color, or null when the value is not a hex color.
 */
export function parseHexColor(value: string): Rgb | null {
  const text = value.trim().replace(/^#/, '');
  const long = text.length === 6;
  const short = text.length === 3;
  if (!long && !short) {
    return null;
  }
  if (!/^[0-9a-f]+$/i.test(text)) {
    return null;
  }
  const step = long ? 2 : 1;
  const channels: number[] = [];
  for (let index = 0; index < text.length; index += step) {
    const part = text.slice(index, index + step);
    channels.push(Number.parseInt(short ? part + part : part, 16));
  }
  return [channels[0], channels[1], channels[2]];
}
