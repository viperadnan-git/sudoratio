//! CLI flags + environment overlay for [`EngineConfig`] (engine infra only).
//! Per-tracker policy lives under presets and is configured via the API.

use std::path::PathBuf;

use clap::Parser;
use sudoratio_core::EngineConfig;

#[derive(Parser, Debug)]
#[command(
    name = "sudoratio-server",
    version,
    about = "HTTP API for sudoratio (tracker announce simulation)"
)]
pub struct Args {
    /// HTTP listen address (host:port).
    #[arg(long, env = "SUDORATIO_LISTEN", default_value = "0.0.0.0:8787")]
    pub listen: String,

    /// Override the tracker `port=`. Unset = announce the bound peer-listener port.
    #[arg(long, env = "SUDORATIO_ANNOUNCE_PORT")]
    pub announce_port: Option<u16>,

    /// BT peer-listener bind address. Empty disables.
    #[arg(long, env = "SUDORATIO_PEER_LISTEN", default_value = "[::]:51413")]
    pub peer_listen: String,

    #[arg(long, env = "SUDORATIO_BANDWIDTH_TICK_MS")]
    pub bandwidth_tick_ms: Option<u64>,

    #[arg(long, env = "SUDORATIO_HTTP_TRACKER_CONNECT_TIMEOUT_SECS")]
    pub http_tracker_connect_timeout_secs: Option<u64>,

    #[arg(long, env = "SUDORATIO_HTTP_TRACKER_REQUEST_TIMEOUT_SECS")]
    pub http_tracker_request_timeout_secs: Option<u64>,

    #[arg(long, env = "SUDORATIO_HTTP_TRACKER_MAX_IDLE_PER_HOST")]
    pub http_tracker_max_idle_per_host: Option<usize>,

    #[arg(long, env = "SUDORATIO_HTTP_TRACKER_MAX_REDIRECTS")]
    pub http_tracker_max_redirects: Option<usize>,

    #[arg(long, env = "SUDORATIO_HTTP_TCP_KEEPALIVE_SECS")]
    pub http_tcp_keepalive_secs: Option<u64>,

    #[arg(long, env = "SUDORATIO_HTTP_POOL_IDLE_TIMEOUT_SECS")]
    pub http_pool_idle_timeout_secs: Option<u64>,

    #[arg(long, env = "SUDORATIO_MAX_CONCURRENT_ANNOUNCES")]
    pub max_concurrent_announces: Option<usize>,

    #[arg(
        long,
        env = "SUDORATIO_HTTP_API_CONCURRENCY",
        default_value_t = 16384usize
    )]
    pub http_api_concurrency: usize,

    #[arg(long, env = "SUDORATIO_CONFIG_DIR", default_value = ".sudoratio")]
    pub config_dir: PathBuf,

    #[arg(long, env = "SUDORATIO_PASSWORD", default_value = "sudoratio")]
    pub password: String,
}

impl Args {
    pub fn apply_to(&self, cfg: &mut EngineConfig) {
        macro_rules! set {
            ($field:ident) => {
                if let Some(v) = self.$field {
                    cfg.$field = v;
                }
            };
        }
        if let Some(p) = self.announce_port {
            cfg.announce_port = Some(p);
        }
        set!(bandwidth_tick_ms);
        set!(max_concurrent_announces);
        cfg.http_tracker.connect_timeout_secs = self
            .http_tracker_connect_timeout_secs
            .or(cfg.http_tracker.connect_timeout_secs);
        cfg.http_tracker.request_timeout_secs = self
            .http_tracker_request_timeout_secs
            .or(cfg.http_tracker.request_timeout_secs);
        cfg.http_tracker.max_idle_per_host = self
            .http_tracker_max_idle_per_host
            .or(cfg.http_tracker.max_idle_per_host);
        cfg.http_tracker.max_redirects = self
            .http_tracker_max_redirects
            .or(cfg.http_tracker.max_redirects);
        cfg.http_tracker.tcp_keepalive_secs = self
            .http_tcp_keepalive_secs
            .or(cfg.http_tracker.tcp_keepalive_secs);
        cfg.http_tracker.pool_idle_timeout_secs = self
            .http_pool_idle_timeout_secs
            .or(cfg.http_tracker.pool_idle_timeout_secs);
    }
}
