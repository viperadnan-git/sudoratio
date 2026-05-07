//! `config.json` on disk: engine infra only. Per-tracker policy lives in presets.

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::config::EngineConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub announce_port: Option<u16>,
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
            bandwidth_tick_ms: self.bandwidth_tick_ms.max(1),
            max_concurrent_announces: self.max_concurrent_announces,
            http_tracker: crate::config::HttpTrackerConfig {
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

#[derive(Debug, Default, Clone, Deserialize)]
pub struct ConfigUpdate {
    pub announce_port: Option<Option<u16>>,
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
    pub fn apply(self, cfg: &mut EngineConfig) {
        macro_rules! set { ($f:ident, $dst:expr) => { if let Some(v) = self.$f { $dst = v; } } }
        set!(announce_port, cfg.announce_port);
        set!(max_concurrent_announces, cfg.max_concurrent_announces);
        set!(
            http_tracker_connect_timeout_secs,
            cfg.http_tracker.connect_timeout_secs
        );
        set!(
            http_tracker_request_timeout_secs,
            cfg.http_tracker.request_timeout_secs
        );
        set!(
            http_tracker_max_idle_per_host,
            cfg.http_tracker.max_idle_per_host
        );
        set!(http_tracker_max_redirects, cfg.http_tracker.max_redirects);
        set!(
            http_tracker_tcp_keepalive_secs,
            cfg.http_tracker.tcp_keepalive_secs
        );
        set!(
            http_tracker_pool_idle_timeout_secs,
            cfg.http_tracker.pool_idle_timeout_secs
        );
        if let Some(v) = self.bandwidth_tick_ms {
            cfg.bandwidth_tick_ms = v.max(1);
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
    let patch: ConfigUpdate = serde_json::from_str(&text)
        .with_context(|| format!("parse core config {}", path.display()))?;
    let mut cfg = EngineConfig::default();
    patch.apply(&mut cfg);
    Ok(cfg)
}
