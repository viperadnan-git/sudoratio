//! Build the announce query string by substituting placeholders in the active profile's
//! `query` template (`{infohash}`, `{peerid}`, `{port}`, `{uploaded}`, `{downloaded}`, `{left}`,
//! `{numwant}`, `{event}`, `{key}`, `{ip}`, `{ipv6}`).

use std::net::IpAddr;
use std::sync::OnceLock;

use regex::Regex;

use crate::error::SudoratioError;
use crate::profile::codec::{
    compile_url_encoder_exclusion, url_encode_query_bytes, url_encode_query_component,
};
use crate::profile::ClientProfileSpec;
use crate::torrent::{AnnounceEvent, AnnounceRequestParams};

/// Inputs to [`build_announce_query`]. Grouping these into one struct keeps the function
/// signature small and gives callers a clear shape to pass through tracing / serialization layers.
pub struct AnnounceQueryInput<'a> {
    pub client: &'a ClientProfileSpec,
    pub info_hash: &'a [u8; 20],
    pub peer_id: &'a str,
    pub tracker_key: Option<&'a str>,
    pub params: AnnounceRequestParams,
    pub public_ip: Option<std::net::IpAddr>,
}

fn re_amp_dupes() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"&{2,}").expect("AMP_DUPES"))
}

/// Match `&*<word>={ip}` or `&*<word>={ipv6}` (ASCII word chars only).
fn re_ip_q() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?-u)&*[A-Za-z0-9_]+=\{ip(?:v6)?\}").expect("IP_Q_PTRN"))
}

/// Match `&*<word>={event}`.
fn re_event_q() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?-u)&*[A-Za-z0-9_]+=\{event\}").expect("EVENT_Q_PTRN"))
}

/// Match `{...}` placeholders (no nested `}` allowed inside).
fn re_placeholder() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\{[^}]*\}").expect("PLACEHOLDER_PTRN"))
}

pub fn join_announce_url(base: &str, query: &str) -> String {
    if query.is_empty() {
        return base.to_string();
    }
    let base = base.trim_end_matches('?');
    if base.contains('?') {
        format!("{}&{}", base, query)
    } else {
        format!("{}?{}", base, query)
    }
}

/// Build the `…?…&…` query suffix (no leading `?`).
pub fn build_announce_query(input: &AnnounceQueryInput<'_>) -> Result<String, SudoratioError> {
    let client = input.client;
    let info_hash = input.info_hash;
    let peer_id = input.peer_id;
    let tracker_key = input.tracker_key;
    let params = &input.params;
    let public_ip = input.public_ip;
    let event = params.event;
    let exclusion = compile_url_encoder_exclusion(&client.url_encoder.encoding_exclusion_pattern)?;
    let hex_upper = client
        .url_encoder
        .encoded_hex_case
        .eq_ignore_ascii_case("upper");
    let ih_enc = url_encode_query_bytes(info_hash, &exclusion, hex_upper);
    let peer_enc = if client.peer_id_generator.should_url_encode {
        url_encode_query_component(peer_id, &exclusion, hex_upper)
    } else {
        peer_id.to_string()
    };
    let numwant = match event {
        AnnounceEvent::Stopped => client.numwant_on_stop,
        _ => client.numwant,
    };

    // Collapse runs of `&` (e.g. `&&foo` → `&foo`) before substitution.
    let mut q = re_amp_dupes().replace_all(&client.query, "&").into_owned();

    // Substitution order: infohash, stats, port, numwant, peerid, ip strip, event, key.
    q = q.replace("{infohash}", &ih_enc);
    q = q.replace("{uploaded}", &params.uploaded.to_string());
    q = q.replace("{downloaded}", &params.downloaded.to_string());
    q = q.replace("{left}", &params.left.to_string());
    q = q.replace("{port}", &params.port.to_string());
    q = q.replace("{numwant}", &numwant.to_string());
    q = q.replace("{peerid}", &peer_enc);

    match public_ip {
        Some(IpAddr::V4(a)) if q.contains("{ip}") => {
            q = q.replace("{ip}", &a.to_string());
        }
        Some(IpAddr::V6(a)) if q.contains("{ipv6}") => {
            let enc = url_encode_query_component(&a.to_string(), &exclusion, hex_upper);
            q = q.replace("{ipv6}", &enc);
        }
        _ => {}
    }
    q = re_ip_q().replace_all(&q, "").into_owned();

    match event {
        AnnounceEvent::None => {
            q = re_event_q().replace_all(&q, "").into_owned();
        }
        AnnounceEvent::Started => {
            q = q.replace("{event}", "started");
        }
        AnnounceEvent::Stopped => {
            q = q.replace("{event}", "stopped");
        }
        AnnounceEvent::Completed => {
            q = q.replace("{event}", "completed");
        }
    }

    if q.contains("{key}") {
        let Some(k) = tracker_key else {
            return Err(SudoratioError::PlaceholderBuild(
                "query contains {key} but no keyGenerator".into(),
            ));
        };
        q = q.replace(
            "{key}",
            &url_encode_query_component(k, &exclusion, hex_upper),
        );
    }

    // Strip kv-pairs whose values weren't substituted (e.g. `event={event}` for None) leaves
    // adjacent `&&` runs and possibly a leading/trailing `&`. Re-collapse and trim.
    q = re_amp_dupes().replace_all(&q, "&").into_owned();
    while q.starts_with('&') {
        q.remove(0);
    }
    while q.ends_with('&') {
        q.pop();
    }

    if let Some(m) = re_placeholder().find(&q) {
        return Err(SudoratioError::PlaceholderBuild(format!(
            "unresolved placeholders in query: {} (at {})",
            q,
            m.as_str()
        )));
    }
    Ok(q)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::*;
    use crate::profile::schema::{
        ClientProfileSpec, PeerAlgorithmSpec, PeerIdGenerator, RefreshOnPolicy, UrlEncoderCfg,
    };

    fn client_spec(
        query: &str,
        numwant: i64,
        numwant_on_stop: i64,
        encoding_exclusion_pattern: &str,
        encoded_hex_case: &str,
        peer_should_encode: bool,
    ) -> ClientProfileSpec {
        ClientProfileSpec {
            id: "test-client".into(),
            name: None,
            version: None,
            query: query.into(),
            numwant,
            numwant_on_stop,
            request_headers: Vec::new(),
            peer_id_generator: PeerIdGenerator {
                algorithm: PeerAlgorithmSpec::Regex {
                    pattern: "-UT1000-[A-Za-z0-9]{12}".into(),
                },
                should_url_encode: peer_should_encode,
                refresh_on: RefreshOnPolicy::Always,
                refresh_every_secs: None,
            },
            url_encoder: UrlEncoderCfg {
                encoding_exclusion_pattern: encoding_exclusion_pattern.into(),
                encoded_hex_case: encoded_hex_case.into(),
            },
            key_generator: None,
        }
    }

    fn client_with_query(query: &str) -> ClientProfileSpec {
        client_spec(query, 50, 0, "[.-]", "lower", true)
    }

    fn run_query(
        client: &ClientProfileSpec,
        port: u16,
        event: AnnounceEvent,
        public_ip: Option<IpAddr>,
    ) -> Result<String, SudoratioError> {
        build_announce_query(&AnnounceQueryInput {
            client,
            info_hash: &[0u8; 20],
            peer_id: "-UT1000-abcdefghij",
            tracker_key: None,
            params: AnnounceRequestParams {
                port,
                uploaded: 0,
                downloaded: 0,
                left: 0,
                event,
            },
            public_ip,
        })
    }

    #[test]
    fn query_substitutes_public_ipv4_in_ip_placeholder() {
        let client = client_with_query(
            "info_hash={infohash}&peer_id={peerid}&port={port}&uploaded={uploaded}&downloaded={downloaded}&left={left}&numwant={numwant}&ip={ip}",
        );
        let q = run_query(
            &client,
            51413,
            AnnounceEvent::None,
            Some(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 7))),
        )
        .unwrap();
        assert!(q.contains("ip=198.51.100.7"), "q={q}");
        assert!(!q.contains("{ip}"));
    }

    #[test]
    fn collapses_repeated_ampersands_and_strips_unfilled_placeholders() {
        let client = client_spec(
            "&&uploaded={uploaded}&&&port={port}&&&&left={left}&&&&&",
            200,
            0,
            ".*",
            "lower",
            true,
        );
        let q = run_query(&client, 12345, AnnounceEvent::Started, None).unwrap();
        assert_eq!(q, "uploaded=0&port=12345&left=0");
    }

    #[test]
    fn numwant_uses_numwant_on_stop_for_stopped_event() {
        let client = client_spec("numwant={numwant}", 10, 50, ".*", "lower", true);
        let started = run_query(&client, 1, AnnounceEvent::Started, None).unwrap();
        assert_eq!(started, "numwant=10");
        let stopped = run_query(&client, 1, AnnounceEvent::Stopped, None).unwrap();
        assert_eq!(stopped, "numwant=50");
    }

    #[test]
    fn ipv4_fills_ip_placeholder_and_strips_ipv6() {
        let client = client_spec("ipv4={ip}&ipv6={ipv6}", 200, 0, ".*", "lower", true);
        let q = run_query(
            &client,
            12,
            AnnounceEvent::Started,
            Some(IpAddr::V4(Ipv4Addr::new(123, 123, 123, 123))),
        )
        .unwrap();
        assert_eq!(q, "ipv4=123.123.123.123");
    }

    #[test]
    fn ipv6_fills_ipv6_placeholder_and_strips_ip() {
        let addr = Ipv6Addr::new(
            0xfd2d, 0x7212, 0x4cd5, 0x2f14, 0xffff, 0xffff, 0xffff, 0xffff,
        );
        let client = client_spec("ipv4={ip}&ipv6={ipv6}", 200, 0, ".*", "lower", true);
        let q = run_query(&client, 12, AnnounceEvent::Started, Some(IpAddr::V6(addr))).unwrap();
        assert_eq!(q, format!("ipv6={addr}"));
    }

    #[test]
    fn event_none_yields_empty_query() {
        let client = client_spec("event={event}", 200, 0, ".*", "lower", true);
        let q = run_query(&client, 1, AnnounceEvent::None, None).unwrap();
        assert!(q.is_empty(), "q={q:?}");
    }

    #[test]
    fn query_substitutes_public_ipv6_in_ipv6_placeholder() {
        let client = client_with_query(
            "info_hash={infohash}&peer_id={peerid}&port={port}&uploaded={uploaded}&downloaded={downloaded}&left={left}&numwant={numwant}&ipv6={ipv6}",
        );
        let addr = Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1);
        let q = run_query(&client, 51413, AnnounceEvent::None, Some(IpAddr::V6(addr))).unwrap();
        assert!(q.contains("ipv6="), "q={q}");
        assert!(!q.contains("{ipv6}"));
    }
}
