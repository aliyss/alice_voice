//! WebSocket event push of the daemon.
//! This module forwards `SystemEventDto` to the frontend.

use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tracing::instrument;

/// Upgrade `GET /api/v1/events` to a WebSocket.
#[utoipa::path(get, path = "/api/v1/events", tag = "events", responses((status = 101, description = "Switching Protocols")))]
#[instrument(skip(state, upgrade))]
pub async fn events_handler(
    State(state): State<crate::server::state::AppState>,
    upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Handle one socket connection and forward broadcast events.
async fn handle_socket(socket: WebSocket, state: crate::server::state::AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.events.subscribe();

    // Forward events to the client.
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let payload = match serde_json::to_string(&event) {
                Ok(json) => json,
                Err(err) => {
                    tracing::error!(error = %err, "failed to serialize event");
                    continue;
                }
            };
            if sender.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive from the client to keep the connection alive and detect close.
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
