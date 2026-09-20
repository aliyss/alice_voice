//! Vectors of the catalog and of a message.
//!
//! The dense pass of the retrieval stage compares a message with the
//! documents of an intent as meaning rather than as spelling, so
//! `launch the browser` finds `open application` although the two share
//! no word.
//!
//! A vector comes from one of two places, and the settings name the one
//! to read. The model server the daemon already talks to answers the
//! `/embeddings` path of the OpenAI interface. A built in model runs in
//! the daemon through ONNX Runtime and needs no server at all.
//!
//! Both places fail with a reason rather than in silence, so the stage
//! that wanted the vectors can fall back to the words.
//!
//! The vector of a document is worth keeping: the catalog changes when a
//! user edits an intent, not when a message arrives, so the second turn
//! of a conversation embeds the message alone.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use alice_core::config::LocalDevice;

use crate::resolver::client::LlamaClient;
use crate::resolver::error::ResolveError;
use crate::resolver::local::LocalEngine;

/// Largest number of vectors the daemon keeps in memory.
///
/// A vector of a small embedding model costs about two kilobytes, so the
/// cap holds about eight megabytes. A catalog of a few hundred intents
/// and the lists of the script entities stay far below it, and a daemon
/// that meets the cap forgets everything and reads the vectors again.
const MAX_ENTRIES: usize = 4096;

/// The vectors the daemon already read, by model and text.
#[derive(Debug, Default)]
pub struct EmbedCache {
    entries: Mutex<HashMap<String, Vec<f32>>>,
}

impl EmbedCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Lock the entries.
    fn lock(&self) -> MutexGuard<'_, HashMap<String, Vec<f32>>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The vector of one text, when the daemon already read it.
    pub fn get(&self, key: &str) -> Option<Vec<f32>> {
        self.lock().get(key).cloned()
    }

    /// Keep the vector of one text.
    pub fn put(&self, key: String, vector: Vec<f32>) {
        let mut entries = self.lock();
        if entries.len() >= MAX_ENTRIES {
            entries.clear();
        }
        entries.insert(key, vector);
    }

    /// How many vectors the daemon holds.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether the daemon holds no vector.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }
}

/// Where one reader of vectors reads them.
#[derive(Clone, Debug)]
enum Place {
    /// A model server answers the `/embeddings` path.
    Server {
        /// The client that asks the server.
        client: LlamaClient,
        /// Base URL of the server.
        base_url: String,
        /// Model name the server answers to.
        model: String,
        /// Time one request may take.
        timeout: Duration,
    },
    /// A built in model runs in the daemon.
    Local {
        /// The engine that runs the graph.
        engine: LocalEngine,
        /// Identifier of the model.
        model: String,
        /// The device the graph runs on.
        device: LocalDevice,
    },
}

impl Place {
    /// The model this place reads, and the place itself, as a short name.
    fn label(&self) -> String {
        match self {
            Self::Server { model, .. } => format!("server\u{1}{model}"),
            Self::Local { model, .. } => format!("local\u{1}{model}"),
        }
    }

    /// Read the vectors of many texts.
    async fn read(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ResolveError> {
        match self {
            Self::Server {
                client,
                base_url,
                model,
                timeout,
            } => client.embed(base_url, model, texts, *timeout).await,
            Self::Local {
                engine,
                model,
                device,
            } => {
                let engine = engine.clone();
                let model = model.clone();
                let device = *device;
                let texts = texts.to_vec();
                // Inference is blocking work, so it runs on the blocking
                // pool of the runtime and never stalls the socket stream.
                let read =
                    tokio::task::spawn_blocking(move || engine.embed(&model, device, &texts))
                        .await
                        .map_err(|err| ResolveError::Embeddings {
                            reason: err.to_string(),
                        })?;
                read.map_err(|err| ResolveError::Embeddings {
                    reason: err.to_string(),
                })
            }
        }
    }
}

/// Reads vectors from one place and remembers them.
#[derive(Clone, Debug)]
pub struct Embedder {
    /// The place the vectors come from.
    place: Place,
    /// The vectors this daemon already read.
    cache: Arc<EmbedCache>,
}

impl Embedder {
    /// Create a reader of vectors that asks a model server.
    pub fn new(
        client: LlamaClient,
        base_url: &str,
        model: &str,
        timeout: Duration,
        cache: Arc<EmbedCache>,
    ) -> Self {
        Self {
            place: Place::Server {
                client,
                base_url: base_url.trim_end_matches('/').to_string(),
                model: model.trim().to_string(),
                timeout,
            },
            cache,
        }
    }

    /// Create a reader of vectors that runs a built in model.
    pub fn local(
        engine: LocalEngine,
        model: &str,
        device: LocalDevice,
        cache: Arc<EmbedCache>,
    ) -> Self {
        Self {
            place: Place::Local {
                engine,
                model: model.trim().to_string(),
                device,
            },
            cache,
        }
    }

    /// The key of one text in the cache.
    ///
    /// The place and the model are part of the key, so a user who picks
    /// another model, or another place, reads another vector for the same
    /// text.
    fn key(&self, text: &str) -> String {
        format!("{}\u{1}{text}", self.place.label())
    }

    /// The vector of one text, from the cache or from the place.
    pub async fn vector(&self, text: &str) -> Result<Vec<f32>, ResolveError> {
        let key = self.key(text);
        if let Some(vector) = self.cache.get(&key) {
            return Ok(vector);
        }
        let vectors = self.place.read(&[text.to_string()]).await?;
        let vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| ResolveError::Embeddings {
                reason: "the reader answered with no vector".to_string(),
            })?;
        let vector = unit(vector);
        self.cache.put(key, vector.clone());
        Ok(vector)
    }

    /// The vectors of many texts, reading only what is not cached.
    pub async fn vectors(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ResolveError> {
        let mut wanted: Vec<String> = Vec::new();
        let mut found: Vec<Option<Vec<f32>>> = Vec::with_capacity(texts.len());
        for text in texts {
            match self.cache.get(&self.key(text)) {
                Some(vector) => found.push(Some(vector)),
                None => {
                    if !wanted.contains(text) {
                        wanted.push(text.clone());
                    }
                    found.push(None);
                }
            }
        }

        if !wanted.is_empty() {
            let read = self.place.read(&wanted).await?;
            let mut fresh: HashMap<String, Vec<f32>> = HashMap::new();
            for (text, vector) in wanted.iter().zip(read) {
                let vector = unit(vector);
                self.cache.put(self.key(text), vector.clone());
                fresh.insert(text.clone(), vector);
            }
            for (position, text) in texts.iter().enumerate() {
                if found[position].is_none() {
                    found[position] = fresh.get(text).cloned();
                }
            }
        }

        found
            .into_iter()
            .map(|vector| {
                vector.ok_or_else(|| ResolveError::Embeddings {
                    reason: "the reader answered with no vector".to_string(),
                })
            })
            .collect()
    }
}

/// Read the value of a list that best fits one of many mentions.
///
/// The words of a message that may name an entry are compared with every
/// entry as vectors, so `open the mail client` reaches the entry
/// `thunderbird` although the two share no word. The best pair has to
/// reach the floor, so a list of unrelated names stays quiet instead of
/// answering with its nearest neighbour.
///
/// The value comes with the similarity that read it, so the metadata of
/// the turn says how sure the match was and not only what it matched.
pub async fn best_value(
    embedder: &Embedder,
    mentions: &[String],
    values: &[String],
    floor: f32,
) -> Result<Option<(String, f32)>, ResolveError> {
    if mentions.is_empty() || values.is_empty() {
        return Ok(None);
    }
    let query = embedder.vectors(mentions).await?;
    let entries = embedder.vectors(values).await?;

    let mut best: Option<(f32, String)> = None;
    for mention in &query {
        for (value, vector) in values.iter().zip(&entries) {
            let score = cosine(mention, vector);
            if best.as_ref().is_none_or(|(held, _)| score > *held) {
                best = Some((score, value.clone()));
            }
        }
    }
    Ok(match best {
        Some((score, value)) if score >= floor => Some((value, score)),
        _ => None,
    })
}

/// Scale a vector to one unit of length, so the dot product is the cosine.
pub fn unit(mut vector: Vec<f32>) -> Vec<f32> {
    let length = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if length > 0.0 && length.is_finite() {
        for value in &mut vector {
            *value /= length;
        }
    }
    vector
}

/// How alike two unit vectors are, between -1 and 1.
///
/// A vector of another length is scaled first, so a caller may compare
/// vectors that did not come from this module.
pub fn cosine(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let dot: f32 = left.iter().zip(right).map(|(a, b)| a * b).sum();
    let left_length = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_length = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_length <= 0.0 || right_length <= 0.0 {
        return 0.0;
    }
    let value = dot / (left_length * right_length);
    value.clamp(-1.0, 1.0)
}

/// Map a cosine into 0 and 1.
///
/// The similarity of two sentences of one language sits between 0 and 1
/// in practice, but a cosine of 0.6 means a weak fit while a cosine of
/// 0.9 means a strong one. The mapping keeps that shape, so the floor of
/// the decision stage reads the same way for words and for vectors.
pub fn as_score(similarity: f32) -> f32 {
    ((similarity - 0.5) / 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A client of a server that never answers, for the cache tests.
    fn reader(cache: Arc<EmbedCache>) -> Embedder {
        Embedder::new(
            LlamaClient::new().expect("the client builds"),
            "http://127.0.0.1:1/v1",
            "bge-small-en-v1.5",
            Duration::from_millis(50),
            cache,
        )
    }

    #[test]
    fn a_unit_vector_has_length_one() {
        let vector = unit(vec![3.0, 4.0]);
        assert!((vector[0] - 0.6).abs() < 1e-6);
        assert!((vector[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn a_vector_of_zeros_stays_as_it_is() {
        assert_eq!(unit(vec![0.0, 0.0]), vec![0.0, 0.0]);
    }

    #[test]
    fn the_cosine_of_a_vector_with_itself_is_one() {
        let vector = unit(vec![1.0, 2.0, 3.0]);
        assert!((cosine(&vector, &vector) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_cosine_of_opposite_vectors_is_minus_one() {
        assert!((cosine(&[1.0, 0.0], &[-1.0, 0.0]) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn a_cosine_of_vectors_of_another_length_is_zero() {
        assert_eq!(cosine(&[1.0, 0.0], &[1.0]), 0.0);
        assert_eq!(cosine(&[], &[]), 0.0);
    }

    #[test]
    fn a_weak_similarity_reads_as_a_weak_score() {
        assert_eq!(as_score(0.5), 0.0);
        assert_eq!(as_score(0.4), 0.0);
        assert!((as_score(0.9) - 0.8).abs() < 1e-6);
        assert_eq!(as_score(1.0), 1.0);
    }

    #[tokio::test]
    async fn a_list_without_a_value_reads_nothing() {
        let reader = reader(Arc::new(EmbedCache::new()));
        let found = best_value(&reader, &["firefox".to_string()], &[], 0.8)
            .await
            .expect("an empty list needs no server");
        let quiet = best_value(&reader, &[], &["firefox".to_string()], 0.8)
            .await
            .expect("a message without a mention needs no server");

        assert!(found.is_none());
        assert!(quiet.is_none());
    }

    #[test]
    fn the_cache_answers_the_text_it_kept() {
        let cache = Arc::new(EmbedCache::new());
        let reader = reader(Arc::clone(&cache));

        assert!(cache.is_empty());
        reader.cache.put(reader.key("open firefox"), vec![1.0, 0.0]);

        assert_eq!(cache.len(), 1);
        assert_eq!(
            reader.cache.get(&reader.key("open firefox")),
            Some(vec![1.0, 0.0])
        );
        assert!(reader.cache.get(&reader.key("open obs")).is_none());
    }

    #[test]
    fn the_cache_tells_apart_the_vectors_of_two_models() {
        let cache = Arc::new(EmbedCache::new());
        let first = reader(Arc::clone(&cache));
        let second = Embedder::new(
            LlamaClient::new().expect("the client builds"),
            "http://127.0.0.1:1/v1",
            "another-model",
            Duration::from_millis(50),
            Arc::clone(&cache),
        );

        first.cache.put(first.key("open firefox"), vec![1.0]);
        assert!(second.cache.get(&second.key("open firefox")).is_none());
    }

    #[test]
    fn the_cache_forgets_everything_when_it_is_full() {
        let cache = EmbedCache::new();
        for index in 0..MAX_ENTRIES {
            cache.put(format!("text-{index}"), vec![1.0]);
        }
        assert_eq!(cache.len(), MAX_ENTRIES);

        cache.put("one-more".to_string(), vec![1.0]);

        assert_eq!(cache.len(), 1);
    }
}
