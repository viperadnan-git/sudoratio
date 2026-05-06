use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sudoratio_core::ApiErrorBody;

use crate::config_io::{self, ConfigResponse, ConfigUpdate};
use crate::error::ApiErrorResponse;
use crate::state::AppState;

pub async fn get_config(State(s): State<Arc<AppState>>) -> Json<ConfigResponse> {
    let body = ConfigResponse::from(s.core.current_config().as_ref());
    tracing::info!(announce_port = body.announce_port, "GET /api/v1/config");
    Json(body)
}

pub async fn update_config(
    State(s): State<Arc<AppState>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<ConfigResponse>, ApiErrorResponse> {
    if matches!(update.max_active_torrents, Some(0)) {
        return Err(invalid("max_active_torrents must be >= 1"));
    }
    if matches!(update.bandwidth_tick_ms, Some(0)) {
        return Err(invalid("bandwidth_tick_ms must be >= 1"));
    }
    let mut cfg = (*s.core.current_config()).clone();
    update.apply(&mut cfg);
    s.core.update_config(cfg.clone()).await;
    let body = ConfigResponse::from(&cfg);
    let path = s.core_config_path.clone();
    let cfg_for_disk = cfg.clone();
    tokio::spawn(async move {
        let res = tokio::task::spawn_blocking(move || config_io::save(&path, &cfg_for_disk)).await;
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "config persisted to memory only; disk write failed")
            }
            Err(e) => tracing::warn!(error = %e, "config disk-write task join failed"),
        }
    });
    Ok(Json(body))
}

fn invalid(msg: &str) -> ApiErrorResponse {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiErrorBody {
            code: "invalid_config",
            message: msg.to_string(),
        }),
    )
}
