//! Wire format for `GET/PATCH /api/v1/config` and `config.json` on disk.

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use sudoratio_core::EngineConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub announce_port: Option<u16>,
    pub min_upload_speed: u64,
    pub max_upload_speed: u64,
    pub min_download_speed: u64,
    pub max_download_speed: u64,
    pub max_active_torrents: usize,
    pub upload_ratio_target: f32,
    pub pause_torrent_with_zero_leechers: bool,
    pub pause_torrent_with_zero_leechers_grace: u64,
    pub bandwidth_tick_ms: u64,
    pub max_concurrent_announces: usize,
    pub http_tracker_connect_timeout_secs: Option<u64>,
    pub http_tracker_request_timeout_secs: Option<u64>,
    pub http_tracker_max_idle_per_host: Option<usize>,
    pub http_tracker_max_redirects: Option<usize>,
    pub http_tracker_tcp_keepalive_secs: Option<u64>,
    pub http_tracker_pool_idle_timeout_secs: Option<u64>,
}

impl From<&EngineConfig> for ConfigResponse {
    fn from(c: &EngineConfig) -> Self {
        Self {
            announce_port: c.announce_port,
            min_upload_speed: c.min_upload_speed,
            max_upload_speed: c.max_upload_speed,
            min_download_speed: c.min_download_speed,
            max_download_speed: c.max_download_speed,
            max_active_torrents: c.max_active_torrents,
            upload_ratio_target: c.upload_ratio_target,
            pause_torrent_with_zero_leechers: c.pause_torrent_with_zero_leechers,
            pause_torrent_with_zero_leechers_grace: c.pause_torrent_with_zero_leechers_grace,
            bandwidth_tick_ms: c.bandwidth_tick_ms,
            max_concurrent_announces: c.max_concurrent_announces,
            http_tracker_connect_timeout_secs: c.http_tracker.connect_timeout_secs,
            http_tracker_request_timeout_secs: c.http_tracker.request_timeout_secs,
            http_tracker_max_idle_per_host: c.http_tracker.max_idle_per_host,
            http_tracker_max_redirects: c.http_tracker.max_redirects,
            http_tracker_tcp_keepalive_secs: c.http_tracker.tcp_keepalive_secs,
            http_tracker_pool_idle_timeout_secs: c.http_tracker.pool_idle_timeout_secs,
        }
    }
}

impl ConfigResponse {
    pub fn into_core(self) -> EngineConfig {
        EngineConfig {
            announce_port: self.announce_port,
            min_upload_speed: self.min_upload_speed,
            max_upload_speed: self.max_upload_speed,
            min_download_speed: self.min_download_speed,
            max_download_speed: self.max_download_speed,
            max_active_torrents: self.max_active_torrents.max(1),
            upload_ratio_target: self.upload_ratio_target,
            pause_torrent_with_zero_leechers: self.pause_torrent_with_zero_leechers,
            pause_torrent_with_zero_leechers_grace: self.pause_torrent_with_zero_leechers_grace,
            bandwidth_tick_ms: self.bandwidth_tick_ms.max(1),
            max_concurrent_announces: self.max_concurrent_announces,
            http_tracker: sudoratio_core::HttpTrackerConfig {
                connect_timeout_secs: self.http_tracker_connect_timeout_secs,
                request_timeout_secs: self.http_tracker_request_timeout_secs,
                max_idle_per_host: self.http_tracker_max_idle_per_host,
                max_redirects: self.http_tracker_max_redirects,
                tcp_keepalive_secs: self.http_tracker_tcp_keepalive_secs,
                pool_idle_timeout_secs: self.http_tracker_pool_idle_timeout_secs,
            },
            ..EngineConfig::default()
        }
    }
}

/// Partial PATCH body: every field is optional and merged onto the live config.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ConfigUpdate {
    pub announce_port: Option<Option<u16>>,
    pub min_upload_speed: Option<u64>,
    pub max_upload_speed: Option<u64>,
    pub min_download_speed: Option<u64>,
    pub max_download_speed: Option<u64>,
    pub max_active_torrents: Option<usize>,
    pub upload_ratio_target: Option<f32>,
    pub pause_torrent_with_zero_leechers: Option<bool>,
    pub pause_torrent_with_zero_leechers_grace: Option<u64>,
    pub bandwidth_tick_ms: Option<u64>,
    pub max_concurrent_announces: Option<usize>,
    pub http_tracker_connect_timeout_secs: Option<Option<u64>>,
    pub http_tracker_request_timeout_secs: Option<Option<u64>>,
    pub http_tracker_max_idle_per_host: Option<Option<usize>>,
    pub http_tracker_max_redirects: Option<Option<usize>>,
    pub http_tracker_tcp_keepalive_secs: Option<Option<u64>>,
    pub http_tracker_pool_idle_timeout_secs: Option<Option<u64>>,
}

impl ConfigUpdate {
    /// Merge `self` onto `cfg`, in place. Each `Some(x)` overwrites; `None` leaves the field
    /// unchanged. For nullable HTTP-tracker fields, the outer Option distinguishes "not present"
    /// (`None`) from "set to null" (`Some(None)`).
    pub fn apply(self, cfg: &mut EngineConfig) {
        if let Some(v) = self.announce_port {
            cfg.announce_port = v;
        }
        if let Some(v) = self.min_upload_speed {
            cfg.min_upload_speed = v;
        }
        if let Some(v) = self.max_upload_speed {
            cfg.max_upload_speed = v;
        }
        if let Some(v) = self.min_download_speed {
            cfg.min_download_speed = v;
        }
        if let Some(v) = self.max_download_speed {
            cfg.max_download_speed = v;
        }
        if let Some(v) = self.max_active_torrents {
            cfg.max_active_torrents = v.max(1);
        }
        if let Some(v) = self.upload_ratio_target {
            cfg.upload_ratio_target = v;
        }
        if let Some(v) = self.pause_torrent_with_zero_leechers {
            cfg.pause_torrent_with_zero_leechers = v;
        }
        if let Some(v) = self.pause_torrent_with_zero_leechers_grace {
            cfg.pause_torrent_with_zero_leechers_grace = v;
        }
        if let Some(v) = self.bandwidth_tick_ms {
            cfg.bandwidth_tick_ms = v.max(1);
        }
        if let Some(v) = self.max_concurrent_announces {
            cfg.max_concurrent_announces = v;
        }
        if let Some(v) = self.http_tracker_connect_timeout_secs {
            cfg.http_tracker.connect_timeout_secs = v;
        }
        if let Some(v) = self.http_tracker_request_timeout_secs {
            cfg.http_tracker.request_timeout_secs = v;
        }
        if let Some(v) = self.http_tracker_max_idle_per_host {
            cfg.http_tracker.max_idle_per_host = v;
        }
        if let Some(v) = self.http_tracker_max_redirects {
            cfg.http_tracker.max_redirects = v;
        }
        if let Some(v) = self.http_tracker_tcp_keepalive_secs {
            cfg.http_tracker.tcp_keepalive_secs = v;
        }
        if let Some(v) = self.http_tracker_pool_idle_timeout_secs {
            cfg.http_tracker.pool_idle_timeout_secs = v;
        }
        if cfg.max_upload_speed < cfg.min_upload_speed {
            std::mem::swap(&mut cfg.min_upload_speed, &mut cfg.max_upload_speed);
        }
        if cfg.max_download_speed < cfg.min_download_speed {
            std::mem::swap(&mut cfg.min_download_speed, &mut cfg.max_download_speed);
        }
    }
}

pub fn save(path: &Path, cfg: &EngineConfig) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(&ConfigResponse::from(cfg))
        .context("serialize core config")?;
    std::fs::write(path, text).with_context(|| format!("write core config {}", path.display()))?;
    Ok(())
}

pub fn load(path: &Path) -> anyhow::Result<EngineConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read core config {}", path.display()))?;
    let body: ConfigResponse = serde_json::from_str(&text)
        .with_context(|| format!("parse core config {}", path.display()))?;
    Ok(body.into_core())
}
