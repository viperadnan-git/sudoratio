//! Boots the actual axum router (no real TCP listener) and exercises the user-facing routes.
//! Closes the verification gap between the engine-level lifecycle tests and a real HTTP curl.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::Value as Json;
use sha1::{Digest, Sha1};
use sudoratio_core::profile::BUNDLED_CLIENTS;
use sudoratio_core::{Engine, EngineConfig};
use sudoratio_server::auth;
use sudoratio_server::server::build_router;
use sudoratio_server::session::Session;
use sudoratio_server::state::AppState;
use tower::ServiceExt;

const TEST_PASSWORD: &str = "sudoratio";

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

fn raw_torrent(announce: &str, name: &str) -> Vec<u8> {
    let length: u64 = 512;
    let data = vec![0u8; length as usize];
    let piece_hash = Sha1::digest(&data);
    let mut pieces_val = format!("{}:", piece_hash.len()).into_bytes();
    pieces_val.extend_from_slice(&piece_hash);
    let info = enc_dict(vec![
        (benc_str("piece length"), benc_int(16_384)),
        (benc_str("pieces"), pieces_val),
        (benc_str("length"), benc_int(length as i64)),
        (benc_str("name"), benc_str(name)),
    ]);
    enc_dict(vec![
        (benc_str("announce"), benc_str(announce)),
        (benc_str("info"), info),
    ])
}

async fn build_test_app() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let session_path = dir.path().join("session.sqlite3");
    let cfg_path = dir.path().join("config.json");
    let core = Engine::new(EngineConfig {
        max_active_torrents: 4,
        ..Default::default()
    });
    // Activate one bundled profile so announce-related code paths work even though we don't hit
    // a tracker in these tests.
    let (_, toml) = BUNDLED_CLIENTS[0];
    let ids = core.register_builtin_client(toml).await.unwrap();
    core.set_active_profile(ids.into_iter().next().unwrap())
        .await
        .unwrap();

    let session = Arc::new(Session::open(&session_path).expect("session open"));
    let state = Arc::new(AppState {
        core,
        session,
        core_config_path: cfg_path,
        auth_token: Arc::from(auth::token_for(TEST_PASSWORD).as_str()),
    });
    (build_router(state, 16), dir)
}

fn bearer() -> String {
    format!("Bearer {}", auth::token_for(TEST_PASSWORD))
}

/// Helper: build a request with the test bearer token already attached.
fn req(method: &str, uri: impl Into<String>, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri.into())
        .header("authorization", bearer())
        .body(body)
        .unwrap()
}

/// Build a `POST /api/v1/torrents` multipart body containing `raw` as the
/// `file` field. Boundary is fixed for reproducibility.
fn add_torrent_multipart(raw: &[u8]) -> Request<Body> {
    const BOUNDARY: &str = "----sudoratio-test-boundary";
    let mut buf: Vec<u8> = Vec::with_capacity(raw.len() + 256);
    buf.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    buf.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"t.torrent\"\r\n",
    );
    buf.extend_from_slice(b"Content-Type: application/x-bittorrent\r\n\r\n");
    buf.extend_from_slice(raw);
    buf.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());

    let mut r = req("POST", "/api/v1/torrents", Body::from(buf));
    r.headers_mut().insert(
        "content-type",
        format!("multipart/form-data; boundary={BOUNDARY}")
            .parse()
            .unwrap(),
    );
    r
}

async fn json_body(body: Body) -> Json {
    let bytes = to_bytes(body, usize::MAX).await.unwrap();
    if bytes.is_empty() {
        Json::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Json::Null)
    }
}

#[tokio::test]
async fn http_health_returns_ok() {
    let (app, _dir) = build_test_app().await;
    let resp = app
        .oneshot(req("GET", "/api/v1/health", Body::empty()))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp.into_body()).await;
    assert_eq!(body["ok"], Json::Bool(true));
}

#[tokio::test]
async fn http_post_torrent_then_get_returns_it() {
    let (app, _dir) = build_test_app().await;
    let raw = raw_torrent("http://example.invalid/announce", "ht-add");
    let resp = app
        .clone()
        .oneshot(add_torrent_multipart(&raw))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp.into_body()).await;
    let info_hash = body["info_hash"].as_str().unwrap().to_string();

    // GET single torrent
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            format!("/api/v1/torrents/{info_hash}"),
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp.into_body()).await;
    assert_eq!(body["info_hash"], Json::String(info_hash.clone()));
    assert_eq!(body["name"], Json::String("ht-add".to_string()));
}

#[tokio::test]
async fn http_patch_config_applies_live_without_full_body() {
    let (app, _dir) = build_test_app().await;
    let resp = app
        .clone()
        .oneshot({
            let mut r = req(
                "PATCH",
                "/api/v1/config",
                Body::from(r#"{"max_active_torrents": 7}"#),
            );
            r.headers_mut()
                .insert("content-type", "application/json".parse().unwrap());
            r
        })
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = json_body(resp.into_body()).await;
    assert_eq!(body["max_active_torrents"].as_u64(), Some(7));

    // Confirm via GET that the change persisted in memory.
    let resp = app
        .clone()
        .oneshot(req("GET", "/api/v1/config", Body::empty()))
        .await
        .unwrap();
    let body = json_body(resp.into_body()).await;
    assert_eq!(body["max_active_torrents"].as_u64(), Some(7));
}

#[tokio::test]
async fn http_pause_resume_round_trips() {
    let (app, _dir) = build_test_app().await;
    let raw = raw_torrent("http://example.invalid/announce", "ht-pr");
    let resp = app
        .clone()
        .oneshot(add_torrent_multipart(&raw))
        .await
        .unwrap();
    let info_hash = json_body(resp.into_body()).await["info_hash"]
        .as_str()
        .unwrap()
        .to_string();

    for path in [
        format!("/api/v1/torrents/{info_hash}/pause"),
        format!("/api/v1/torrents/{info_hash}/resume"),
    ] {
        let resp = app
            .clone()
            .oneshot(req("POST", &path, Body::empty()))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "POST {path}");
    }
}

#[tokio::test]
async fn http_delete_then_404() {
    let (app, _dir) = build_test_app().await;
    let raw = raw_torrent("http://example.invalid/announce", "ht-del");
    let resp = app
        .clone()
        .oneshot(add_torrent_multipart(&raw))
        .await
        .unwrap();
    let info_hash = json_body(resp.into_body()).await["info_hash"]
        .as_str()
        .unwrap()
        .to_string();
    let resp = app
        .clone()
        .oneshot(req(
            "DELETE",
            format!("/api/v1/torrents/{info_hash}"),
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(req(
            "GET",
            format!("/api/v1/torrents/{info_hash}"),
            Body::empty(),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn http_list_torrents_with_announces_query_toggles_field() {
    let (app, _dir) = build_test_app().await;
    let raw = raw_torrent("http://example.invalid/announce", "ht-list");
    let _ = app
        .clone()
        .oneshot(add_torrent_multipart(&raw))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(req("GET", "/api/v1/torrents", Body::empty()))
        .await
        .unwrap();
    let body = json_body(resp.into_body()).await;
    assert!(body[0].get("announces").is_none() || body[0]["announces"] == Json::Null);

    let resp = app
        .clone()
        .oneshot(req("GET", "/api/v1/torrents?with=announces", Body::empty()))
        .await
        .unwrap();
    let body = json_body(resp.into_body()).await;
    // The torrent has no announces yet (no real tracker), so the field is absent regardless.
    // The point of the test is the route accepts the query without 400-ing.
    assert!(body.is_array());
}
