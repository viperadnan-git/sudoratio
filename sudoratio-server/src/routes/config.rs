use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sudoratio_core::config_io::{ConfigResponse, ConfigUpdate};
use sudoratio_core::{ApiErrorBody, EngineConfig};

use crate::error::ApiErrorResponse;
use crate::state::AppState;

pub async fn get_config(State(s): State<Arc<AppState>>) -> Json<ConfigResponse> {
    Json(ConfigResponse::from(s.core.current_config().as_ref()))
}

pub async fn get_config_defaults() -> Json<ConfigResponse> {
    Json(ConfigResponse::from(&EngineConfig::default()))
}

pub async fn update_config(
    State(s): State<Arc<AppState>>,
    Json(update): Json<ConfigUpdate>,
) -> Result<Json<ConfigResponse>, ApiErrorResponse> {
    if matches!(update.bandwidth_tick_ms, Some(0)) {
        return Err(invalid("bandwidth_tick_ms must be >= 1"));
    }
    let mut cfg = (*s.core.current_config()).clone();
    update.apply(&mut cfg);
    s.core
        .update_config(cfg.clone())
        .await
        .map_err(|e| invalid(&format!("update_config: {e}")))?;
    Ok(Json(ConfigResponse::from(&cfg)))
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
