//! Preset mapping of the layered router.
//!
//! The settings page draws two linked sliders: response speed and
//! response quality. Both control the same `quality` value where 0 is
//! fastest and 100 is best. Speed is `100 - quality`. The preset lerps
//! between the minimum successful settings and the maximum quality settings:
//! every value between `min` and `max` is a linear interpolation. The
//! recommended default is also a lerp that weights the host variables.

use alice_core::config::{DecideEngine, EmbedSource, ExtractEngine, ListMatch, RetrieveEngine};

/// Inputs the recommendation reads.
/// Every field is a variable the recommendation lerps over.
#[derive(Clone, Debug)]
pub struct RecommendationInputs {
    /// Total RAM in bytes.
    pub total_ram_bytes: u64,
    /// Available RAM in bytes.
    pub available_ram_bytes: u64,
    /// Used RAM in bytes.
    #[allow(dead_code)]
    pub used_ram_bytes: u64,
    /// Logical cores.
    pub logical_cores: usize,
    /// Physical cores, or 0 when not reported.
    pub physical_cores: usize,
    /// Average frequency of the processor in MHz.
    pub frequency_mhz: u64,
    /// Average CPU usage 0..100.
    pub cpu_usage_percent: f32,
    /// Whether ONNX Runtime can use CUDA now.
    pub cuda_available: bool,
    /// Whether this build carries CUDA support.
    pub cuda_build: bool,
    /// Whether the llama server answers.
    pub llama_reachable: bool,
    /// Whether embeddings answer (server or local).
    pub embeddings_reachable: bool,
    /// Whether the selected GLiNER model is on disk.
    pub gliner_installed: bool,
    /// Whether the embedding model is on disk.
    pub embedding_installed: bool,
    /// Whether the reranker is on disk.
    pub reranker_installed: bool,
    /// Number of labels a GLiNER model reads for this catalog.
    pub label_count: usize,
    /// Number of intents in the catalog.
    pub intent_count: usize,
    /// Available bytes on the disk that holds the router models.
    pub disk_available_bytes: Option<u64>,
    /// Total bytes of that disk.
    pub disk_total_bytes: Option<u64>,
}

/// Convert a quality value to speed.
pub fn quality_to_speed(quality: u8) -> u8 {
    100u8.saturating_sub(quality.min(100))
}

/// Convert a speed value to quality.
#[allow(dead_code)]
pub fn speed_to_quality(speed: u8) -> u8 {
    100u8.saturating_sub(speed.min(100))
}

/// Clamp a quality value to 0..100.
pub fn clamp_quality(value: u8) -> u8 {
    value.min(100)
}

/// Smallest and largest `top_k` the presets use.
const TOP_K_MIN: usize = 4;
const TOP_K_MAX: usize = 12;

/// Floor when quality is 0 and when it is 100.
const FLOOR_MIN: f32 = 0.35;
const FLOOR_MAX: f32 = 0.55;

/// Margin when quality is 0 and when it is 100.
const MARGIN_MIN: f32 = 0.05;
const MARGIN_MAX: f32 = 0.15;

/// `list_floor` when quality is 0 and when it is 100.
const LIST_FLOOR_MIN: f32 = 0.75;
const LIST_FLOOR_MAX: f32 = 0.85;

/// Linear interpolation between two `f32` values.
fn lerp_f32(min: f32, max: f32, t: f32) -> f32 {
    min + (max - min) * t.clamp(0.0, 1.0)
}

/// Linear interpolation between two `usize` values.
fn lerp_usize(min: usize, max: usize, t: f32) -> usize {
    let value = lerp_f32(min as f32, max as f32, t);
    value.round() as usize
}

/// Minimum and maximum router settings for a successful response.
///
/// `min` is the fastest configuration that still resolves an intent.
/// `max` is the best quality configuration. Every preset is a lerp between
/// these two.
const MIN_RETRIEVE: RetrieveEngine = RetrieveEngine::Lexical;
const MAX_RETRIEVE: RetrieveEngine = RetrieveEngine::Hybrid;
const MIN_DECIDE: DecideEngine = DecideEngine::Score;
const MAX_DECIDE: DecideEngine = DecideEngine::Generative;
const MIN_EXTRACT: ExtractEngine = ExtractEngine::Lists;
const MAX_EXTRACT: ExtractEngine = ExtractEngine::Generative;

/// Map a quality value to a router configuration.
///
/// The mapping lerps between `min` (quality 0) and `max` (quality 100).
/// Discrete stages are chosen by thresholds that split the 0..100 line
/// linearly. Numeric fields are true `lerp(min, max, t)` where `t` is
/// `quality / 100`. The defaults carry the model identifiers, the model
/// directory, and the device. The preset keeps those values and lerps
/// only the fields that trade speed for quality.
pub fn quality_to_router(
    quality: u8,
    defaults: &alice_core::config::RouterConfig,
) -> alice_core::config::RouterConfig {
    let quality = clamp_quality(quality);
    let t = f32::from(quality) / 100.0;

    // 1. Retrieval: lerp from Lexical (min) to Hybrid (max).
    let retrieve = if t < 0.55 { MIN_RETRIEVE } else { MAX_RETRIEVE };

    // 2. Decision: lerp Score -> Rerank -> Generative.
    let decide = if t < 0.35 {
        MIN_DECIDE
    } else if t < 0.75 {
        DecideEngine::Rerank
    } else {
        MAX_DECIDE
    };

    // 3. Extraction: lerp Lists -> Spans -> Generative.
    let extract = if t < 0.30 {
        MIN_EXTRACT
    } else if t < 0.70 {
        ExtractEngine::Spans
    } else {
        MAX_EXTRACT
    };

    // 4. Where the vectors come from when a stage needs them.
    let embed_source = if retrieve.uses_embeddings() || matches!(extract, ExtractEngine::Spans) {
        if t >= 0.45 {
            EmbedSource::Local
        } else {
            EmbedSource::Server
        }
    } else {
        defaults.embed_source
    };

    // 5. How to match a mention against a list. Lerps Lexical -> Both.
    let list_match = if t < 0.40 {
        ListMatch::Lexical
    } else {
        ListMatch::Both
    };

    // 6. Short list size: lerp 4 -> 12.
    let top_k = lerp_usize(TOP_K_MIN, TOP_K_MAX, t);

    // 7. Floors and margins: true lerp min -> max.
    let floor = lerp_f32(FLOOR_MIN, FLOOR_MAX, t);
    let margin = lerp_f32(MARGIN_MIN, MARGIN_MAX, t);
    let list_floor = lerp_f32(LIST_FLOOR_MIN, LIST_FLOOR_MAX, t);

    // 8. Weights for the hybrid retrieval: lerp dense weight 0.5 -> 1.0.
    let (lexical_weight, dense_weight) = if matches!(retrieve, RetrieveEngine::Hybrid) {
        let dense = lerp_f32(0.5, 1.0, t);
        (1.0, dense)
    } else {
        (defaults.lexical_weight, defaults.dense_weight)
    };

    alice_core::config::RouterConfig {
        fast_path: true,
        retrieve,
        decide,
        extract,
        top_k: top_k.clamp(1, 64),
        floor,
        margin,
        lexical_weight,
        dense_weight,
        embed_model: defaults.embed_model.clone(),
        models_dir: defaults.models_dir.clone(),
        embed_source,
        embed_local_model: defaults.embed_local_model.clone(),
        rerank_model: defaults.rerank_model.clone(),
        laya_model: defaults.laya_model.clone(),
        local_device: defaults.local_device,
        phrase_gate: true,
        list_match,
        list_floor,
        // The fallbacks are a behaviour and not a trade of speed for
        // quality, so a preset keeps the values the settings hold.
        fallback_llm: defaults.fallback_llm,
        script_fallback: defaults.script_fallback,
        open_values_llm: defaults.open_values_llm,
    }
}

/// Normalize a value between `min` and `max` to 0..1.
fn norm(value: f64, min: f64, max: f64) -> f64 {
    if max <= min {
        return 0.0;
    }
    ((value - min) / (max - min)).clamp(0.0, 1.0)
}

/// Recommend a quality value for the host.
///
/// Every host variable is normalized to 0..1 and lerped with a weight.
/// `quality = lerp(min, max, weighted_sum)` where the weights sum to 1.
/// The default is therefore the lerp position that fits the machine.
pub fn recommend_quality(inputs: &RecommendationInputs) -> (u8, Vec<String>, String) {
    let mut factors: Vec<String> = Vec::new();

    // Helper to push a factor with its normalized value and weight.
    let mut weighted: f64 = 0.0;

    // 1. RAM total 1..32 GB -> t 0..1, weight 0.22
    let ram_gb = inputs.total_ram_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let ram_t = norm(ram_gb, 1.0, 32.0);
    weighted += ram_t * 0.22;
    factors.push(format!("{ram_gb:.1} GB RAM -> {}", format_factor(ram_t)));

    // 2. RAM pressure: available / total -> t 0..1, weight 0.05
    let avail_ratio = if inputs.total_ram_bytes > 0 {
        inputs.available_ram_bytes as f64 / inputs.total_ram_bytes as f64
    } else {
        0.5
    };
    weighted += avail_ratio * 0.05;
    factors.push(format!(
        "{:.0}% RAM free -> {}",
        avail_ratio * 100.0,
        format_factor(avail_ratio)
    ));

    // 3. Cores logical 1..16 -> t, weight 0.18
    let cores_t = norm(inputs.logical_cores as f64, 1.0, 16.0);
    weighted += cores_t * 0.18;
    factors.push(format!(
        "{} logical cores -> {}",
        inputs.logical_cores,
        format_factor(cores_t)
    ));

    // 4. Physical cores when known -> small bonus 0.02
    if inputs.physical_cores > 0 {
        let phys_t = norm(inputs.physical_cores as f64, 1.0, 8.0);
        weighted += phys_t * 0.02;
        factors.push(format!(
            "{} physical cores -> {}",
            inputs.physical_cores,
            format_factor(phys_t)
        ));
    }

    // 5. Frequency 1.5..4.0 GHz -> t, weight 0.05
    let freq_ghz = inputs.frequency_mhz as f64 / 1000.0;
    let freq_t = norm(freq_ghz, 1.5, 4.0);
    weighted += freq_t * 0.05;
    factors.push(format!("{:.2} GHz -> {}", freq_ghz, format_factor(freq_t)));

    // 6. CPU usage inverse (lower usage = more headroom) -> weight 0.03
    let cpu_free = 1.0 - (f64::from(inputs.cpu_usage_percent).clamp(0.0, 100.0) / 100.0);
    weighted += cpu_free * 0.03;
    factors.push(format!(
        "{:.0}% CPU free -> {}",
        cpu_free * 100.0,
        format_factor(cpu_free)
    ));

    // 7. CUDA -> 1.0 if available, 0.5 if build present, 0 else, weight 0.10
    let cuda_t = if inputs.cuda_available {
        1.0
    } else if inputs.cuda_build {
        0.5
    } else {
        0.0
    };
    weighted += cuda_t * 0.10;
    factors.push(format!(
        "CUDA {} -> {}",
        if inputs.cuda_available {
            "available"
        } else if inputs.cuda_build {
            "build present"
        } else {
            "none"
        },
        format_factor(cuda_t)
    ));

    // 8. Llama server -> weight 0.08
    let llama_t = if inputs.llama_reachable { 1.0 } else { 0.0 };
    weighted += llama_t * 0.08;
    factors.push(format!(
        "Model server {} -> {}",
        if inputs.llama_reachable { "up" } else { "down" },
        format_factor(llama_t)
    ));

    // 9. Embeddings -> weight 0.04
    let embed_t = if inputs.embeddings_reachable {
        1.0
    } else {
        0.0
    };
    weighted += embed_t * 0.04;
    factors.push(format!(
        "Embeddings {} -> {}",
        if inputs.embeddings_reachable {
            "up"
        } else {
            "down"
        },
        format_factor(embed_t)
    ));

    // 10. Models on disk -> each 0..1, weight 0.03 each (total 0.09)
    let embed_inst_t = if inputs.embedding_installed { 1.0 } else { 0.0 };
    weighted += embed_inst_t * 0.03;
    factors.push(format!(
        "Embedding model {} -> {}",
        if inputs.embedding_installed {
            "on disk"
        } else {
            "missing"
        },
        format_factor(embed_inst_t)
    ));
    let rerank_inst_t = if inputs.reranker_installed { 1.0 } else { 0.0 };
    weighted += rerank_inst_t * 0.03;
    factors.push(format!(
        "Reranker {} -> {}",
        if inputs.reranker_installed {
            "on disk"
        } else {
            "missing"
        },
        format_factor(rerank_inst_t)
    ));
    let gliner_inst_t = if inputs.gliner_installed { 1.0 } else { 0.0 };
    weighted += gliner_inst_t * 0.03;
    factors.push(format!(
        "GLiNER {} -> {}",
        if inputs.gliner_installed {
            "on disk"
        } else {
            "missing"
        },
        format_factor(gliner_inst_t)
    ));

    // 11. Catalog size inverse: small catalog allows higher quality -> weight 0.06
    // 0 labels -> t=1, 40+ -> t=0
    let catalog_t = 1.0 - norm(inputs.label_count as f64, 0.0, 40.0);
    weighted += catalog_t * 0.06;
    factors.push(format!(
        "{} labels, {} intents -> {}",
        inputs.label_count,
        inputs.intent_count,
        format_factor(catalog_t)
    ));

    // 12. Disk free 0..100 GB -> t, weight 0.04
    let disk_t = if let Some(avail) = inputs.disk_available_bytes {
        let free_gb = avail as f64 / (1024.0 * 1024.0 * 1024.0);
        let total_gb = inputs
            .disk_total_bytes
            .map(|total| total as f64 / (1024.0 * 1024.0 * 1024.0))
            .unwrap_or(100.0);
        let t = norm(free_gb, 0.5, total_gb.min(100.0));
        factors.push(format!("{free_gb:.1} GB free -> {}", format_factor(t)));
        t
    } else {
        factors.push("Disk free unknown -> balanced".to_string());
        0.5
    };
    weighted += disk_t * 0.04;

    // Weighted sum is already 0..1 (weights sum to ~1.0). Lerp 0..100.
    let quality = (weighted * 100.0).round().clamp(0.0, 100.0) as u8;
    let reason = if quality < 25 {
        "Host is lean, favour speed and local words".to_string()
    } else if quality < 50 {
        "Host is modest, balanced but lean to fast".to_string()
    } else if quality < 70 {
        "Host fits balanced quality and speed".to_string()
    } else if quality < 85 {
        "Host fits quality, use embeddings and reranker".to_string()
    } else {
        "Host fits best quality, use hybrid retrieval and generative stages".to_string()
    };

    (quality, factors, reason)
}

/// Format a 0..1 factor as a short label.
fn format_factor(t: f64) -> String {
    if t < 0.2 {
        "very low".to_string()
    } else if t < 0.4 {
        "low".to_string()
    } else if t < 0.6 {
        "moderate".to_string()
    } else if t < 0.8 {
        "high".to_string()
    } else {
        "very high".to_string()
    }
}

/// Label of a preset.
pub fn preset_label(quality: u8) -> (&'static str, &'static str) {
    match quality {
        0..=20 => ("Ultra fast", "Words and scores only. No model waits."),
        21..=40 => ("Fast", "Words and scores, short list. Minimal model use."),
        41..=60 => (
            "Balanced",
            "Words with a local reranker and spans. Good speed and quality.",
        ),
        61..=80 => (
            "Quality",
            "Hybrid retrieval with reranker and spans. Needs local models.",
        ),
        _ => (
            "Maximum",
            "Hybrid retrieval with generative decision and values. Needs the model server.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alice_core::config::RouterConfig;

    #[test]
    fn speed_and_quality_are_inverses() {
        assert_eq!(quality_to_speed(0), 100);
        assert_eq!(quality_to_speed(100), 0);
        assert_eq!(speed_to_quality(0), 100);
        assert_eq!(speed_to_quality(100), 0);
        assert_eq!(quality_to_speed(30), 70);
        assert_eq!(speed_to_quality(70), 30);
    }

    #[test]
    fn clamp_keeps_quality_inside_the_range() {
        assert_eq!(clamp_quality(255), 100);
        assert_eq!(clamp_quality(0), 0);
        assert_eq!(clamp_quality(50), 50);
    }

    #[test]
    fn fastest_preset_uses_no_model() {
        let cfg = quality_to_router(0, &RouterConfig::default());
        assert_eq!(cfg.retrieve, RetrieveEngine::Lexical);
        assert_eq!(cfg.decide, DecideEngine::Score);
        assert_eq!(cfg.extract, ExtractEngine::Lists);
        assert_eq!(cfg.top_k, TOP_K_MIN);
    }

    #[test]
    fn best_preset_uses_the_model() {
        let cfg = quality_to_router(100, &RouterConfig::default());
        assert_eq!(cfg.retrieve, RetrieveEngine::Hybrid);
        assert_eq!(cfg.decide, DecideEngine::Generative);
        assert_eq!(cfg.extract, ExtractEngine::Generative);
        assert_eq!(cfg.top_k, TOP_K_MAX);
    }

    #[test]
    fn balanced_preset_uses_the_reranker() {
        let cfg = quality_to_router(50, &RouterConfig::default());
        assert_eq!(cfg.decide, DecideEngine::Rerank);
        assert_eq!(cfg.extract, ExtractEngine::Spans);
    }

    #[test]
    fn preset_labels_cover_the_range() {
        assert_eq!(preset_label(0).0, "Ultra fast");
        assert_eq!(preset_label(30).0, "Fast");
        assert_eq!(preset_label(50).0, "Balanced");
        assert_eq!(preset_label(70).0, "Quality");
        assert_eq!(preset_label(90).0, "Maximum");
    }

    #[test]
    fn recommendation_moves_with_ram_and_cuda() {
        let base = RecommendationInputs {
            total_ram_bytes: 8 * 1024 * 1024 * 1024,
            available_ram_bytes: 4 * 1024 * 1024 * 1024,
            used_ram_bytes: 4 * 1024 * 1024 * 1024,
            logical_cores: 8,
            physical_cores: 4,
            frequency_mhz: 3000,
            cpu_usage_percent: 50.0,
            cuda_available: false,
            cuda_build: false,
            llama_reachable: false,
            embeddings_reachable: false,
            gliner_installed: false,
            embedding_installed: false,
            reranker_installed: false,
            label_count: 10,
            intent_count: 10,
            disk_available_bytes: Some(10 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(100 * 1024 * 1024 * 1024),
        };
        let (low, _, _) = recommend_quality(&base);

        let high_inputs = RecommendationInputs {
            total_ram_bytes: 32 * 1024 * 1024 * 1024,
            available_ram_bytes: 28 * 1024 * 1024 * 1024,
            used_ram_bytes: 4 * 1024 * 1024 * 1024,
            logical_cores: 16,
            physical_cores: 8,
            frequency_mhz: 4000,
            cpu_usage_percent: 10.0,
            cuda_available: true,
            cuda_build: true,
            llama_reachable: true,
            embeddings_reachable: true,
            gliner_installed: true,
            embedding_installed: true,
            reranker_installed: true,
            label_count: 10,
            intent_count: 10,
            disk_available_bytes: Some(100 * 1024 * 1024 * 1024),
            disk_total_bytes: Some(200 * 1024 * 1024 * 1024),
        };
        let (high, _, _) = recommend_quality(&high_inputs);
        assert!(high > low, "high spec should recommend higher quality");
    }
}
