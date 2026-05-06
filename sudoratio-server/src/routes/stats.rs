use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::state::AppState;

pub async fn stats(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let st = s.core.stats().await;
    Json(serde_json::to_value(&st).unwrap_or_default())
}
