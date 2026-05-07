//! Client profile routes — engine handles persistence + bundled defaults internally.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use sudoratio_core::{ClientProfileId, ClientProfileSummary, SudoratioError};

use crate::error::{api_error, ApiErrorResponse};
use crate::state::AppState;

pub async fn list(State(s): State<Arc<AppState>>) -> Json<Vec<ClientProfileSummary>> {
    let rows = s.core.list_profiles().await;
    tracing::info!(count = rows.len(), "GET /api/v1/clients");
    Json(rows)
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub toml: String,
}

#[derive(serde::Serialize)]
pub struct RegisterResponse {
    pub client: String,
    pub ids: Vec<String>,
}

pub async fn register(
    State(s): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> Result<Json<RegisterResponse>, ApiErrorResponse> {
    let ids = s
        .core
        .register_user_client_doc(&body.toml)
        .await
        .map_err(|e| api_error(SudoratioError::ClientProfileParse(e.to_string())))?;
    let client = ids
        .first()
        .and_then(|id| id.0.split_once('@').map(|(c, _)| c.to_string()))
        .unwrap_or_default();
    tracing::info!(client = %client, variants = ids.len(), "POST /api/v1/clients ok");
    Ok(Json(RegisterResponse {
        client,
        ids: ids.into_iter().map(|id| id.0).collect(),
    }))
}

pub async fn activate(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let pid = ClientProfileId::from(id.as_str());
    s.core.set_active_profile(pid).await.map_err(api_error)?;
    tracing::info!(profile_id = %id, "activate ok");
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Serialize)]
pub struct ClientSourceResponse {
    pub client: String,
    pub editable: bool,
    pub toml: String,
}

pub async fn source(
    State(s): State<Arc<AppState>>,
    Path(client): Path<String>,
) -> Result<Json<ClientSourceResponse>, ApiErrorResponse> {
    let toml = s.core.client_source(&client).ok_or_else(|| {
        api_error(SudoratioError::UnknownClientProfile(ClientProfileId(
            client.clone(),
        )))
    })?;
    let editable = s
        .core
        .list_profiles()
        .await
        .into_iter()
        .find(|p| p.client == client)
        .map(|p| p.editable)
        .unwrap_or(false);
    Ok(Json(ClientSourceResponse {
        client,
        editable,
        toml,
    }))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(client): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    s.core
        .remove_user_client_doc(&client)
        .await
        .map_err(|e| api_error(SudoratioError::ClientProfileParse(e.to_string())))?;
    Ok(Json(serde_json::json!({ "ok": true })))
}
