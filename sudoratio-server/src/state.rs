//! Shared [`AppState`] passed to every axum handler.

use std::sync::Arc;

use sudoratio_core::Engine;

#[derive(Clone)]
pub struct AppState {
    pub core: Arc<Engine>,
    /// Lowercase hex of the configured password; expected `Authorization: Bearer <token>`.
    pub auth_token: Arc<str>,
}
