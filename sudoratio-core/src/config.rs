//! [`EngineConfig`] — engine-level infra. Per-tracker policy lives in [`crate::preset`].

use std::time::Duration;

#[derive(Debug, Clone, Default)]
pub struct HttpTrackerConfig {
    pub connect_timeout_secs: Option<u64>,
    pub request_timeout_secs: Option<u64>,
    pub max_idle_per_host: Option<usize>,
    pub max_redirects: Option<usize>,
    pub tcp_keepalive_secs: Option<u64>,
    pub pool_idle_timeout_secs: Option<u64>,
}

impl HttpTrackerConfig {
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

/// Engine infra knobs: HTTP/peer-listener config and resource caps. Bandwidth/lifecycle policy
/// lives per-preset in [`crate::preset::PresetPolicy`].
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub announce_port: Option<u16>,
    pub bandwidth_tick_ms: u64,
    pub http_tracker: HttpTrackerConfig,
    pub max_concurrent_announces: usize,
    pub outbound_dial_enabled: bool,
    pub outbound_dial_max_concurrent_global: usize,
    pub outbound_dial_max_per_announce: usize,
    pub outbound_dial_allow_loopback: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            announce_port: None,
            bandwidth_tick_ms: 5000,
            http_tracker: HttpTrackerConfig::default(),
            max_concurrent_announces: 16,
            outbound_dial_enabled: true,
            outbound_dial_max_concurrent_global: 64,
            outbound_dial_max_per_announce: 12,
            outbound_dial_allow_loopback: false,
        }
    }
}
