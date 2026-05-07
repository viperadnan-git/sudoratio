//! **sudoratio-core** — HTTP tracker announces, bandwidth simulation, and a seeding orchestrator.
//!
//! There is no BitTorrent peer wire protocol and no on-disk payload download: only metainfo
//! fields, TOML-defined client profiles, and HTTP(S) requests to trackers.

#![forbid(unsafe_code)]

/// Last-resort fallback for the tracker `port=` query.
pub const DEFAULT_ANNOUNCE_PORT: u16 = 51413;

mod announce;
mod bandwidth;
mod config;
mod engine;
mod metainfo;
mod persistence;
mod profile_store;
mod scheduler;
mod state;
mod wire;

pub mod config_io;
pub mod error;
pub mod preset;
pub mod profile;
pub mod torrent;

pub use bandwidth::SwarmSpeedDerivation;
pub use config::{EngineConfig, HttpTrackerConfig};
pub use config_io::{ConfigResponse, ConfigUpdate};
pub use error::{ApiErrorBody, SudoratioError};
pub use metainfo::parse as parse_metainfo;
pub use preset::{
    Preset, PresetError, PresetPolicy, PresetPolicyUpdate, PresetRegistry, PresetRollup,
    PresetSnapshot, DEFAULT_PRESET_ID,
};
pub use profile::{parse_client_doc, ClientDoc};
pub use state::Engine;
pub use torrent::{
    AnnounceEvent, AnnounceHttpHeader, AnnounceOutcome, AnnounceQueryOverrides,
    AnnounceRequestParams, AnnounceRequestTrace, AnnounceResponseTrace, AnnounceTrace,
    ClientProfileId, ClientProfileSummary, HealthStatus, MetainfoTorrent, SeedingStatus,
    StopReason, Torrent, TorrentId, TorrentRuntime, TorrentState, TrackersHttp, TransferPhase,
    TransferStats,
};
