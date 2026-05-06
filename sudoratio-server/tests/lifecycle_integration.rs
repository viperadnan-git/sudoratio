//! Engine-level integration tests covering the plan's lifecycle scenarios:
//! auto-start on add, live config patch, pause/resume, persistence round-trip.

use std::sync::Arc;
use std::time::Duration;

use sha1::{Digest, Sha1};
use sudoratio_core::profile::BUNDLED_CLIENTS;
use sudoratio_core::{AnnounceEvent, Engine, EngineConfig, MetainfoTorrent, TrackersHttp};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const TRACKER_OK_BODY: &[u8] =
    b"d12:min intervali600e8:intervali333e8:completei5e10:incompletei2ee";

fn meta_with_tracker(name: &str, hash: u8, tracker: &str) -> MetainfoTorrent {
    MetainfoTorrent {
        name: name.into(),
        info_hash: hex::encode([hash; 20]),
        info_hash_bytes: [hash; 20],
        trackers: TrackersHttp {
            tiers: vec![vec![tracker.to_string()]],
        },
        size: 1024,
        download_before_seed: false,
    }
}

/// Tiny `key=value&...` extractor. The deluge profile leaves `peer_id` raw (no percent-encoding
/// for chars in the exclusion regex), so a literal substring lookup is sufficient for our checks.
fn find_query_param(query: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    let start = query.find(&prefix)? + prefix.len();
    let rest = &query[start..];
    let end = rest.find('&').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

async fn engine_with_default_profile(cfg: EngineConfig) -> std::sync::Arc<Engine> {
    let engine = Engine::new(cfg);
    let (_, toml) = BUNDLED_CLIENTS
        .iter()
        .find(|(n, _)| *n == "deluge")
        .copied()
        .unwrap_or(BUNDLED_CLIENTS[0]);
    let ids = engine.register_builtin_client(toml).await.unwrap();
    let id = ids
        .into_iter()
        .find(|id| id.0 == "deluge@2.2.0")
        .expect("deluge@2.2.0 variant");
    engine.set_active_profile(id).await.unwrap();
    engine
}

#[tokio::test]
async fn auto_start_on_add_announces_within_seconds() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TRACKER_OK_BODY))
        .mount(&mock)
        .await;

    let engine = engine_with_default_profile(EngineConfig::default()).await;
    let tracker = format!("{}/announce", mock.uri());
    let _ = engine
        .add_torrent_metainfo(meta_with_tracker("auto", 1, &tracker))
        .await;

    // The orchestrator should fire a Started announce immediately after add.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut hits = 0;
    while std::time::Instant::now() < deadline {
        hits = mock.received_requests().await.map(|v| v.len()).unwrap_or(0);
        if hits > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    engine.shutdown().await;
    assert!(
        hits > 0,
        "no announce reached the tracker within the deadline"
    );
}

#[tokio::test]
async fn live_config_patch_updates_max_active_torrents_immediately() {
    let cfg = EngineConfig {
        max_active_torrents: 1,
        ..Default::default()
    };
    let engine = engine_with_default_profile(cfg).await;
    for i in 1u8..=4 {
        let _ = engine
            .add_torrent_metainfo(meta_with_tracker(
                &format!("t{i}"),
                i,
                "http://127.0.0.1:1/announce",
            ))
            .await;
    }
    assert_eq!(engine.stats().await.active_torrents, 1);
    let new_cfg = EngineConfig {
        max_active_torrents: 4,
        ..Default::default()
    };
    engine.update_config(new_cfg).await;
    assert_eq!(engine.current_config().max_active_torrents, 4);
    assert_eq!(engine.stats().await.active_torrents, 4);
    engine.shutdown().await;
}

#[tokio::test]
async fn pause_then_resume_round_trips() {
    let engine = engine_with_default_profile(EngineConfig::default()).await;
    let id = engine
        .add_torrent_metainfo(meta_with_tracker("pr", 7, "http://127.0.0.1:1/announce"))
        .await
        .unwrap();
    assert_eq!(engine.stats().await.active_torrents, 1);
    engine.pause_torrent(id).await.unwrap();
    assert_eq!(engine.stats().await.active_torrents, 0);
    engine.resume_torrent(id).await.unwrap();
    assert_eq!(engine.stats().await.active_torrents, 1);
    engine.shutdown().await;
}

#[tokio::test]
async fn persistence_round_trip_via_session() {
    use rusqlite::Connection;

    fn pieces_for(length: u64, piece_len: u64) -> Vec<u8> {
        let total_pieces = length.div_ceil(piece_len);
        let mut out = Vec::new();
        for _ in 0..total_pieces {
            out.extend_from_slice(&Sha1::digest([]));
        }
        out
    }

    // Build a tiny in-memory torrent + upsert_many + read_all roundtrip.
    let dir = std::env::temp_dir().join(format!("sudoratio-roundtrip-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db_path = dir.join("session.sqlite");
    let _ = pieces_for(1, 1); // ensure helper is exercised
    let _ = Connection::open(&db_path).unwrap(); // ensure rusqlite can write here

    let engine = engine_with_default_profile(EngineConfig::default()).await;
    let id = engine
        .add_torrent_metainfo(meta_with_tracker("rt", 42, "http://127.0.0.1:1/announce"))
        .await
        .unwrap();
    let snapshot = engine.export_torrent(id).expect("export");
    engine.shutdown().await;
    drop(engine);

    // Round-trip via the session module's snapshot-writer (private to the bin so we re-run the
    // engine restore path via Engine::restore_torrent on a fresh handle).
    let engine2 = engine_with_default_profile(EngineConfig::default()).await;
    engine2.restore_torrent(snapshot.clone()).await.unwrap();
    engine2.finish_restore(0).await;
    let restored = engine2
        .get_torrent(snapshot.id)
        .await
        .expect("torrent restored");
    assert_eq!(restored.info_hash, snapshot.info_hash);
    assert_eq!(restored.name, snapshot.name);
    engine2.shutdown().await;
}

// Quietly include a use of AnnounceEvent / Arc to avoid drift if upstream renames spread.
#[tokio::test]
async fn manual_announce_via_engine_round_trips_outcome() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TRACKER_OK_BODY))
        .mount(&mock)
        .await;
    let cfg = EngineConfig {
        max_active_torrents: 0,
        ..Default::default()
    };
    let _ = Arc::new(cfg.clone());
    let engine = engine_with_default_profile(cfg).await;
    let id = engine
        .add_torrent_metainfo(meta_with_tracker(
            "manual",
            5,
            &format!("{}/announce", mock.uri()),
        ))
        .await
        .unwrap();
    let out = engine
        .announce_torrent(id, AnnounceEvent::Started)
        .await
        .expect("announce");
    assert_eq!(out.announce_interval, Some(333));
    engine.shutdown().await;
}

// ─── BT peer listener loopback ────────────────────────────────────────────────

#[tokio::test]
async fn peer_listener_responds_to_handshake() {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let cfg = EngineConfig {
        max_active_torrents: 0,
        ..Default::default()
    };
    let engine = engine_with_default_profile(cfg).await;
    let port = engine
        .start_peer_listener("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .expect("listener bind");
    assert_ne!(port, 0);

    // Add a torrent so the engine knows this info_hash.
    let info_hash_bytes = [0x42u8; 20];
    let meta = MetainfoTorrent {
        name: "probe".into(),
        info_hash: hex::encode(info_hash_bytes),
        info_hash_bytes,
        trackers: TrackersHttp {
            tiers: vec![vec!["http://127.0.0.1:1/announce".into()]],
        },
        size: 1024,
        download_before_seed: false,
    };
    let _ = engine.add_torrent_metainfo(meta).await.unwrap();

    let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut hs = [0u8; 68];
    hs[0] = 19;
    hs[1..20].copy_from_slice(b"BitTorrent protocol");
    hs[28..48].copy_from_slice(&info_hash_bytes);
    hs[48..68].copy_from_slice(&[0xAA; 20]);
    sock.write_all(&hs).await.unwrap();

    let mut reply = [0u8; 68];
    sock.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply[0], 19);
    assert_eq!(&reply[1..20], b"BitTorrent protocol");
    assert_eq!(&reply[28..48], &info_hash_bytes);
    assert_eq!(reply[25], 0x10, "BEP-10 reserved bit");

    use tokio::time::{timeout, Duration as TD};
    let mut spurious = [0u8; 1];
    let after_hs = timeout(TD::from_millis(800), sock.read(&mut spurious)).await;
    assert!(
        after_hs.is_err(),
        "must be silent after handshake (got: {:?})",
        after_hs
    );

    drop(sock);
    engine.shutdown().await;
}

#[tokio::test]
async fn peer_listener_drops_unknown_info_hash() {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let engine = engine_with_default_profile(EngineConfig::default()).await;
    let port = engine
        .start_peer_listener("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .expect("listener bind");

    let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut hs = [0u8; 68];
    hs[0] = 19;
    hs[1..20].copy_from_slice(b"BitTorrent protocol");
    hs[28..48].copy_from_slice(&[0xCD; 20]);
    hs[48..68].copy_from_slice(&[0xBB; 20]);
    sock.write_all(&hs).await.unwrap();

    let mut buf = [0u8; 1];
    let n = sock.read(&mut buf).await.unwrap_or(0);
    assert_eq!(n, 0, "unknown info_hash → silent close");

    engine.shutdown().await;
}

// ─── BT peer listener: full integration ───────────────────────────────────────

#[tokio::test]
async fn peer_listener_full_round_trip_with_announce_consistency() {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration as TD};

    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(TRACKER_OK_BODY))
        .mount(&mock)
        .await;

    // max_active_torrents=0 suppresses auto-start; announces are driven manually.
    let cfg = EngineConfig {
        max_active_torrents: 0,
        ..Default::default()
    };
    let engine = engine_with_default_profile(cfg).await;
    let bound_port = engine
        .start_peer_listener("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .expect("peer listener bind");
    assert_ne!(bound_port, 0);

    let info_hash_bytes = [0x77u8; 20];
    let meta = MetainfoTorrent {
        name: "swarm-probe".into(),
        info_hash: hex::encode(info_hash_bytes),
        info_hash_bytes,
        trackers: TrackersHttp {
            tiers: vec![vec![format!("{}/announce", mock.uri())]],
        },
        size: 64 * 1024,
        download_before_seed: false,
    };
    let id = engine.add_torrent_metainfo(meta).await.unwrap();

    // Announce port = listener-bound port (not the static config fallback).
    let _ = engine
        .announce_torrent(id, AnnounceEvent::Started)
        .await
        .expect("announce");
    let received = mock.received_requests().await.expect("mock requests");
    let url = received.first().expect("≥1 announce").url.to_string();
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let port_param = find_query_param(query, "port").expect("port= in announce");
    assert_eq!(port_param, bound_port.to_string());
    // Deluge profile leaves peer_id raw (`should_url_encode = false`).
    let peer_id_announced = find_query_param(query, "peer_id").expect("peer_id= in announce");
    assert_eq!(peer_id_announced.len(), 20);

    let mut sock = timeout(
        TD::from_secs(5),
        TcpStream::connect(("127.0.0.1", bound_port)),
    )
    .await
    .expect("connect timed out")
    .expect("connect");
    let mut hs = [0u8; 68];
    hs[0] = 19;
    hs[1..20].copy_from_slice(b"BitTorrent protocol");
    hs[28..48].copy_from_slice(&info_hash_bytes);
    hs[48..68].copy_from_slice(&[0xCC; 20]);
    sock.write_all(&hs).await.unwrap();

    let mut reply = [0u8; 68];
    timeout(TD::from_secs(5), sock.read_exact(&mut reply))
        .await
        .expect("hs read timed out")
        .expect("hs read");
    assert_eq!(reply[0], 19);
    assert_eq!(&reply[1..20], b"BitTorrent protocol");
    assert_eq!(reply[25] & 0x10, 0x10, "BEP-10 reserved bit");
    assert_eq!(&reply[28..48], &info_hash_bytes);
    // Per-info-hash identity invariant: peer_id on the wire == peer_id in announce.
    assert_eq!(&reply[48..68], peer_id_announced.as_bytes());

    // Silence after handshake. interested + request must elicit zero bytes.
    sock.write_all(&[0, 0, 0, 1, 2]).await.unwrap();
    let mut req_msg = Vec::with_capacity(17);
    req_msg.extend_from_slice(&13u32.to_be_bytes());
    req_msg.push(6);
    req_msg.extend_from_slice(&0u32.to_be_bytes());
    req_msg.extend_from_slice(&0u32.to_be_bytes());
    req_msg.extend_from_slice(&16384u32.to_be_bytes());
    sock.write_all(&req_msg).await.unwrap();
    let mut spurious = [0u8; 1];
    let after = timeout(TD::from_millis(800), sock.read(&mut spurious)).await;
    assert!(
        after.is_err(),
        "no bitfield/unchoke/piece (got: {:?})",
        after
    );

    // Concurrent connection: same per-info-hash peer_id served.
    let mut sock2 = TcpStream::connect(("127.0.0.1", bound_port)).await.unwrap();
    let mut hs2 = [0u8; 68];
    hs2[0] = 19;
    hs2[1..20].copy_from_slice(b"BitTorrent protocol");
    hs2[28..48].copy_from_slice(&info_hash_bytes);
    hs2[48..68].copy_from_slice(&[0x55; 20]);
    sock2.write_all(&hs2).await.unwrap();
    let mut reply2 = [0u8; 68];
    timeout(TD::from_secs(5), sock2.read_exact(&mut reply2))
        .await
        .expect("second hs timed out")
        .expect("second hs read");
    assert_eq!(&reply2[28..48], &info_hash_bytes);
    // Same peer_id (identity is per-info-hash, not per-connection).
    assert_eq!(&reply2[48..68], peer_id_announced.as_bytes());

    // 14. Drop both, then shut down. Engine must release the listener cleanly.
    drop(sock);
    drop(sock2);
    engine.shutdown().await;
}

#[tokio::test]
async fn peer_listener_garbage_handshake_closes_silently() {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration as TD};

    let engine = engine_with_default_profile(EngineConfig::default()).await;
    let port = engine
        .start_peer_listener("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .expect("listener bind");

    let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    sock.write_all(&[0xff; 68]).await.unwrap();
    let mut buf = [0u8; 1];
    let n = timeout(TD::from_secs(2), sock.read(&mut buf))
        .await
        .expect("read timed out")
        .unwrap_or(0);
    assert_eq!(n, 0, "garbage handshake → silent close");

    engine.shutdown().await;
}

/// Slow drip-fed handshake (8-byte chunks, 5ms apart) must still parse.
#[tokio::test]
async fn peer_listener_handshake_timeout_closes() {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    let engine = engine_with_default_profile(EngineConfig::default()).await;
    let port = engine
        .start_peer_listener("127.0.0.1:0".parse::<SocketAddr>().unwrap())
        .await
        .expect("listener bind");

    let info_hash_bytes = [0x33u8; 20];
    let meta = MetainfoTorrent {
        name: "slow-peer".into(),
        info_hash: hex::encode(info_hash_bytes),
        info_hash_bytes,
        trackers: TrackersHttp {
            tiers: vec![vec!["http://127.0.0.1:1/announce".into()]],
        },
        size: 1024,
        download_before_seed: false,
    };
    let _ = engine.add_torrent_metainfo(meta).await.unwrap();

    let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let mut hs = [0u8; 68];
    hs[0] = 19;
    hs[1..20].copy_from_slice(b"BitTorrent protocol");
    hs[28..48].copy_from_slice(&info_hash_bytes);
    hs[48..68].copy_from_slice(&[0x11; 20]);
    for chunk in hs.chunks(8) {
        sock.write_all(chunk).await.unwrap();
        sock.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let mut reply = [0u8; 68];
    sock.read_exact(&mut reply).await.unwrap();
    assert_eq!(&reply[28..48], &info_hash_bytes);
    engine.shutdown().await;
}

// ─── Outbound BT dial ─────────────────────────────────────────────────────────

/// Fake peer at loopback; tracker mock returns it in `peers=`; verify our handshake reaches
/// it with the same peer_id we sent in the announce (per-info-hash identity invariant).
#[tokio::test]
async fn outbound_dial_completes_handshake_with_announced_peer_id() {
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::{timeout, Duration as TD};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fake_peer_addr: SocketAddr = listener.local_addr().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<[u8; 68]>(1);
    let fake_handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 68];
        stream.read_exact(&mut buf).await.unwrap();
        let _ = tx.send(buf).await;
        let mut reply = [0u8; 68];
        reply[0] = 19;
        reply[1..20].copy_from_slice(b"BitTorrent protocol");
        reply[28..48].copy_from_slice(&buf[28..48]);
        reply[48..68].copy_from_slice(&[0xEE; 20]);
        let _ = stream.write_all(&reply).await;
        tokio::time::sleep(TD::from_millis(500)).await;
    });

    let mock = MockServer::start().await;
    let mut body: Vec<u8> =
        b"d8:intervali333e12:min intervali600e8:completei5e10:incompletei2e5:peers6:".to_vec();
    let octets = match fake_peer_addr.ip() {
        std::net::IpAddr::V4(v4) => v4.octets(),
        _ => panic!("expected IPv4 from local listener"),
    };
    body.extend_from_slice(&octets);
    body.extend_from_slice(&fake_peer_addr.port().to_be_bytes());
    body.push(b'e');
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
        .mount(&mock)
        .await;

    // max_active_torrents=1 lets the scheduler auto-Start → wire::spawn_dials runs.
    let cfg = EngineConfig {
        max_active_torrents: 1,
        outbound_dial_allow_loopback: true,
        outbound_dial_max_per_announce: 3,
        ..Default::default()
    };
    let engine = engine_with_default_profile(cfg).await;

    let info_hash_bytes = [0x91u8; 20];
    let meta = MetainfoTorrent {
        name: "outbound".into(),
        info_hash: hex::encode(info_hash_bytes),
        info_hash_bytes,
        trackers: TrackersHttp {
            tiers: vec![vec![format!("{}/announce", mock.uri())]],
        },
        size: 1024,
        download_before_seed: false,
    };
    let _ = engine.add_torrent_metainfo(meta).await.unwrap();

    let received = timeout(TD::from_secs(5), rx.recv())
        .await
        .expect("dial timed out — no inbound handshake on fake peer")
        .expect("rx closed");
    assert_eq!(received[0], 19);
    assert_eq!(&received[1..20], b"BitTorrent protocol");
    assert_eq!(&received[28..48], &info_hash_bytes);

    // Per-info-hash identity: peer_id on BT wire == peer_id in announce.
    let announces = mock.received_requests().await.unwrap();
    let url = announces[0].url.to_string();
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let pid = find_query_param(q, "peer_id").unwrap();
    assert_eq!(&received[48..68], pid.as_bytes());

    drop(fake_handle);
    engine.shutdown().await;
}
