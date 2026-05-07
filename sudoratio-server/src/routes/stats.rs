use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct StatsQuery {
    pub preset_id: Option<String>,
}

pub async fn stats(
    State(s): State<Arc<AppState>>,
    Query(q): Query<StatsQuery>,
) -> Json<serde_json::Value> {
    let st = s.core.stats_for_preset(q.preset_id.as_deref()).await;
    Json(serde_json::to_value(&st).unwrap_or_default())
}
