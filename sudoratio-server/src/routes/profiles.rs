//! Client profile routes.
//!
//! API model:
//!   GET    /api/v1/clients                    -> list of variants (id = client@version)
//!   POST   /api/v1/clients                    -> register/replace a user client doc
//!   GET    /api/v1/clients/{client}/source    -> raw doc TOML by client family
//!   DELETE /api/v1/clients/{client}           -> remove a user client (all variants)
//!   POST   /api/v1/clients/variants/{id}/activate
//!                                                     -> activate one variant by id

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use sudoratio_core::{ClientProfileId, ClientProfileSummary, SudoratioError};

use crate::bundled;
use crate::error::{api_error, ApiErrorResponse};
use crate::profile_store;
use crate::state::AppState;

pub async fn list(State(s): State<Arc<AppState>>) -> Json<Vec<ClientProfileSummary>> {
    let rows = s.core.list_profiles().await;
    tracing::info!(count = rows.len(), "GET /api/v1/clients");
    Json(rows)
}

#[derive(Deserialize)]
pub struct RegisterBody {
    /// Full client doc TOML (top-level `client = "..."` + base + `[[variant]]` blocks).
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
        .register_client(&body.toml)
        .await
        .map_err(api_error)?;
    let client = ids
        .first()
        .and_then(|id| id.0.split_once('@').map(|(c, _)| c.to_string()))
        .unwrap_or_default();
    if let Err(e) = profile_store::save(&s.core_config_path, &client, &body.toml).await {
        tracing::warn!(client = %client, error = %e, "client doc persist failed");
    }
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
    tracing::info!(client = %client, editable, "GET source");
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
    let active_was_in_client = s
        .core
        .current_active_profile()
        .await
        .as_ref()
        .map(|id| id.0.starts_with(&format!("{client}@")))
        .unwrap_or(false);
    s.core.remove_client(&client).await.map_err(api_error)?;
    if let Err(e) = profile_store::delete(&s.core_config_path, &client).await {
        tracing::warn!(client = %client, error = %e, "client doc disk delete failed");
    }
    if active_was_in_client {
        if let Err(e) = bundled::activate_default_or_fallback(&s.core).await {
            tracing::warn!(error = %e, "no fallback client after deleting active");
        }
    }
    tracing::info!(client = %client, "DELETE ok");
    Ok(Json(serde_json::json!({ "ok": true })))
}
