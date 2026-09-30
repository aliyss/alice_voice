//! Script handlers of the daemon.
//! One function handles one endpoint of the scripts that wait for approval.
//!
//! The daemon runs no script of its own accord. A message that no intent
//! matched may be answered with a script the model wrote, and the daemon
//! stores that script and waits. These endpoints read the script back, run
//! one the user approved, and close one the user denied. A decision lands
//! on the store, so two approvals cannot both run the same script.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use tracing::instrument;
use uuid::Uuid;

use alice_core::dto::{ChatRoleDto, MessageMetaDto, ScriptDto, SystemEventPayloadDto};

use crate::conversation::NewMessage;
use crate::execution::CommandOutcome;
use crate::pending_script::{Claim, PendingScript, STATUS_APPROVED, STATUS_DENIED};
use crate::server::reply::{database_failed, not_found, RestError};
use crate::server::state::AppState;

/// The name the events report for an approved script.
///
/// The run of a script has no intent, so the events name a pseudo intent.
/// The surface reads the same shapes it reads for the command of an intent.
const SCRIPT_INTENT: &str = "shell script";

/// Reply of `GET /api/v1/scripts/{id}`.
#[utoipa::path(
    get,
    path = "/api/v1/scripts/{id}",
    tag = "scripts",
    params(("id" = Uuid, Path, description = "Script id")),
    responses(
        (status = 200, description = "The stored script", body = ScriptDto),
        (status = 404, description = "Script not found")
    )
)]
#[instrument(skip(state))]
pub async fn get_script_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let script = read_script(&state, id).await?;
    Ok(Json(to_dto(&script)))
}

/// Reply of `POST /api/v1/scripts/{id}/approve`.
///
/// The daemon claims the pending script and, when the claim belongs to
/// this call, runs it. A second approval of the same script reports the
/// stored script and runs nothing.
#[utoipa::path(
    post,
    path = "/api/v1/scripts/{id}/approve",
    tag = "scripts",
    params(("id" = Uuid, Path, description = "Script id")),
    responses(
        (status = 200, description = "The stored script after the run", body = ScriptDto),
        (status = 404, description = "Script not found")
    )
)]
#[instrument(skip(state))]
pub async fn approve_script_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let script = match claim(&state, id, STATUS_APPROVED).await? {
        ClaimOutcome::Claimed(script) => script,
        ClaimOutcome::Decided(script) => return Ok(Json(to_dto(&script))),
    };

    state
        .events
        .publish(SystemEventPayloadDto::ScriptApproved { id });
    let reply = run_script(&state, &script).await;
    finish(&state, &script, reply).await?;
    let stored = read_script(&state, id).await?;
    Ok(Json(to_dto(&stored)))
}

/// Reply of `POST /api/v1/scripts/{id}/deny`.
///
/// A denied script stays in the store with its decision, so the surface
/// still reads it and the daemon never runs it.
#[utoipa::path(
    post,
    path = "/api/v1/scripts/{id}/deny",
    tag = "scripts",
    params(("id" = Uuid, Path, description = "Script id")),
    responses(
        (status = 200, description = "The stored script after the decision", body = ScriptDto),
        (status = 404, description = "Script not found")
    )
)]
#[instrument(skip(state))]
pub async fn deny_script_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, RestError> {
    let script = match claim(&state, id, STATUS_DENIED).await? {
        ClaimOutcome::Claimed(script) => script,
        ClaimOutcome::Decided(script) => return Ok(Json(to_dto(&script))),
    };
    state
        .events
        .publish(SystemEventPayloadDto::ScriptDenied { id });
    Ok(Json(to_dto(&script)))
}

/// What a claim of one script leaves the handler to do.
enum ClaimOutcome {
    /// The handler owns the decision and moves the script on.
    Claimed(PendingScript),
    /// Another decision reached the script first.
    Decided(PendingScript),
}

/// Read one stored script, or report that it does not exist.
async fn read_script(state: &AppState, id: Uuid) -> Result<PendingScript, RestError> {
    state
        .stores
        .pending_scripts
        .get(id)
        .await
        .map_err(|err| {
            database_failed(err, "script read", "The daemon could not read the script.")
        })?
        .ok_or_else(not_found)
}

/// Move one script to its decision and read back the row it left.
async fn claim(state: &AppState, id: Uuid, next: &str) -> Result<ClaimOutcome, RestError> {
    let store = &state.stores.pending_scripts;
    match store.claim(id, next).await.map_err(|err| {
        database_failed(
            err,
            "script claim",
            "The daemon could not claim the script.",
        )
    })? {
        Claim::Claimed(script) => Ok(ClaimOutcome::Claimed(script)),
        Claim::Decided(_) => Ok(ClaimOutcome::Decided(read_script(state, id).await?)),
        Claim::Missing => Err(not_found()),
    }
}

/// Run one approved script and build the reply of its run.
///
/// The run streams every output line on the event bus, so the surface
/// shows the work of the script the way it shows the work of an intent.
async fn run_script(state: &AppState, script: &PendingScript) -> String {
    state
        .events
        .publish(SystemEventPayloadDto::ExecutionStarted {
            intent: SCRIPT_INTENT.to_string(),
            command: script.script.clone(),
        });

    let events = &state.events;
    let result = state
        .stores
        .runner
        .run(&script.script, |stream, line| {
            events.publish(SystemEventPayloadDto::ExecutionOutput {
                stream,
                line: line.to_string(),
            });
        })
        .await;

    match result {
        Ok(outcome) => {
            events.publish(SystemEventPayloadDto::ExecutionCompleted {
                intent: SCRIPT_INTENT.to_string(),
                exit_code: outcome.exit_code,
                duration_ms: outcome.duration_ms,
                output: output_text(&outcome),
            });
            reply_of(&outcome)
        }
        Err(err) => {
            tracing::warn!(error = %err, "the script failed to run");
            events.publish(SystemEventPayloadDto::ExecutionFailed {
                intent: SCRIPT_INTENT.to_string(),
                error: err.to_string(),
            });
            format!("The script did not run: {err}")
        }
    }
}

/// Mark one script as run and store the reply of its run.
///
/// The script belongs to a conversation only when the queue stored the
/// turn, and a script of an unstored turn still runs and still reports its
/// answer over the socket.
async fn finish(state: &AppState, script: &PendingScript, reply: String) -> Result<(), RestError> {
    state
        .stores
        .pending_scripts
        .mark_ran(script.id)
        .await
        .map_err(|err| {
            database_failed(err, "script run", "The daemon could not close the script.")
        })?;

    let Some(conversation_id) = script.conversation_id else {
        return Ok(());
    };
    state
        .stores
        .conversations
        .append_message(
            conversation_id,
            NewMessage {
                id: Uuid::new_v4(),
                role: ChatRoleDto::Assistant,
                text: approved_reply(&script.script, &reply),
                created_at: Utc::now(),
                intent_id: None,
                intent_name: None,
                confidence: None,
                meta: Some(script_meta(&script.script)),
            },
        )
        .await
        .map_err(|err| {
            database_failed(
                err,
                "script reply",
                "The daemon could not store the answer of the script.",
            )
        })?;
    Ok(())
}

/// Build the reply of one approved script.
///
/// The script stays in the answer with its run, so the transcript carries
/// the script the user approved after the card is gone. The answer reads
/// as the script and then what it wrote.
fn approved_reply(script: &str, reply: &str) -> String {
    format!(
        "I ran this script:\n```sh\n{}\n```\n\n{reply}",
        script.trim()
    )
}

/// Read the metadata of one approved script run.
///
/// The command of the run is the script itself, so the row of the turn
/// names what ran the way it names the command of an intent.
fn script_meta(script: &str) -> MessageMetaDto {
    MessageMetaDto {
        command: Some(script.to_string()),
        ..MessageMetaDto::default()
    }
}

/// Read the text of one script run.
///
/// A run that wrote to both streams reports its standard output, so a
/// warning does not replace the result.
fn output_text(outcome: &CommandOutcome) -> String {
    let stdout = outcome.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_string();
    }
    outcome.stderr.trim().to_string()
}

/// Build the reply of one script run.
fn reply_of(outcome: &CommandOutcome) -> String {
    let output = output_text(outcome);
    if outcome.exit_code != 0 {
        if output.is_empty() {
            return format!("The script failed with exit code {}.", outcome.exit_code);
        }
        return format!(
            "The script failed with exit code {}:\n{}",
            outcome.exit_code, output
        );
    }
    if output.is_empty() {
        return "The script ran and wrote no output.".to_string();
    }
    output
}

/// Map one stored script to the reply of the endpoints.
fn to_dto(script: &PendingScript) -> ScriptDto {
    ScriptDto {
        id: script.id,
        conversation_id: script.conversation_id,
        request_text: script.request_text.clone(),
        summary: script.summary.clone(),
        script: script.script.clone(),
        destructiveness: script.destructiveness,
        status: script.status.clone(),
        created_at: script.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one command outcome for the tests.
    fn outcome(exit_code: i32, stdout: &str, stderr: &str) -> CommandOutcome {
        CommandOutcome {
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            duration_ms: 7,
        }
    }

    #[test]
    fn the_reply_of_a_good_run_is_the_output() {
        assert_eq!(reply_of(&outcome(0, "total 12", "")), "total 12");
    }

    #[test]
    fn the_reply_of_a_run_without_output_says_so() {
        assert!(reply_of(&outcome(0, "  ", "")).contains("wrote no output"));
    }

    #[test]
    fn the_reply_of_a_failed_run_names_the_code_and_the_error() {
        let reply = reply_of(&outcome(2, "", "permission denied"));
        assert!(reply.contains("exit code 2"));
        assert!(reply.contains("permission denied"));
    }

    #[test]
    fn the_output_prefers_the_standard_output() {
        assert_eq!(output_text(&outcome(0, "  done  ", "warning")), "done");
    }

    #[test]
    fn the_reply_of_an_approved_script_keeps_the_script_and_the_run() {
        let reply = approved_reply("ls -la", "total 12");

        assert!(reply.contains("```sh\nls -la\n```"));
        assert!(reply.ends_with("total 12"));
    }

    #[test]
    fn the_metadata_of_an_approved_script_names_the_command() {
        let meta = script_meta("ls -la");
        assert_eq!(meta.command, Some("ls -la".to_string()));
    }
}
