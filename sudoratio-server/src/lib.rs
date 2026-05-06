//! Library surface of `sudoratio-server` — exposed so integration tests (and embedders) can build
//! the same axum router the binary runs.

pub mod auth;
pub mod bundled;
pub mod cli;
pub mod config_io;
pub mod error;
pub mod profile_store;
pub mod routes;
pub mod server;
pub mod session;
pub mod state;
pub mod static_assets;
