import type { AuraLook } from './aura';

import { describe, expect, it } from 'vitest';

import { AURA_BY_STATE } from './aura';
import { auraPulse, createAuraMotion, stepAuraMotion } from './aura-motion';

const LOOK: AuraLook = {
  tone: 'accent',
  speed: 0.5,
  amplitude: 1.1,
  frequency: 0.4,
  scale: 0.5,
  blur: 0.3,
  shift: 0.06,
  gain: 0.85,
  saturation: 1,
  pulse: { depth: 0.2, period: 4 },
};

const FAST: AuraLook = { ...LOOK, speed: 4, gain: 2 };

describe('createAuraMotion', () => {
  it('starts at the look of the state', () => {
    const motion = createAuraMotion(FAST);
    expect(motion.speed).toBe(FAST.speed);
    expect(motion.gain).toBe(FAST.gain);
    expect(motion.breath).toBe(0);
  });
});

describe('stepAuraMotion', () => {
  it('eases toward the look instead of jumping to it', () => {
    const motion = stepAuraMotion(createAuraMotion(LOOK), FAST, 0.016);
    expect(motion.speed).toBeGreaterThan(LOOK.speed);
    expect(motion.speed).toBeLessThan(FAST.speed);
  });

  it('settles on the look, and stays there', () => {
    let motion = createAuraMotion(LOOK);
    for (let frame = 0; frame < 600; frame += 1) {
      motion = stepAuraMotion(motion, FAST, 0.016);
    }
    expect(motion.speed).toBeCloseTo(FAST.speed, 5);
    expect(motion.gain).toBeCloseTo(FAST.gain, 5);
  });

  it('holds a frame that was too long, so a hidden tab does not jump', () => {
    const motion = stepAuraMotion(createAuraMotion(LOOK), FAST, 30);
    expect(motion.speed).toBeLessThanOrEqual(FAST.speed);
    expect(motion.speed).toBeGreaterThan(LOOK.speed);
    expect(motion.breath).toBeCloseTo(0.1, 5);
  });

  it('advances the breath with the frame time, whatever the tempo', () => {
    let motion = createAuraMotion(LOOK);
    for (let frame = 0; frame < 20; frame += 1) {
      motion = stepAuraMotion(motion, FAST, 0.05);
    }
    expect(motion.breath).toBeCloseTo(1, 5);
  });

  it('carries the aura from the success of a turn back into the idle state', () => {
    let motion = createAuraMotion(AURA_BY_STATE.success);
    expect(motion.gain).toBe(AURA_BY_STATE.success.gain);
    for (let frame = 0; frame < 600; frame += 1) {
      motion = stepAuraMotion(motion, AURA_BY_STATE.idle, 0.016);
    }
    expect(motion.gain).toBeCloseTo(AURA_BY_STATE.idle.gain, 5);
    expect(motion.blur).toBeCloseTo(AURA_BY_STATE.idle.blur, 5);
  });
});

describe('auraPulse', () => {
  it('returns one when the state has no breath in it', () => {
    const motion = stepAuraMotion(createAuraMotion(LOOK), LOOK, 0.5);
    expect(auraPulse(motion, { depth: 0, period: 4 })).toBe(1);
    expect(auraPulse(motion, { depth: 0.2, period: 0 })).toBe(1);
  });

  it('starts at the look and swings by the depth', () => {
    const motion = createAuraMotion(LOOK);
    expect(auraPulse(motion, LOOK.pulse)).toBe(1);

    const quarter = { ...motion, breath: 1 };
    expect(auraPulse(quarter, LOOK.pulse)).toBeCloseTo(1.2, 5);

    const half = { ...motion, breath: 2 };
    expect(auraPulse(half, LOOK.pulse)).toBeCloseTo(1, 5);
  });
});
