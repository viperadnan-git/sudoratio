use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use sudoratio_core::HealthStatus;

use crate::state::AppState;

pub async fn health(State(s): State<Arc<AppState>>) -> Json<HealthStatus> {
    let h = s.core.health();
    tracing::info!(ok = h.ok, version = %h.version, "GET /api/v1/health");
    Json(h)
}
