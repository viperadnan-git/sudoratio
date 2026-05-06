//! Bencode announce dictionary (`interval`, `complete`, `incomplete`, `failure reason`, `peers`).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde_bencode::value::Value as BValue;
use serde_json::{json, Map, Value as JValue};

use crate::error::SudoratioError;

/// Successful parse: swarm fields used by the engine plus the full bencode dict converted to JSON
/// (preserved as-is in announce traces, including tracker-specific extensions).
#[derive(Debug)]
pub(crate) struct ParsedAnnounceOk {
    pub interval: u32,
    pub min_interval: Option<u32>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    pub peers: Vec<SocketAddr>,
    pub trace_body: JValue,
}

/// Lossless bencode → JSON: utf-8 byte strings become JSON strings, non-utf8 bytes become
/// `{"_b64": "<base64>"}`. Dict keys are utf-8-decoded with `from_utf8_lossy`.
fn bencode_to_json(v: BValue) -> JValue {
    match v {
        BValue::Int(i) => json!(i),
        BValue::Bytes(b) => match std::str::from_utf8(&b) {
            Ok(s) => JValue::String(s.to_string()),
            Err(_) => json!({ "_b64": B64.encode(&b) }),
        },
        BValue::List(items) => JValue::Array(items.into_iter().map(bencode_to_json).collect()),
        BValue::Dict(map) => {
            let mut out = Map::new();
            for (k, val) in map {
                out.insert(
                    String::from_utf8_lossy(&k).into_owned(),
                    bencode_to_json(val),
                );
            }
            JValue::Object(out)
        }
    }
}

fn dict_get<'a>(map: &'a HashMap<Vec<u8>, BValue>, key: &str) -> Option<&'a BValue> {
    map.get(key.as_bytes())
}

fn as_int(v: &BValue) -> Option<i64> {
    if let BValue::Int(i) = v {
        Some(*i)
    } else {
        None
    }
}

/// BEP-23 compact `peers`: 6-byte chunks (4 IPv4 + 2 port BE).
fn decode_compact_v4(bytes: &[u8]) -> Vec<SocketAddr> {
    bytes
        .chunks_exact(6)
        .map(|c| {
            let ip = Ipv4Addr::new(c[0], c[1], c[2], c[3]);
            let port = u16::from_be_bytes([c[4], c[5]]);
            SocketAddr::new(IpAddr::V4(ip), port)
        })
        .collect()
}

/// BEP-7 compact `peers6`: 18-byte chunks (16 IPv6 + 2 port BE).
fn decode_compact_v6(bytes: &[u8]) -> Vec<SocketAddr> {
    bytes
        .chunks_exact(18)
        .map(|c| {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&c[..16]);
            let port = u16::from_be_bytes([c[16], c[17]]);
            SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port)
        })
        .collect()
}

/// BEP-3 dict `peers`: list of `{ip, port}`.
fn decode_dict_peers(items: &[BValue]) -> Vec<SocketAddr> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let BValue::Dict(d) = item else { continue };
        let Some(BValue::Bytes(ip_bytes)) = dict_get(d, "ip") else {
            continue;
        };
        let Some(BValue::Int(port)) = dict_get(d, "port") else {
            continue;
        };
        if *port <= 0 || *port > u16::MAX as i64 {
            continue;
        }
        let port = *port as u16;
        let ip_str = String::from_utf8_lossy(ip_bytes);
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            out.push(SocketAddr::new(ip, port));
        }
    }
    out
}

fn extract_peers(map: &HashMap<Vec<u8>, BValue>) -> Vec<SocketAddr> {
    let mut peers = Vec::new();
    match dict_get(map, "peers") {
        Some(BValue::Bytes(b)) => peers.extend(decode_compact_v4(b)),
        Some(BValue::List(items)) => peers.extend(decode_dict_peers(items)),
        _ => {}
    }
    if let Some(BValue::Bytes(b)) = dict_get(map, "peers6") {
        peers.extend(decode_compact_v6(b));
    }
    peers
}

/// Parse tracker HTTP body once: extract swarm fields used by the engine and capture the full
/// bencode dict in `trace_body` for API consumers (no fields elided).
pub(crate) fn parse_tracker_announce(body: &[u8]) -> Result<ParsedAnnounceOk, SudoratioError> {
    let v: BValue = serde_bencode::from_bytes(body)
        .map_err(|e| SudoratioError::TrackerBencode(e.to_string()))?;
    let BValue::Dict(map) = v else {
        return Err(SudoratioError::TrackerBencode(
            "announce response not a dict".into(),
        ));
    };
    if let Some(BValue::Bytes(b)) = dict_get(&map, "failure reason") {
        return Err(SudoratioError::TrackerFailure(
            String::from_utf8_lossy(b).into_owned(),
        ));
    }

    let interval_i = dict_get(&map, "interval")
        .and_then(as_int)
        .ok_or_else(|| SudoratioError::TrackerBencode("missing interval".into()))?;
    if interval_i < 0 {
        return Err(SudoratioError::TrackerBencode("negative interval".into()));
    }
    let interval = interval_i as u32;
    let min_interval = dict_get(&map, "min interval")
        .and_then(as_int)
        .and_then(|i| if i < 0 { None } else { Some(i as u32) });
    let seeders = dict_get(&map, "complete")
        .and_then(as_int)
        .map(|c| c.max(0) as u32);
    let leechers = dict_get(&map, "incomplete")
        .and_then(as_int)
        .map(|i| i.max(0) as u32);
    let peers = extract_peers(&map);

    let trace_body = bencode_to_json(BValue::Dict(map));

    Ok(ParsedAnnounceOk {
        interval,
        min_interval,
        seeders,
        leechers,
        peers,
        trace_body,
    })
}

/// Best-effort trace body when bdecode fails (HTTP 200 garbage, truncated payload, etc.).
pub(crate) fn trace_body_for_bencode_decode_error(err: &SudoratioError) -> serde_json::Value {
    json!({ "failure_reason": err.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_body_errors_cleanly() {
        let err = parse_tracker_announce(b"d8:inter").unwrap_err();
        assert!(matches!(err, SudoratioError::TrackerBencode(_)));
    }

    #[test]
    fn parses_min_interval() {
        let body = b"d8:intervali1800e12:min intervali300e8:completei5e10:incompletei2ee";
        let p = parse_tracker_announce(body).unwrap();
        assert_eq!(p.interval, 1800);
        assert_eq!(p.min_interval, Some(300));
        assert_eq!(p.seeders, Some(5));
        assert_eq!(p.leechers, Some(2));
        assert!(p.peers.is_empty());
    }

    #[test]
    fn parses_compact_v4_peers() {
        // Two peers: 192.168.1.1:6881, 10.0.0.5:51413
        // ports BE: 6881 = 0x1AE1, 51413 = 0xC8D5
        let mut body: Vec<u8> = b"d8:intervali1800e5:peers12:".to_vec();
        body.extend_from_slice(&[192, 168, 1, 1, 0x1A, 0xE1]);
        body.extend_from_slice(&[10, 0, 0, 5, 0xC8, 0xD5]);
        body.push(b'e');
        let p = parse_tracker_announce(&body).unwrap();
        assert_eq!(p.peers.len(), 2);
        assert_eq!(p.peers[0].to_string(), "192.168.1.1:6881");
        assert_eq!(p.peers[1].to_string(), "10.0.0.5:51413");
    }

    #[test]
    fn parses_dict_peers() {
        // d8:intervali1800e5:peersld2:ip9:127.0.0.14:porti6881eeee
        let body = b"d8:intervali1800e5:peersld2:ip9:127.0.0.14:porti6881eeee";
        let p = parse_tracker_announce(body).unwrap();
        assert_eq!(p.peers.len(), 1);
        assert_eq!(p.peers[0].to_string(), "127.0.0.1:6881");
    }

    #[test]
    fn parses_compact_v6_peers() {
        // peers6: one IPv6 peer ::1 port 6881
        let mut body: Vec<u8> = b"d8:intervali1800e6:peers618:".to_vec();
        body.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        body.extend_from_slice(&[0x1A, 0xE1]);
        body.push(b'e');
        let p = parse_tracker_announce(&body).unwrap();
        assert_eq!(p.peers.len(), 1);
        assert_eq!(p.peers[0].to_string(), "[::1]:6881");
    }

    #[test]
    fn ignores_garbage_compact_remainder() {
        // 13 bytes (not divisible by 6) — the last partial chunk is dropped silently.
        let mut body: Vec<u8> = b"d8:intervali1800e5:peers13:".to_vec();
        body.extend_from_slice(&[192, 168, 1, 1, 0x1A, 0xE1]);
        body.extend_from_slice(&[10, 0, 0, 5, 0xC8, 0xD5]);
        body.push(0xff); // dangling
        body.push(b'e');
        let p = parse_tracker_announce(&body).unwrap();
        assert_eq!(p.peers.len(), 2);
    }
}
