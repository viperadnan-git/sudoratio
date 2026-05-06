//! Embedded SPA serving.
//!
//! At compile time `rust-embed` bundles `web/dist/client/` (built by `build.rs`) into the binary.
//! Routing rules, in order of precedence:
//!
//! 1. `/api/v1/*` is matched by the API router *before* this fallback runs (registered with
//!    `nest`), so API paths can never collide with static files.
//! 2. Exact-match: try the request path against the embedded asset tree (`/favicon.ico`,
//!    `/assets/foo.js`, …). Long-lived hashed assets get a `Cache-Control: public, immutable`
//!    header; everything else gets `no-cache` so deploys don't get pinned to stale HTML.
//! 3. SPA fallback: any unmatched path returns the prerendered shell (`_shell.html`) so the
//!    client-side router can resolve it. Same `no-cache` policy as HTML.
//!
//! This is the same pattern Vercel/Netlify/Cloudflare Pages use for SPA hosting, just compiled
//! into a single Rust binary instead.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../web/dist/"]
struct Assets;

const SHELL_FILE: &str = "index.html";

/// Axum handler used as the router's fallback. Serves embedded static files or, on miss, the SPA
/// shell. Caching policy mirrors how a CDN would treat a Vite build:
///
/// * `assets/<hash>.{js,css,woff2,…}` — content-addressed, safe to cache forever.
/// * Top-level files (`/favicon.ico`, `/manifest.json`) and the shell — `no-cache` so a redeploy
///   is picked up immediately on next navigation.
pub async fn fallback(uri: Uri) -> Response {
    // Strip the leading `/`. `Uri::path()` always begins with `/` for absolute-form requests
    // (which is what axum gives us).
    let path = uri.path().trim_start_matches('/');

    if !path.is_empty() {
        if let Some(resp) = serve_embedded(path) {
            return resp;
        }
        // `/assets/*` is a content-addressed namespace owned by the build pipeline. A miss here
        // is a real 404 — never the SPA shell — so a stale `<script src="/assets/old.js">` fails
        // visibly instead of getting served HTML and parse-erroring on the client.
        if path.starts_with("assets/") {
            return (StatusCode::NOT_FOUND, "asset not found").into_response();
        }
    }

    // SPA fallback: anything not matched on disk is a client-side route. Serve the shell so
    // TanStack Router can resolve it. If the shell is missing the build step misbehaved.
    serve_embedded(SHELL_FILE)
        .unwrap_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "SPA shell missing").into_response())
}

fn serve_embedded(path: &str) -> Option<Response> {
    let file = Assets::get(path)?;
    let mime = file.metadata.mimetype();
    let cache_control = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, cache_control)
        .body(Body::from(file.data.into_owned()))
        .expect("static response builds");
    if let Ok(etag) =
        HeaderValue::from_str(&format!("\"{}\"", hex::encode(file.metadata.sha256_hash())))
    {
        resp.headers_mut().insert(header::ETAG, etag);
    }
    Some(resp)
}
