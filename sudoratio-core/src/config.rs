//! [`EngineConfig`] — runtime knobs for announce behaviour and simulated seeding.

use std::time::Duration;

/// Optional overrides for the HTTP(S) tracker `reqwest` client.
///
/// Defaults match [librqbit](https://github.com/ikatson/rqbit): a plain
/// [`reqwest::Client::builder()`] with no connect or request timeout (same as rqbit’s session
/// client for tracker GETs). Set fields when you need stricter limits.
#[derive(Debug, Clone, Default)]
pub struct HttpTrackerConfig {
    /// If set, [`reqwest::ClientBuilder::connect_timeout`]. If `None`, reqwest default (none).
    pub connect_timeout_secs: Option<u64>,
    /// If set, [`reqwest::ClientBuilder::timeout`] (total per request). If `None`, none.
    pub request_timeout_secs: Option<u64>,
    /// If set, [`reqwest::ClientBuilder::pool_max_idle_per_host`]. If `None`, reqwest default.
    pub max_idle_per_host: Option<usize>,
    /// If set, [`reqwest::ClientBuilder::redirect`] with [`reqwest::redirect::Policy::limited`].
    /// If `None`, reqwest default (`limited(10)`).
    pub max_redirects: Option<usize>,
    /// If set, [`reqwest::ClientBuilder::tcp_keepalive`] for tracker connections.
    pub tcp_keepalive_secs: Option<u64>,
    /// If set, [`reqwest::ClientBuilder::pool_idle_timeout`] for the connection pool.
    pub pool_idle_timeout_secs: Option<u64>,
}

impl HttpTrackerConfig {
    /// Build a client aligned with librqbit’s tracker HTTP usage, plus any optional overrides.
    pub(crate) fn build_reqwest_client(&self) -> reqwest::Client {
        let mut b = reqwest::Client::builder().tcp_nodelay(true);
        if let Some(secs) = self.connect_timeout_secs {
            b = b.connect_timeout(Duration::from_secs(secs.max(1)));
        }
        if let Some(secs) = self.request_timeout_secs {
            b = b.timeout(Duration::from_secs(secs.max(1)));
        }
        if let Some(n) = self.max_idle_per_host {
            b = b.pool_max_idle_per_host(n.max(1));
        }
        if let Some(n) = self.max_redirects {
            b = b.redirect(reqwest::redirect::Policy::limited(n.max(1)));
        }
        if let Some(secs) = self.tcp_keepalive_secs {
            b = b.tcp_keepalive(Some(Duration::from_secs(secs.max(1))));
        }
        if let Some(secs) = self.pool_idle_timeout_secs {
            b = b.pool_idle_timeout(Some(Duration::from_secs(secs.max(1))));
        }
        b.build().expect("reqwest tracker client")
    }
}

/// Configuration for [`crate::Sudoratio`]: announce/bind knobs and seeding simulation.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Override for the tracker `port=` query parameter. `None` = announce the bound peer-listener port.
    pub announce_port: Option<u16>,
    /// Min/max simulated **per-torrent upload** rate in **decimal KB/s** (×1000 → bytes/s). Each
    /// active torrent samples an independent cap within this range on refresh.
    pub min_upload_speed: u64,
    pub max_upload_speed: u64,
    /// Min/max simulated **per-torrent download** cap (decimal KB/s, ×1000 → B/s). Each torrent
    /// samples an independent cap on refresh, same pattern as upload bounds.
    pub min_download_speed: u64,
    pub max_download_speed: u64,
    /// Max torrents running in the orchestrator at once (announce + bandwidth). Others wait in a FIFO queue.
    pub max_active_torrents: usize,
    /// `-1` disables upload-ratio stop.
    pub upload_ratio_target: f32,
    /// When `true`, auto-pause a torrent whose latest scrape reports zero
    /// leechers (`TorrentQueuePause::NoLeechers`). When `false` (default),
    /// torrents stay active regardless of swarm composition and only pause
    /// on user action, ratio-target, or removal.
    pub pause_torrent_with_zero_leechers: bool,
    /// Seconds of zero leechers before auto-pause fires; resets on any leecher.
    pub pause_torrent_with_zero_leechers_grace: u64,
    /// Bandwidth simulator tick interval (5000 ms).
    pub bandwidth_tick_ms: u64,
    /// Max additional random delay (seconds) added on top of tracker `interval`. `0` disables.
    pub max_announce_jitter: u32,
    /// Auto-pause when scrape reports fewer seeders than this. `0` disables.
    pub min_swarm_seeders_to_seed: u32,
    pub http_tracker: HttpTrackerConfig,
    /// Cap concurrent in-flight tracker HTTP announces (`0` = unlimited).
    pub max_concurrent_announces: usize,
    /// Outbound BT peer dial after each successful announce (silence-after-handshake).
    pub outbound_dial_enabled: bool,
    pub outbound_dial_max_concurrent_global: usize,
    pub outbound_dial_max_per_announce: usize,
    pub outbound_dial_allow_loopback: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            announce_port: None,
            min_upload_speed: 27,
            max_upload_speed: 183,
            min_download_speed: 800,
            max_download_speed: 1200,
            max_active_torrents: 5,
            upload_ratio_target: 3.0,
            pause_torrent_with_zero_leechers: false,
            pause_torrent_with_zero_leechers_grace: 3 * 60 * 60,
            bandwidth_tick_ms: 5000,
            max_announce_jitter: 8,
            min_swarm_seeders_to_seed: 0,
            http_tracker: HttpTrackerConfig::default(),
            max_concurrent_announces: 16,
            outbound_dial_enabled: true,
            // Sized to per_announce × max_active_torrents (12×5=60) + headroom for the 180s idle-cap.
            outbound_dial_max_concurrent_global: 64,
            // Real qBT/Transmission open 20–50 per torrent on first burst; 12 stays in distribution.
            outbound_dial_max_per_announce: 12,
            outbound_dial_allow_loopback: false,
        }
    }
}
