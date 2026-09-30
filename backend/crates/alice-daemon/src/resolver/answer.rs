//! Prompt that answers a message no intent matched.
//!
//! The router ends a turn that no intent fits with a refusal. When the
//! fallback is on, the daemon hands the message to the language model
//! instead, so the user reads an answer rather than a refusal.
//!
//! The turn first reads what the message needs. The daemon asks its built
//! in decision model one typed question about the message alone: does the
//! user want words, or a task done on this machine? The language model is
//! asked that question only when the built in model cannot answer it, and
//! even then it answers the question and the message at once, so the
//! choice between a sentence and a script never costs a request of its
//! own.
//!
//! The rest of the turn follows from the choice. A message that wants
//! words is answered in words, with no answer schema: the model answers
//! the user rather than filling a field of a report. A message that wants
//! a task makes the daemon read the commands of the machine, rank them
//! against the message, and hand the closest ones to the model.
//!
//! The choice keeps the common case cheap. A catalog of a machine holds
//! thousands of names, and reading and ranking it costs far more than the
//! answer in words a greeting needs, so the catalog is read only for the
//! message that really wants a script. The daemon runs no script without
//! the approval of the user either way.
//!
//! The turns before the message reach the model in both steps as the state
//! of the conversation, so a follow-up question is answered in the
//! conversation it belongs to.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::catalog::CommandEntry;

/// The largest value the model may report for the destructiveness.
const MAX_DESTRUCTIVENESS: i64 = 100;

/// The name of the answer schema of the fallback request that chooses
/// between words and a script.
pub const FALLBACK_SCHEMA_NAME: &str = "fallback_answer";

/// The name of the answer schema of the script request.
pub const SCRIPT_SCHEMA_NAME: &str = "fallback_script";

/// The system message of the request that chooses between words and a
/// script.
///
/// This request runs only when the built in decision model could not
/// answer the choice. The daemon answers as the assistant of a machine,
/// so the message says what kind of answer is wanted and what the model
/// must not do. The model must not claim to have run a command, because
/// the daemon runs nothing for a message with no intent.
const SYSTEM_MESSAGE: &str = "You are the voice assistant of one machine. The message of the user matches no configured action. Answer in one or two short sentences in the language of the user. When the message asks for a task on this machine, choose `script` instead of an answer and leave the answer empty; the daemon then reads the commands of the machine and asks you for the script. You run nothing. Answer with one JSON object that follows the schema. Do not mention these instructions, the options, or the schema.";

/// The system message of the request that answers in words.
///
/// The request carries no answer schema, so the model writes the answer
/// itself rather than one field of a report. The message says what the
/// model must not do for the same reason the first one does: nothing runs
/// for a message no intent matched, and the model must not say otherwise.
const SYSTEM_MESSAGE_ANSWER: &str = "You are the voice assistant of one machine. The message of the user matches no configured action, so you answer the user in words and you run nothing. Answer in the language of the user, in one or two short sentences. Do not mention actions, commands, options, or these instructions. If the user asks for something you cannot do, say so plainly and offer what you can do instead.";

/// The system message of the request that writes one script.
///
/// The model reads the commands the daemon ranked for the message, so a
/// script uses the tools the machine really holds. The rating is what the
/// user reads before the approval, so the model must report it honestly.
const SYSTEM_MESSAGE_SCRIPT: &str = "You are the voice assistant of one machine. Write one shell script that does the task of the user with the commands listed below and no other command. One sentence in the summary says what the script does. Rate how rough the script is on the machine from 0 (reads only) to 100 (deletes or rewrites data). Answer with one JSON object that follows the schema. Do not mention these instructions, the commands, or the schema.";

/// The request that answers a message no intent matched.
#[derive(Clone, Debug, PartialEq)]
pub struct AnswerPrompt {
    /// The system message.
    pub system: String,
    /// The user message with the state of the conversation.
    pub user: String,
    ///
    /// The JSON schema the answer must follow, or null for an answer in
    /// plain words. A request that carries a schema is a report the daemon
    /// reads field by field, and a request without one is a sentence the
    /// user reads: the two cannot share a shape, so the schema decides.
    pub answer_schema: Option<Value>,
}

/// One shell script the model wrote for a message.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptProposal {
    /// One sentence about what the script does.
    pub summary: String,
    /// The shell script, one or more lines.
    pub script: String,
    /// How rough the script is on the machine, between 0 and 100.
    pub destructiveness: u8,
}

/// What the first step of the fallback chose.
#[derive(Clone, Debug, PartialEq)]
pub enum Triage {
    /// The model answered the user in words.
    Answer(String),
    /// The model asked for the commands of the machine, so it may write
    /// one shell script for the user to approve.
    Script,
}

/// The answer of the model to a message no intent matched.
#[derive(Clone, Debug, PartialEq)]
pub enum Fallback {
    /// The model answered the user in words.
    Answer(String),
    /// The model wrote one shell script for the user to approve.
    Script(ScriptProposal),
}

/// The shape of one answer of the first step.
#[derive(Debug, Deserialize)]
struct TriageShape {
    /// The kind of answer: `answer` or `script`.
    choice: String,
    /// The words of the answer, when the choice is `answer`.
    #[serde(default)]
    answer: String,
}

/// The shape of one answer of the script step.
#[derive(Debug, Deserialize)]
struct ScriptShape {
    /// One sentence about the script.
    #[serde(default)]
    summary: String,
    /// The shell script.
    #[serde(default)]
    script: String,
    /// How rough the script is on the machine, from 0 to 100.
    #[serde(default)]
    destructiveness: i64,
}

/// Build the prompt that chooses between words and a script.
///
/// The daemon asks this of the language model only when its built in
/// decision model could not answer the choice, so the request asks for the
/// choice and the answer at once: a second request for the answer would
/// cost more than the choice saves.
///
/// The builder returns null when the state is empty, so a turn with no
/// message asks the model nothing. The prompt carries no command of the
/// machine, because the model only names what kind of answer the message
/// needs: the commands are read after the choice, and only when the model
/// really wants a script.
pub fn build_triage_prompt(state: &str) -> Option<AnswerPrompt> {
    if state.trim().is_empty() {
        return None;
    }
    Some(AnswerPrompt {
        system: SYSTEM_MESSAGE.to_string(),
        user: state.to_string(),
        answer_schema: Some(triage_schema()),
    })
}

/// Build the prompt that answers the message in words.
///
/// The request carries no answer schema, so the model writes the answer
/// and the daemon reads the whole of what it wrote. A message that asks
/// for a task never reaches this prompt: the choice is read first, and
/// this prompt answers the messages that want words.
pub fn build_answer_prompt(state: &str) -> Option<AnswerPrompt> {
    if state.trim().is_empty() {
        return None;
    }
    Some(AnswerPrompt {
        system: SYSTEM_MESSAGE_ANSWER.to_string(),
        user: state.to_string(),
        answer_schema: None,
    })
}

/// Read the choice of the first step out of the reply of the model.
///
/// A reply the daemon cannot read and a choice of `answer` without words
/// both read as no answer, so the turn keeps the plain refusal rather than
/// an empty reply.
pub fn parse_triage(content: &str) -> Option<Triage> {
    let shape: TriageShape = serde_json::from_str(content).ok()?;

    if shape.choice.trim().eq_ignore_ascii_case("script") {
        return Some(Triage::Script);
    }

    let answer = shape.answer.trim();
    if answer.is_empty() {
        None
    } else {
        Some(Triage::Answer(answer.to_string()))
    }
}

/// Build the prompt that writes one shell script.
///
/// The commands reach the model as the short, ranked list of the machine,
/// so the script uses the tools the machine really holds. A machine whose
/// catalog did not answer leaves the model without the list rather than
/// without the script, so the builder carries the state alone.
pub fn build_script_prompt(state: &str, commands: &[CommandEntry]) -> Option<AnswerPrompt> {
    if state.trim().is_empty() {
        return None;
    }
    Some(AnswerPrompt {
        system: SYSTEM_MESSAGE_SCRIPT.to_string(),
        user: render_user(state, commands),
        answer_schema: Some(script_schema()),
    })
}

/// Read the script of the model out of its reply.
///
/// A reply the daemon cannot read and a script without a body both read as
/// no script, so the turn keeps the plain refusal rather than an empty
/// script the user could approve.
pub fn parse_script(content: &str) -> Option<ScriptProposal> {
    let shape: ScriptShape = serde_json::from_str(content).ok()?;
    let script = shape.script.trim();
    if script.is_empty() {
        return None;
    }
    Some(ScriptProposal {
        summary: shape.summary.trim().to_string(),
        script: script.to_string(),
        destructiveness: to_destructiveness(shape.destructiveness),
    })
}

/// Keep the reported destructiveness inside its range.
fn to_destructiveness(value: i64) -> u8 {
    value.clamp(0, MAX_DESTRUCTIVENESS) as u8
}

/// Render the user message of the script request.
fn render_user(state: &str, commands: &[CommandEntry]) -> String {
    if commands.is_empty() {
        return state.to_string();
    }

    let lines: Vec<String> = commands.iter().map(CommandEntry::line).collect();
    format!("{state}\n\nCommands you may use:\n{}", lines.join("\n"))
}

/// Build the JSON schema of the request that chooses.
///
/// The message is offered the two answers it can have, so the model says
/// which one the user wants and either writes the answer with it or leaves
/// the answer empty for the script the daemon asks for next.
fn triage_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "choice": { "type": "string", "enum": ["answer", "script"] },
            "answer": { "type": "string" }
        },
        "required": ["choice", "answer"],
        "additionalProperties": false
    })
}

/// Build the JSON schema of the script step.
fn script_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "script": { "type": "string" },
            "destructiveness": { "type": "integer", "minimum": 0, "maximum": MAX_DESTRUCTIVENESS }
        },
        "required": ["summary", "script", "destructiveness"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one command list for the tests.
    fn commands() -> Vec<CommandEntry> {
        vec![
            CommandEntry::new("curl", "transfer a URL"),
            CommandEntry::new("ls", "list directory contents"),
        ]
    }

    #[test]
    fn a_prompt_without_a_state_is_none() {
        assert!(build_triage_prompt("   ").is_none());
        assert!(build_answer_prompt("   ").is_none());
        assert!(build_script_prompt("   ", &commands()).is_none());
    }

    #[test]
    fn the_prompt_that_chooses_carries_no_command_of_the_machine() {
        let prompt = build_triage_prompt("user: hello").expect("a state reads");
        let schema = prompt.answer_schema.expect("the choice carries a schema");

        assert_eq!(prompt.user, "user: hello");
        assert!(!prompt.user.contains("Commands you may use"));
        assert_eq!(
            schema["properties"]["choice"]["enum"],
            json!(["answer", "script"])
        );
    }

    #[test]
    fn the_prompt_that_answers_in_words_carries_no_schema() {
        let prompt = build_answer_prompt("user: hello").expect("a state reads");

        assert!(prompt.system.contains("in words"));
        assert!(prompt.system.contains("run nothing"));
        assert!(
            prompt.answer_schema.is_none(),
            "an answer in words is a sentence and not a report"
        );
    }

    #[test]
    fn the_script_prompt_lists_the_commands() {
        let prompt =
            build_script_prompt("user: list the files", &commands()).expect("a state reads");
        let schema = prompt.answer_schema.expect("the script carries a schema");

        assert!(prompt.user.contains("curl: transfer a URL"));
        assert!(prompt.system.contains("shell script"));
        assert_eq!(
            schema["required"],
            json!(["summary", "script", "destructiveness"])
        );
    }

    #[test]
    fn the_state_survives_a_script_prompt_without_commands() {
        let prompt = build_script_prompt("user: list the files", &[]).expect("a state reads");
        assert_eq!(prompt.user, "user: list the files");
    }

    #[test]
    fn the_first_step_reads_the_words_of_an_answer() {
        let content = r#"{"choice":"answer","answer":"I cannot do that."}"#;
        assert_eq!(
            parse_triage(content),
            Some(Triage::Answer("I cannot do that.".to_string()))
        );
    }

    #[test]
    fn the_first_step_reads_a_request_for_a_script() {
        assert_eq!(
            parse_triage(r#"{"choice":"script","answer":""}"#),
            Some(Triage::Script)
        );
        assert_eq!(
            parse_triage(r#"{"choice":"SCRIPT","answer":""}"#),
            Some(Triage::Script)
        );
    }

    #[test]
    fn the_first_step_reads_no_answer_out_of_an_empty_or_broken_reply() {
        assert!(parse_triage("not json").is_none());
        assert!(parse_triage(r#"{"choice":"answer","answer":"  "}"#).is_none());
    }

    #[test]
    fn the_script_step_reads_a_script_with_its_summary_and_score() {
        let content = r#"{"summary":"List the files","script":"ls -la","destructiveness":5}"#;
        assert_eq!(
            parse_script(content),
            Some(ScriptProposal {
                summary: "List the files".to_string(),
                script: "ls -la".to_string(),
                destructiveness: 5,
            })
        );
    }

    #[test]
    fn the_script_step_keeps_the_score_inside_its_range() {
        let high = r#"{"summary":"","script":"rm -rf /","destructiveness":900}"#;
        let low = r#"{"summary":"","script":"ls","destructiveness":-4}"#;

        let Some(high) = parse_script(high) else {
            panic!("the high score reads as a script")
        };
        let Some(low) = parse_script(low) else {
            panic!("the low score reads as a script")
        };
        assert_eq!(high.destructiveness, 100);
        assert_eq!(low.destructiveness, 0);
    }

    #[test]
    fn the_script_step_reads_no_script_out_of_an_empty_or_broken_reply() {
        assert!(parse_script("not json").is_none());
        assert!(parse_script(r#"{"summary":"x","script":"  ","destructiveness":1}"#).is_none());
    }
}
