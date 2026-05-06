//! Bearer-token auth middleware for `/api/v1/*`.
//!
//! The expected token is the lowercase hex encoding of the configured password's UTF-8 bytes.
//! Clients send `Authorization: Bearer <hex>` on every request. Comparison is constant-time.

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header::AUTHORIZATION, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::Json;
use sudoratio_core::ApiErrorBody;

use crate::state::AppState;

/// Compute the expected bearer token from a password (hex of its UTF-8 bytes).
pub fn token_for(password: &str) -> String {
    hex::encode(password.as_bytes())
}

pub async fn require_bearer(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let supplied = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if !constant_time_eq(supplied.as_bytes(), state.auth_token.as_bytes()) {
        let body = ApiErrorBody {
            code: "unauthorized",
            message: "missing or invalid Bearer token".into(),
        };
        return (StatusCode::UNAUTHORIZED, Json(body)).into_response();
    }
    next.run(req).await
}

/// Constant-time equality. Returns false on length mismatch without leaking the length.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

use axum::response::IntoResponse;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_for_matches_hex() {
        assert_eq!(token_for("sudoratio"), "7375646f726174696f");
    }

    #[test]
    fn constant_time_eq_handles_lengths() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }
}
