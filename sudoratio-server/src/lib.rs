//! Library surface of `sudoratio-server` — exposed so integration tests can build
//! the same axum router the binary runs. Persistence and engine config live in `sudoratio-core`.

pub mod auth;
pub mod cli;
pub mod error;
pub mod routes;
pub mod server;
pub mod state;
pub mod static_assets;
