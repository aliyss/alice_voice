import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

import {
  AURA_FRAGMENT_SOURCE,
  AURA_VERTEX_SOURCE,
  createAuraStage,
} from './aura-stage';

/** The uniforms the fragment shader declares. */
function declaredUniforms(): string[] {
  const found = AURA_FRAGMENT_SOURCE.matchAll(/^uniform \w+ (\w+);/gm);
  return [...found].map((match) => match[1]).sort();
}

/** The uniforms the stage resolves and sets on every frame. */
function setUniforms(): string[] {
  const source = readFileSync('src/components/viz/aura-stage.ts', 'utf8');
  const list = source.match(/locations\(gl, program, \[([\s\S]*?)\]\)/);
  expect(list).not.toBeNull();
  const names = (list?.[1] ?? '').matchAll(/'(\w+)'/g);
  return [...names].map((match) => match[1]).sort();
}

/** The constants of the shader, as a lookup of their values. */
function constant(name: string): number {
  const found = AURA_FRAGMENT_SOURCE.match(
    new RegExp(`const (?:int|float) ${name} = ([\\d.]+);`),
  );
  expect(found, `${name} is not declared`).not.toBeNull();
  return Number(found?.[1]);
}

describe('AURA_VERTEX_SOURCE', () => {
  it('is a WebGL2 shader that hands the fragment shader the space of the face', () => {
    expect(AURA_VERTEX_SOURCE.startsWith('#version 300 es')).toBe(true);
    expect(AURA_VERTEX_SOURCE).toContain('in vec2 a_pos');
    expect(AURA_VERTEX_SOURCE).toContain('v_uv = a_pos');
  });
});

describe('AURA_FRAGMENT_SOURCE', () => {
  it('is a WebGL2 shader that draws one color out', () => {
    expect(AURA_FRAGMENT_SOURCE.startsWith('#version 300 es')).toBe(true);
    expect(AURA_FRAGMENT_SOURCE).toContain('precision highp float');
    expect(AURA_FRAGMENT_SOURCE).toContain('fragColor');
  });

  it('sets every uniform it declares, and declares every uniform it sets', () => {
    expect(declaredUniforms()).toEqual(setUniforms());
  });

  it('reads the field through one loop of layers', () => {
    expect(constant('LAYERS')).toBeGreaterThan(1);
    expect(AURA_FRAGMENT_SOURCE).toContain('for (int i = 1; i <= LAYERS; i++)');
  });

  it('fades the field out before the border of the square it is drawn in', () => {
    expect(constant('FADE_START')).toBeLessThan(constant('FADE_END'));
    expect(AURA_FRAGMENT_SOURCE).toContain('smoothstep(FADE_START, FADE_END');
  });

  it('holds the glow to the outside of the ring, so the middle stays open', () => {
    expect(AURA_FRAGMENT_SOURCE).toContain('smoothstep(-width, width, span)');
    expect(constant('GLOW_REACH')).toBeGreaterThan(0);
    expect(constant('GLOW_WEIGHT')).toBeGreaterThan(0);
  });
});

describe('createAuraStage', () => {
  it('is a function that builds a stage from a canvas', () => {
    expect(createAuraStage).toBeTypeOf('function');
    expect(createAuraStage.length).toBe(1);
  });
});
