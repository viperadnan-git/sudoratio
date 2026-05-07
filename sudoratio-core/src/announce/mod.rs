//! HTTP tracker announce: build query, GET, parse response, update torrent row and history.

pub(crate) mod headers;
pub(crate) mod identity;
pub(crate) mod public_ip;
pub(crate) mod query;
pub(crate) mod request;
pub(crate) mod response;
pub mod trace;

use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use self::headers::build_request_headers;
use self::query::{build_announce_query, join_announce_url, AnnounceQueryInput};
use self::request::send_announce_get;
use self::response::{parse_tracker_announce, trace_body_for_bencode_decode_error};
use crate::state::Engine;
use crate::torrent::{
    AnnounceEvent, AnnounceHttpHeader, AnnounceOutcome, AnnounceQueryOverrides,
    AnnounceRequestParams, AnnounceRequestTrace, AnnounceResponseTrace, AnnounceTrace, TorrentId,
    TransferPhase,
};
use crate::SudoratioError;

fn unix_millis_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn cap_chars(s: &str, max_chars: usize) -> String {
    let n = s.chars().count();
    if n <= max_chars {
        s.to_string()
    } else {
        s.chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>()
            + "..."
    }
}

fn announce_headers_record(src: &[(String, String)]) -> Vec<AnnounceHttpHeader> {
    src.iter()
        .map(|(k, v)| AnnounceHttpHeader {
            name: cap_chars(k, 128),
            value: cap_chars(v, 512),
        })
        .collect()
}

fn response_headers_record(headers: &reqwest::header::HeaderMap) -> Vec<AnnounceHttpHeader> {
    let mut out = Vec::new();
    for (name, value) in headers.iter() {
        let key = name.as_str();
        if let Ok(v) = value.to_str() {
            out.push(AnnounceHttpHeader {
                name: cap_chars(key, 128),
                value: cap_chars(v, 512),
            });
        } else {
            out.push(AnnounceHttpHeader {
                name: cap_chars(key, 128),
                value: "(non-utf8)".to_string(),
            });
        }
    }
    out
}

fn request_layer_protocol(url: &str) -> &'static str {
    let u = url.as_bytes();
    if u.len() >= 8 && u[..8].eq_ignore_ascii_case(b"https://") {
        "https"
    } else {
        "http"
    }
}

fn announce_request_trace(
    url: &str,
    headers: &[(String, String)],
    params: AnnounceRequestParams,
) -> AnnounceRequestTrace {
    AnnounceRequestTrace {
        method: "GET".to_string(),
        protocol: request_layer_protocol(url).to_string(),
        url: cap_chars(url, 8192),
        headers: announce_headers_record(headers),
        params,
    }
}

fn make_trace(
    tracker_index: usize,
    event: AnnounceEvent,
    success: bool,
    err: Option<&SudoratioError>,
    request: AnnounceRequestTrace,
    response: AnnounceResponseTrace,
) -> AnnounceTrace {
    AnnounceTrace {
        tracker_index,
        event,
        announced_at: unix_millis_now(),
        success,
        request,
        response,
        error_code: err.map(|e| e.api_code().to_string()),
        error_message: err.map(|e| e.to_string()),
    }
}

fn body_snippet_utf8(body: &[u8], max_chars: usize) -> Option<String> {
    let t: String = String::from_utf8_lossy(body)
        .chars()
        .take(max_chars)
        .collect();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

struct AnnounceInflight<'a>(&'a std::sync::atomic::AtomicUsize);

impl Drop for AnnounceInflight<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Engine {
    #[tracing::instrument(skip(self, overrides), fields(torrent_id = %torrent_id, ?event))]
    pub(crate) async fn exec_tracker_announce(
        &self,
        torrent_id: TorrentId,
        event: AnnounceEvent,
        overrides: &AnnounceQueryOverrides,
    ) -> Result<AnnounceOutcome, SudoratioError> {
        let _announce_slot = if let Some(sem) = self.announce_concurrency.as_ref() {
            Some(
                sem.clone()
                    .acquire_owned()
                    .await
                    .expect("announce concurrency semaphore open"),
            )
        } else {
            None
        };

        self.announce_inflight.fetch_add(1, Ordering::Relaxed);
        let _inflight = AnnounceInflight(&self.announce_inflight);

        let pid = self
            .active_profile
            .read()
            .await
            .clone()
            .ok_or(SudoratioError::NoActiveClientProfile)?;
        let spec = self
            .profiles
            .get(&pid)
            .ok_or_else(|| SudoratioError::UnknownClientProfile(pid.clone()))?
            .spec
            .clone();
        let client = spec.as_ref();

        let (info_hash_bytes, tracker_idx, announce_base, downloaded, left) = {
            let ent = self
                .torrents
                .get(&torrent_id)
                .ok_or(SudoratioError::TorrentNotFound)?;
            let ih = *ent
                .info_hash_bytes
                .as_ref()
                .ok_or(SudoratioError::TorrentNoMetainfo)?;
            let base = ent
                .current_tracker()
                .ok_or(SudoratioError::NoHttpTrackers)?;
            if base.is_empty() {
                return Err(SudoratioError::MissingAnnounceUrl);
            }
            let idx = ent.flat_tracker_index();
            let dl = overrides.downloaded.unwrap_or_else(|| ent.downloaded());
            let le = overrides.left.unwrap_or_else(|| ent.left());
            (ih, idx, base, dl, le)
        };

        let uploaded = overrides
            .uploaded
            .unwrap_or_else(|| self.torrent_uploaded(torrent_id));

        let bound = self
            .listening_port
            .load(std::sync::atomic::Ordering::Relaxed);
        let cfg_override = self.config.load().announce_port;
        let port = overrides.port.or(cfg_override).unwrap_or(if bound != 0 {
            bound
        } else {
            crate::DEFAULT_ANNOUNCE_PORT
        });
        let peer_id = self.resolve_announce_peer_id(client, &info_hash_bytes, event)?;
        let tracker_key = self.resolve_announce_key(client, &info_hash_bytes, event)?;
        let public_ip = self
            .resolve_tracker_reported_ip_for_query(&client.query)
            .await;
        let params = AnnounceRequestParams {
            port,
            uploaded,
            downloaded,
            left,
            event,
        };
        let q = build_announce_query(&AnnounceQueryInput {
            client,
            info_hash: &info_hash_bytes,
            peer_id: &peer_id,
            tracker_key: tracker_key.as_deref(),
            params,
            public_ip,
        })?;
        let url = join_announce_url(&announce_base, &q);
        let headers = build_request_headers(client);
        let request = announce_request_trace(&url, &headers, params);
        tracing::debug!(%url, "announce GET");

        let resp = match send_announce_get(&self.http, &url, &headers).await {
            Ok(r) => r,
            Err(e) => {
                if let Some(e_ent) = self.torrents.get(&torrent_id) {
                    e_ent.advance_tracker();
                }
                let response = AnnounceResponseTrace {
                    status: 0,
                    headers: vec![],
                    body: json!({ "failure_reason": e.to_string() }),
                };
                let trace = make_trace(tracker_idx, event, false, Some(&e), request, response);
                self.emit_announce_trace(torrent_id, trace);
                return Err(e);
            }
        };
        let status_u16 = resp.status().as_u16();
        let resp_headers = response_headers_record(resp.headers());
        if !resp.status().is_success() {
            if let Some(e_ent) = self.torrents.get(&torrent_id) {
                e_ent.advance_tracker();
            }
            let body_text = resp.text().await.unwrap_or_default();
            let err = SudoratioError::AnnounceHttp(format!(
                "HTTP {status_u16}: {}",
                body_text.chars().take(512).collect::<String>()
            ));
            let response = AnnounceResponseTrace {
                status: status_u16,
                headers: resp_headers,
                body: json!({
                    "failure_reason": body_snippet_utf8(body_text.as_bytes(), 512)
                        .unwrap_or_else(|| format!("HTTP {status_u16}"))
                }),
            };
            let trace = make_trace(tracker_idx, event, false, Some(&err), request, response);
            self.emit_announce_trace(torrent_id, trace);
            return Err(err);
        }
        let body = match resp.bytes().await {
            Ok(b) => b,
            Err(e) => {
                let err = SudoratioError::AnnounceHttp(e.to_string());
                let response = AnnounceResponseTrace {
                    status: status_u16,
                    headers: resp_headers,
                    body: json!({ "failure_reason": e.to_string() }),
                };
                let trace = make_trace(tracker_idx, event, false, Some(&err), request, response);
                self.emit_announce_trace(torrent_id, trace);
                return Err(err);
            }
        };
        match parse_tracker_announce(&body) {
            Ok(p) => {
                tracing::debug!(interval = p.interval, ?p.seeders, ?p.leechers, "announce parsed");
                if let Some(e_ent) = self.torrents.get(&torrent_id) {
                    e_ent.promote_current_tracker();
                    e_ent.store_uploaded(uploaded);
                    e_ent.with_state(|s| {
                        s.announce_count = s.announce_count.saturating_add(1);
                        s.last_successful_announce_unix_ms = unix_millis_now();
                        s.last_announce_interval_seconds = p.interval.max(1);
                        s.last_min_interval_seconds = p.min_interval.unwrap_or(0);
                        s.left = left;
                        if let Some(seeders) = p.seeders {
                            s.last_seeders = Some(seeders);
                        }
                        if let Some(leechers) = p.leechers {
                            s.last_leechers = Some(leechers);
                        }
                        // Missing leechers counts as zero; matches `should_pause_no_leechers`.
                        if p.leechers.unwrap_or(0) == 0 {
                            if s.zero_leechers_since.is_none() {
                                s.zero_leechers_since = Some(unix_millis_now());
                            }
                        } else {
                            s.zero_leechers_since = None;
                        }
                    });
                }
                let response = AnnounceResponseTrace {
                    status: status_u16,
                    headers: resp_headers,
                    body: p.trace_body.clone(),
                };
                let trace = make_trace(tracker_idx, event, true, None, request, response);
                self.emit_announce_trace(torrent_id, trace);
                Ok(AnnounceOutcome {
                    announce_interval: Some(p.interval),
                    min_interval: p.min_interval,
                    seeders: p.seeders,
                    leechers: p.leechers,
                    peers: p.peers,
                })
            }
            Err(SudoratioError::TrackerFailure(msg)) => {
                if let Some(e_ent) = self.torrents.get(&torrent_id) {
                    e_ent.advance_tracker();
                }
                let err = SudoratioError::TrackerFailure(msg.clone());
                let response = AnnounceResponseTrace {
                    status: status_u16,
                    headers: resp_headers,
                    body: json!({ "failure_reason": msg }),
                };
                let trace = make_trace(tracker_idx, event, false, Some(&err), request, response);
                self.emit_announce_trace(torrent_id, trace);
                Err(err)
            }
            Err(e) => {
                if let Some(e_ent) = self.torrents.get(&torrent_id) {
                    e_ent.advance_tracker();
                }
                let body_json = trace_body_for_bencode_decode_error(&e);
                let response = AnnounceResponseTrace {
                    status: status_u16,
                    headers: resp_headers,
                    body: body_json,
                };
                let trace = make_trace(tracker_idx, event, false, Some(&e), request, response);
                self.emit_announce_trace(torrent_id, trace);
                Err(e)
            }
        }
    }

    pub(crate) fn reset_consecutive_fails(&self, tid: TorrentId) {
        if let Some(e) = self.torrents.get(&tid) {
            e.reset_consecutive_fails();
        }
    }

    pub(crate) fn bump_consecutive_fails(&self, tid: TorrentId) -> u32 {
        if let Some(e) = self.torrents.get(&tid) {
            return e.bump_consecutive_fails();
        }
        0
    }

    /// Seconds to wait before the next announce retry: `max(interval, min_interval, 1)` from last success.
    pub(crate) fn last_interval_secs(&self, tid: TorrentId) -> u32 {
        self.torrents
            .get(&tid)
            .map(|e| {
                e.last_announce_interval_seconds()
                    .max(e.last_min_interval_seconds())
                    .max(1)
            })
            .unwrap_or(5)
    }

    pub(crate) fn has_reached_upload_ratio_limit(&self, tid: TorrentId) -> bool {
        let Some(e) = self.torrents.get(&tid) else {
            return false;
        };
        let target = e.policy_snapshot().upload_ratio_target;
        if target < 0.0 {
            return false;
        }
        if e.download_first && e.transfer_phase() != TransferPhase::Seeding {
            return false;
        }
        let size = e.size.max(1);
        let up = e.uploaded() as f64;
        up >= (target as f64) * (size as f64)
    }
}
