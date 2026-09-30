/**
 * Quality preset mapping, mirrored from `backend/src/system/preset.rs`.
 *
 * `quality` is 0..100 where 0 is fastest and 100 is best. `speed` is
 * `100 - quality`. The preset lerps between the minimum successful
 * settings and the maximum quality settings. This file is used to
 * estimate the current quality from the stored router settings so the
 * sliders do not reset to the recommendation on every load.
 */
import type {
  DecideEngine,
  ExtractEngine,
  ListMatch,
  RetrieveEngine,
  RouterStatusDto,
  SettingsDto,
} from '~/types/dto';

function lerpF32(min: number, max: number, t: number): number {
  return min + (max - min) * Math.max(0, Math.min(1, t));
}

const TOP_K_MIN = 4;
const TOP_K_MAX = 12;
const FLOOR_MIN = 0.35;
const FLOOR_MAX = 0.55;
const MARGIN_MIN = 0.05;
const MARGIN_MAX = 0.15;
const LIST_FLOOR_MIN = 0.75;
const LIST_FLOOR_MAX = 0.85;

/** The same thresholds as the backend. Exported for live flow sync. */
export function presetForQuality(
  quality: number,
  defaults: Pick<
    SettingsDto,
    | 'routerRetrieve'
    | 'routerDecide'
    | 'routerExtract'
    | 'routerTopK'
    | 'routerFloor'
    | 'routerMargin'
    | 'routerLexicalWeight'
    | 'routerDenseWeight'
    | 'routerEmbedSource'
    | 'routerEmbedModel'
    | 'routerEmbedLocalModel'
    | 'routerRerankModel'
    | 'routerLayaModel'
    | 'routerLocalDevice'
    | 'routerPhraseGate'
    | 'routerFastPath'
    | 'routerListMatch'
    | 'routerListFloor'
  >,
): Pick<
  SettingsDto,
  | 'routerRetrieve'
  | 'routerDecide'
  | 'routerExtract'
  | 'routerTopK'
  | 'routerFloor'
  | 'routerMargin'
  | 'routerLexicalWeight'
  | 'routerDenseWeight'
  | 'routerEmbedSource'
  | 'routerEmbedModel'
  | 'routerEmbedLocalModel'
  | 'routerRerankModel'
  | 'routerLayaModel'
  | 'routerLocalDevice'
  | 'routerPhraseGate'
  | 'routerFastPath'
  | 'routerListMatch'
  | 'routerListFloor'
> {
  const q = Math.max(0, Math.min(100, quality));
  const t = q / 100;

  const retrieve: RetrieveEngine = t < 0.55 ? 'lexical' : 'hybrid';
  const decide: DecideEngine =
    t < 0.35 ? 'score' : t < 0.75 ? 'rerank' : 'generative';
  const extract: ExtractEngine =
    t < 0.3 ? 'lists' : t < 0.7 ? 'spans' : 'generative';
  const embedSource =
    retrieve !== 'lexical' || extract === 'spans'
      ? t >= 0.45
        ? 'local'
        : 'server'
      : defaults.routerEmbedSource;
  const listMatch: ListMatch = t < 0.4 ? 'lexical' : 'both';

  const topK = Math.round(lerpF32(TOP_K_MIN, TOP_K_MAX, t));
  const floor = lerpF32(FLOOR_MIN, FLOOR_MAX, t);
  const margin = lerpF32(MARGIN_MIN, MARGIN_MAX, t);
  const listFloor = lerpF32(LIST_FLOOR_MIN, LIST_FLOOR_MAX, t);
  const denseWeight =
    retrieve === 'hybrid' ? lerpF32(0.5, 1, t) : defaults.routerDenseWeight;
  const lexicalWeight =
    retrieve === 'hybrid' ? 1 : defaults.routerLexicalWeight;

  return {
    routerRetrieve: retrieve,
    routerDecide: decide,
    routerExtract: extract,
    routerTopK: topK,
    routerFloor: floor,
    routerMargin: margin,
    routerLexicalWeight: lexicalWeight,
    routerDenseWeight: denseWeight,
    routerEmbedSource: embedSource,
    routerEmbedModel: defaults.routerEmbedModel,
    routerEmbedLocalModel: defaults.routerEmbedLocalModel,
    routerRerankModel: defaults.routerRerankModel,
    // The slider never moves the decision model: Laya is a reader the user
    // picks in the flow, so a preset keeps the file the flow names.
    routerLayaModel: defaults.routerLayaModel,
    routerLocalDevice: defaults.routerLocalDevice,
    routerPhraseGate: true,
    routerFastPath: true,
    routerListMatch: listMatch,
    routerListFloor: listFloor,
  };
}

/**
 * Estimate the quality that best matches the current router settings.
 *
 * The backend stores the full router config, not the quality. To keep the
 * sliders on the value the user applied, we search 0..100 for the preset
 * with the smallest distance to the stored config.
 */
export function estimateQuality(settings: SettingsDto): number {
  let bestQ = 50;
  let bestDist = Number.POSITIVE_INFINITY;

  for (let q = 0; q <= 100; q++) {
    const preset = presetForQuality(q, settings);
    let dist = 0;

    // Categorical distance: 0 if same, large penalty if different.
    dist += preset.routerRetrieve !== settings.routerRetrieve ? 10 : 0;
    dist += preset.routerDecide !== settings.routerDecide ? 10 : 0;
    dist += preset.routerExtract !== settings.routerExtract ? 10 : 0;
    dist += preset.routerEmbedSource !== settings.routerEmbedSource ? 5 : 0;
    dist += preset.routerListMatch !== settings.routerListMatch ? 5 : 0;

    // Numeric distance, normalized to 0..1 then scaled.
    const topKDist =
      Math.abs(preset.routerTopK - settings.routerTopK) /
      (TOP_K_MAX - TOP_K_MIN);
    const floorDist =
      Math.abs(preset.routerFloor - settings.routerFloor) /
      (FLOOR_MAX - FLOOR_MIN);
    const marginDist =
      Math.abs(preset.routerMargin - settings.routerMargin) /
      (MARGIN_MAX - MARGIN_MIN);
    const listFloorDist =
      Math.abs(preset.routerListFloor - settings.routerListFloor) /
      (LIST_FLOOR_MAX - LIST_FLOOR_MIN);

    dist += topKDist * 3;
    dist += floorDist * 3;
    dist += marginDist * 3;
    dist += listFloorDist * 2;

    // Weights
    const denseDist = Math.abs(
      preset.routerDenseWeight - settings.routerDenseWeight,
    );
    const lexDist = Math.abs(
      preset.routerLexicalWeight - settings.routerLexicalWeight,
    );
    dist += denseDist * 1;
    dist += lexDist * 1;

    if (dist < bestDist) {
      bestDist = dist;
      bestQ = q;
    }
  }

  return bestQ;
}

/**
 * Whether the current router is custom for the given quality.
 * A custom setting is one that does not match the lerp for that quality.
 */
export function isCustomForQuality(
  settings: SettingsDto,
  quality: number,
): boolean {
  const preset = presetForQuality(quality, settings);
  const eps = 0.001;
  return (
    preset.routerRetrieve !== settings.routerRetrieve ||
    preset.routerDecide !== settings.routerDecide ||
    preset.routerExtract !== settings.routerExtract ||
    preset.routerTopK !== settings.routerTopK ||
    Math.abs(preset.routerFloor - settings.routerFloor) > eps ||
    Math.abs(preset.routerMargin - settings.routerMargin) > eps ||
    Math.abs(preset.routerLexicalWeight - settings.routerLexicalWeight) > eps ||
    Math.abs(preset.routerDenseWeight - settings.routerDenseWeight) > eps ||
    preset.routerEmbedSource !== settings.routerEmbedSource ||
    preset.routerListMatch !== settings.routerListMatch ||
    Math.abs(preset.routerListFloor - settings.routerListFloor) > eps
  );
}

/**
 * Convert a RouterStatusDto to the shape needed for estimation.
 * The status mirrors the stored config, so we can estimate from it as well.
 */
export function estimateQualityFromStatus(
  status: RouterStatusDto,
  fallback: SettingsDto,
): number {
  const asSettings: SettingsDto = {
    ...fallback,
    routerRetrieve: status.retrieve as RetrieveEngine,
    routerDecide: status.decide as DecideEngine,
    routerExtract: status.extract as ExtractEngine,
    routerTopK: status.topK,
    routerFloor: status.floor,
    routerMargin: status.margin,
    routerLexicalWeight: status.lexicalWeight,
    routerDenseWeight: status.denseWeight,
    routerEmbedSource: status.embedSource as SettingsDto['routerEmbedSource'],
    routerListMatch: status.listMatch as ListMatch,
    routerListFloor: status.listFloor,
  };
  return estimateQuality(asSettings);
}
