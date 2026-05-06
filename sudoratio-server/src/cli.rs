//! CLI flags + environment overlay for [`EngineConfig`].

use std::path::PathBuf;

use clap::Parser;
use sudoratio_core::EngineConfig;

/// Process-wide defaults for `EngineConfig`, overridable with CLI flags and environment variables.
#[derive(Parser, Debug)]
#[command(
    name = "sudoratio-server",
    version,
    about = "HTTP API for sudoratio (tracker announce simulation)"
)]
pub struct Args {
    /// HTTP listen address (host:port), e.g. `0.0.0.0:8787`.
    #[arg(long, env = "SUDORATIO_LISTEN", default_value = "0.0.0.0:8787")]
    pub listen: String,

    /// Override the tracker `port=`. Unset = announce the bound peer-listener port.
    #[arg(long, env = "SUDORATIO_ANNOUNCE_PORT")]
    pub announce_port: Option<u16>,

    /// BT peer-listener bind address. Empty disables.
    #[arg(long, env = "SUDORATIO_PEER_LISTEN", default_value = "[::]:51413")]
    pub peer_listen: String,

    #[arg(long, env = "SUDORATIO_MIN_UPLOAD_SPEED")]
    pub min_upload_speed: Option<u64>,

    #[arg(long, env = "SUDORATIO_MAX_UPLOAD_SPEED")]
    pub max_upload_speed: Option<u64>,

    #[arg(long, env = "SUDORATIO_MIN_DOWNLOAD_SPEED")]
    pub min_download_speed: Option<u64>,

    #[arg(long, env = "SUDORATIO_MAX_DOWNLOAD_SPEED")]
    pub max_download_speed: Option<u64>,

    #[arg(long, env = "SUDORATIO_MAX_ACTIVE_TORRENTS")]
    pub max_active_torrents: Option<usize>,

    #[arg(long, env = "SUDORATIO_UPLOAD_RATIO_TARGET")]
    pub upload_ratio_target: Option<f32>,

    #[arg(
        long,
        env = "SUDORATIO_PAUSE_TORRENT_WITH_ZERO_LEECHERS",
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub pause_torrent_with_zero_leechers: Option<bool>,

    #[arg(long, env = "SUDORATIO_PAUSE_TORRENT_WITH_ZERO_LEECHERS_GRACE")]
    pub pause_torrent_with_zero_leechers_grace: Option<u64>,

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

    /// Max concurrent tracker HTTP announces in the core (`0` = unlimited).
    #[arg(long, env = "SUDORATIO_MAX_CONCURRENT_ANNOUNCES")]
    pub max_concurrent_announces: Option<usize>,

    /// Max concurrent HTTP API requests being handled (Tower); additional requests wait in queue.
    #[arg(
        long,
        env = "SUDORATIO_HTTP_API_CONCURRENCY",
        default_value_t = 16384usize
    )]
    pub http_api_concurrency: usize,

    /// Data/config directory (created if missing). Holds `config.json` and `session.sqlite3`.
    #[arg(long, env = "SUDORATIO_CONFIG_DIR", default_value = ".sudoratio")]
    pub config_dir: PathBuf,

    /// Single-password auth secret. Clients hex-encode this and send it as
    /// `Authorization: Bearer <hex>` on every `/api/v1/*` request.
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
        set!(min_upload_speed);
        set!(max_upload_speed);
        set!(min_download_speed);
        set!(max_download_speed);
        if let Some(v) = self.max_active_torrents {
            cfg.max_active_torrents = v.max(1);
        }
        set!(upload_ratio_target);
        set!(pause_torrent_with_zero_leechers);
        set!(pause_torrent_with_zero_leechers_grace);
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

pub fn normalize_speed_ranges(cfg: &mut EngineConfig) {
    if cfg.max_upload_speed < cfg.min_upload_speed {
        std::mem::swap(&mut cfg.min_upload_speed, &mut cfg.max_upload_speed);
    }
    if cfg.max_download_speed < cfg.min_download_speed {
        std::mem::swap(&mut cfg.min_download_speed, &mut cfg.max_download_speed);
    }
}
