//! Resolve the public IP used for tracker `{ip}` / `{ipv6}` query placeholders.
//!
//! Picks a provider from the [`IP_PROVIDERS`] list (shuffled per attempt), GETs it, and parses
//! the first line of the body. Caches the result for ~90 minutes; falls back to localhost if no
//! provider is reachable.

use parking_lot::Mutex;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;

/// Cached address + next refresh time (~90 minutes between refetches).
#[derive(Debug)]
pub(crate) struct TrackerReportedIpCache {
    pub(crate) ip: Option<IpAddr>,
    pub(crate) refresh_after: Instant,
}

impl Default for TrackerReportedIpCache {
    fn default() -> Self {
        Self {
            ip: None,
            refresh_after: Instant::now(),
        }
    }
}

/// Public-IP echo endpoints (order shuffled per attempt).
const IP_PROVIDERS: &[&str] = &[
    "http://whatismyip.akamai.com",
    "http://ipecho.net/plain",
    "http://ip.tyk.nu/",
    "http://l2.io/ip",
    "http://ident.me/",
    "http://icanhazip.com",
    "https://api.ipify.org",
    "https://ipinfo.io/ip",
    "https://checkip.amazonaws.com",
];

/// User-Agent sent to public-IP echo providers; chosen to avoid bot rate-limiting.
const IP_FETCH_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/65.0.3325.181 Safari/537.36";

/// Try each shuffled provider until one returns a parseable IP (first line of body).
pub async fn try_fetch_public_ip(http: &reqwest::Client) -> Option<IpAddr> {
    let mut urls: Vec<&str> = IP_PROVIDERS.to_vec();
    {
        let mut rng = rand::rng();
        urls.shuffle(&mut rng);
    }
    for url in urls {
        match read_ip_from_provider(http, url).await {
            Ok(Some(ip)) => {
                tracing::info!(%url, %ip, "fetched public IP for tracker placeholders");
                return Some(ip);
            }
            Ok(None) => tracing::debug!(%url, "ip provider returned empty body"),
            Err(e) => tracing::warn!(%url, error = %e, "ip provider failed"),
        }
    }
    None
}

/// Resolve IP for `build_announce_query` when the profile `query` uses `{ip}` or `{ipv6}`.
pub(crate) async fn resolve_tracker_reported_ip(
    http: &reqwest::Client,
    cache: &Mutex<TrackerReportedIpCache>,
    query: &str,
) -> Option<IpAddr> {
    if !query.contains("{ip}") && !query.contains("{ipv6}") {
        return None;
    }
    let now = Instant::now();
    {
        let g = cache.lock();
        if g.ip.is_some() && now < g.refresh_after {
            return g.ip;
        }
    }
    let fetched = try_fetch_public_ip(http).await;
    let mut g = cache.lock();
    let now = Instant::now();
    match fetched {
        Some(ip) => {
            g.ip = Some(ip);
            g.refresh_after = now + Duration::from_secs(90 * 60);
        }
        None => {
            // No public IP: leave `g.ip` as-is (None or stale) so the query builder strips
            // `ip=` entirely. Real clients omit the parameter rather than send loopback.
            tracing::warn!(
                "failed to resolve public IP; ip= placeholder will be stripped from announce"
            );
            g.refresh_after = now + Duration::from_secs(5 * 60);
        }
    }
    g.ip
}

async fn read_ip_from_provider(
    http: &reqwest::Client,
    url: &str,
) -> Result<Option<IpAddr>, String> {
    let resp = http
        .get(url)
        .header(reqwest::header::USER_AGENT, IP_FETCH_UA)
        .timeout(Duration::from_secs(12))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    let line = text.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return Ok(None);
    }
    line.parse::<IpAddr>()
        .map(Some)
        .map_err(|_| format!("unparseable ip line: {line:?}"))
}
