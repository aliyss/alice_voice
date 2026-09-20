//! Prompt of one decision.
//!
//! The builder turns the options of a decision into letters and asks the
//! model for the letter of the option it chooses. A letter is one token,
//! so the answer of the model stays short and the log probabilities of
//! that token are the choice distribution.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::resolver::decision::Decision;
use crate::resolver::error::ResolveError;

/// Largest number of options of one decision.
pub const MAX_DECISION_OPTIONS: usize = 26;

/// The letter the builder assigns to the first option.
const FIRST_LETTER: u8 = b'A';

/// The system message of every decision.
const SYSTEM_MESSAGE: &str = "You are a decision model. You read a state and choose the single option that fits it best. You answer with the letter of one option and one short reason.";

/// One letter and the option it selects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionLetter {
    /// The letter the model answers with.
    pub letter: char,
    /// Identifier of the option the letter selects.
    pub option_id: String,
}

/// The request of one decision.
#[derive(Clone, Debug)]
pub struct Prompt {
    /// The system message.
    pub system: String,
    /// The user message with the state and the options.
    pub user: String,
    /// The JSON schema the answer must follow.
    pub answer_schema: Value,
    /// The letters and the options they select.
    pub letters: Vec<OptionLetter>,
}

/// The answer the model sends back.
#[derive(Debug, Deserialize)]
struct AnswerShape {
    /// The letter of the chosen option.
    choice: String,
}

/// Read the letter of an option by its position.
fn letter_at(index: usize) -> char {
    char::from(FIRST_LETTER + index as u8)
}

/// Render one option as a line of the option list.
fn render_option(letter: char, label: &str, detail: &str) -> String {
    if detail.is_empty() {
        return format!("{letter}. {label}");
    }
    format!("{letter}. {label} — {detail}")
}

/// Render the user message of a decision.
fn render_user_message(decision: &Decision, letters: &[OptionLetter]) -> String {
    let mut lines = vec!["State:".to_string(), decision.state.clone(), String::new()];

    lines.push(format!("Question: {}", decision.question));
    lines.push(String::new());
    lines.push("Options:".to_string());
    for (index, option) in decision.options.iter().enumerate() {
        let letter = letters.get(index).map_or('?', |entry| entry.letter);
        lines.push(render_option(letter, &option.label, &option.detail));
    }
    lines.push(String::new());
    lines.push("Answer with the letter of one option.".to_string());

    lines.join("\n")
}

/// Build the prompt and the answer schema of one decision.
pub fn build_prompt(decision: &Decision) -> Result<Prompt, ResolveError> {
    if decision.options.len() > MAX_DECISION_OPTIONS {
        return Err(ResolveError::TooManyOptions {
            max: MAX_DECISION_OPTIONS,
            count: decision.options.len(),
        });
    }

    let letters: Vec<OptionLetter> = decision
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| OptionLetter {
            letter: letter_at(index),
            option_id: option.id.clone(),
        })
        .collect();

    let allowed: Vec<String> = letters
        .iter()
        .map(|entry| entry.letter.to_string())
        .collect();
    let answer_schema = json!({
        "type": "object",
        "properties": {
            "choice": { "type": "string", "enum": allowed },
            "reason": { "type": "string" }
        },
        "required": ["choice", "reason"],
        "additionalProperties": false
    });

    Ok(Prompt {
        system: SYSTEM_MESSAGE.to_string(),
        user: render_user_message(decision, &letters),
        answer_schema,
        letters,
    })
}

/// Read the chosen option out of the answer of the model.
///
/// The model answers with JSON. A model that ignores the schema still
/// writes the letter, so the reader falls back to the first quoted letter
/// of the answer. The function returns the position of the option.
pub fn parse_choice(content: &str, letters: &[OptionLetter]) -> Option<usize> {
    if let Some(found) = choice_from_answer(content, letters) {
        return Some(found);
    }
    first_quoted_letter(content, letters)
}

/// Read the chosen option out of a complete answer.
fn choice_from_answer(content: &str, letters: &[OptionLetter]) -> Option<usize> {
    let answer: AnswerShape = serde_json::from_str(content).ok()?;
    let first = answer.choice.trim().chars().next()?;
    letters
        .iter()
        .position(|entry| entry.letter.eq_ignore_ascii_case(&first))
}

/// Find the option of the first quoted letter of the answer.
fn first_quoted_letter(content: &str, letters: &[OptionLetter]) -> Option<usize> {
    let mut best: Option<(usize, usize)> = None;
    for (index, entry) in letters.iter().enumerate() {
        let upper = format!("\"{}\"", entry.letter);
        let lower = format!("\"{}\"", entry.letter.to_ascii_lowercase());
        let at = match (content.find(&upper), content.find(&lower)) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(found), None) | (None, Some(found)) => Some(found),
            (None, None) => None,
        };
        let Some(at) = at else { continue };
        match best {
            Some((position, _)) if position <= at => {}
            _ => best = Some((at, index)),
        }
    }
    best.map(|(_, index)| index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::decision::DecisionOption;

    /// Build one decision for the tests.
    fn decision(count: usize) -> Decision {
        let options = (0..count)
            .map(|index| DecisionOption {
                id: format!("option-{index}"),
                label: format!("option {index}"),
                detail: String::new(),
            })
            .collect();
        Decision {
            state: "the light in the hall stays on".to_string(),
            question: "Which option fits the state best?".to_string(),
            options,
        }
    }

    #[test]
    fn build_prompt_assigns_one_letter_per_option() {
        let prompt = build_prompt(&decision(3)).expect("the decision is valid");
        let letters: Vec<char> = prompt.letters.iter().map(|entry| entry.letter).collect();
        assert_eq!(letters, vec!['A', 'B', 'C']);
        assert_eq!(prompt.letters[1].option_id, "option-1");
    }

    #[test]
    fn build_prompt_lists_the_state_and_the_options() {
        let prompt = build_prompt(&decision(2)).expect("the decision is valid");
        assert!(prompt.user.contains("the light in the hall stays on"));
        assert!(prompt.user.contains("A. option 0"));
        assert!(prompt.user.contains("B. option 1"));
    }

    #[test]
    fn build_prompt_restricts_the_answer_to_the_letters() {
        let prompt = build_prompt(&decision(2)).expect("the decision is valid");
        let allowed = &prompt.answer_schema["properties"]["choice"]["enum"];
        assert_eq!(allowed, &json!(["A", "B"]));
    }

    #[test]
    fn build_prompt_rejects_more_options_than_letters() {
        assert!(build_prompt(&decision(MAX_DECISION_OPTIONS + 1)).is_err());
    }

    #[test]
    fn parse_choice_reads_the_letter_of_a_complete_answer() {
        let prompt = build_prompt(&decision(3)).expect("the decision is valid");
        let found = parse_choice(r#"{"choice":"C","reason":"it is dark"}"#, &prompt.letters);
        assert_eq!(found, Some(2));
    }

    #[test]
    fn parse_choice_reads_the_letter_of_a_cut_answer() {
        let prompt = build_prompt(&decision(2)).expect("the decision is valid");
        let found = parse_choice(r#"{"choice":"B","reason":"because the"#, &prompt.letters);
        assert_eq!(found, Some(1));
    }

    #[test]
    fn parse_choice_ignores_a_letter_inside_a_word() {
        let prompt = build_prompt(&decision(2)).expect("the decision is valid");
        assert_eq!(parse_choice("the reason is vague", &prompt.letters), None);
    }

    #[test]
    fn parse_choice_returns_none_for_an_empty_answer() {
        let prompt = build_prompt(&decision(2)).expect("the decision is valid");
        assert_eq!(parse_choice("", &prompt.letters), None);
    }
}
