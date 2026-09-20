//! Confidence of one decision.
//!
//! The model server reports the log probability of the token it wrote.
//! The answer starts with one letter, so the candidates of that token are
//! the choice distribution of the model. The reader keeps the probability
//! of the chosen letter and drops the rest.

use serde_json::Value;

/// One token the model could have written, with its log probability.
#[derive(Clone, Debug, PartialEq)]
pub struct LogprobCandidate {
    /// The text of the token.
    pub token: String,
    /// The log probability the model gave the token.
    pub logprob: f64,
}

/// One token the model wrote, with the candidates it weighed.
#[derive(Clone, Debug, PartialEq)]
pub struct LogprobEntry {
    /// The text of the token the model wrote.
    pub token: String,
    /// The candidates of the token, best first.
    pub top: Vec<LogprobCandidate>,
}

/// Parse the log probability object of one answer chunk.
pub fn parse_entries(value: &Value) -> Vec<LogprobEntry> {
    let Some(content) = value.get("content").and_then(Value::as_array) else {
        return Vec::new();
    };
    content.iter().filter_map(parse_entry).collect()
}

/// Parse one entry of the log probability content.
fn parse_entry(value: &Value) -> Option<LogprobEntry> {
    let token = value.get("token")?.as_str()?.to_string();
    let top = value
        .get("top_logprobs")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(parse_candidate).collect())
        .unwrap_or_default();
    Some(LogprobEntry { token, top })
}

/// Parse one candidate of an entry.
fn parse_candidate(value: &Value) -> Option<LogprobCandidate> {
    Some(LogprobCandidate {
        token: value.get("token")?.as_str()?.to_string(),
        logprob: value.get("logprob")?.as_f64()?,
    })
}

/// Check that a token is one letter.
fn is_letter(token: &str) -> bool {
    let trimmed = token.trim();
    trimmed.chars().count() == 1
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
}

/// Check that a token is one given letter.
fn matches_letter(token: &str, choice: char) -> bool {
    let trimmed = token.trim();
    is_letter(trimmed)
        && trimmed
            .chars()
            .next()
            .is_some_and(|c| c.eq_ignore_ascii_case(&choice))
}

/// Turn one log probability into a probability between 0 and 1.
fn to_probability(logprob: f64) -> Option<f32> {
    if !logprob.is_finite() {
        return None;
    }
    let probability = logprob.exp();
    if !probability.is_finite() {
        return None;
    }
    Some((probability as f32).clamp(0.0, 1.0))
}

/// Read the probability of one chosen letter, or null.
///
/// The letter of the choice is the first single letter token of the
/// answer. The function returns null when the server sent no log
/// probabilities, so a caller never invents a confidence.
pub fn confidence_for_choice(entries: &[LogprobEntry], choice: char) -> Option<f32> {
    let entry = entries.iter().find(|entry| is_letter(&entry.token))?;
    let candidate = entry
        .top
        .iter()
        .find(|candidate| matches_letter(&candidate.token, choice))?;
    to_probability(candidate.logprob)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_entries_reads_the_tokens_and_the_candidates() {
        let value = json!({
            "content": [{
                "token": "A",
                "logprob": -0.1,
                "top_logprobs": [
                    { "token": "A", "logprob": -0.1 },
                    { "token": "B", "logprob": -2.4 }
                ]
            }]
        });
        let entries = parse_entries(&value);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].token, "A");
        assert_eq!(entries[0].top.len(), 2);
        assert_eq!(entries[0].top[1].token, "B");
    }

    #[test]
    fn parse_entries_reads_an_empty_object() {
        assert!(parse_entries(&json!({})).is_empty());
    }

    #[test]
    fn confidence_for_choice_reads_the_probability_of_the_letter() {
        // Two equally likely letters: a log probability of minus ln 2 each.
        let half = -std::f64::consts::LN_2;
        let value = json!({
            "content": [{
                "token": "A",
                "logprob": half,
                "top_logprobs": [
                    { "token": "A", "logprob": half },
                    { "token": "B", "logprob": half }
                ]
            }]
        });
        let confidence = confidence_for_choice(&parse_entries(&value), 'A').expect("a value");
        assert!((confidence - 0.5).abs() < 0.001);
    }

    #[test]
    fn confidence_for_choice_reads_the_letter_after_the_json_punctuation() {
        let value = json!({
            "content": [
                { "token": "{", "logprob": -0.0, "top_logprobs": [{ "token": "{", "logprob": -0.0 }] },
                { "token": "B", "logprob": -0.01, "top_logprobs": [{ "token": "B", "logprob": -0.01 }] }
            ]
        });
        let confidence = confidence_for_choice(&parse_entries(&value), 'B').expect("a value");
        assert!(confidence > 0.9);
    }

    #[test]
    fn confidence_for_choice_returns_none_without_candidates() {
        assert!(confidence_for_choice(&[], 'A').is_none());
    }

    #[test]
    fn confidence_for_choice_returns_none_for_a_letter_the_model_never_weighed() {
        let value = json!({
            "content": [{ "token": "A", "logprob": -0.0, "top_logprobs": [{ "token": "A", "logprob": -0.0 }] }]
        });
        assert!(confidence_for_choice(&parse_entries(&value), 'C').is_none());
    }
}
