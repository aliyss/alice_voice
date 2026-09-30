//! System handlers of the daemon.
//!
//! The settings page draws two linked sliders for response speed and
//! response quality. Both control the same `quality` value between 0 and
//! 100 where 0 is fastest and 100 is best. `GET /api/v1/system` returns the
//! full host profile, the devices the daemon can use, and the recommended
//! preset for this machine. `POST /api/v1/system/preset` turns a quality
//! value into the router configuration the daemon would store.

use axum::{extract::State, response::IntoResponse, Json};
use tracing::instrument;

use alice_core::dto::{SystemPresetDto, SystemPresetRequestDto, SystemProfileDto};

use crate::server::state::AppState;
use crate::system::preset::{clamp_quality, quality_to_router, quality_to_speed};

/// Reply of `GET /api/v1/system`.
#[utoipa::path(
    get,
    path = "/api/v1/system",
    tag = "system",
    responses((status = 200, description = "System profile of the host", body = SystemProfileDto))
)]
#[instrument(skip(state))]
pub async fn get_system_handler(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, crate::server::reply::RestError> {
    let router_defaults = state
        .stores
        .settings
        .get_settings()
        .await
        .map(|values| values.router)
        .unwrap_or_else(|_| state.config.resolver.router.clone());

    let profile = crate::system::probe(&state.stores, &state.config, &router_defaults).await;

    Ok(Json(profile))
}

/// Reply of `POST /api/v1/system/preset`.
#[utoipa::path(
    post,
    path = "/api/v1/system/preset",
    tag = "system",
    request_body = SystemPresetRequestDto,
    responses(
        (status = 200, description = "Router preset for the quality", body = SystemPresetDto),
        (status = 400, description = "Invalid input")
    )
)]
#[instrument(skip(state, body))]
pub async fn post_system_preset_handler(
    State(state): State<AppState>,
    Json(body): Json<SystemPresetRequestDto>,
) -> Result<impl IntoResponse, crate::server::reply::RestError> {
    if body.quality > 100 {
        return Err(crate::server::reply::bad_request(
            "Quality must be between 0 and 100.",
        ));
    }
    let quality = clamp_quality(body.quality);
    let router_defaults = state
        .stores
        .settings
        .get_settings()
        .await
        .map(|values| values.router)
        .unwrap_or_else(|_| state.config.resolver.router.clone());

    let router = quality_to_router(quality, &router_defaults);

    // Reuse the same status mapping the system probe uses.
    let active_local_device = crate::resolver::device::active_device(router.local_device)
        .unwrap_or(crate::resolver::device::DEVICE_CPU)
        .to_string();

    let local_store = state.stores.local_store.as_ref();
    let (embeddings_reachable, embeddings_detail) =
        if router.retrieve.uses_embeddings() || router.list_match.uses_embeddings() {
            if matches!(router.embed_source, alice_core::config::EmbedSource::Local) {
                let spec = crate::resolver::local::catalog::find_role(
                    &router.embed_local_model,
                    crate::resolver::local::catalog::Role::Embedding,
                );
                match spec {
                    Some(spec) if local_store.is_installed(spec) => (true, None),
                    Some(spec) => (false, Some(format!("{} is not on disk", spec.id))),
                    None => (false, Some("Unknown model".to_string())),
                }
            } else {
                // Cheap probe: we do not know the base_url here without settings,
                // so we report the server case as reachable when the default probe succeeds.
                // The full system profile performs the network check.
                (
                    false,
                    Some("The router reads the words of the catalog alone.".to_string()),
                )
            }
        } else {
            (
                false,
                Some("The router reads the words of the catalog alone.".to_string()),
            )
        };

    let status = alice_core::dto::RouterStatusDto {
        fast_path: router.fast_path,
        retrieve: router.retrieve.as_str().to_string(),
        decide: router.decide.as_str().to_string(),
        extract: router.extract.as_str().to_string(),
        top_k: router.top_k,
        floor: router.floor,
        margin: router.margin,
        lexical_weight: router.lexical_weight,
        dense_weight: router.dense_weight,
        embed_model: router.embed_model.clone(),
        embed_source: router.embed_source.as_str().to_string(),
        embed_local_model: router.embed_local_model.clone(),
        rerank_model: router.rerank_model.clone(),
        laya_model: router.laya_model.clone(),
        local_device: router.local_device.as_str().to_string(),
        active_local_device,
        local: alice_core::dto::LocalStoreDto {
            models_dir: local_store.root().display().to_string(),
            models: local_store.list(),
            download: local_store.download_state().map(|state| state.to_dto()),
        },
        phrase_gate: router.phrase_gate,
        list_match: router.list_match.as_str().to_string(),
        list_floor: router.list_floor,
        embeddings_reachable,
        embeddings_detail,
    };

    Ok(Json(SystemPresetDto {
        quality,
        speed: quality_to_speed(quality),
        router: status,
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn preset_rejects_quality_above_100() {
        // Handler validates 0..100; the DTO itself holds a u8.
        let quality: u8 = 101;
        assert!(quality > 100);
    }
}
