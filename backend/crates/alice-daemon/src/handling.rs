//! Handling of one queued message.
//!
//! Handling runs in stages: it resolves the intent of the message, runs
//! the command of that intent, and builds the reply. Every stage
//! publishes one event, so the interface shows the turn while it runs.

use std::sync::Arc;

use alice_core::config::{CoreConfig, ResolverEngine};
use alice_core::dto::SystemEventPayloadDto;

use crate::event_bus::EventBus;
use crate::execution::{CommandOutcome, CommandRunner};
use crate::intent::{render_command, strip_placeholders};
use crate::resolver::{
    Resolution, ResolveError, ResolveRequest, ResolvedIntent, ResolverService, RouteReport,
};

/// The reply the daemon sends when no intent fits the message.
pub const NO_INTENT_REPLY: &str = "I could not match that to an intent.";

/// The reply the daemon sends when the resolver does not answer.
pub const RESOLVER_DOWN_REPLY: &str = "I could not reach the intent resolver.";

/// The reply the daemon sends when the model answers without a choice.
pub const RESOLVER_UNREADABLE_REPLY: &str = "I could not read an intent from that.";

/// The reply the daemon sends when the built in resolver cannot read.
pub const GLINER_NOT_READY_REPLY: &str = "The built in intent resolver is not ready.";

/// How the resolver read a message it chose no intent for.
///
/// A turn without an intent is still a turn the resolver read, so the
/// transcript can report the engine and the model that read it.
#[derive(Clone, Debug, PartialEq)]
pub struct UnmatchedRead {
    /// The engine that read the message.
    pub engine: ResolverEngine,
    /// The model that engine ran, or null when the daemon cannot name it.
    pub model: Option<String>,
    /// How the layered router read the message, or empty for the other
    /// engines. A refusal names the stage that refused and the short list
    /// it refused from, so a user reads what the catalog offered.
    pub route: RouteReport,
}

/// What one handled message produced.
#[derive(Clone, Debug)]
pub struct HandlingOutcome {
    /// The intent the resolver chose, or null when it chose none.
    pub intent: Option<ResolvedIntent>,
    /// What the command of the intent did, or null when none ran.
    pub execution: Option<CommandOutcome>,
    /// The command of the intent with the values of the turn, or null when
    /// the daemon did not get that far.
    pub command: Option<String>,
    /// How the resolver read a message it chose no intent for, or null when
    /// the resolver did not read the message at all.
    pub unmatched: Option<UnmatchedRead>,
    /// The reply the daemon stores.
    pub reply: String,
}

/// The reply of a message the daemon could not resolve, and how the
/// resolver read it.
///
/// The read is boxed, because a failed resolution travels this type and
/// the route of a turn is much larger than the reply beside it.
#[derive(Clone, Debug)]
struct ResolveReply {
    /// The reply the daemon stores.
    reply: String,
    /// How the resolver read the message, or null when it did not read it.
    unmatched: Option<Box<UnmatchedRead>>,
}

impl ResolveReply {
    /// The reply of a resolver that did not read the message.
    fn unresolved(reply: String) -> Self {
        Self {
            reply,
            unmatched: None,
        }
    }

    /// The reply of a message the resolver read and chose no intent for.
    fn unmatched(read: UnmatchedRead) -> Self {
        Self {
            reply: NO_INTENT_REPLY.to_string(),
            unmatched: Some(Box::new(read)),
        }
    }
}

/// The services one handled message needs.
#[derive(Clone, Debug)]
pub struct HandlingDeps {
    /// The resolver of the message.
    pub resolver: ResolverService,
    /// The runner of the command of the intent.
    pub runner: CommandRunner,
    /// The publisher of the stage events.
    pub events: EventBus,
    /// The typed configuration.
    pub config: Arc<CoreConfig>,
}

/// How much of a turn the daemon carries out.
///
/// A preview reads a message the way a turn would and runs nothing, so the
/// settings page can show the route of a sentence before the user sends
/// it. A preview publishes no stage on the event bus either, because the
/// stream belongs to the turns the user really sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnRun {
    /// Read the message, run the command, and report every stage.
    Full,
    /// Read the message and run nothing, quietly.
    Preview,
}

impl TurnRun {
    /// Whether the run publishes its stages on the event bus.
    fn publishes(&self) -> bool {
        matches!(self, Self::Full)
    }
}

/// Handle one message and return what it produced.
pub async fn handle(deps: &HandlingDeps, request: &ResolveRequest) -> HandlingOutcome {
    read(deps, request, TurnRun::Full).await
}

/// Read one message without running the command of its intent.
///
/// The settings page tries a sentence with this, so a user reads the route
/// it would take before a turn runs anything. Everything but the run and
/// the events is what a turn does, so the report of a preview and the
/// metadata of a stored turn are the same report.
pub async fn preview(deps: &HandlingDeps, request: &ResolveRequest) -> HandlingOutcome {
    read(deps, request, TurnRun::Preview).await
}

/// Read one message and carry out the part of the turn the run asks for.
async fn read(deps: &HandlingDeps, request: &ResolveRequest, run: TurnRun) -> HandlingOutcome {
    // 1. Resolve the intent of the message.
    let intent = match resolve_intent(deps, request, run).await {
        Ok(intent) => intent,
        Err(failure) => {
            return HandlingOutcome {
                intent: None,
                execution: None,
                command: None,
                unmatched: failure.unmatched.map(|read| *read),
                reply: failure.reply,
            }
        }
    };

    // 1b. Report the values the resolver read, so the surface names the
    //     engine that read them while the command starts.
    if let (true, Some(engine), entities) =
        (run.publishes(), intent.value_engine, intent.entity_values())
    {
        if !entities.is_empty() {
            deps.events
                .publish(SystemEventPayloadDto::IntentValuesRead {
                    engine: engine.as_str().to_string(),
                    model: intent.value_model.clone(),
                    entities,
                });
        }
    }

    // 2. Ask for the values the intent needs and the daemon could not
    //    read. A command that misses a required value does the wrong
    //    thing, so the turn stops before the command runs.
    let missing = intent.missing_required();
    if !missing.is_empty() {
        tracing::info!(
            intent = %intent.name,
            entities = %missing.join(", "),
            "the daemon asks the user for the values the intent needs"
        );
        let reply = missing_reply(&intent, &missing);
        return HandlingOutcome {
            intent: Some(intent),
            execution: None,
            command: None,
            unmatched: None,
            reply,
        };
    }

    // 3. Put the values of the entities into the command of the intent. An
    //    optional entity the daemon read no value for leaves no trace in
    //    the command.
    let command = strip_placeholders(&intent.command, &intent.missing_optional());
    let rendered = render_command(&command, &intent.entities, &intent.values);
    if !rendered.unreadable.is_empty() {
        tracing::warn!(
            intent = %intent.name,
            entities = %rendered.unreadable.join(", "),
            "the daemon read no value for an entity of the command"
        );
        return HandlingOutcome {
            intent: Some(intent),
            execution: None,
            command: None,
            unmatched: None,
            reply: unreadable_reply(&rendered.unreadable),
        };
    }

    // 4. Run the command of the intent and build the reply. A preview
    //    stops before the run, so the message of a preview carries the
    //    command it would run and no reply.
    let (execution, reply) = match run {
        TurnRun::Full => {
            let execution = run_command(deps, &intent, &rendered.command).await;
            let reply = build_reply(&intent, &rendered.command, execution.as_ref());
            (execution, reply)
        }
        TurnRun::Preview => (None, String::new()),
    };

    HandlingOutcome {
        intent: Some(intent),
        execution,
        command: Some(rendered.command),
        unmatched: None,
        reply,
    }
}

/// Build the reply that asks the user for the values the intent needs.
///
/// The reply names the intent, because the user reads why the daemon asks,
/// and it lists the entities that are still missing. The next message of
/// the conversation reaches the resolver with this turn as its history,
/// so the user can answer with the values alone.
fn missing_reply(intent: &ResolvedIntent, names: &[String]) -> String {
    match names {
        [one] => format!("To run {} I need a value for {one}.", intent.name),
        many => format!(
            "To run {} I need values for {}.",
            intent.name,
            join_names(many)
        ),
    }
}

/// Join entity names the way a sentence reads them.
fn join_names(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Build the reply of a value the daemon must not run.
///
/// The value a model read can carry the characters that end a command, so
/// the renderer refuses it. The reply says that the daemon read a value
/// and will not run it rather than that it read nothing.
fn unreadable_reply(names: &[String]) -> String {
    format!(
        "I cannot run the command with the value I read for {}.",
        names.join(", ")
    )
}

/// Resolve the intent of one message and publish the resolver stages.
///
/// The function returns the reply for the user when the daemon found no
/// intent, so the caller does not store a reply it cannot explain. The
/// reply carries how the resolver read the message, so a turn without an
/// intent still reports the engine and the model that read it. A resolver
/// that is not reachable is not an error of the message: the daemon
/// answers that it could not read the intent.
async fn resolve_intent(
    deps: &HandlingDeps,
    request: &ResolveRequest,
    run: TurnRun,
) -> Result<ResolvedIntent, ResolveReply> {
    let events = &deps.events;
    let publishes = run.publishes();
    let result = deps
        .resolver
        .resolve(request, |delta| {
            if publishes {
                events.publish(SystemEventPayloadDto::IntentThinking {
                    delta: delta.to_string(),
                });
            }
        })
        .await;

    match result {
        Ok(Resolution::Matched(intent)) => {
            if publishes {
                events.publish(SystemEventPayloadDto::IntentResolved {
                    intent: intent.name.clone(),
                    confidence: intent.confidence,
                    engine: intent.intent_engine.as_str().to_string(),
                    model: intent.intent_model.clone(),
                });
            }
            Ok(intent)
        }
        Ok(Resolution::Unmatched {
            engine,
            model,
            route,
        }) => {
            if publishes {
                events.publish(SystemEventPayloadDto::IntentNotFound {
                    text: request.text.clone(),
                });
            }
            Err(ResolveReply::unmatched(UnmatchedRead {
                engine,
                model,
                route,
            }))
        }
        // An empty catalog is a message no engine read, so the turn has
        // nothing to report about how it was read.
        Err(ResolveError::NoIntents) => {
            if publishes {
                events.publish(SystemEventPayloadDto::IntentNotFound {
                    text: request.text.clone(),
                });
            }
            Err(ResolveReply::unresolved(NO_INTENT_REPLY.to_string()))
        }
        Err(ResolveError::NoChoice) => {
            tracing::warn!("the resolver answered without a choice");
            Err(ResolveReply::unresolved(
                RESOLVER_UNREADABLE_REPLY.to_string(),
            ))
        }
        // The built in resolver cannot read the message, and the user
        // chooses which engine reads it. The reply names both ways out.
        Err(ResolveError::GlinerNotReady { reason }) => {
            tracing::warn!(%reason, "the built in GLiNER resolver is not ready");
            Err(ResolveReply::unresolved(format!(
                "{GLINER_NOT_READY_REPLY} {reason}. Install the model in the settings, or switch the resolver back to llama.cpp."
            )))
        }
        Err(err) => {
            tracing::warn!(error = %err, "the resolver failed");
            Err(ResolveReply::unresolved(RESOLVER_DOWN_REPLY.to_string()))
        }
    }
}

/// Run the rendered command of one intent and publish every stage of the run.
async fn run_command(
    deps: &HandlingDeps,
    intent: &ResolvedIntent,
    command: &str,
) -> Option<CommandOutcome> {
    if !deps.config.execution.enabled {
        tracing::info!(
            intent = %intent.name,
            "execution is off, the daemon answers without a command"
        );
        return None;
    }

    deps.events
        .publish(SystemEventPayloadDto::ExecutionStarted {
            intent: intent.name.clone(),
            command: command.to_string(),
        });

    let events = &deps.events;
    let result = deps
        .runner
        .run(command, |stream, line| {
            events.publish(SystemEventPayloadDto::ExecutionOutput {
                stream,
                line: line.to_string(),
            });
        })
        .await;

    match result {
        Ok(outcome) => {
            let output = output_text(&outcome);
            deps.events
                .publish(SystemEventPayloadDto::ExecutionCompleted {
                    intent: intent.name.clone(),
                    exit_code: outcome.exit_code,
                    duration_ms: outcome.duration_ms,
                    output,
                });
            Some(outcome)
        }
        Err(err) => {
            tracing::warn!(error = %err, intent = %intent.name, "the command failed");
            deps.events.publish(SystemEventPayloadDto::ExecutionFailed {
                intent: intent.name.clone(),
                error: err.to_string(),
            });
            None
        }
    }
}

/// Read the text of one command run.
///
/// A command that wrote to both streams reports its standard output, so a
/// warning does not replace the result.
fn output_text(outcome: &CommandOutcome) -> String {
    let stdout = outcome.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }
    outcome.stderr.trim().to_string()
}

/// Build the reply of one handled message.
fn build_reply(
    intent: &ResolvedIntent,
    command: &str,
    execution: Option<&CommandOutcome>,
) -> String {
    let Some(outcome) = execution else {
        return format!(
            "Execution is off. The intent {} would run: {}",
            intent.name, command
        );
    };

    let output = output_text(outcome);
    if outcome.exit_code != 0 {
        if output.is_empty() {
            return format!(
                "The intent {} failed with exit code {}.",
                intent.name, outcome.exit_code
            );
        }
        return format!(
            "The intent {} failed with exit code {}:\n{}",
            intent.name, outcome.exit_code, output
        );
    }
    if output.is_empty() {
        return format!("The intent {} ran and wrote no output.", intent.name);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    use uuid::Uuid;

    /// Build one resolved intent for the tests.
    fn intent() -> ResolvedIntent {
        ResolvedIntent {
            id: Uuid::new_v4(),
            name: "get weather".to_string(),
            command: "curl wttr.in".to_string(),
            entities: Vec::new(),
            required: Vec::new(),
            values: BTreeMap::new(),
            origins: BTreeMap::new(),
            confidence: Some(0.8),
            intent_engine: alice_core::config::ResolverEngine::Llama,
            intent_model: Some("qwen3.5-4b".to_string()),
            value_engine: None,
            value_model: None,
            route: crate::resolver::RouteReport::default(),
        }
    }

    /// Build one command outcome for the tests.
    fn outcome(exit_code: i32, stdout: &str, stderr: &str) -> CommandOutcome {
        CommandOutcome {
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            duration_ms: 12,
        }
    }

    #[test]
    fn output_text_prefers_the_standard_output() {
        let found = output_text(&outcome(0, "  Berlin: 12 C  ", "warning"));
        assert_eq!(found, "Berlin: 12 C");
    }

    #[test]
    fn output_text_falls_back_to_the_standard_error() {
        assert_eq!(output_text(&outcome(1, "  ", "not found")), "not found");
    }

    #[test]
    fn build_reply_states_that_execution_is_off() {
        let reply = build_reply(&intent(), "curl wttr.in", None);
        assert!(reply.contains("get weather"));
        assert!(reply.contains("curl wttr.in"));
    }

    #[test]
    fn build_reply_returns_the_output_of_a_good_run() {
        assert_eq!(
            build_reply(
                &intent(),
                "curl wttr.in",
                Some(&outcome(0, "Berlin: 12 C", ""))
            ),
            "Berlin: 12 C"
        );
    }

    #[test]
    fn build_reply_reports_a_failed_run() {
        let reply = build_reply(
            &intent(),
            "curl wttr.in",
            Some(&outcome(2, "", "no route to host")),
        );
        assert!(reply.contains("failed with exit code 2"));
        assert!(reply.contains("no route to host"));
    }

    #[test]
    fn build_reply_reports_a_run_without_output() {
        let reply = build_reply(&intent(), "curl wttr.in", Some(&outcome(0, " ", "")));
        assert!(reply.contains("wrote no output"));
    }

    #[test]
    fn build_reply_names_the_rendered_command() {
        let reply = build_reply(&intent(), r#"curl -s "wttr.in/Berlin""#, None);
        assert!(reply.contains(r#"curl -s "wttr.in/Berlin""#));
    }

    #[test]
    fn unreadable_reply_names_the_entities() {
        let reply = unreadable_reply(&["city".to_string()]);
        assert_eq!(
            reply,
            "I cannot run the command with the value I read for city."
        );
    }

    #[test]
    fn missing_reply_asks_for_the_one_value_the_intent_needs() {
        let reply = missing_reply(&intent(), &["city".to_string()]);
        assert_eq!(reply, "To run get weather I need a value for city.");
    }

    #[test]
    fn missing_reply_asks_for_every_value_the_intent_needs() {
        let reply = missing_reply(&intent(), &["city".to_string(), "when".to_string()]);
        assert_eq!(reply, "To run get weather I need values for city and when.");
    }

    #[test]
    fn join_names_reads_a_list_the_way_a_sentence_does() {
        assert_eq!(join_names(&[]), "");
        assert_eq!(join_names(&["city".to_string()]), "city");
        assert_eq!(
            join_names(&["city".to_string(), "when".to_string()]),
            "city and when"
        );
        assert_eq!(
            join_names(&["city".to_string(), "when".to_string(), "unit".to_string()]),
            "city, when and unit"
        );
    }

    #[test]
    fn missing_required_names_the_entities_without_a_value() {
        let mut found = intent();
        found.entities = vec!["city".to_string(), "when".to_string()];
        found.required = vec!["city".to_string()];

        assert_eq!(found.missing_required(), vec!["city"]);
        assert_eq!(found.missing_optional(), vec!["when"]);
    }

    #[test]
    fn missing_required_is_empty_when_every_value_was_read() {
        let mut found = intent();
        found.entities = vec!["city".to_string(), "when".to_string()];
        found.required = vec!["city".to_string()];
        found
            .values
            .insert("city".to_string(), "Berlin".to_string());

        assert!(found.missing_required().is_empty());
        assert_eq!(found.missing_optional(), vec!["when"]);
    }
}
