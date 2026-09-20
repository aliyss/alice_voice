/**
 * The stage: the WebGL2 renderer of the aura.
 *
 * The presence of the daemon is a ring of light, like the corona of an
 * eclipse. The field is one flow, read many times. Each layer warps the face
 * a little further along the flow and paints a ring where the warped point
 * sits at the radius of the ring, so the layers smear into one another and
 * the ring reads as light moving in water instead of as a circle.
 *
 * THE FLOW
 *
 * A point is carried by three waves. Each wave is finer than the one before
 * and pushes in a direction of its own, and each displacement is divided by
 * its own frequency. That is what a turbulence field is: it disturbs the
 * plane by the same amount at every scale it has, so the ring keeps its
 * shape in the large and is broken in the small. A single wave would only
 * wobble the ring; three give it a crest, a bend, and a ripple.
 *
 * THE RING
 *
 * The distance to the ring is measured on the warped point and not on the
 * point of the face, which is why a perfectly round shape comes out molten.
 * The layers differ only in where they read the flow, so where the flow is
 * slow they agree and the light is solid, and where it runs fast they part
 * and the light opens into ribbons.
 *
 * ONE PASS
 *
 * ONE PASS, AND NO FALLBACK
 *
 * Everything is one draw and one fragment shader. The look of a state
 * reaches it as the numbers in `utils/aura.ts`, and the color is read from
 * the accent token of the interface by the component, so the aura is the
 * color of the app. Nothing is baked into the source and no texture is
 * fetched.
 *
 * `createAuraStage` returns null when the browser has no WebGL2, or when a
 * shader does not build. The aura is then not drawn at all. There is no
 * 2D canvas fallback, because a field this dense cannot be rasterized on one
 * at any speed, and a still one would not be the same object.
 */
import type { Rgb } from '~/utils/aura';

/**
 * The largest side of the aura canvas, in device pixels.
 *
 * The field is soft, so it survives being drawn at 448 and scaled up.
 * Without a cap the canvas follows the element and the display scale, and an
 * aura in a wide window on a dense screen costs four times the pixels of
 * this for no visible gain. The field is also the most expensive thing the
 * shell draws, so the cap is what keeps the catalog on `/ui` inside a frame.
 */
const AURA_MAX_SIDE = 448;

/** The vertex shader. It hands the fragment shader the space of the face. */
export const AURA_VERTEX_SOURCE = `#version 300 es
layout(location = 0) in vec2 a_pos;
out vec2 v_uv;
void main() {
  v_uv = a_pos;
  gl_Position = vec4(a_pos, 0.0, 1.0);
}
`;

/**
 * The ring, in one pass.
 *
 * The face runs from -1 to 1 on both axes, with the origin at its center, so
 * the ring of light sits at a radius of about two fifths of the way to the
 * corner and the middle of the ring stays open.
 */
export const AURA_FRAGMENT_SOURCE = `#version 300 es
precision highp float;

in vec2 v_uv;
out vec4 fragColor;

uniform float u_time;
uniform float u_speed;
uniform float u_amplitude;
uniform float u_frequency;
uniform float u_scale;
uniform float u_blur;
uniform float u_shift;
uniform float u_gain;
uniform float u_sat;
uniform vec3 u_color;

const float TAU = 6.28318530718;

/** How many layers the field is read through. Each one costs a flow. */
const int LAYERS = 16;

/** How far along the flow each layer reads it, in radians. */
const float LAYER_SPREAD = 2.4;

/**
 * How hard the flow pushes a point, against its own frequency.
 *
 * It has to stay well under the radius of the ring. The flow is what makes
 * the ring molten, but a displacement as large as the ring is what makes
 * the middle of the field fill in and the hole close.
 */
const float FLOW_PUSH = 0.25;

/**
 * How far the glow throws, as a fraction of the radius of the ring.
 *
 * The glow is what a ring of light throws away from itself, and it is what
 * makes the light read as standing in air rather than as a drawn circle. It
 * dies out well before the corner of the face, so the box the aura is drawn
 * in never shows an edge.
 */
const float GLOW_REACH = 0.42;

/** How much of the field is a glow rather than the ring itself. */
const float GLOW_WEIGHT = 0.7;

/**
 * Where the field fades out, as a distance from the centre of the face.
 *
 * The face is a square and the field is a ring, so the light is faint by the
 * border, but a tail this long would still end in a straight line there.
 * The fade takes it to nothing before the border, whatever the flow does.
 */
const float FADE_START = 0.82;
const float FADE_END = 1.15;

/**
 * How hard the field burns before the state scales it.
 *
 * The field is a mean over its layers, so a point that the layers agree on
 * still comes out well under one. This is what brings the ring back up to
 * the light it should throw.
 */
const float FIELD_GAIN = 2.6;

/**
 * The flow. A point of the face is carried by three waves, each finer than
 * the last and pushed a little further round than the one before.
 */
vec2 flow(vec2 p, float phase) {
  float frequency = mix(1.7, 7.0, u_frequency);
  float amplitude = u_amplitude * FLOW_PUSH;
  vec2 q = p;
  for (int i = 0; i < 3; i++) {
    vec2 wave = vec2(
      sin(q.y * frequency + phase + float(i) * 1.7),
      cos(q.x * frequency * 1.13 - phase - float(i) * 1.3)
    );
    q += (amplitude / frequency) * wave;
    frequency *= 2.2;
    amplitude *= 0.75;
  }
  return q;
}

/**
 * Turn the hue of a color, by the formula of a color matrix rotation.
 *
 * A film of light does not hold one color: every layer of it sits at a
 * slightly different angle, and the angle is what decides the hue. The shift
 * is a fraction of a full turn of the wheel.
 */
vec3 hueShift(vec3 color, float turns) {
  float angle = turns * TAU;
  float c = cos(angle);
  float s = sin(angle);
  vec3 toRed = vec3(
    0.213 + c * 0.787 - s * 0.213,
    0.715 - c * 0.715 - s * 0.715,
    0.072 - c * 0.072 + s * 0.928
  );
  vec3 toGreen = vec3(
    0.213 - c * 0.213 + s * 0.143,
    0.715 + c * 0.285 + s * 0.140,
    0.072 - c * 0.072 - s * 0.283
  );
  vec3 toBlue = vec3(
    0.213 - c * 0.213 - s * 0.787,
    0.715 - c * 0.715 + s * 0.715,
    0.072 + c * 0.928 + s * 0.072
  );
  return vec3(dot(toRed, color), dot(toGreen, color), dot(toBlue, color));
}

void main() {
  float t = u_time * u_speed;
  vec2 p = v_uv;

  float ring = u_scale;
  float soft = 0.010 + u_blur * 0.05;

  // The two ends of the hue: the layers run from one to the other.
  vec3 base = u_color;
  vec3 shifted = clamp(hueShift(u_color, u_shift), 0.0, 1.0);

  vec3 light = vec3(0.0);
  vec3 glow = vec3(0.0);
  // The layer before the first one is the flow at the same phase, so the
  // width of the ring is measured between neighbours and not against a
  // layer that reads the flow at another time.
  vec2 previous = flow(p, t);
  for (int i = 1; i <= LAYERS; i++) {
    float f = float(i) / float(LAYERS);
    vec2 q = flow(p, f * LAYER_SPREAD + t);
    // Where the flow runs fast, a layer parts from the one before it, and
    // the ring is soft there. That is what makes it move.
    float drift = length(q - previous);
    previous = q;
    float span = length(q) - ring;
    float width = soft + drift;
    // The ring: the light where the flow crosses the radius of the ring,
    // against how far the layers have parted there.
    float d = abs(span) / width;
    // The glow throws away from the core of the ring and not into it, so
    // the middle stays open and the light reads as a ring.
    float reach = span / max(ring * GLOW_REACH + soft, 0.001);
    float gate = smoothstep(-width, width, span);
    vec3 color = mix(base, shifted, 1.0 - f);
    light += color * exp(-d * d);
    glow += color * exp(-reach * reach) * gate;
  }
  light /= float(LAYERS);
  glow /= float(LAYERS);

  // The fade, so the canvas never shows the straight line of its own edge.
  float fade = 1.0 - smoothstep(FADE_START, FADE_END, length(p));
  light *= fade;
  glow *= fade;

  vec3 col = (light + glow * GLOW_WEIGHT) * FIELD_GAIN * u_gain;

  // The brightness of the field, which is what the canvas covers. It is
  // taken before the color is graded, so a red field covers as much as an
  // amber one: the brightness of a hue is not the brightness of its light.
  float lit = dot(col, vec3(0.2126, 0.7152, 0.0722));
  col = col / (1.0 + col);
  lit = lit / (1.0 + lit);

  // The state: how much of its color the light keeps. This is what drains
  // the ring of the offline state to a grey.
  float luma = dot(col, vec3(0.2126, 0.7152, 0.0722));
  col = mix(vec3(luma), col, clamp(u_sat, 0.0, 1.0));

  // The field is a light, so its own brightness is what it covers. The
  // output is premultiplied, which is what the blend of the canvas takes.
  float alpha = clamp(lit, 0.0, 1.0);
  fragColor = vec4(col * alpha, alpha);
}
`;

/** One frame of the aura. */
export interface AuraFrame {
  /** The time the stage has been drawing, in seconds. It drives the flow. */
  time: number;
  /** How fast the flow runs, as the phase it turns through per second. */
  speed: number;
  /** How far the flow carries a point of the field. */
  amplitude: number;
  /** How fine the flow is, between 0 and 1. */
  frequency: number;
  /** The radius of the ring of light. */
  scale: number;
  /** The softness of the ring, between 0 and 1. */
  blur: number;
  /** How far each layer is shifted in hue from the one before it. */
  shift: number;
  /** The brightness of the light. */
  gain: number;
  /** How much of its color the light keeps, between 0 and 1. */
  saturation: number;
  /** The color of the light, between 0 and 1. */
  color: Rgb;
}

/** The renderer of one aura. */
export interface AuraStage {
  /** Draw one frame. */
  draw: (frame: AuraFrame) => void;
  /** Release the context. */
  dispose: () => void;
}

/**
 * Compile one shader, or return null when it does not build.
 *
 * A shader that does not build leaves the aura undrawn, and a silent
 * fallback hides a typo in the shader for as long as it takes to notice that
 * the aura has stopped glowing. The log goes to the console.
 */
function compileShader(
  gl: WebGL2RenderingContext,
  kind: number,
  source: string,
): WebGLShader | null {
  const shader = gl.createShader(kind);
  if (!shader) {
    return null;
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    console.warn(
      '[aura] the shader did not build:',
      gl.getShaderInfoLog(shader),
    );
    gl.deleteShader(shader);
    return null;
  }
  return shader;
}

/** Build the program from one vertex and one fragment shader. */
function compileProgram(
  gl: WebGL2RenderingContext,
  vertexSource: string,
  fragmentSource: string,
): WebGLProgram | null {
  const vertex = compileShader(gl, gl.VERTEX_SHADER, vertexSource);
  const fragment = compileShader(gl, gl.FRAGMENT_SHADER, fragmentSource);
  if (!vertex || !fragment) {
    return null;
  }

  const program = gl.createProgram();
  if (!program) {
    return null;
  }
  gl.attachShader(program, vertex);
  gl.attachShader(program, fragment);
  gl.linkProgram(program);
  gl.deleteShader(vertex);
  gl.deleteShader(fragment);

  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.warn(
      '[aura] the program did not link:',
      gl.getProgramInfoLog(program),
    );
    gl.deleteProgram(program);
    return null;
  }
  return program;
}

/** Every uniform location of the program, resolved once. */
function locations(
  gl: WebGL2RenderingContext,
  program: WebGLProgram,
  names: readonly string[],
): Record<string, WebGLUniformLocation | null> {
  const found: Record<string, WebGLUniformLocation | null> = {};
  for (const name of names) {
    found[name] = gl.getUniformLocation(program, name);
  }
  return found;
}

/** Scale a color of the palette, between 0 and 255, into the shader's range. */
function unitColor(color: Rgb): [number, number, number] {
  return [color[0] / 255, color[1] / 255, color[2] / 255];
}

/**
 * Create the aura renderer on one canvas.
 *
 * @param canvas The canvas to draw into.
 * @returns The stage, or null when WebGL2 is not available or a shader does
 *   not build.
 */
export function createAuraStage(canvas: HTMLCanvasElement): AuraStage | null {
  const gl = canvas.getContext('webgl2', {
    alpha: true,
    premultipliedAlpha: true,
    // The field is a glow, so it has no hard edge for a multisampled canvas
    // to smooth, and the canvas is the largest thing the shell draws.
    antialias: false,
    depth: false,
    stencil: false,
    preserveDrawingBuffer: false,
    powerPreference: 'low-power',
  });
  if (!gl) {
    return null;
  }

  const program = compileProgram(gl, AURA_VERTEX_SOURCE, AURA_FRAGMENT_SOURCE);
  if (!program) {
    return null;
  }

  // One triangle covers the quad, and the interpolated position is the same
  // `uv` the shader works in.
  const buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 3, -1, -1, 3]),
    gl.STATIC_DRAW,
  );
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

  const uniforms = locations(gl, program, [
    'u_time',
    'u_speed',
    'u_amplitude',
    'u_frequency',
    'u_scale',
    'u_blur',
    'u_shift',
    'u_gain',
    'u_sat',
    'u_color',
  ]);

  let size = 0;

  return {
    draw(frame) {
      // The canvas follows the element and the display scale up to a cap.
      // The size is only applied when it changes, because resizing a canvas
      // clears it and reallocates it.
      const scale = Math.min(window.devicePixelRatio || 1, 2);
      const side = Math.min(
        AURA_MAX_SIDE,
        Math.max(1, Math.round((canvas.clientWidth || 1) * scale)),
      );
      if (side !== size) {
        size = side;
        canvas.width = side;
        canvas.height = side;
      }

      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.viewport(0, 0, size, size);
      // The field is light over the page, so the shader writes premultiplied
      // color and the blend keeps the page behind the glow.
      gl.enable(gl.BLEND);
      gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
      gl.useProgram(program);
      gl.uniform1f(uniforms.u_time, frame.time);
      gl.uniform1f(uniforms.u_speed, frame.speed);
      gl.uniform1f(uniforms.u_amplitude, frame.amplitude);
      gl.uniform1f(uniforms.u_frequency, frame.frequency);
      gl.uniform1f(uniforms.u_scale, frame.scale);
      gl.uniform1f(uniforms.u_blur, frame.blur);
      gl.uniform1f(uniforms.u_shift, frame.shift);
      gl.uniform1f(uniforms.u_gain, frame.gain);
      gl.uniform1f(uniforms.u_sat, frame.saturation);
      gl.uniform3f(uniforms.u_color, ...unitColor(frame.color));

      gl.clearColor(0, 0, 0, 0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      gl.drawArrays(gl.TRIANGLES, 0, 3);
    },
    dispose() {
      gl.deleteBuffer(buffer);
      gl.deleteProgram(program);
      const lose = gl.getExtension('WEBGL_lose_context');
      lose?.loseContext();
    },
  };
}
