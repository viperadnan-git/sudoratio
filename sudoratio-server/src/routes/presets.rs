use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use sudoratio_core::{ApiErrorBody, PresetPolicy, PresetPolicyUpdate, PresetSnapshot};

use crate::error::ApiErrorResponse;
use crate::state::AppState;

pub async fn list(State(s): State<Arc<AppState>>) -> Json<Vec<PresetSnapshot>> {
    Json(
        s.core
            .list_presets()
            .iter()
            .map(|p| p.snapshot())
            .collect(),
    )
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<PresetSnapshot>, ApiErrorResponse> {
    s.core
        .get_preset(&id)
        .map(|p| Json(p.snapshot()))
        .ok_or_else(|| not_found(&id))
}

#[derive(Deserialize)]
pub struct CreateBody {
    pub id: Option<String>,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub policy: Option<PresetPolicy>,
}

pub async fn create(
    State(s): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<PresetSnapshot>, ApiErrorResponse> {
    let policy = body.policy.unwrap_or_default();
    s.core
        .create_preset(body.id, body.name, body.color, policy)
        .await
        .map(|p| Json(p.snapshot()))
        .map_err(|e| invalid(&e.to_string()))
}

#[derive(Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub policy: Option<PresetPolicyUpdate>,
}

pub async fn update(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<PresetSnapshot>, ApiErrorResponse> {
    if body.name.is_some() || body.color.is_some() {
        s.core
            .rename_preset(&id, body.name, body.color)
            .await
            .map_err(|e| invalid(&e.to_string()))?;
    }
    if let Some(patch) = body.policy {
        s.core
            .update_preset_policy(&id, patch)
            .await
            .map_err(|e| invalid(&e.to_string()))?;
    }
    s.core
        .get_preset(&id)
        .map(|p| Json(p.snapshot()))
        .ok_or_else(|| not_found(&id))
}

#[derive(Deserialize)]
pub struct DeleteQuery {
    pub reassign_to: Option<String>,
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<DeleteQuery>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    s.core
        .delete_preset(&id, q.reassign_to.as_deref())
        .await
        .map_err(|e| invalid(&e.to_string()))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn invalid(msg: &str) -> ApiErrorResponse {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiErrorBody {
            code: "invalid_preset",
            message: msg.to_string(),
        }),
    )
}

fn not_found(id: &str) -> ApiErrorResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ApiErrorBody {
            code: "preset_not_found",
            message: format!("preset {id:?} not found"),
        }),
    )
}
