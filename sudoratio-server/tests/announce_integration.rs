//! Verifies HTTP announce against a mock tracker (seeders = complete − 1).

use http::header::HOST;
use sha1::{Digest, Sha1};
use sudoratio_core::{AnnounceEvent, Engine, EngineConfig};
use sudoratio_server::bundled::seed_profiles;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn benc_int(i: i64) -> Vec<u8> {
    format!("i{i}e").into_bytes()
}

fn benc_str(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut v = format!("{}:", b.len()).into_bytes();
    v.extend_from_slice(b);
    v
}

fn enc_dict(pairs: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    let mut o = vec![b'd'];
    for (k, v) in pairs {
        o.extend(k);
        o.extend(v);
    }
    o.push(b'e');
    o
}

fn torrent_bytes_with_announce(announce: &str) -> Vec<u8> {
    let length: u64 = 512;
    let data = vec![0u8; length as usize];
    let piece_hash = Sha1::digest(&data);
    let mut pieces_val = format!("{}:", piece_hash.len()).into_bytes();
    pieces_val.extend_from_slice(&piece_hash);
    let info = enc_dict(vec![
        (benc_str("piece length"), benc_int(16_384)),
        (benc_str("pieces"), pieces_val),
        (benc_str("length"), benc_int(length as i64)),
        (benc_str("name"), benc_str("announce-test")),
    ]);
    enc_dict(vec![
        (benc_str("announce"), benc_str(announce)),
        (benc_str("created by"), benc_str("sudoratio-test")),
        (benc_str("info"), info),
    ])
}

#[tokio::test]
async fn announce_parses_tracker_response() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(
            b"d12:min intervali600e8:intervali333e8:completei5e10:incompletei2ee".as_slice(),
        ))
        .mount(&mock)
        .await;

    let announce = format!("{}/announce", mock.uri());
    let bytes = torrent_bytes_with_announce(&announce);
    let mut meta = sudoratio_core::parse_metainfo(&bytes).expect("parse metainfo");
    meta.name = "announce-test".into();
    assert!(!meta.trackers.is_empty());

    let cfg = EngineConfig {
        announce_port: 19191,
        ..Default::default()
    };
    let h = Engine::new(cfg);
    seed_profiles(&h).await.expect("seed bundled profiles");
    let tid = h.add_torrent_metainfo(meta).await.unwrap();

    let out = h
        .announce_torrent(tid, AnnounceEvent::Started)
        .await
        .expect("announce");

    assert_eq!(out.announce_interval, Some(333));
    assert_eq!(out.min_interval, Some(600));
    assert_eq!(out.seeders, Some(5));
    assert_eq!(out.leechers, Some(2));

    let requests = mock.received_requests().await.expect("mock requests");
    assert!(!requests.is_empty());
    let expected_host = mock.address().to_string();
    let host = requests[0]
        .headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        host, expected_host,
        "explicit Host header matches tracker authority"
    );
    let line = format!("{} {}", requests[0].method, requests[0].url);
    assert!(
        line.contains("GET ") && line.contains("/announce?"),
        "expected GET …/announce?…, got first line fragment: {:?}",
        line.chars().take(120).collect::<String>()
    );
    assert!(line.contains("info_hash="));
    assert!(line.contains("peer_id="));
    assert!(line.contains("port=19191"));
    assert!(line.contains("uploaded=0"));
    assert!(line.contains("downloaded=0"));
    // BEP 3: a from-storage seeder (download_before_seed=false) announces `left=0`.
    assert!(line.contains("left=0"));
    assert!(line.contains("started"));
    // Deluge 2.2.0 (the bundled default) requests up to 200 peers.
    assert!(line.contains("numwant=200"));
    assert!(line.contains("key="));
}
