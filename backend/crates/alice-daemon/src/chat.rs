//! Storing of one chat turn.
//!
//! This module resolves the conversation, queues the message, handles it,
//! and moves the handled messages into the conversation history. Handling
//! publishes every stage of the turn on the event bus, so the interface
//! shows the resolver and the command while the turn runs.

use chrono::{DateTime, Utc};
use thiserror::Error;
use uuid::Uuid;

use alice_core::dto::{
    ChatMessageDto, ChatReplyDto, ChatRoleDto, MessageMetaDto, MessageRouteDto,
    SystemEventPayloadDto,
};

use crate::conversation::{ConversationService, NewMessage};
use crate::handling::{self, HandlingDeps, HandlingOutcome};
use crate::queue::{NewQueuedMessage, QueueService};
use crate::resolver::{ContextTurn, ResolveRequest, RouteReport};

/// Failure of the store turn action.
#[derive(Debug, Error)]
pub enum StoreTurnError {
    /// The client sent a conversation that does not exist.
    #[error("conversation {id} does not exist")]
    ConversationNotFound { id: Uuid },
    /// The database rejected the write.
    #[error("database failed: {0}")]
    Database(#[from] sea_orm::DbErr),
}

/// What one handled turn puts on its messages.
///
/// The intent is a column of its own, and everything else the daemon
/// knows about the turn travels as one metadata object. A turn that read
/// no intent carries no intent columns, and it still reports how the
/// resolver read the message.
#[derive(Clone, Debug, Default, PartialEq)]
struct TurnMeta {
    /// The identifier of the intent, or null.
    id: Option<String>,
    /// The name of the intent, or null.
    name: Option<String>,
    /// The probability of the choice, or null.
    confidence: Option<f32>,
    /// How the daemon read and ran the turn, or null when it read nothing.
    meta: Option<MessageMetaDto>,
}

impl TurnMeta {
    /// Read what one handled turn puts on its messages.
    fn from_outcome(outcome: &HandlingOutcome) -> Self {
        let Some(intent) = &outcome.intent else {
            return Self {
                id: None,
                name: None,
                confidence: None,
                meta: outcome.unmatched.as_ref().map(|read| MessageMetaDto {
                    intent_engine: Some(read.engine.as_str().to_string()),
                    intent_model: read.model.clone(),
                    value_engine: None,
                    value_model: None,
                    stage: read.route.stage.clone(),
                    candidates: read.route.candidates.clone(),
                    entities: Vec::new(),
                    route: route_of(&read.route),
                    command: None,
                    exit_code: None,
                    duration_ms: None,
                }),
            };
        };
        Self {
            id: Some(intent.id.to_string()),
            name: Some(intent.name.clone()),
            confidence: intent.confidence,
            meta: Some(MessageMetaDto {
                intent_engine: Some(intent.intent_engine.as_str().to_string()),
                intent_model: intent.intent_model.clone(),
                value_engine: intent
                    .value_engine
                    .map(|engine| engine.as_str().to_string()),
                value_model: intent.value_model.clone(),
                stage: intent.route.stage.clone(),
                candidates: intent.route.candidates.clone(),
                entities: intent.entity_values(),
                route: route_of(&intent.route),
                command: outcome.command.clone(),
                exit_code: outcome.execution.as_ref().map(|run| run.exit_code),
                duration_ms: outcome.execution.as_ref().map(|run| run.duration_ms),
            }),
        }
    }
}

/// Read how the layered router read one turn.
///
/// A backend that is not the router reads no stage, so it reports no
/// route: the metadata of a stored turn carries a route when there is one
/// to read and nothing when the turn never passed a stage.
fn route_of(report: &RouteReport) -> Option<MessageRouteDto> {
    if report.steps.is_empty() && report.stage.is_none() {
        return None;
    }
    Some(MessageRouteDto {
        stage: report.stage.clone(),
        matched: report.matched,
        reason: report.reason.clone(),
        steps: report.steps.clone(),
    })
}

/// Read the metadata of one handled turn.
///
/// The chat stores the metadata with the messages and the preview of the
/// settings page reports it, so both read the same turn the same way.
pub fn meta_of(outcome: &HandlingOutcome) -> Option<MessageMetaDto> {
    TurnMeta::from_outcome(outcome).meta
}

/// Store one chat turn and return the conversation, the user message, and the reply.
///
/// The daemon creates the conversation when `conversation_id` is null. The
/// title of a new conversation comes from `text`, its first message.
pub async fn store_turn(
    conversations: &ConversationService,
    queue: &QueueService,
    deps: &HandlingDeps,
    text: String,
    conversation_id: Option<Uuid>,
) -> Result<ChatReplyDto, StoreTurnError> {
    let now = Utc::now();

    // 1. Resolve the conversation, or start one from this first message.
    let mut conversation = match conversation_id {
        Some(id) => conversations
            .find_conversation(id)
            .await?
            .ok_or(StoreTurnError::ConversationNotFound { id })?,
        None => conversations.create_conversation(&text, now).await?,
    };

    // 2. Queue the user message.
    let message_id = Uuid::new_v4();
    queue
        .enqueue(&NewQueuedMessage {
            id: message_id,
            conversation_id: conversation.id,
            role: ChatRoleDto::User,
            text: text.clone(),
            created_at: now,
        })
        .await?;
    deps.events
        .publish(SystemEventPayloadDto::MessageQueued { id: message_id });

    // 3. Handle the message. The stages stream on the event bus.
    let history = build_history(conversations, conversation.id, deps).await?;
    let outcome = handling::handle(
        deps,
        &ResolveRequest {
            text: text.clone(),
            history,
        },
    )
    .await;
    let intent = TurnMeta::from_outcome(&outcome);
    if let Some(execution) = &outcome.execution {
        tracing::debug!(
            exit_code = execution.exit_code,
            duration_ms = execution.duration_ms,
            "the command of the turn finished"
        );
    }

    // 4. Move the message to the store, store the reply, and close the queue row.
    let user = conversations
        .append_message(
            conversation.id,
            new_message(message_id, ChatRoleDto::User, text, now, &intent),
        )
        .await?;
    let reply = conversations
        .append_message(
            conversation.id,
            new_message(
                Uuid::new_v4(),
                ChatRoleDto::Assistant,
                outcome.reply,
                now,
                &intent,
            ),
        )
        .await?;
    queue.mark_handled(message_id, now).await?;

    // 5. Return the stored turn.
    conversation.updated_at = now;
    Ok(ChatReplyDto {
        stored: true,
        conversation: Some(conversation),
        user,
        reply,
    })
}

/// Build the ephemeral turn that the daemon returns while the queue is off.
///
/// Nothing is stored, so the messages carry no conversation. The daemon
/// still resolves the intent and runs its command, because the queue
/// toggle controls the store only.
pub async fn build_ephemeral_turn(deps: &HandlingDeps, text: &str) -> ChatReplyDto {
    let now = Utc::now();
    let user_id = Uuid::new_v4();
    deps.events
        .publish(SystemEventPayloadDto::MessageQueued { id: user_id });

    let outcome = handling::handle(
        deps,
        &ResolveRequest {
            text: text.to_string(),
            history: Vec::new(),
        },
    )
    .await;
    let intent = TurnMeta::from_outcome(&outcome);

    ChatReplyDto {
        stored: false,
        conversation: None,
        user: ephemeral_message(user_id, ChatRoleDto::User, text.to_string(), now, &intent),
        reply: ephemeral_message(
            Uuid::new_v4(),
            ChatRoleDto::Assistant,
            outcome.reply,
            now,
            &intent,
        ),
    }
}

/// Read the earlier turns of a conversation as the context of the resolver.
async fn build_history(
    conversations: &ConversationService,
    conversation_id: Uuid,
    deps: &HandlingDeps,
) -> Result<Vec<ContextTurn>, sea_orm::DbErr> {
    let limit = deps.config.resolver.context_turns;
    if limit == 0 {
        return Ok(Vec::new());
    }

    let messages = conversations
        .list_recent_messages(conversation_id, limit)
        .await?;
    Ok(messages
        .into_iter()
        .map(|message| ContextTurn {
            role: message.role,
            text: message.text,
        })
        .collect())
}

/// Build one stored message of a turn.
fn new_message(
    id: Uuid,
    role: ChatRoleDto,
    text: String,
    at: DateTime<Utc>,
    intent: &TurnMeta,
) -> NewMessage {
    NewMessage {
        id,
        role,
        text,
        created_at: at,
        intent_id: intent.id.clone(),
        intent_name: intent.name.clone(),
        confidence: intent.confidence,
        meta: intent.meta.clone(),
    }
}

/// Build one message that the daemon does not store.
fn ephemeral_message(
    id: Uuid,
    role: ChatRoleDto,
    text: String,
    at: DateTime<Utc>,
    intent: &TurnMeta,
) -> ChatMessageDto {
    ChatMessageDto {
        id,
        conversation_id: None,
        role,
        text,
        created_at: at,
        intent_id: intent.id.clone(),
        intent_name: intent.name.clone(),
        confidence: intent.confidence,
        meta: intent.meta.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use alice_core::config::ResolverEngine;
    use alice_core::dto::MessageEntityDto;

    use crate::execution::CommandOutcome;
    use crate::handling::UnmatchedRead;
    use crate::resolver::service::EntityRead;
    use crate::resolver::ResolvedIntent;

    /// Build one resolved intent for the tests.
    fn resolved() -> ResolvedIntent {
        let mut values = BTreeMap::new();
        values.insert("city".to_string(), "Berlin".to_string());
        let mut origins = BTreeMap::new();
        origins.insert(
            "city".to_string(),
            EntityRead {
                source: "spans",
                engine: Some(ResolverEngine::Gliner),
                model: Some("gliner_small-v2.1".to_string()),
                read: None,
                score: None,
            },
        );
        ResolvedIntent {
            id: Uuid::nil(),
            name: "get weather".to_string(),
            command: "curl wttr.in/{city}".to_string(),
            entities: vec!["city".to_string()],
            required: vec!["city".to_string()],
            values,
            origins,
            confidence: Some(0.7),
            intent_engine: ResolverEngine::Llama,
            intent_model: Some("qwen3.5-4b".to_string()),
            value_engine: Some(ResolverEngine::Gliner),
            value_model: Some("gliner_small-v2.1".to_string()),
            route: crate::resolver::RouteReport::default(),
        }
    }

    /// Build one handled turn for the tests.
    fn outcome(
        intent: Option<ResolvedIntent>,
        unmatched: Option<UnmatchedRead>,
    ) -> HandlingOutcome {
        HandlingOutcome {
            intent,
            execution: None,
            command: None,
            unmatched,
            reply: "ok".to_string(),
        }
    }

    #[test]
    fn turn_meta_reads_the_resolved_intent() {
        let mut outcome = outcome(Some(resolved()), None);
        outcome.command = Some("curl wttr.in/Berlin".to_string());
        let intent = TurnMeta::from_outcome(&outcome);

        assert_eq!(intent.id, Some(Uuid::nil().to_string()));
        assert_eq!(intent.name, Some("get weather".to_string()));
        assert_eq!(intent.confidence, Some(0.7));
    }

    #[test]
    fn turn_meta_names_the_engine_and_the_model_of_each_step() {
        let mut outcome = outcome(Some(resolved()), None);
        outcome.command = Some("curl wttr.in/Berlin".to_string());
        outcome.execution = Some(CommandOutcome {
            exit_code: 0,
            duration_ms: 412,
            stdout: "Berlin: sun".to_string(),
            stderr: String::new(),
        });
        let meta = TurnMeta::from_outcome(&outcome)
            .meta
            .expect("a matched turn carries metadata");

        assert_eq!(meta.intent_engine, Some("llama".to_string()));
        assert_eq!(meta.intent_model, Some("qwen3.5-4b".to_string()));
        assert_eq!(meta.value_engine, Some("gliner".to_string()));
        assert_eq!(meta.value_model, Some("gliner_small-v2.1".to_string()));
        assert_eq!(
            meta.entities,
            vec![MessageEntityDto {
                name: "city".to_string(),
                value: "Berlin".to_string(),
                source: Some("spans".to_string()),
                engine: Some("gliner".to_string()),
                model: Some("gliner_small-v2.1".to_string()),
                read: None,
                score: None,
            }]
        );
        assert_eq!(meta.command, Some("curl wttr.in/Berlin".to_string()));
        assert_eq!(meta.exit_code, Some(0));
        assert_eq!(meta.duration_ms, Some(412));
        assert!(
            meta.route.is_none(),
            "a turn of another backend reads no route"
        );
    }

    #[test]
    fn turn_meta_reports_how_a_message_with_no_intent_was_read() {
        let outcome = outcome(
            None,
            Some(UnmatchedRead {
                engine: ResolverEngine::Gliner,
                model: Some("gliner_small-v2.1".to_string()),
                route: crate::resolver::RouteReport::default(),
            }),
        );
        let meta = TurnMeta::from_outcome(&outcome);

        assert_eq!(meta.id, None);
        assert_eq!(meta.name, None);
        assert_eq!(meta.confidence, None);
        let detail = meta.meta.expect("the resolver read the message");
        assert_eq!(detail.intent_engine, Some("gliner".to_string()));
        assert_eq!(detail.intent_model, Some("gliner_small-v2.1".to_string()));
        assert_eq!(detail.value_engine, None);
        assert_eq!(detail.command, None);
        assert_eq!(detail.exit_code, None);
        assert!(detail.route.is_none());
    }

    #[test]
    fn turn_meta_carries_every_stage_of_a_router_turn() {
        let mut intent = resolved();
        intent.intent_engine = ResolverEngine::Router;
        intent.route = crate::resolver::RouteReport {
            stage: Some("retrieve".to_string()),
            candidates: vec![alice_core::dto::MessageCandidateDto {
                name: "get weather".to_string(),
                score: 0.7,
                evidence: "words".to_string(),
            }],
            matched: true,
            reason: None,
            steps: vec![alice_core::dto::MessageRouteStepDto {
                stage: "retrieve".to_string(),
                outcome: "passed".to_string(),
                reader: "lexical".to_string(),
                model: None,
                detail: Some("ranked 2 intents and kept the best 1".to_string()),
                candidates: Vec::new(),
                duration_ms: 4,
            }],
        };
        let mut outcome = outcome(Some(intent), None);
        outcome.command = Some("curl wttr.in/Berlin".to_string());

        let meta = meta_of(&outcome).expect("the turn carries metadata");
        let route = meta.route.expect("a router turn carries its route");

        assert_eq!(meta.stage.as_deref(), Some("retrieve"));
        assert!(route.matched);
        assert_eq!(route.stage.as_deref(), Some("retrieve"));
        assert_eq!(route.steps.len(), 1);
        assert_eq!(route.steps[0].reader, "lexical");
        assert_eq!(route.steps[0].duration_ms, 4);
    }

    #[test]
    fn a_route_reports_itself_only_when_the_router_read_it() {
        assert!(route_of(&RouteReport::default()).is_none());

        let report = RouteReport {
            stage: Some("none".to_string()),
            matched: false,
            reason: Some("below the floor".to_string()),
            ..RouteReport::default()
        };

        let route = route_of(&report).expect("a refused turn still read the message");
        assert!(!route.matched);
        assert_eq!(route.reason.as_deref(), Some("below the floor"));
    }

    #[test]
    fn turn_meta_stays_empty_when_nothing_read_the_message() {
        assert_eq!(
            TurnMeta::from_outcome(&outcome(None, None)),
            TurnMeta::default()
        );
    }

    #[test]
    fn ephemeral_message_carries_the_intent_of_the_turn() {
        let intent = TurnMeta {
            id: Some("id".to_string()),
            name: Some("get weather".to_string()),
            confidence: None,
            meta: None,
        };
        let message = ephemeral_message(
            Uuid::new_v4(),
            ChatRoleDto::User,
            "hi".to_string(),
            Utc::now(),
            &intent,
        );

        assert!(message.conversation_id.is_none());
        assert_eq!(message.intent_name, Some("get weather".to_string()));
    }
}
