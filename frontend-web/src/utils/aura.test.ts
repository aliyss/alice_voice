import type { AuraTone } from './aura';

import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import {
  AURA_BY_STATE,
  AURA_SUCCESS_HOLD,
  AURA_TONES,
  parseHexColor,
} from './aura';
import { AURA_STATES } from './status';

/** The value of one token, as it is written in the stylesheet. */
function token(name: string): string {
  const styles = readFileSync('src/styles/tokens.css', 'utf8');
  const found = styles.match(new RegExp(`${name}:\\s*(#[0-9a-f]{3,8});`, 'i'));
  expect(found, `${name} is not in the tokens`).not.toBeNull();
  return found?.[1] ?? '';
}

describe('parseHexColor', () => {
  it('reads a six digit color', () => {
    expect(parseHexColor('#ff6f4d')).toEqual([255, 111, 77]);
  });

  it('reads a three digit color, doubling every channel', () => {
    expect(parseHexColor('#abc')).toEqual([170, 187, 204]);
  });

  it('accepts a color without the hash, and with space around it', () => {
    expect(parseHexColor(' 00ff00 ')).toEqual([0, 255, 0]);
  });

  it('returns null for anything that is not a hex color', () => {
    expect(parseHexColor('rgb(255 111 77)')).toBeNull();
    expect(parseHexColor('transparent')).toBeNull();
    expect(parseHexColor('#12345')).toBeNull();
    expect(parseHexColor('#12 3456')).toBeNull();
    expect(parseHexColor('')).toBeNull();
  });
});

describe('AURA_TONES', () => {
  it('takes the accent of the interface, so the aura is the color of the app', () => {
    expect(parseHexColor(token('--ds-color-accent'))).toEqual(
      AURA_TONES.accent.fallback,
    );
  });

  it('takes the error color for the one state that warns', () => {
    expect(parseHexColor(token('--ds-color-error'))).toEqual(
      AURA_TONES.attention.fallback,
    );
  });

  it('reads its fallbacks back as colors', () => {
    for (const tone of Object.keys(AURA_TONES) as AuraTone[]) {
      const { fallback } = AURA_TONES[tone];
      for (const channel of fallback) {
        expect(channel).toBeGreaterThanOrEqual(0);
        expect(channel).toBeLessThanOrEqual(255);
      }
    }
  });
});

describe('AURA_BY_STATE', () => {
  it('has a look for every state of the aura', () => {
    expect([...AURA_STATES].sort()).toEqual(
      (Object.keys(AURA_BY_STATE) as string[]).sort(),
    );
  });

  it('keeps every look inside the range of the shader', () => {
    for (const state of AURA_STATES) {
      const look = AURA_BY_STATE[state];
      expect(look.saturation).toBeGreaterThanOrEqual(0);
      expect(look.saturation).toBeLessThanOrEqual(1);
      expect(look.frequency).toBeGreaterThanOrEqual(0);
      expect(look.frequency).toBeLessThanOrEqual(1);
      expect(look.blur).toBeGreaterThanOrEqual(0);
      expect(look.blur).toBeLessThanOrEqual(1);
      expect(look.shift).toBeGreaterThanOrEqual(0);
      expect(look.shift).toBeLessThanOrEqual(1);
      expect(look.gain).toBeGreaterThan(0);
      expect(look.speed).toBeGreaterThan(0);
      expect(look.pulse.depth).toBeGreaterThanOrEqual(0);
    }
  });

  it('draws every state at the same radius, so the ring does not jump', () => {
    for (const state of AURA_STATES) {
      expect(AURA_BY_STATE[state].scale).toBe(0.5);
    }
  });

  it('is the accent at every state but the one that warns', () => {
    for (const state of AURA_STATES) {
      const expected = state === 'error' ? 'attention' : 'accent';
      expect(AURA_BY_STATE[state].tone).toBe(expected);
    }
  });

  it('is faster and brighter when the daemon is busy than when it is idle', () => {
    const idle = AURA_BY_STATE.idle;
    for (const state of ['listening', 'executing'] as const) {
      expect(AURA_BY_STATE[state].speed).toBeGreaterThan(idle.speed);
      expect(AURA_BY_STATE[state].gain).toBeGreaterThan(idle.gain);
    }
  });

  it('gives the offline state no pulse, so a dead daemon does not look alive', () => {
    expect(AURA_BY_STATE.offline.pulse.depth).toBe(0);
    expect(AURA_BY_STATE.offline.saturation).toBeLessThan(0.2);
  });

  it('celebrates a turn with the accent, and settles instead of stirring', () => {
    expect(AURA_BY_STATE.success.tone).toBe('accent');
    expect(AURA_BY_STATE.success.gain).toBeGreaterThan(AURA_BY_STATE.idle.gain);
    expect(AURA_SUCCESS_HOLD).toBeGreaterThan(0);
  });
});
