//! HTTP API for **sudoratio**.

use anyhow::Context;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use sudoratio_core::{config_io, Engine, EngineConfig};

use sudoratio_server::auth;
use sudoratio_server::cli::Args;
use sudoratio_server::server;
use sudoratio_server::state::AppState;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "sudoratio_server=info,sudoratio_core=info,tower_http=info".into()
            }),
        )
        .init();

    let args = Args::parse();
    std::fs::create_dir_all(&args.config_dir)
        .with_context(|| format!("create config dir {}", args.config_dir.display()))?;
    let core_config_path = args.config_dir.join("config.json");

    let mut cfg = if core_config_path.exists() {
        config_io::load(&core_config_path)?
    } else {
        let mut c = EngineConfig::default();
        args.apply_to(&mut c);
        config_io::save(&core_config_path, &c)?;
        c
    };
    args.apply_to(&mut cfg);

    let addr: SocketAddr = args
        .listen
        .parse()
        .map_err(|e| anyhow::anyhow!("--listen invalid address: {e}"))?;

    let core = Engine::new(cfg, Some(args.config_dir.clone())).await?;
    if !args.peer_listen.is_empty() {
        match args.peer_listen.parse::<SocketAddr>() {
            Ok(peer_addr) => match core.start_peer_listener(peer_addr).await {
                Ok(port) => tracing::info!(peer_port = port, "BT peer listener up"),
                Err(e) => {
                    tracing::warn!(error = %e, "BT peer listener bind failed; inbound peers unreachable")
                }
            },
            Err(e) => tracing::warn!(error = %e, "--peer-listen invalid; skipping listener"),
        }
    }

    let auth_token: Arc<str> = Arc::from(auth::token_for(&args.password).as_str());
    tracing::info!(token_len = auth_token.len(), "bearer auth enabled");

    let state = Arc::new(AppState {
        core: core.clone(),
        auth_token,
    });

    state
        .core
        .finish_restore(Engine::RESTORE_STARTED_JITTER_SECS)
        .await;

    server::run(state, addr, args.http_api_concurrency).await
}
