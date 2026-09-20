//! Inference of the built in models of the router.
//!
//! One loaded model serves every text. Loading costs about a second and
//! one text of a few words stays under ten milliseconds on a processor, so
//! the engine keeps the session and loads a model again only when the
//! model or the device changes.
//!
//! Inference is blocking work, so the resolver runs it on the blocking
//! pool of the runtime. A slow turn must not stall the socket stream.
//!
//! Both roles read the same kind of graph. An embedding model reports one
//! vector per token, and this module pools the tokens into one vector per
//! text. A reranker reports one score per pair, and this module maps that
//! score into a probability. The graph decides which inputs it reads, so a
//! model that declares no token types is read without them.

use std::sync::{Arc, Mutex, MutexGuard};

use alice_core::config::LocalDevice;
use ort::execution_providers::CUDAExecutionProvider;
use ort::session::Session;
use ort::session::SessionInputValue;
use ort::value::{DynValue, Tensor};
use tokenizers::{Encoding, Tokenizer, TruncationParams};

use crate::resolver::device::{self, DEVICE_CUDA};
use crate::resolver::local::catalog::{ModelSpec, Role};
use crate::resolver::local::error::LocalError;
use crate::resolver::local::store::LocalStore;

/// Largest number of tokens one text may hold.
///
/// A message is a sentence and an intent is a short document, so the cap
/// never cuts a turn short. It keeps one long list entry from taking the
/// whole window of the model.
const MAX_TOKENS: usize = 512;

/// Largest number of loaded models the daemon keeps.
///
/// An embedding model and a reranker are read at the same time, so two
/// sessions cover every role. A third model replaces the oldest one.
const MAX_LOADED: usize = 2;

/// Name of the tensor that holds the tokens.
const INPUT_IDS: &str = "input_ids";

/// Name of the tensor that marks the real tokens.
const ATTENTION_MASK: &str = "attention_mask";

/// Name of the tensor that tells the sentence of a token.
const TOKEN_TYPE_IDS: &str = "token_type_ids";

/// Name of the output of an embedding model.
const LAST_HIDDEN_STATE: &str = "last_hidden_state";

/// Name of the output of a reranker.
const LOGITS: &str = "logits";

/// One model that is loaded and ready.
#[derive(Debug)]
struct Model {
    /// Identifier of the model.
    id: String,
    /// The device the session runs on.
    device: &'static str,
    /// The session that answers every text.
    session: Arc<Session>,
    /// The tokenizer of the model.
    tokenizer: Arc<Tokenizer>,
}

/// A session and the tokenizer that go with it.
#[derive(Clone, Debug)]
struct Reader {
    /// The session that answers.
    session: Arc<Session>,
    /// The tokenizer of the model.
    tokenizer: Arc<Tokenizer>,
}

/// The engine that runs the built in models of the router.
#[derive(Clone, Debug)]
pub struct LocalEngine {
    store: Arc<LocalStore>,
    threads: usize,
    loaded: Arc<Mutex<Vec<Model>>>,
}

impl LocalEngine {
    /// Create a new engine.
    pub fn new(store: Arc<LocalStore>, threads: usize) -> Self {
        Self {
            store,
            threads: threads.max(1),
            loaded: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Turn each text into one unit vector.
    ///
    /// This blocks on the model, so the caller runs it on the blocking
    /// pool of the runtime.
    pub fn embed(
        &self,
        id: &str,
        device: LocalDevice,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, LocalError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let spec = self.store.installed_spec(id, Role::Embedding)?;
        let reader = self.reader(spec, device)?;
        let encodings = encode(&reader.tokenizer, None, texts)?;
        let batch = Batch::read(&rows_of(&encodings), pad_id(&reader.tokenizer));
        let hidden = self.run(&reader, &batch)?;
        Ok(pool(&hidden, &batch.mask))
    }

    /// Score how well one message fits each candidate.
    ///
    /// The model reads the message and one candidate in one sequence, so
    /// the score says how the two fit rather than how each reads alone.
    /// This blocks on the model, so the caller runs it on the blocking
    /// pool of the runtime.
    pub fn rerank(
        &self,
        id: &str,
        device: LocalDevice,
        message: &str,
        candidates: &[String],
    ) -> Result<Vec<f32>, LocalError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        let spec = self.store.installed_spec(id, Role::Reranker)?;
        let reader = self.reader(spec, device)?;
        let encodings = encode(&reader.tokenizer, Some(message), candidates)?;
        let batch = Batch::read(&rows_of(&encodings), pad_id(&reader.tokenizer));
        let scored = self.run(&reader, &batch)?;

        let rows = scored.shape.first().copied().unwrap_or(0);
        let width = scored.shape.get(1).copied().unwrap_or(1).max(1);
        let mut scores = Vec::with_capacity(rows);
        for row in 0..rows {
            let logit = scored.data.get(row * width).copied().unwrap_or(0.0);
            scores.push(sigmoid(logit));
        }
        Ok(scores)
    }

    /// Run one batch and read the first output as numbers.
    fn run(&self, reader: &Reader, batch: &Batch) -> Result<Read, LocalError> {
        let inputs = batch.inputs(&reader.session)?;
        let outputs = reader
            .session
            .run(inputs)
            .map_err(|err| LocalError::Inference(err.to_string()))?;
        let named = if outputs.contains_key(LAST_HIDDEN_STATE) {
            LAST_HIDDEN_STATE
        } else if outputs.contains_key(LOGITS) {
            LOGITS
        } else {
            ""
        };
        let value = if named.is_empty() {
            &outputs[0]
        } else {
            &outputs[named]
        };
        let view = value
            .try_extract_tensor::<f32>()
            .map_err(|err| LocalError::Inference(err.to_string()))?;
        Ok(Read {
            shape: view.shape().to_vec(),
            data: view.iter().copied().collect(),
        })
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
            return Ok(Reader {
                session: Arc::clone(&found.session),
                tokenizer: Arc::clone(&found.tokenizer),
            });
        }
        if loaded.len() >= MAX_LOADED {
            loaded.remove(0);
        }
        let fresh = self.load(spec, device)?;
        let reader = Reader {
            session: Arc::clone(&fresh.session),
            tokenizer: Arc::clone(&fresh.tokenizer),
        };
        loaded.push(fresh);
        Ok(reader)
    }

    /// Load one model onto one device.
    fn load(&self, spec: &'static ModelSpec, device: &'static str) -> Result<Model, LocalError> {
        let dir = self.store.dir(spec.id);
        let graph = spec
            .files
            .iter()
            .find(|file| file.path.ends_with(".onnx"))
            .map(|file| dir.join(file.path))
            .ok_or_else(|| LocalError::Load(format!("{} carries no graph", spec.id)))?;
        let tokenizer_path = dir.join("tokenizer.json");

        let mut builder = Session::builder()
            .map_err(|err| LocalError::Load(err.to_string()))?
            .with_intra_threads(self.threads)
            .map_err(|err| LocalError::Load(err.to_string()))?;
        if device == DEVICE_CUDA {
            builder = builder
                .with_execution_providers([CUDAExecutionProvider::default().build()])
                .map_err(|err| LocalError::Load(err.to_string()))?;
        }
        let session = builder
            .commit_from_file(&graph)
            .map_err(|err| LocalError::Load(err.to_string()))?;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|err| LocalError::Tokenize(err.to_string()))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TOKENS,
                ..TruncationParams::default()
            }))
            .map_err(|err| LocalError::Tokenize(err.to_string()))?;

        tracing::info!(
            model = spec.id,
            role = spec.role.label(),
            device,
            threads = self.threads,
            "a built in model of the router is ready"
        );
        Ok(Model {
            id: spec.id.to_string(),
            device,
            session: Arc::new(session),
            tokenizer: Arc::new(tokenizer),
        })
    }

    /// Lock the loaded models.
    fn lock(&self) -> MutexGuard<'_, Vec<Model>> {
        self.loaded
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// One output of one inference, as numbers with its shape.
#[derive(Debug)]
struct Read {
    /// The shape the model answered with.
    shape: Vec<usize>,
    /// The numbers of the output, in the order of the shape.
    data: Vec<f32>,
}

/// One row of tokens of one text.
#[derive(Clone, Copy, Debug)]
struct Tokens<'a> {
    /// The tokens.
    ids: &'a [u32],
    /// One for a real token and zero for a padding token.
    mask: &'a [u32],
    /// The sentence a token belongs to.
    types: &'a [u32],
}

/// Read the tokens of many encodings.
fn rows_of(encodings: &[Encoding]) -> Vec<Tokens<'_>> {
    encodings
        .iter()
        .map(|encoding| Tokens {
            ids: encoding.get_ids(),
            mask: encoding.get_attention_mask(),
            types: encoding.get_type_ids(),
        })
        .collect()
}

/// One batch of tokens, padded so every row has the same length.
#[derive(Debug)]
struct Batch {
    /// Number of rows, one per text.
    rows: usize,
    /// Number of tokens in one row.
    cols: usize,
    /// The tokens.
    ids: Vec<i64>,
    /// One for a real token and zero for a padding token.
    mask: Vec<i64>,
    /// The sentence a token belongs to, zero for the first.
    types: Vec<i64>,
}

impl Batch {
    /// Read the tokens of many rows into one batch.
    fn read(rows: &[Tokens<'_>], pad: i64) -> Self {
        let cols = rows
            .iter()
            .map(|row| row.ids.len())
            .max()
            .unwrap_or(0)
            .max(1);
        let count = rows.len();
        let mut ids = vec![pad; count * cols];
        let mut mask = vec![0_i64; count * cols];
        let mut types = vec![0_i64; count * cols];

        for (row, tokens) in rows.iter().enumerate() {
            for (column, token) in tokens.ids.iter().enumerate() {
                if column >= cols {
                    break;
                }
                let position = row * cols + column;
                ids[position] = i64::from(*token);
                mask[position] = i64::from(tokens.mask.get(column).copied().unwrap_or(1));
                types[position] = i64::from(tokens.types.get(column).copied().unwrap_or(0));
            }
        }
        Self {
            rows: count,
            cols,
            ids,
            mask,
            types,
        }
    }

    /// The inputs of one session, with only the tensors the graph reads.
    fn inputs(
        &self,
        session: &Session,
    ) -> Result<Vec<(&'static str, SessionInputValue<'static>)>, LocalError> {
        let declares = |name: &str| {
            session
                .inputs
                .iter()
                .any(|input| input.name.as_str() == name)
        };
        if !declares(INPUT_IDS) {
            return Err(LocalError::Load(format!(
                "the graph declares no `{INPUT_IDS}` input"
            )));
        }
        let shape = vec![self.rows as i64, self.cols as i64];
        let mut inputs: Vec<(&'static str, SessionInputValue<'static>)> = Vec::new();
        inputs.push((INPUT_IDS, tensor(&shape, self.ids.clone())?));
        if declares(ATTENTION_MASK) {
            inputs.push((ATTENTION_MASK, tensor(&shape, self.mask.clone())?));
        }
        if declares(TOKEN_TYPE_IDS) {
            inputs.push((TOKEN_TYPE_IDS, tensor(&shape, self.types.clone())?));
        }
        Ok(inputs)
    }
}

/// Build one integer tensor out of a shape and its numbers.
fn tensor(shape: &[i64], data: Vec<i64>) -> Result<SessionInputValue<'static>, LocalError> {
    let value: DynValue = Tensor::from_array((shape.to_vec(), data))
        .map_err(|err| LocalError::Inference(err.to_string()))?
        .into_dyn();
    Ok(SessionInputValue::from(value))
}

/// Turn many texts into tokens, and, with a query, every text into a pair
/// with that query.
fn encode(
    tokenizer: &Tokenizer,
    query: Option<&str>,
    texts: &[String],
) -> Result<Vec<Encoding>, LocalError> {
    let mut encodings = Vec::with_capacity(texts.len());
    for text in texts {
        let encoded = match query {
            Some(query) => tokenizer.encode((query, text.as_str()), true),
            None => tokenizer.encode(text.as_str(), true),
        }
        .map_err(|err| LocalError::Tokenize(err.to_string()))?;
        encodings.push(encoded);
    }
    Ok(encodings)
}

/// The token the tokenizer pads with.
fn pad_id(tokenizer: &Tokenizer) -> i64 {
    tokenizer
        .get_padding()
        .map_or(0, |padding| i64::from(padding.pad_id))
}

/// Map a score of a reranker into a probability between 0 and 1.
fn sigmoid(logit: f32) -> f32 {
    1.0 / (1.0 + (-logit).exp())
}

/// Fold the tokens of every row into one unit vector per row.
///
/// A token the mask does not mark is padding and stays out of the
/// average, so a short text of a batch reads the same vector as it would
/// alone.
fn pool(read: &Read, mask: &[i64]) -> Vec<Vec<f32>> {
    let Some(&rows) = read.shape.first() else {
        return Vec::new();
    };
    let tokens = read.shape.get(1).copied().unwrap_or(0);
    let Some(&width) = read.shape.get(2) else {
        return Vec::new();
    };

    let mut vectors = Vec::with_capacity(rows);
    for row in 0..rows {
        let mut sums = vec![0.0_f32; width];
        let mut counted = 0.0_f32;
        for token in 0..tokens {
            if mask.get(row * tokens + token).copied().unwrap_or(1) == 0 {
                continue;
            }
            counted += 1.0;
            let start = (row * tokens + token) * width;
            for (position, sum) in sums.iter_mut().enumerate() {
                *sum += read.data.get(start + position).copied().unwrap_or(0.0);
            }
        }
        if counted > 0.0 {
            for sum in &mut sums {
                *sum /= counted;
            }
        }
        vectors.push(crate::resolver::router::embed::unit(sums));
    }
    vectors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::local::catalog::{BGE_SMALL, RERANKER};

    /// Build an engine over a store of a directory of its own.
    fn engine() -> (LocalEngine, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("alice-encoder-{}", uuid::Uuid::new_v4()));
        let store = Arc::new(LocalStore::new(root.clone()));
        (LocalEngine::new(store, 2), root)
    }

    #[test]
    fn a_score_of_a_reranker_reads_as_a_probability() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.99);
        assert!(sigmoid(-10.0) < 0.01);
    }

    #[test]
    fn the_pool_of_a_row_ignores_the_tokens_it_is_not_told_about() {
        // Two rows of three tokens with two numbers each. The second token
        // of the first row is padding and stays out of the average, so the
        // row reads the same vector as a text of the two real tokens.
        let read = Read {
            shape: vec![2, 3, 2],
            data: vec![
                2.0, 4.0, 100.0, 100.0, 4.0, 8.0, // first row
                1.0, 1.0, 1.0, 1.0, 1.0, 1.0, // second row
            ],
        };
        let mask = vec![1, 0, 1, 1, 1, 1];

        let vectors = pool(&read, &mask); // The average of (2, 4) and (4, 8) is (3, 6), and (1, 1) with
                                          // itself is (1, 1). Both read as one unit of length.
        let three = 3.0_f32 / 45.0_f32.sqrt();
        let six = 6.0_f32 / 45.0_f32.sqrt();
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!((vectors[0][0] - three).abs() < 1e-5, "{:?}", vectors[0]);
        assert!((vectors[0][1] - six).abs() < 1e-5, "{:?}", vectors[0]);
        assert!((vectors[1][0] - half).abs() < 1e-5, "{:?}", vectors[1]);
        assert!((vectors[1][1] - half).abs() < 1e-5, "{:?}", vectors[1]);
    }

    #[test]
    fn a_read_without_a_row_reads_no_vector() {
        assert!(pool(
            &Read {
                shape: vec![],
                data: vec![]
            },
            &[]
        )
        .is_empty());
        assert!(pool(
            &Read {
                shape: vec![1, 2],
                data: vec![1.0, 2.0]
            },
            &[1, 1]
        )
        .is_empty());
    }

    #[test]
    fn a_batch_pads_every_row_to_the_longest_one() {
        let short = Tokens {
            ids: &[1, 2],
            mask: &[1, 1],
            types: &[0, 0],
        };
        let long = Tokens {
            ids: &[3, 4, 5],
            mask: &[1, 1, 1],
            types: &[0, 0, 0],
        };

        let batch = Batch::read(&[short, long], 9);

        assert_eq!(batch.rows, 2);
        assert_eq!(batch.cols, 3);
        assert_eq!(batch.ids, vec![1, 2, 9, 3, 4, 5]);
        assert_eq!(batch.mask, vec![1, 1, 0, 1, 1, 1]);
        assert_eq!(batch.types, vec![0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn a_batch_without_a_text_reads_no_row() {
        let batch = Batch::read(&[], 0);
        assert_eq!(batch.rows, 0);
        assert_eq!(batch.cols, 1);
    }

    /// The directory the models of the dev tree live in.
    ///
    /// The tests that read a real model are marked `#[ignore]`, because a
    /// model is a download of a hundred megabytes. Run them with
    /// `cargo test -- --ignored` after `POST /api/v1/resolver/local/\
    /// models/{id}/download`, or point `ALICE_ROUTER_MODELS_DIR` at the
    /// directory of the models.
    fn models_root() -> std::path::PathBuf {
        std::env::var("ALICE_ROUTER_MODELS_DIR").map_or_else(
            |_| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/router"),
            std::path::PathBuf::from,
        )
    }

    /// Build an engine over the downloaded models.
    fn installed() -> LocalEngine {
        LocalEngine::new(Arc::new(LocalStore::new(models_root())), 4)
    }

    #[test]
    #[ignore = "reads the downloaded models from disk"]
    fn the_phrase_max_separates_the_way_the_document_read_does() {
        use crate::resolver::router::doc::catalog;
        use crate::resolver::router::embed::as_score;

        let engine = installed();
        let intents = crate::resolver::router::eval::intents();
        let docs = catalog(&intents);
        for message in [
            "could you tell me the current time please",
            "how much room is left on my disk",
            "open firefox",
            "eat a sandwich",
        ] {
            let start = std::time::Instant::now();
            let doc_texts: Vec<String> = docs.iter().map(|doc| doc.embed_text()).collect();
            let docs_scores = engine
                .rerank(RERANKER.id, LocalDevice::Cpu, message, &doc_texts)
                .expect("the reranker answers");
            let doc_ms = start.elapsed().as_secs_f64() * 1000.0;

            // Every phrase of the catalog in one batch, the way a stage
            // that read phrases would read them.
            let mut pairs: Vec<(usize, String)> = Vec::new();
            for doc in &docs {
                for phrase in &doc.phrases {
                    pairs.push((doc.index, phrase.text.clone()));
                }
            }
            let texts: Vec<String> = pairs.iter().map(|(_, text)| text.clone()).collect();
            let start = std::time::Instant::now();
            let phrase_scores = engine
                .rerank(RERANKER.id, LocalDevice::Cpu, message, &texts)
                .expect("the reranker answers");
            let phrase_ms = start.elapsed().as_secs_f64() * 1000.0;
            let mut best: std::collections::BTreeMap<usize, f32> = Default::default();
            for ((index, _), score) in pairs.iter().zip(&phrase_scores) {
                let entry = best.entry(*index).or_insert(0.0);
                if *score > *entry {
                    *entry = *score;
                }
            }

            let mut doc_ranked: Vec<(f32, &str)> = docs
                .iter()
                .zip(&docs_scores)
                .map(|(doc, score)| (as_score(*score), doc.name.as_str()))
                .collect();
            doc_ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
            let mut phrase_ranked: Vec<(f32, &str)> = docs
                .iter()
                .map(|doc| (*best.get(&doc.index).unwrap_or(&0.0), doc.name.as_str()))
                .collect();
            phrase_ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

            println!("\nmessage: {message}");
            println!(
                "  document read ({doc_ms:.1} ms over {} pairs):",
                docs.len()
            );
            for (score, name) in doc_ranked.iter().take(4) {
                println!("    {score:.4}  {name}");
            }
            println!(
                "  phrase read ({phrase_ms:.1} ms over {} pairs):",
                pairs.len()
            );
            for (score, name) in phrase_ranked.iter().take(4) {
                println!("    {score:.4}  {name}");
            }
        }
    }

    #[test]
    #[ignore = "reads the downloaded models from disk"]
    fn the_embedding_model_reads_a_command_close_to_its_intent() {
        let engine = installed();
        let texts = vec![
            "open firefox".to_string(),
            "open application".to_string(),
            "launch an app".to_string(),
            "get weather".to_string(),
            "turn the volume up".to_string(),
        ];

        let vectors = engine
            .embed(BGE_SMALL.id, LocalDevice::Cpu, &texts)
            .expect("the embedding model answers");

        assert_eq!(vectors.len(), texts.len());
        let length: f32 = vectors[0].iter().map(|value| value * value).sum();
        assert!((length - 1.0).abs() < 1e-3, "the vector is not a unit");
        let command = crate::resolver::router::embed::cosine(&vectors[0], &vectors[1]);
        let stranger = crate::resolver::router::embed::cosine(&vectors[0], &vectors[3]);
        assert!(
            command > stranger,
            "`open firefox` reads `open application` at {command} and `get weather` at {stranger}"
        );
    }

    #[test]
    #[ignore = "reads the downloaded models from disk"]
    fn the_reranker_scores_a_fitting_intent_above_a_stranger() {
        let engine = installed();
        let candidates = vec![
            "open application. launch an app. open firefox".to_string(),
            "get weather. what is the weather in Porto".to_string(),
        ];

        let scores = engine
            .rerank(RERANKER.id, LocalDevice::Cpu, "open firefox", &candidates)
            .expect("the reranker answers");

        assert_eq!(scores.len(), candidates.len());
        assert!(
            scores.iter().all(|score| (0.0..=1.0).contains(score)),
            "a score is not a probability: {scores:?}"
        );
        assert!(
            scores[0] > scores[1],
            "the fitting intent scores {:?}",
            scores
        );
    }

    #[test]
    fn a_model_that_is_not_on_disk_is_reported() {
        let (engine, root) = engine();
        let err = engine
            .embed(BGE_SMALL.id, LocalDevice::Cpu, &["hello".to_string()])
            .expect_err("the model is not on disk");
        assert!(matches!(err, LocalError::NotInstalled(_)));

        let err = engine
            .rerank(
                RERANKER.id,
                LocalDevice::Cpu,
                "open firefox",
                &["open application".to_string()],
            )
            .expect_err("the model is not on disk");
        assert!(matches!(err, LocalError::NotInstalled(_)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn an_embedding_model_is_not_read_as_a_reranker() {
        let (engine, root) = engine();
        let err = engine
            .rerank(
                BGE_SMALL.id,
                LocalDevice::Cpu,
                "open firefox",
                &["open application".to_string()],
            )
            .expect_err("that model is an embedding model");
        assert!(matches!(err, LocalError::WrongRole { .. }));
        let _ = std::fs::remove_dir_all(root);
    }
}
