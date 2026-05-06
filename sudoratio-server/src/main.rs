//! HTTP API for **sudoratio** — thin wrapper around `sudoratio-core` for web/desktop clients.

use anyhow::Context;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use sudoratio_core::{Engine, EngineConfig};

use sudoratio_server::auth;
use sudoratio_server::cli::{self, Args};
use sudoratio_server::session::Session;
use sudoratio_server::state::{persist_all, AppState};
use sudoratio_server::{bundled, config_io, profile_store, server};

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
    let session_path = args.config_dir.join("session.sqlite3");

    let mut cfg = if core_config_path.exists() {
        config_io::load(&core_config_path)?
    } else {
        let mut c = EngineConfig::default();
        args.apply_to(&mut c);
        cli::normalize_speed_ranges(&mut c);
        config_io::save(&core_config_path, &c)?;
        c
    };
    cli::normalize_speed_ranges(&mut cfg);

    let addr: SocketAddr = args
        .listen
        .parse()
        .map_err(|e| anyhow::anyhow!("--listen invalid address: {e}"))?;

    let core = Engine::new(cfg);
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
    if let Err(e) = bundled::seed_profiles(&core).await {
        tracing::warn!(error = %e, "bundled client profiles not fully loaded");
    }
    // User profiles registered via the API are persisted alongside config.json. Loading them
    // *after* bundled profiles means a user file with the same `id` shadows the bundled copy.
    match profile_store::load_all(&core_config_path, &core).await {
        Ok(n) if n > 0 => tracing::info!(count = n, "loaded user client profiles from config dir"),
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "failed to scan user client profiles"),
    }

    let path_log = session_path.display().to_string();
    let db = tokio::task::spawn_blocking(move || Session::open(session_path.as_path()))
        .await
        .context("session sqlite open join")?
        .context("session sqlite open")?;
    let session = Arc::new(db);
    let db_read = session.clone();
    let bundles = tokio::task::spawn_blocking(move || db_read.read_all())
        .await
        .context("session sqlite read_all join")?
        .context("session sqlite read_all")?;
    tracing::info!(path = %path_log, count = bundles.len(), "session sqlite restored");
    for b in bundles {
        if let Err(e) = core.restore_torrent(b).await {
            tracing::warn!(error = %e, "restore_torrent skipped");
        }
    }

    let auth_token: Arc<str> = Arc::from(auth::token_for(&args.password).as_str());
    tracing::info!(token_len = auth_token.len(), "bearer auth enabled");

    // Subscribe BEFORE finish_restore so the immediate (jitter=0) Started traces are captured
    // by the persistence pipeline instead of being dropped into a None sink.
    let mut announce_rx = core.subscribe_announces();
    let trace_session = session.clone();
    let trace_engine = core.clone();
    tokio::spawn(async move {
        while let Some((tid, trace)) = announce_rx.recv().await {
            let Some(t) = trace_engine.export_torrent(tid) else {
                continue;
            };
            let Some(info_hash) = t.info_hash.clone() else {
                continue;
            };
            trace_engine.clear_dirty(tid);
            let db = trace_session.clone();
            let res = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                db.upsert_torrent(&t)?;
                db.append_announce(&info_hash, &trace)?;
                Ok(())
            })
            .await;
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => tracing::warn!(error = %e, "post-announce persist failed"),
                Err(e) => tracing::warn!(error = %e, "post-announce persist join failed"),
            }
        }
    });

    let state = Arc::new(AppState {
        core,
        session,
        core_config_path,
        auth_token,
    });

    state
        .core
        .finish_restore(Engine::RESTORE_STARTED_JITTER_SECS)
        .await;

    // Coalescing persist worker: wakes on engine-emit (auto-pause, phase flip) OR a 10 s
    // backstop tick. Drains all dirty rows in one pass so bursts collapse to a single write.
    let bg = state.clone();
    let bg_notify = bg.core.state_change_notify.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = tick.tick() => {}
                _ = bg_notify.notified() => {}
            }
            persist_all(&bg).await;
        }
    });

    server::run(state, addr, args.http_api_concurrency).await
}
