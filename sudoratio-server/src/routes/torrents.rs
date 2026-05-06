use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use sudoratio_core::{
    AnnounceEvent, AnnounceOutcome, AnnounceQueryOverrides, MetainfoTorrent, SudoratioError,
    TorrentId,
};

use crate::error::{api_error, ApiErrorResponse};
use crate::state::{forget_torrent, persist_torrent, AppState};

fn resolve(s: &AppState, info_hash: &str) -> Result<TorrentId, ApiErrorResponse> {
    s.core
        .torrent_id_by_info_hash(info_hash)
        .ok_or_else(|| api_error(SudoratioError::TorrentNotFound))
}

pub async fn list(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let v = s.core.list_torrents().await;
    tracing::info!(count = v.len(), "GET /api/v1/torrents");
    Json(serde_json::to_value(&v).unwrap_or_default())
}

pub async fn get(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let id = resolve(&s, &info_hash)?;
    let row = s
        .core
        .get_torrent(id)
        .await
        .ok_or_else(|| api_error(SudoratioError::TorrentNotFound))?;
    Ok(Json(serde_json::to_value(&row).unwrap_or_default()))
}

#[derive(Deserialize)]
pub struct PatchBody {
    #[serde(default)]
    pub downloaded: Option<u64>,
    #[serde(default)]
    pub left: Option<u64>,
    #[serde(default)]
    pub uploaded: Option<u64>,
}

pub async fn patch(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
    Json(body): Json<PatchBody>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let id = resolve(&s, &info_hash)?;
    s.core
        .update_torrent_transfer(id, body.downloaded, body.left, body.uploaded)
        .await
        .map_err(api_error)?;
    persist_torrent(&s, &info_hash).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn pause(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let id = resolve(&s, &info_hash)?;
    s.core.pause_torrent(id).await.map_err(api_error)?;
    persist_torrent(&s, &info_hash).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn resume(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let id = resolve(&s, &info_hash)?;
    s.core.resume_torrent(id).await.map_err(api_error)?;
    persist_torrent(&s, &info_hash).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn delete(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let id = resolve(&s, &info_hash)?;
    s.core.remove_torrent(id).await.map_err(api_error)?;
    forget_torrent(&s, info_hash).await;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct AnnounceBody {
    pub event: AnnounceEvent,
    #[serde(default)]
    pub overrides: AnnounceQueryOverrides,
}

#[derive(Deserialize)]
pub struct AnnouncesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(serde::Serialize)]
pub struct AnnouncesPage {
    pub items: Vec<sudoratio_core::AnnounceTrace>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}

pub async fn announces(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
    Query(q): Query<AnnouncesQuery>,
) -> Result<Json<AnnouncesPage>, ApiErrorResponse> {
    // Resolve only to surface 404 when the torrent is unknown to the engine; the actual rows live
    // in SQLite and persist independently of in-memory state.
    let _id = resolve(&s, &info_hash)?;
    // Cap at 200 per page so a misbehaving client can't pull the entire history in one shot.
    let limit = q.limit.unwrap_or(50).min(200);
    let offset = q.offset.unwrap_or(0);
    let db = s.session.clone();
    let hash = info_hash.clone();
    let (items, total) =
        tokio::task::spawn_blocking(move || db.read_announces(&hash, limit, offset))
            .await
            .map_err(|e| api_error(SudoratioError::Io(e.to_string())))?
            .map_err(|e| api_error(SudoratioError::Io(e.to_string())))?;
    Ok(Json(AnnouncesPage {
        items,
        total,
        limit,
        offset,
    }))
}

pub async fn announce(
    State(s): State<Arc<AppState>>,
    Path(info_hash): Path<String>,
    Json(body): Json<AnnounceBody>,
) -> Result<Json<AnnounceOutcome>, ApiErrorResponse> {
    let id = resolve(&s, &info_hash)?;
    let out = s
        .core
        .announce_torrent_with_overrides(id, body.event, body.overrides)
        .await
        .map_err(api_error)?;
    persist_torrent(&s, &info_hash).await;
    Ok(Json(out))
}

fn parse_metainfo(bytes: &[u8]) -> anyhow::Result<MetainfoTorrent> {
    sudoratio_core::parse_metainfo(bytes).map_err(|e| anyhow::anyhow!(e))
}

/// `multipart/form-data` only. Fields:
///   `file`                — the `.torrent` payload (required)
///   `download_before_seed` — `"true"` to enable download phase (optional, default false)
pub async fn add(State(s): State<Arc<AppState>>, mut multipart: Multipart) -> Response {
    let mut file_bytes: Option<Bytes> = None;
    let mut download_before_seed = false;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(error = %e, "POST /api/v1/torrents multipart parse error");
                return StatusCode::BAD_REQUEST.into_response();
            }
        };
        match field.name() {
            Some("file") => match field.bytes().await {
                Ok(b) => file_bytes = Some(b),
                Err(e) => {
                    tracing::warn!(error = %e, "POST /api/v1/torrents file read error");
                    return StatusCode::BAD_REQUEST.into_response();
                }
            },
            Some("download_before_seed") => match field.text().await {
                Ok(s) => download_before_seed = matches!(s.as_str(), "true" | "1" | "on"),
                Err(e) => {
                    tracing::warn!(error = %e, "POST /api/v1/torrents bool field read error");
                    return StatusCode::BAD_REQUEST.into_response();
                }
            },
            _ => {}
        }
    }

    let Some(bytes) = file_bytes else {
        tracing::warn!("POST /api/v1/torrents missing `file` field");
        return StatusCode::BAD_REQUEST.into_response();
    };
    if bytes.is_empty() {
        tracing::warn!("POST /api/v1/torrents empty file");
        return StatusCode::BAD_REQUEST.into_response();
    }
    let mut meta = match parse_metainfo(&bytes) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "metainfo parse failed");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    meta.download_before_seed = download_before_seed;

    tracing::info!(
        name = %meta.name,
        info_hash = %meta.info_hash,
        primary_tracker = ?meta.trackers.tiers.iter().flatten().next(),
        size = meta.size,
        download_before_seed = meta.download_before_seed,
        "POST /api/v1/torrents"
    );
    let info_hash = meta.info_hash.clone();
    match s.core.add_torrent_metainfo(meta).await {
        Ok(_id) => {
            persist_torrent(&s, &info_hash).await;
            Json(serde_json::json!({ "info_hash": info_hash })).into_response()
        }
        Err(e) => crate::error::api_error(e).into_response(),
    }
}
