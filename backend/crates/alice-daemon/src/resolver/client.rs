//! Client of the model server.
//!
//! The client sends one decision to an OpenAI compatible llama.cpp
//! server, streams the answer, and reads the chosen option back. The
//! request restricts the answer with a JSON schema, so the model can only
//! answer with one of the option letters.

use std::time::Duration;

use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};

use crate::resolver::decision::DecisionOutcome;
use crate::resolver::error::ResolveError;
use crate::resolver::probability::{confidence_for_choice, parse_entries, LogprobEntry};
use crate::resolver::prompt::{parse_choice, Prompt};

/// The chat path of the model server.
const CHAT_PATH: &str = "chat/completions";

/// The path that lists the models of the server.
const MODELS_PATH: &str = "models";

/// The path that turns texts into vectors.
const EMBEDDINGS_PATH: &str = "embeddings";

/// The largest part of a failure body the daemon keeps.
const MAX_BODY_LEN: usize = 200;

/// The number of candidates the daemon asks for at every token.
const TOP_LOGPROBS: u8 = 8;

/// One request for one decision.
#[derive(Clone, Debug)]
pub struct DecisionRequest {
    /// Base URL of the model server.
    pub base_url: String,
    /// Model name the server answers to.
    pub model: String,
    /// The prompt and the answer schema.
    pub prompt: Prompt,
    /// The largest answer the daemon reads.
    pub max_tokens: u32,
    /// Whether the model may reason before it answers.
    pub thinking: bool,
    /// The time the daemon waits for the answer.
    pub timeout: Duration,
}

/// One request for one answer of the model.
#[derive(Clone, Debug)]
pub struct AnswerRequest {
    /// Base URL of the model server.
    pub base_url: String,
    /// Model name the server answers to.
    pub model: String,
    /// The system message of the request.
    pub system: String,
    /// The user message of the request.
    pub user: String,
    /// The JSON schema the answer must follow.
    pub answer_schema: Value,
    /// The name of the schema in the request.
    pub schema_name: String,
    /// The largest answer the daemon reads.
    pub max_tokens: u32,
    /// Whether the model may reason before it answers.
    pub thinking: bool,
    /// The time the daemon waits for the answer.
    pub timeout: Duration,
}

/// Client of the OpenAI compatible model server.
#[derive(Clone, Debug)]
pub struct LlamaClient {
    http: Client,
}

impl LlamaClient {
    /// Create a new client.
    pub fn new() -> Result<Self, ResolveError> {
        let http = Client::builder()
            .build()
            .map_err(|err| ResolveError::Configuration(err.to_string()))?;
        Ok(Self { http })
    }

    /// Ask the model for one decision and stream the answer to `on_delta`.
    ///
    /// The caller keeps the stream open, because one turn asks the model
    /// more than once: the decision first, then the values of the
    /// entities of the chosen intent.
    pub async fn decide<F>(
        &self,
        request: &DecisionRequest,
        on_delta: &mut F,
    ) -> Result<DecisionOutcome, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        // 1. Send the request and read the answer.
        let (content, entries) = self
            .send_stream(
                &request.base_url,
                &build_body(request),
                request.timeout,
                on_delta,
            )
            .await?;

        // 2. Read the chosen option and its probability.
        let letters = &request.prompt.letters;
        let index = parse_choice(&content, letters).ok_or(ResolveError::NoChoice)?;
        let chosen = letters.get(index).ok_or(ResolveError::NoChoice)?;

        Ok(DecisionOutcome {
            option_id: chosen.option_id.clone(),
            confidence: confidence_for_choice(&entries, chosen.letter),
        })
    }

    /// Ask the model for one answer and return the content it wrote.
    ///
    /// The caller reads the content with its own parser, so this function
    /// serves the requests that are not a choice among letters.
    pub async fn read_answer<F>(
        &self,
        request: &AnswerRequest,
        on_delta: &mut F,
    ) -> Result<String, ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        let (content, _entries) = self
            .send_stream(
                &request.base_url,
                &build_answer_body(request),
                request.timeout,
                on_delta,
            )
            .await?;
        Ok(content)
    }

    /// Read the identifiers of the models the server offers.
    ///
    /// The settings page shows this list, so the user picks a model the
    /// server really answers to instead of typing a name that may not
    /// exist. A server that does not answer fails with a reason the page
    /// can show next to the field it disables.
    pub async fn list_models(
        &self,
        base_url: &str,
        timeout: Duration,
    ) -> Result<Vec<String>, ResolveError> {
        let url = format!("{}/{MODELS_PATH}", base_url.trim_end_matches('/'));
        let response = self
            .http
            .get(&url)
            .timeout(timeout)
            .send()
            .await
            .map_err(|err| ResolveError::Unreachable(err.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| ResolveError::Unreachable(err.to_string()))?;
        if !status.is_success() {
            return Err(ResolveError::Rejected {
                status: status.as_u16(),
                body: truncate(&body, MAX_BODY_LEN),
            });
        }
        Ok(parse_models(&body))
    }

    /// Read the vector of every text the caller sends.
    ///
    /// The retrieval pass of the router ranks the catalog with these
    /// vectors, so a message and the phrases of an intent are compared as
    /// meaning rather than as spelling. The server reads a whole batch at
    /// once, because the vector of a document is worth caching and a
    /// round trip per document would cost more than the inference.
    pub async fn embed(
        &self,
        base_url: &str,
        model: &str,
        input: &[String],
        timeout: Duration,
    ) -> Result<Vec<Vec<f32>>, ResolveError> {
        if input.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!("{}/{EMBEDDINGS_PATH}", base_url.trim_end_matches('/'));
        let response = self
            .http
            .post(&url)
            .json(&json!({ "model": model, "input": input }))
            .timeout(timeout)
            .send()
            .await
            .map_err(|err| ResolveError::Unreachable(err.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| ResolveError::Unreachable(err.to_string()))?;
        if !status.is_success() {
            return Err(ResolveError::Rejected {
                status: status.as_u16(),
                body: truncate(&body, MAX_BODY_LEN),
            });
        }
        let vectors = parse_embeddings(&body);
        if vectors.len() != input.len() {
            return Err(ResolveError::Embeddings {
                reason: format!(
                    "the server answered with {} vectors for {} texts",
                    vectors.len(),
                    input.len()
                ),
            });
        }
        Ok(vectors)
    }

    /// Send one request to the model server and read the streamed answer.
    async fn send_stream<F>(
        &self,
        base_url: &str,
        body: &Value,
        timeout: Duration,
        on_delta: &mut F,
    ) -> Result<(String, Vec<LogprobEntry>), ResolveError>
    where
        F: FnMut(&str) + Send,
    {
        // 1. Send the request.
        let url = format!("{}/{}", base_url.trim_end_matches('/'), CHAT_PATH);
        let response = self
            .http
            .post(&url)
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|err| ResolveError::Unreachable(err.to_string()))?;

        // 2. Check the status.
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(ResolveError::Rejected {
                status,
                body: truncate(&text, MAX_BODY_LEN),
            });
        }

        // 3. Read the streamed answer.
        read_stream(response, on_delta).await
    }
}

/// Build the body of one decision request.
fn build_body(request: &DecisionRequest) -> Value {
    json!({
        "model": request.model,
        "messages": [
            { "role": "system", "content": request.prompt.system },
            { "role": "user", "content": request.prompt.user }
        ],
        "temperature": 0,
        "max_tokens": request.max_tokens,
        "stream": true,
        "logprobs": true,
        "top_logprobs": TOP_LOGPROBS,
        "chat_template_kwargs": { "enable_thinking": request.thinking },
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "intent_choice",
                "strict": true,
                "schema": request.prompt.answer_schema
            }
        }
    })
}

/// Build the body of one answer request.
fn build_answer_body(request: &AnswerRequest) -> Value {
    json!({
        "model": request.model,
        "messages": [
            { "role": "system", "content": request.system },
            { "role": "user", "content": request.user }
        ],
        "temperature": 0,
        "max_tokens": request.max_tokens,
        "stream": true,
        "chat_template_kwargs": { "enable_thinking": request.thinking },
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": request.schema_name,
                "strict": true,
                "schema": request.answer_schema
            }
        }
    })
}

/// Read the streamed answer of the model.
async fn read_stream<F>(
    response: reqwest::Response,
    on_delta: &mut F,
) -> Result<(String, Vec<LogprobEntry>), ResolveError>
where
    F: FnMut(&str) + Send,
{
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut content = String::new();
    let mut entries: Vec<LogprobEntry> = Vec::new();

    'reading: while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| ResolveError::Unreachable(err.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(end) = buffer.find('\n') {
            let line = buffer[..end].trim().to_string();
            buffer.drain(..=end);

            let Some(data) = line.strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                break 'reading;
            }
            let Ok(value) = serde_json::from_str::<Value>(data) else {
                continue;
            };
            apply_chunk(&value, &mut content, &mut entries, on_delta);
        }
    }

    Ok((content, entries))
}

/// Fold one answer chunk into the answer text and the log probabilities.
fn apply_chunk<F>(
    value: &Value,
    content: &mut String,
    entries: &mut Vec<LogprobEntry>,
    on_delta: &mut F,
) where
    F: FnMut(&str) + Send,
{
    let Some(choice) = value.get("choices").and_then(|choices| choices.get(0)) else {
        return;
    };

    // A reasoning model sends its trace first, under its own key. Both
    // streams reach the interface, so the reader watches the model work.
    let reasoning = choice
        .get("delta")
        .and_then(|delta| delta.get("reasoning_content"))
        .and_then(Value::as_str);
    if let Some(reasoning) = reasoning {
        if !reasoning.is_empty() {
            on_delta(reasoning);
        }
    }

    let delta = choice
        .get("delta")
        .and_then(|delta| delta.get("content"))
        .and_then(Value::as_str);
    if let Some(delta) = delta {
        if !delta.is_empty() {
            content.push_str(delta);
            on_delta(delta);
        }
    }

    if let Some(logprobs) = choice.get("logprobs") {
        if !logprobs.is_null() {
            entries.extend(parse_entries(logprobs));
        }
    }
}

/// Read the model identifiers out of the reply of one model server.
///
/// The OpenAI compatible interface answers with `data`, and older builds
/// of the server answer with `models`. A reply the daemon cannot read
/// holds no model, so the settings page shows an empty list.
fn parse_models(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let entries = value
        .get("data")
        .or_else(|| value.get("models"))
        .and_then(Value::as_array);
    let Some(entries) = entries else {
        return Vec::new();
    };

    let mut models: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            entry
                .get("id")
                .or_else(|| entry.get("name"))
                .or_else(|| entry.get("model"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|id| !id.trim().is_empty())
        .collect();
    models.sort();
    models.dedup();
    models
}

/// Read the vectors out of the reply of one embedding server.
///
/// The OpenAI compatible interface answers with `data`, one entry per
/// text, each entry carrying its own position. A reply without positions
/// reads in the order the server wrote it.
fn parse_embeddings(body: &str) -> Vec<Vec<f32>> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let Some(entries) = value.get("data").and_then(Value::as_array) else {
        return Vec::new();
    };

    let mut read: Vec<(usize, Vec<f32>)> = Vec::with_capacity(entries.len());
    for (position, entry) in entries.iter().enumerate() {
        let vector = entry.get("embedding").and_then(Value::as_array);
        let Some(vector) = vector else {
            continue;
        };
        let numbers: Vec<f32> = vector
            .iter()
            .filter_map(Value::as_f64)
            .map(|number| number as f32)
            .collect();
        if numbers.is_empty() {
            continue;
        }
        let at = entry
            .get("index")
            .and_then(Value::as_u64)
            .map_or(position, |index| index as usize);
        read.push((at, numbers));
    }
    read.sort_by_key(|(at, _)| *at);
    read.into_iter().map(|(_, vector)| vector).collect()
}

/// Cut a text and mark the cut.
fn truncate(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(max).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::decision::{Decision, DecisionOption};
    use crate::resolver::prompt::build_prompt;

    /// Build one request for the tests.
    fn request() -> DecisionRequest {
        let prompt = build_prompt(&Decision {
            state: "hello".to_string(),
            question: "Which option?".to_string(),
            options: vec![DecisionOption {
                id: "one".to_string(),
                label: "one".to_string(),
                detail: String::new(),
            }],
        })
        .expect("the decision is valid");
        DecisionRequest {
            base_url: "http://127.0.0.1:8012/v1".to_string(),
            model: "qwen3.5-4b".to_string(),
            prompt,
            max_tokens: 96,
            thinking: false,
            timeout: Duration::from_secs(5),
        }
    }

    #[test]
    fn build_body_restricts_the_answer_with_the_schema() {
        let body = build_body(&request());
        assert_eq!(body["model"], "qwen3.5-4b");
        assert_eq!(body["stream"], true);
        assert_eq!(body["temperature"], 0);
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"], false);
        assert_eq!(
            body["response_format"]["json_schema"]["schema"]["properties"]["choice"]["enum"],
            json!(["A"])
        );
    }

    #[test]
    fn parse_models_reads_the_openai_interface() {
        let body = r#"{"object":"list","data":[{"id":"qwen3.5-4b","object":"model"},{"id":"gpt-oss-20b"}]}"#;

        assert_eq!(parse_models(body), vec!["gpt-oss-20b", "qwen3.5-4b"]);
    }

    #[test]
    fn parse_models_reads_the_older_interface() {
        let body = r#"{"models":[{"name":"qwen3.5-4b"}]}"#;

        assert_eq!(parse_models(body), vec!["qwen3.5-4b"]);
    }

    #[test]
    fn parse_models_keeps_one_spelling_of_a_model() {
        let body = r#"{"data":[{"id":"m"},{"id":"m"}]}"#;

        assert_eq!(parse_models(body), vec!["m"]);
    }

    #[test]
    fn parse_models_reads_a_reply_it_cannot_use_as_no_model() {
        assert!(parse_models("not json").is_empty());
        assert!(parse_models(r#"{"data":[{"id":"  "}]}"#).is_empty());
        assert!(parse_models(r#"{"data":"one"}"#).is_empty());
    }

    #[test]
    fn parse_embeddings_reads_one_vector_per_text_in_order() {
        let body = r#"{"data":[
            {"index":1,"embedding":[0.5,0.25]},
            {"index":0,"embedding":[1.0,0.0]}
        ]}"#;

        assert_eq!(
            parse_embeddings(body),
            vec![vec![1.0, 0.0], vec![0.5, 0.25]]
        );
    }

    #[test]
    fn parse_embeddings_reads_a_reply_it_cannot_use_as_nothing() {
        assert!(parse_embeddings("not json").is_empty());
        assert!(parse_embeddings(r#"{"data":[{"embedding":[]}]}"#).is_empty());
        assert!(parse_embeddings(r#"{"object":"list"}"#).is_empty());
    }

    #[tokio::test]
    async fn an_empty_batch_asks_the_server_nothing() {
        let client = LlamaClient::new().expect("the client builds");
        let vectors = client
            .embed("http://127.0.0.1:1/v1", "m", &[], Duration::from_millis(50))
            .await
            .expect("an empty batch needs no server");

        assert!(vectors.is_empty());
    }

    #[test]
    fn truncate_keeps_a_short_text_as_it_is() {
        assert_eq!(truncate("  hello  ", 10), "hello");
    }

    #[test]
    fn truncate_cuts_a_long_text() {
        let cut = truncate(&"a".repeat(300), 20);
        assert_eq!(cut.chars().count(), 21);
        assert!(cut.ends_with('…'));
    }
}
