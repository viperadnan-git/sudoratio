//! Shared [`AppState`] passed to every axum handler.

use std::path::PathBuf;
use std::sync::Arc;

use sudoratio_core::Engine;

use crate::session::Session;

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<Engine>,
    pub session: Arc<Session>,
    pub core_config_path: PathBuf,
    /// Lowercase hex of the configured password; expected `Authorization: Bearer <token>`.
    pub auth_token: Arc<str>,
}

/// Periodic write-out: only torrents whose state has changed since the last call.
pub async fn persist_all(state: &Arc<AppState>) {
    let db = state.session.clone();
    let core = state.core.clone();
    let dirty = core.take_dirty_ids();
    if dirty.is_empty() {
        return;
    }
    let bundles: Vec<_> = dirty
        .iter()
        .filter_map(|id| core.export_torrent(*id))
        .collect();
    match tokio::task::spawn_blocking(move || db.upsert_many(&bundles)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "session sqlite snapshot failed"),
        Err(e) => tracing::warn!(error = %e, "session persist task join failed"),
    }
}

/// Per-torrent upsert; called by route handlers after mutations to avoid rewriting every row.
pub async fn persist_torrent(state: &Arc<AppState>, info_hash: &str) {
    let Some(id) = state.core.torrent_id_by_info_hash(info_hash) else {
        return;
    };
    state.core.clear_dirty(id);
    let Some(t) = state.core.export_torrent(id) else {
        return;
    };
    let db = state.session.clone();
    match tokio::task::spawn_blocking(move || db.upsert_torrent(&t)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "session sqlite upsert failed"),
        Err(e) => tracing::warn!(error = %e, "session upsert task join failed"),
    }
}

pub async fn forget_torrent(state: &Arc<AppState>, info_hash: String) {
    let db = state.session.clone();
    match tokio::task::spawn_blocking(move || db.delete_torrent(&info_hash)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "session sqlite delete failed"),
        Err(e) => tracing::warn!(error = %e, "session delete task join failed"),
    }
}
