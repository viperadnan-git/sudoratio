//! Axum router build, [`AppState`] wire-up, and graceful shutdown.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::Level;

use crate::auth;
use crate::routes;
use crate::state::AppState;
use crate::static_assets;

/// `/api/v1/*` routes — gated by [`auth::require_bearer`].
fn api_router(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/config",
            get(routes::config::get_config).patch(routes::config::update_config),
        )
        .route("/config/defaults", get(routes::config::get_config_defaults))
        .route(
            "/clients",
            get(routes::profiles::list).post(routes::profiles::register),
        )
        .route(
            "/clients/{client}",
            axum::routing::delete(routes::profiles::delete),
        )
        .route("/clients/{client}/source", get(routes::profiles::source))
        .route(
            "/clients/variants/{id}/activate",
            post(routes::profiles::activate),
        )
        .route(
            "/torrents",
            get(routes::torrents::list).post(routes::torrents::add),
        )
        .route(
            "/torrents/{info_hash}",
            get(routes::torrents::get)
                .patch(routes::torrents::patch)
                .delete(routes::torrents::delete),
        )
        .route(
            "/torrents/{info_hash}/announce",
            post(routes::torrents::announce),
        )
        .route(
            "/torrents/{info_hash}/announces",
            get(routes::torrents::announces),
        )
        .route("/torrents/{info_hash}/pause", post(routes::torrents::pause))
        .route(
            "/torrents/{info_hash}/resume",
            post(routes::torrents::resume),
        )
        .route(
            "/torrents/{info_hash}/preset",
            post(routes::torrents::assign_preset),
        )
        .route(
            "/presets",
            get(routes::presets::list).post(routes::presets::create),
        )
        .route("/presets/defaults", get(routes::presets::defaults))
        .route(
            "/presets/{id}",
            get(routes::presets::get)
                .patch(routes::presets::update)
                .delete(routes::presets::delete),
        )
        .route("/stats", get(routes::stats::stats))
        .route(
            "/diagnostics/connectivity",
            post(routes::diagnostics::check_connectivity),
        )
        .layer(from_fn_with_state(state, auth::require_bearer))
}

pub fn build_router(state: Arc<AppState>, api_concurrency: usize) -> Router {
    let trace = TraceLayer::new_for_http()
        .make_span_with(tower_http::trace::DefaultMakeSpan::new().level(Level::INFO))
        .on_response(tower_http::trace::DefaultOnResponse::new().level(Level::INFO));

    Router::new()
        // `/api/v1/health` is unauthenticated and registered explicitly so the SPA fallback below
        // can never shadow it. Every other API route lives under the gated `/api/v1/*` nest.
        .route("/api/v1/health", get(routes::health::health))
        .nest("/api/v1", api_router(state.clone()))
        // SPA + static files: registered last as a fallback service, so anything that didn't
        // match an API route is served from the embedded `web/dist/client/` bundle (with an
        // `_shell.html` fallback for client-side routes).
        .fallback(static_assets::fallback)
        .layer(ConcurrencyLimitLayer::new(api_concurrency.max(1)))
        .layer(trace)
        .layer(CorsLayer::permissive())
        .with_state(state)
}

pub async fn run(
    state: Arc<AppState>,
    addr: SocketAddr,
    api_concurrency: usize,
) -> anyhow::Result<()> {
    let app = build_router(state.clone(), api_concurrency);
    tracing::info!(%addr, api_concurrency, "sudoratio-server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let shutdown_state = state.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            shutdown_state.core.shutdown().await;
        })
        .await?;
    Ok(())
}

/// Resolves on SIGINT (ctrl_c) or SIGTERM (containers / systemd).
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = %e, "ctrl_c listener failed");
        }
    };
    #[cfg(unix)]
    let sigterm = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(e) => tracing::warn!(error = %e, "SIGTERM listener failed"),
        }
    };
    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => tracing::info!("shutdown signal received (SIGINT)"),
        _ = sigterm => tracing::info!("shutdown signal received (SIGTERM)"),
    }
}
