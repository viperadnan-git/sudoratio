//! `POST /api/v1/diagnostics/connectivity` — probe inbound port reachability via `ifconfig.co`, v4 + v6.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

const PROBE_HOST: &str = "ifconfig.co";
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const VIA: &str = "ifconfig.co";

#[derive(Debug, Clone, Serialize)]
pub struct FamilyResult {
    pub reachable: bool,
    pub public_ip: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectivityResponse {
    pub port: u16,
    pub checked_at_ms: u64,
    pub ipv4: FamilyResult,
    pub ipv6: FamilyResult,
    pub via: &'static str,
}

#[derive(Debug, Default, Deserialize)]
pub struct CheckBody {
    pub port: Option<u16>,
}

#[derive(Deserialize)]
struct IfconfigPortResp {
    ip: String,
    reachable: bool,
}

pub async fn check_connectivity(
    State(s): State<Arc<AppState>>,
    body: Option<Json<CheckBody>>,
) -> Json<ConnectivityResponse> {
    let req_port = body.and_then(|Json(b)| b.port);
    let port = req_port.unwrap_or_else(|| s.core.resolved_announce_port());

    let (v4_addr, v6_addr) = resolve_families().await;

    let (ipv4, ipv6) = tokio::join!(
        probe_family("IPv4", v4_addr, port),
        probe_family("IPv6", v6_addr, port),
    );

    Json(ConnectivityResponse {
        port,
        checked_at_ms: unix_ms(),
        ipv4,
        ipv6,
        via: VIA,
    })
}

async fn resolve_families() -> (Option<SocketAddr>, Option<SocketAddr>) {
    let addrs = match tokio::net::lookup_host((PROBE_HOST, 443)).await {
        Ok(it) => it,
        Err(_) => return (None, None),
    };
    let mut v4 = None;
    let mut v6 = None;
    for a in addrs {
        match a.ip() {
            IpAddr::V4(_) if v4.is_none() => v4 = Some(a),
            IpAddr::V6(_) if v6.is_none() => v6 = Some(a),
            _ => {}
        }
    }
    (v4, v6)
}

async fn probe_family(label: &str, addr: Option<SocketAddr>, port: u16) -> FamilyResult {
    let Some(addr) = addr else {
        return error_result(format!("no {label} address for {PROBE_HOST}"));
    };
    let client = match reqwest::Client::builder().resolve(PROBE_HOST, addr).build() {
        Ok(c) => c,
        Err(e) => return error_result(format!("client build: {e}")),
    };
    let url = format!("https://{PROBE_HOST}/port/{port}");
    let fut = async {
        let resp = client
            .get(&url)
            .header("accept", "application/json")
            .send()
            .await?;
        let status = resp.status();
        let body = resp.json::<IfconfigPortResp>().await?;
        Ok::<_, reqwest::Error>((status, body))
    };
    match tokio::time::timeout(PROBE_TIMEOUT, fut).await {
        Ok(Ok((status, body))) if status.is_success() => FamilyResult {
            reachable: body.reachable,
            public_ip: Some(body.ip),
            error: None,
        },
        Ok(Ok((status, _))) => error_result(format!("upstream HTTP {status}")),
        Ok(Err(e)) => error_result(classify_reqwest_error(&e, label)),
        Err(_) => error_result("timeout".into()),
    }
}

fn classify_reqwest_error(e: &reqwest::Error, label: &str) -> String {
    if e.is_connect() {
        format!("no route ({label})")
    } else if e.is_timeout() {
        "timeout".into()
    } else if e.is_decode() {
        format!("decode: {e}")
    } else {
        format!("network: {e}")
    }
}

fn error_result(msg: String) -> FamilyResult {
    FamilyResult {
        reachable: false,
        public_ip: None,
        error: Some(msg),
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
