//! Inference of the built in decision model.
//!
//! Laya answers typed questions about one state in a single forward pass.
//! The decision stage asks it one `choice` question: the state is the
//! message and the options are the short list of the retrieval pass, so
//! the model answers with one option and a probability for every option
//! without writing a word of text.
//!
//! The model is not an encoder of the usual shape. It reads one sequence
//! per question, holds a marker token in front of every option, and reads
//! the option scores off those markers:
//!
//! ```text
//! [CLS] choice question: <instructions> [SEP] [MASK] opt0 [MASK] opt1 ...
//!       [SEP] <state> [SEP]
//! ```
//!
//! The scores are uncalibrated logits, so the model ships the temperature
//! of every answer shape and this module scales the logits with it before
//! the softmax. A calibration taken from the checkpoint keeps the
//! probability of the chosen option a number the floor and the margin of
//! the router can hold.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use alice_core::config::LocalDevice;
use ort::execution_providers::{CUDAExecutionProvider, OpenVINOExecutionProvider};
use ort::session::Session;
use ort::session::SessionInputValue;
use ort::value::{DynValue, Tensor};
use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::resolver::device::{self, DEVICE_CUDA, DEVICE_GPU};
use crate::resolver::local::catalog::{ModelSpec, Role};
use crate::resolver::local::error::LocalError;
use crate::resolver::local::store::LocalStore;

/// The question type of a choice, as the model numbers it.
const QTYPE_CHOICE: i64 = 0;

/// The name of the question type, as the temperature table keys it.
const QTYPE_NAME_CHOICE: &str = "choice";

/// Name of the tensor that holds the tokens of the question.
const INPUT_IDS: &str = "input_ids";

/// Name of the tensor that marks the real tokens.
const ATTENTION_MASK: &str = "attention_mask";

/// Name of the tensor that holds the position of every option marker.
const MARKER_POS: &str = "marker_pos";

/// Name of the tensor that marks the real option markers.
const MARKER_MASK: &str = "marker_mask";

/// Name of the tensor that tells the question type.
const QTYPE: &str = "qtype";

/// Name of the output that holds the score of every option.
const LOGITS: &str = "logits";

/// The literal mask token, scrubbed from the state and the options so a
/// user cannot inject a marker of their own.
const MASK_TOKEN: &str = "[MASK]";

/// The token that opens the sequence.
const CLS_TOKEN: &str = "[CLS]";

/// The token that closes a part of the sequence.
const SEP_TOKEN: &str = "[SEP]";

/// The token a row shorter than the batch is padded with.
const PAD_TOKEN: &str = "[PAD]";

/// Largest number of tokens one option text may hold before the head
/// budget shrinks every option evenly.
const OPTION_TOKENS: usize = 48;

/// Smallest head budget left for the instructions once the options are in.
const HEAD_MIN: i64 = 16;

/// Smallest part of the head that survives when the options take it all.
const HEAD_KEEP: usize = 8;

/// Smallest number of tokens one option keeps when the budget is tight.
const OPTION_FLOOR: usize = 4;

/// Largest number of loaded models the daemon keeps.
const MAX_LOADED: usize = 3;

/// The calibration the checkpoint ships beside the graph.
///
/// `temperature` is indexed by the question type; `temperature_by_options`
/// holds a finer temperature per answer shape, because a two-option choice
/// and a twenty-option choice need different scaling.
#[derive(Clone, Debug, Deserialize)]
struct LayaConfig {
    /// Largest number of tokens of one sequence.
    max_len: usize,
    /// Largest number of tokens of the head: the instructions and the
    /// options together.
    head_max_len: usize,
    /// Temperature of every question type, in the order choice, score,
    /// noul.
    #[serde(default)]
    temperature: Vec<f32>,
    /// Temperature of a question type at a given number of options.
    #[serde(default)]
    temperature_by_options: BTreeMap<String, f32>,
}

/// The special tokens the sequence builder needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SpecialIds {
    /// The token that opens the sequence.
    cls: u32,
    /// The token that closes a part of the sequence.
    sep: u32,
    /// The token that marks an option.
    mask: u32,
    /// The token a shorter row is padded with.
    pad: u32,
}

/// One typed choice question.
///
/// The instruction is what the model is asked and the options are the
/// answers it may give, as `(label, detail)` pairs. The order of the
/// options is the order the probabilities come back in.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceQuestion {
    /// The instruction of the question.
    pub instructions: String,
    /// The answers the question allows.
    pub options: Vec<(String, String)>,
}

/// The answer of one choice question.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceOutcome {
    /// Position of the chosen option in the list the caller passed.
    pub index: usize,
    /// Probability the model gave the chosen option, between 0 and 1.
    pub probability: f32,
    /// Probability of every option, in the order the caller passed them.
    pub probabilities: Vec<f32>,
}

/// A session, its tokenizer, and its calibration.
#[derive(Clone, Debug)]
struct Reader {
    /// The session that answers every question.
    session: Arc<Session>,
    /// The tokenizer of the model.
    tokenizer: Arc<Tokenizer>,
    /// The calibration of the model.
    config: Arc<LayaConfig>,
}

/// One model that is loaded and ready.
#[derive(Debug)]
struct Model {
    /// Identifier of the model.
    id: String,
    /// The device the session runs on.
    device: &'static str,
    /// The session and everything that goes with it.
    reader: Reader,
}

/// The engine that runs the built in decision model.
#[derive(Clone, Debug)]
pub struct LayaEngine {
    /// The store that holds the model files.
    store: Arc<LocalStore>,
    /// Threads one inference may use.
    threads: usize,
    /// The models that are loaded right now.
    loaded: Arc<Mutex<Vec<Model>>>,
}

impl LayaEngine {
    /// Create a new engine.
    pub fn new(store: Arc<LocalStore>, threads: usize) -> Self {
        Self {
            store,
            threads: threads.max(1),
            loaded: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Answer one choice question about one state.
    ///
    /// The options are `(label, detail)` pairs, in the order the caller
    /// wants them back: the outcome names the position of the chosen
    /// option and the probability of every option.
    ///
    /// This blocks on the model, so the caller runs it on the blocking
    /// pool of the runtime.
    pub fn decide(
        &self,
        id: &str,
        device: LocalDevice,
        state: &str,
        instructions: &str,
        options: &[(String, String)],
    ) -> Result<ChoiceOutcome, LocalError> {
        let questions = [ChoiceQuestion {
            instructions: instructions.to_string(),
            options: options.to_vec(),
        }];
        let mut outcomes = self.answer_choices(id, device, state, &questions)?;
        outcomes
            .pop()
            .ok_or_else(|| LocalError::Inference("the model answered nothing".to_string()))
    }

    /// Answer one or many choice questions about one state in one forward
    /// pass.
    ///
    /// The questions share a batch, so a turn that reads the value of
    /// every list entity of an intent pays one forward pass rather than
    /// one per entity. A question whose options do not fit the head budget
    /// is shrunk by the sequence builder rather than dropped, so the
    /// outcomes come back one per question, in the order the questions
    /// were passed.
    pub fn answer_choices(
        &self,
        id: &str,
        device: LocalDevice,
        state: &str,
        questions: &[ChoiceQuestion],
    ) -> Result<Vec<ChoiceOutcome>, LocalError> {
        if questions.is_empty() {
            return Err(LocalError::Inference(
                "a decision needs at least one question".to_string(),
            ));
        }
        if questions.iter().any(|question| question.options.is_empty()) {
            return Err(LocalError::Inference(
                "a question needs at least one option".to_string(),
            ));
        }
        let spec = self.store.installed_spec(id, Role::Decision)?;
        let reader = self.reader(spec, device)?;
        self.answer(&reader, state, questions)
    }

    /// Load the decision model, so the first turn of the daemon does not
    /// pay for the load.
    pub fn warm(&self, id: &str, device: LocalDevice) -> Result<(), LocalError> {
        let spec = self.store.installed_spec(id, Role::Decision)?;
        self.reader(spec, device)?;
        Ok(())
    }

    /// Run a batch of choice questions and read every chosen option back.
    fn answer(
        &self,
        reader: &Reader,
        state: &str,
        questions: &[ChoiceQuestion],
    ) -> Result<Vec<ChoiceOutcome>, LocalError> {
        let specials = SpecialIds {
            cls: special_id(&reader.tokenizer, CLS_TOKEN)?,
            sep: special_id(&reader.tokenizer, SEP_TOKEN)?,
            mask: special_id(&reader.tokenizer, MASK_TOKEN)?,
            pad: special_id(&reader.tokenizer, PAD_TOKEN)?,
        };
        let tokenizer = Arc::clone(&reader.tokenizer);
        let mut built: Vec<(Vec<i64>, Vec<i64>)> = Vec::with_capacity(questions.len());
        for question in questions {
            let texts: Vec<String> = question
                .options
                .iter()
                .map(|(label, detail)| render_option(label, detail))
                .collect();
            let (ids, markers) = build_sequence(
                &specials,
                |text| encode_text(&tokenizer, text),
                state,
                &question.instructions,
                &texts,
                reader.config.max_len,
                reader.config.head_max_len,
            )?;
            if markers.is_empty() {
                return Err(LocalError::Inference(
                    "no option fitted in the head of the sequence".to_string(),
                ));
            }
            built.push((ids, markers));
        }

        let batch = Batch::read(&built, i64::from(specials.pad));
        let outputs = reader
            .session
            .run(batch.inputs()?)
            .map_err(|err| LocalError::Inference(err.to_string()))?;
        let value = outputs
            .get(LOGITS)
            .ok_or_else(|| LocalError::Inference(format!("the model answered no `{LOGITS}`")))?;
        let view = value
            .try_extract_tensor::<f32>()
            .map_err(|err| LocalError::Inference(err.to_string()))?;
        let logits: Vec<f32> = view.iter().copied().collect();

        let mut outcomes = Vec::with_capacity(built.len());
        for (row, (_, markers)) in built.iter().enumerate() {
            let width = markers.len();
            let start = row * batch.width;
            let end = start + width;
            if end > logits.len() {
                return Err(LocalError::Inference(format!(
                    "the model scored no option of question {row}"
                )));
            }
            let temperature = reader
                .config
                .temperature_by_options
                .get(&temp_bucket(width))
                .copied()
                .or_else(|| reader.config.temperature.first().copied())
                .unwrap_or(1.0);
            let scaled: Vec<f32> = logits[start..end]
                .iter()
                .map(|logit| logit / temperature)
                .collect();
            let probabilities = softmax(&scaled);
            let index = argmax(&probabilities);
            outcomes.push(ChoiceOutcome {
                index,
                probability: probabilities.get(index).copied().unwrap_or(0.0),
                probabilities,
            });
        }
        Ok(outcomes)
    }

    /// The session of one model on one device, loading it when it is new.
    fn reader(&self, spec: &'static ModelSpec, device: LocalDevice) -> Result<Reader, LocalError> {
        let device = device::active_device(device).map_err(|err| LocalError::Device {
            device: err.device,
            reason: err.reason,
        })?;
        let mut loaded = self.lock();
        if let Some(found) = loaded
            .iter()
            .find(|model| model.id == spec.id && model.device == device)
        {
            return Ok(found.reader.clone());
        }
        if loaded.len() >= MAX_LOADED {
            loaded.remove(0);
        }
        let fresh = self.load(spec, device)?;
        let reader = fresh.reader.clone();
        loaded.push(fresh);
        Ok(reader)
    }

    /// Load one model onto one device.
    fn load(&self, spec: &'static ModelSpec, device: &'static str) -> Result<Model, LocalError> {
        let dir = self.store.dir(spec.id);
        let graph = find_file(spec, ".onnx")
            .map(|file| dir.join(file))
            .ok_or_else(|| LocalError::Load(format!("{} carries no graph", spec.id)))?;
        let tokenizer_path = find_file(spec, "tokenizer.json")
            .map(|file| dir.join(file))
            .ok_or_else(|| LocalError::Load(format!("{} carries no tokenizer", spec.id)))?;
        let config_path = find_file(spec, "laya_config.json")
            .map(|file| dir.join(file))
            .ok_or_else(|| LocalError::Load(format!("{} carries no calibration", spec.id)))?;

        let mut builder = Session::builder()
            .map_err(|err| LocalError::Load(err.to_string()))?
            .with_intra_threads(self.threads)
            .map_err(|err| LocalError::Load(err.to_string()))?;
        if device == DEVICE_CUDA {
            builder = builder
                .with_execution_providers([CUDAExecutionProvider::default().build()])
                .map_err(|err| LocalError::Load(err.to_string()))?;
        } else if device == DEVICE_GPU {
            builder = builder
                .with_execution_providers([OpenVINOExecutionProvider::default().build()])
                .map_err(|err| LocalError::Load(err.to_string()))?;
        }
        let session = builder
            .commit_from_file(&graph)
            .map_err(|err| LocalError::Load(err.to_string()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|err| LocalError::Tokenize(err.to_string()))?;
        let config: LayaConfig = serde_json::from_slice(
            &std::fs::read(&config_path).map_err(|err| LocalError::Load(err.to_string()))?,
        )
        .map_err(|err| LocalError::Load(err.to_string()))?;

        tracing::info!(
            model = spec.id,
            role = spec.role.label(),
            device,
            threads = self.threads,
            max_len = config.max_len,
            head_max_len = config.head_max_len,
            "the built in decision model of the router is ready"
        );
        Ok(Model {
            id: spec.id.to_string(),
            device,
            reader: Reader {
                session: Arc::new(session),
                tokenizer: Arc::new(tokenizer),
                config: Arc::new(config),
            },
        })
    }

    /// Lock the loaded models.
    fn lock(&self) -> MutexGuard<'_, Vec<Model>> {
        self.loaded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The path of the first file of a model whose path ends with a suffix.
fn find_file(spec: &ModelSpec, suffix: &str) -> Option<&'static str> {
    spec.files
        .iter()
        .find(|file| file.path.ends_with(suffix))
        .map(|file| file.path)
}

/// Render one option as a line of the option list.
fn render_option(label: &str, detail: &str) -> String {
    if detail.trim().is_empty() {
        return label.to_string();
    }
    format!("{label}: {detail}")
}

/// Read the identifier of one special token.
fn special_id(tokenizer: &Tokenizer, token: &str) -> Result<u32, LocalError> {
    tokenizer
        .token_to_id(token)
        .ok_or_else(|| LocalError::Tokenize(format!("the tokenizer carries no `{token}` token")))
}

/// Turn one text into tokens without the special tokens the builder adds
/// itself.
fn encode_text(tokenizer: &Tokenizer, text: &str) -> Result<Vec<u32>, LocalError> {
    tokenizer
        .encode(text, false)
        .map(|encoding| encoding.get_ids().to_vec())
        .map_err(|err| LocalError::Tokenize(err.to_string()))
}

/// Build the sequence of one choice question and the position of every
/// option marker.
///
/// This follows the reference implementation token for token: the head
/// holds the instructions and the marked options, the state follows, and
/// the options share the head budget. An option that does not fit is cut,
/// and when the options take the whole budget every option is cut evenly
/// rather than the first ones crowding the rest out.
fn build_sequence<F>(
    specials: &SpecialIds,
    mut encode: F,
    state: &str,
    instructions: &str,
    options: &[String],
    max_len: usize,
    head_max_len: usize,
) -> Result<(Vec<i64>, Vec<i64>), LocalError>
where
    F: FnMut(&str) -> Result<Vec<u32>, LocalError>,
{
    let scrub = |text: &str| text.replace(MASK_TOKEN, " ");

    let mut head = encode(&format!(
        "{QTYPE_NAME_CHOICE} question: {}",
        scrub(instructions)
    ))?;
    let mut option_ids: Vec<Vec<u32>> = Vec::with_capacity(options.len());
    for option in options {
        let mut ids = vec![specials.mask];
        ids.extend(
            encode(&format!(" {}", scrub(option)))?
                .into_iter()
                .take(OPTION_TOKENS),
        );
        option_ids.push(ids);
    }

    let total = |ids: &[Vec<u32>]| -> i64 { ids.iter().map(|o| o.len() as i64).sum() };
    let budget = head_max_len as i64 - total(&option_ids);
    if budget < HEAD_MIN {
        let per = std::cmp::max(
            OPTION_FLOOR as i64,
            (head_max_len as i64 - HEAD_MIN) / std::cmp::max(1, option_ids.len()) as i64,
        ) as usize;
        for ids in &mut option_ids {
            ids.truncate(per);
        }
    }
    let budget = head_max_len as i64 - total(&option_ids);
    let keep = std::cmp::max(HEAD_KEEP as i64, budget).max(0) as usize;
    head.truncate(keep);

    let mut sequence: Vec<u32> = Vec::with_capacity(max_len);
    sequence.push(specials.cls);
    sequence.extend(head);
    sequence.push(specials.sep);
    let mut markers: Vec<usize> = Vec::with_capacity(option_ids.len());
    for ids in &option_ids {
        markers.push(sequence.len());
        sequence.extend(ids);
    }
    sequence.push(specials.sep);

    let room = max_len.saturating_sub(sequence.len() + 1);
    let trimmed = encode(&scrub(state))?;
    sequence.extend(trimmed.into_iter().take(room));
    sequence.push(specials.sep);

    sequence.truncate(max_len);
    let markers: Vec<i64> = markers
        .into_iter()
        .filter(|position| *position < max_len)
        .map(|position| position as i64)
        .collect();
    let ids: Vec<i64> = sequence.into_iter().map(i64::from).collect();
    Ok((ids, markers))
}

/// One batch of choice questions, padded so every row has the same length
/// and every question the same number of option slots.
///
/// A padded token carries an attention of zero, which keeps a short row
/// from reading the padding of a long one, and a padded marker is masked
/// out, so a question with three options is not scored on the twenty slots
/// of its neighbour.
#[derive(Debug)]
struct Batch {
    /// Number of questions, one per row.
    rows: usize,
    /// Number of tokens in one row.
    length: usize,
    /// Number of option slots in one row.
    width: usize,
    /// The tokens of every row.
    ids: Vec<i64>,
    /// One for a real token and zero for a padding token.
    attention: Vec<i64>,
    /// The position of every option marker.
    positions: Vec<i64>,
    /// Whether an option slot holds a marker of its own.
    mask: Vec<bool>,
}

impl Batch {
    /// Read the sequences of many questions into one batch.
    fn read(questions: &[(Vec<i64>, Vec<i64>)], pad: i64) -> Self {
        let rows = questions.len();
        let length = questions
            .iter()
            .map(|(ids, _)| ids.len())
            .max()
            .unwrap_or(0)
            .max(1);
        let width = questions
            .iter()
            .map(|(_, markers)| markers.len())
            .max()
            .unwrap_or(0)
            .max(1);
        let mut ids = vec![pad; rows * length];
        let mut attention = vec![0_i64; rows * length];
        let mut positions = vec![0_i64; rows * width];
        let mut mask = vec![false; rows * width];

        for (row, (row_ids, row_markers)) in questions.iter().enumerate() {
            for (column, id) in row_ids.iter().enumerate().take(length) {
                ids[row * length + column] = *id;
                attention[row * length + column] = 1;
            }
            for (column, position) in row_markers.iter().enumerate().take(width) {
                positions[row * width + column] = *position;
                mask[row * width + column] = true;
            }
        }
        Self {
            rows,
            length,
            width,
            ids,
            attention,
            positions,
            mask,
        }
    }

    /// The inputs of one session.
    fn inputs(&self) -> Result<Vec<(&'static str, SessionInputValue<'static>)>, LocalError> {
        let rows = self.rows as i64;
        let qtype = vec![QTYPE_CHOICE; self.rows];
        Ok(vec![
            (
                INPUT_IDS,
                int_tensor(&[rows, self.length as i64], self.ids.clone())?,
            ),
            (
                ATTENTION_MASK,
                int_tensor(&[rows, self.length as i64], self.attention.clone())?,
            ),
            (
                MARKER_POS,
                int_tensor(&[rows, self.width as i64], self.positions.clone())?,
            ),
            (
                MARKER_MASK,
                bool_tensor(&[rows, self.width as i64], self.mask.clone())?,
            ),
            (QTYPE, int_tensor(&[rows], qtype)?),
        ])
    }
}

/// Build one integer tensor out of a shape and its numbers.
fn int_tensor(shape: &[i64], data: Vec<i64>) -> Result<SessionInputValue<'static>, LocalError> {
    let value: DynValue = Tensor::from_array((shape.to_vec(), data))
        .map_err(|err| LocalError::Inference(err.to_string()))?
        .into_dyn();
    Ok(SessionInputValue::from(value))
}

/// Build one boolean tensor out of a shape and its numbers.
fn bool_tensor(shape: &[i64], data: Vec<bool>) -> Result<SessionInputValue<'static>, LocalError> {
    let value: DynValue = Tensor::from_array((shape.to_vec(), data))
        .map_err(|err| LocalError::Inference(err.to_string()))?
        .into_dyn();
    Ok(SessionInputValue::from(value))
}

/// The number of options a bucket names.
fn size_bucket(options: usize) -> &'static str {
    match options {
        0..=2 => "2",
        3..=5 => "3-5",
        6..=10 => "6-10",
        _ => "11+",
    }
}

/// The key of the temperature of one answer shape.
fn temp_bucket(options: usize) -> String {
    format!("{QTYPE_NAME_CHOICE}:{}", size_bucket(options))
}

/// Turn scaled logits into a probability distribution.
fn softmax(logits: &[f32]) -> Vec<f32> {
    let Some(maximum) = logits
        .iter()
        .copied()
        .fold(None, |best: Option<f32>, value| {
            Some(best.map_or(value, |best| best.max(value)))
        })
    else {
        return Vec::new();
    };
    let exponentials: Vec<f32> = logits.iter().map(|value| (value - maximum).exp()).collect();
    let sum: f32 = exponentials.iter().sum();
    if sum <= 0.0 {
        return vec![0.0; logits.len()];
    }
    exponentials.into_iter().map(|value| value / sum).collect()
}

/// The position of the largest number, the first one when two are equal.
fn argmax(values: &[f32]) -> usize {
    let mut best = 0;
    for (index, value) in values.iter().enumerate().skip(1) {
        if *value > values[best] {
            best = index;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tokenizer that reads every character as its own token, so the
    /// sequence builder can be tested without the weights.
    fn characters(text: &str) -> Result<Vec<u32>, LocalError> {
        Ok(text.chars().map(|character| character as u32).collect())
    }

    fn specials() -> SpecialIds {
        SpecialIds {
            cls: 1,
            sep: 2,
            mask: 3,
            pad: 0,
        }
    }

    #[test]
    fn a_batch_pads_a_short_question_without_scoring_it() {
        let questions = vec![
            (vec![1_i64, 2, 3, 4], vec![1_i64, 3]),
            (vec![1_i64, 2], vec![1_i64]),
        ];
        let batch = Batch::read(&questions, 0);

        assert_eq!(batch.rows, 2);
        assert_eq!(batch.length, 4);
        assert_eq!(batch.width, 2);
        // The second row is padded with the pad token and no attention.
        assert_eq!(batch.ids[4], 1);
        assert_eq!(batch.ids[5], 2);
        assert_eq!(batch.ids[6], 0);
        assert_eq!(batch.attention[6], 0);
        // The second option slot of the second row holds no marker.
        assert!(batch.mask[2]);
        assert!(!batch.mask[3]);
    }

    #[test]
    fn the_sequence_marks_every_option_and_ends_with_the_state() {
        let options = vec!["open application".to_string(), "close window".to_string()];
        let (ids, markers) = build_sequence(
            &specials(),
            characters,
            "launch firefox",
            "Which option does the message ask for?",
            &options,
            512,
            192,
        )
        .expect("the sequence is built");

        assert_eq!(ids[0], 1, "the sequence opens with the class token");
        assert!(ids.contains(&2), "the sequence holds separators");
        assert_eq!(markers.len(), 2, "one marker per option");

        // The marker of the first option holds the mask token.
        let first = markers[0] as usize;
        assert_eq!(ids[first], 3);
        // The second marker comes after the first option.
        assert!(markers[1] > markers[0]);
    }

    #[test]
    fn a_state_that_does_not_fit_is_cut_and_the_sequence_keeps_its_length() {
        let long = "x".repeat(4000);
        let options = vec!["a".to_string(), "b".to_string()];
        let (ids, markers) = build_sequence(&specials(), characters, &long, "q", &options, 64, 32)
            .expect("the sequence is built");

        assert!(ids.len() <= 64, "the sequence fits the context");
        assert!(markers.iter().all(|marker| (*marker as usize) < ids.len()));
    }

    #[test]
    fn many_long_options_share_the_head_budget() {
        let options: Vec<String> = (0..40)
            .map(|index| "word ".repeat(20) + &index.to_string())
            .collect();
        let (ids, markers) = build_sequence(
            &specials(),
            characters,
            "state",
            "which one",
            &options,
            512,
            192,
        )
        .expect("the sequence is built");

        // Every option keeps a marker, and the head stayed within budget.
        assert_eq!(markers.len(), 40);
        let last = *markers.last().expect("there is a last marker") as usize;
        assert!(last < ids.len());
    }

    #[test]
    fn the_mask_token_of_a_user_is_scrubbed() {
        let options = vec!["[MASK] sneaky".to_string()];
        let (ids, _markers) = build_sequence(
            &specials(),
            characters,
            "the state says [MASK] too",
            "pick one",
            &options,
            512,
            192,
        )
        .expect("the sequence is built");

        // Only the token of the builder itself (3) survives; the text of
        // the user became spaces.
        assert_eq!(ids.iter().filter(|id| **id == 3).count(), 1);
    }

    #[test]
    fn the_temperature_bucket_names_the_answer_shape() {
        assert_eq!(temp_bucket(2), "choice:2");
        assert_eq!(temp_bucket(4), "choice:3-5");
        assert_eq!(temp_bucket(9), "choice:6-10");
        assert_eq!(temp_bucket(40), "choice:11+");
    }

    #[test]
    fn the_softmax_is_a_distribution_and_keeps_the_order() {
        let probabilities = softmax(&[2.0, 1.0, 0.0]);
        let sum: f32 = probabilities.iter().sum();
        assert!((sum - 1.0).abs() < 1e-5, "the probabilities sum to one");
        assert!(probabilities[0] > probabilities[1]);
        assert!(probabilities[1] > probabilities[2]);
        assert_eq!(argmax(&probabilities), 0);
    }

    #[test]
    fn a_tie_reads_as_the_first_option() {
        assert_eq!(argmax(&[1.0, 1.0, 0.5]), 0);
    }

    #[test]
    fn an_option_without_a_detail_is_its_label_alone() {
        assert_eq!(render_option("open firefox", ""), "open firefox");
        assert_eq!(render_option("open firefox", "   "), "open firefox");
        assert_eq!(
            render_option("open firefox", "launch the browser"),
            "open firefox: launch the browser"
        );
    }

    /// The directory the downloaded models live in.
    ///
    /// The tests that read a real model are marked `#[ignore]`, because a
    /// model is a download of more than a gigabyte. Point
    /// `ALICE_ROUTER_MODELS_DIR` at the directory of the models and run
    /// them with `cargo test -- --ignored`.
    fn models_root() -> std::path::PathBuf {
        std::env::var("ALICE_ROUTER_MODELS_DIR").map_or_else(
            |_| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/router"),
            std::path::PathBuf::from,
        )
    }

    /// The question and the answers the fallback turn really asks.
    ///
    /// These are the constants of the resolver, so the measurement below
    /// guards the wording the daemon ships rather than a copy of it.
    fn need_question() -> (&'static str, Vec<(String, String)>) {
        (
            crate::resolver::service::NEED_INSTRUCTIONS,
            crate::resolver::service::need_options(),
        )
    }

    /// Measure what the built in decision model reads a message as needing.
    ///
    /// The model answers a choice about the state, so the wording of the
    /// question and of the two answers is what separates a greeting from a
    /// command. A wording that only names the two nouns leaves the model
    /// near a coin flip on every message; the wording the daemon ships
    /// separates them. This test is the measurement behind that wording:
    /// it asserts the separation rather than printing it, so a change to
    /// the wording that loses it fails here before it reaches a turn.
    /// Measure what the earlier turns of a conversation do to the answer.
    ///
    /// This is the measurement behind `need_state`, which gives the
    /// question the message alone. A turn sends the earlier turns with the
    /// message, so the state would be a transcript rather than one line,
    /// and the transcript moves the reading: the same greeting is read as
    /// words alone and as a task of the machine once a turn before it ran a
    /// command. It prints rather than asserts, because it measures what a
    /// reader must not be handed and not what the daemon must do.
    #[test]
    #[ignore = "reads the downloaded models from disk"]
    fn the_earlier_turns_bend_what_a_message_needs() {
        let engine = LayaEngine::new(
            Arc::new(LocalStore::new(models_root())),
            std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get),
        );
        let (instructions, options) = need_question();
        let one_command = "user: open firefox\nassistant: Opened firefox.\n";
        let one_script =
            "user: create a file on my desktop\nassistant: This script creates a file.\n";
        let states = [
            "user: hello there".to_string(),
            format!("{one_command}user: hello there"),
            format!("{one_script}user: hello there"),
            format!("{one_command}user: how are you"),
            format!("{one_command}user: do that again"),
            format!("{one_script}user: do that again"),
        ];
        for state in states {
            let outcome = engine
                .decide("laya", LocalDevice::Cpu, &state, instructions, &options)
                .expect("the built in decision model answers");
            println!(
                "{:<62} -> {:<38} p={:?}",
                state.replace('\n', " | "),
                render_option(&options[outcome.index].0, &options[outcome.index].1),
                outcome.probabilities
            );
        }
    }

    #[test]
    #[ignore = "reads the downloaded models from disk"]
    fn the_question_separates_a_greeting_from_a_command() {
        let engine = LayaEngine::new(
            Arc::new(LocalStore::new(models_root())),
            std::thread::available_parallelism().map_or(4, std::num::NonZeroUsize::get),
        );
        let (instructions, options) = need_question();
        // The message and the answer it needs: `false` wants words and
        // `true` wants a task of the machine.
        let messages = [
            ("hello there", false),
            ("how are you", false),
            ("thanks, that helps", false),
            ("what is the capital of France", false),
            ("open firefox", true),
            ("create a file on my desktop", true),
            ("list the files of my downloads folder", true),
        ];
        for (message, wants_a_script) in messages {
            let outcome = engine
                .decide("laya", LocalDevice::Cpu, message, instructions, &options)
                .expect("the built in decision model answers");
            assert_eq!(
                outcome.index == crate::resolver::service::NEED_SCRIPT_INDEX,
                wants_a_script,
                "`{message}` read as {} at p={:?}",
                render_option(&options[outcome.index].0, &options[outcome.index].1),
                outcome.probabilities
            );
            assert!(
                outcome.probability >= crate::resolver::service::NEED_FLOOR,
                "`{message}` read as {} at only p={}",
                render_option(&options[outcome.index].0, &options[outcome.index].1),
                outcome.probability
            );
        }
    }
}
