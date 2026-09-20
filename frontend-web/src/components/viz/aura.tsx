/**
 * `Aura` is the presence of the daemon: a ring of light.
 *
 * It is one canvas and one WebGL2 pass, `./aura-stage`. The look of the
 * daemon state comes from `~/utils/aura`, the motion that carries one state
 * into the next from `~/utils/aura-motion`, and the color is the tone of the
 * state read out of the tokens of the interface, so the aura is the color of
 * the app. Each layer of the field is turned a little in hue from the one
 * before it, which is what gives the ring its depth.
 *
 * The one state the daemon does not report is `success`. A turn that ran and
 * a turn that failed both leave the daemon idle, so the aura watches the
 * count of completed turns and holds the success state for
 * `AURA_SUCCESS_HOLD` before it settles back into the state of the daemon.
 *
 * The aura draws at its own tempo, at most 30 frames a second, and only
 * while it is on screen. It is soft light and it moves slowly, so the extra
 * frames would cost the machine and buy nothing. The catalog on `/ui` draws
 * one of these.
 *
 * There is no fallback renderer. A browser without WebGL2 draws nothing: a
 * field of sixteen warped layers per pixel cannot be rasterized on a 2D
 * canvas at any speed, and a still one would not be the same object.
 */
import type { AuraTone, Rgb } from '~/utils/aura';
import type { AuraState, TurnOutcome } from '~/utils/status';

import {
  component$,
  useSignal,
  useTask$,
  useVisibleTask$,
} from '@builder.io/qwik';

import { createAuraStage } from '~/components/viz/aura-stage';

import {
  AURA_BY_STATE,
  AURA_SUCCESS_HOLD,
  AURA_TONES,
  parseHexColor,
} from '~/utils/aura';
import {
  auraPulse,
  createAuraMotion,
  stepAuraMotion,
} from '~/utils/aura-motion';
import { joinClassNames } from '~/utils/class-names';

/**
 * The time between frames of the aura, in milliseconds.
 *
 * The ring is slow, soft light, so it reads the same at 30 frames a second
 * and the shader costs half as much.
 */
const AURA_FRAME_MS = 1000 / 30;

/**
 * Read the color of every tone out of the tokens of the interface.
 *
 * @returns The color of each tone, or its fallback when the token cannot be
 *   read.
 */
function toneColors(): Record<AuraTone, Rgb> {
  const style = getComputedStyle(document.documentElement);
  const colors = {} as Record<AuraTone, Rgb>;
  for (const tone of Object.keys(AURA_TONES) as AuraTone[]) {
    const { token, fallback } = AURA_TONES[tone];
    colors[tone] = parseHexColor(style.getPropertyValue(token)) ?? fallback;
  }
  return colors;
}

/** The props of `Aura`. */
export interface AuraProps {
  /** The state of the daemon. */
  state: AuraState;
  /** How many turns have completed. A change starts the moment of success. */
  turns?: number;
  /** What completed the last turn. */
  outcome?: TurnOutcome | null;
  /** Extra utility classes from the caller. */
  class?: string;
}

export const Aura = component$<AuraProps>((props) => {
  const canvasRef = useSignal<HTMLCanvasElement>();

  // The loop runs for the life of the element and reads the state on every
  // frame, so the props are mirrored into signals it can read without the
  // task closing over a value that has gone stale.
  const state = useSignal<AuraState>(props.state);
  const turns = useSignal(props.turns ?? 0);
  const outcome = useSignal<TurnOutcome | null>(props.outcome ?? null);

  useTask$(({ track }) => {
    state.value = track(() => props.state);
    turns.value = track(() => props.turns ?? 0);
    outcome.value = track(() => props.outcome ?? null);
  });

  // The canvas. The task runs once: the loop it starts lives until the
  // element goes away.
  // eslint-disable-next-line qwik/no-use-visible-task
  useVisibleTask$(({ cleanup }) => {
    const canvas = canvasRef.value;
    if (!canvas) {
      return;
    }

    const stage = createAuraStage(canvas);
    if (!stage) {
      return;
    }

    const colors = toneColors();
    // A reader who asked for less motion gets a ring that drifts instead of
    // one that runs, and no breath at all.
    const calm = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

    let lastDraw = performance.now();
    // The clock counts the time the aura was actually drawn, so an aura that
    // stopped drawing picks its flow up where it left off instead of
    // jumping to where the wall clock has gone.
    let elapsed = 0;
    let motion = createAuraMotion(AURA_BY_STATE[state.value]);
    // The state of the first frame is where the shell starts, so a tab that
    // opens onto a turn that has already finished does not celebrate it.
    let seenTurns = turns.value;
    let successUntil = 0;
    let frame = 0;

    // An aura that is scrolled out of view still costs a frame per frame.
    let onScreen = true;
    const observer = new IntersectionObserver(
      (entries) => {
        onScreen = entries.some((entry) => entry.isIntersecting);
      },
      { rootMargin: '160px' },
    );
    observer.observe(canvas);

    const render = (nowMs: number) => {
      frame = requestAnimationFrame(render);
      const gap = nowMs - lastDraw;
      if (!onScreen || gap < AURA_FRAME_MS) {
        return;
      }
      lastDraw = nowMs;
      elapsed += gap / 1000;

      if (turns.value !== seenTurns) {
        seenTurns = turns.value;
        if (outcome.value === 'ok') {
          successUntil = elapsed + AURA_SUCCESS_HOLD;
        }
      }

      // The success of a turn outranks the idle state of the daemon, but
      // only for its moment, and only while the daemon really is idle: a
      // turn that starts, or a link that drops, is the news instead.
      const holding = elapsed < successUntil && state.value === 'idle';
      const look = holding ? AURA_BY_STATE.success : AURA_BY_STATE[state.value];
      motion = stepAuraMotion(motion, look, gap / 1000);

      stage.draw({
        time: elapsed,
        speed: motion.speed * (calm ? 0.25 : 1),
        amplitude: motion.amplitude,
        frequency: motion.frequency,
        scale: motion.scale,
        blur: motion.blur,
        shift: motion.shift,
        gain: motion.gain * (calm ? 1 : auraPulse(motion, look.pulse)),
        saturation: motion.saturation,
        color: colors[look.tone],
      });
    };

    frame = requestAnimationFrame(render);

    cleanup(() => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      stage.dispose();
    });
  });

  return (
    <div
      class={joinClassNames('relative aspect-square w-full', props.class)}
      data-aura-state={props.state}
      aria-hidden="true"
    >
      <canvas ref={canvasRef} class="absolute inset-0 size-full" />
    </div>
  );
});
