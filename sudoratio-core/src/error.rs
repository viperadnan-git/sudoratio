use crate::torrent::ClientProfileId;
use serde::Serialize;
use thiserror::Error;

/// Stable machine-oriented code for HTTP APIs (paired with a human `message`).
#[derive(Debug, Clone, Serialize)]
pub struct ApiErrorBody {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum SudoratioError {
    #[error("IO: {0}")]
    Io(String),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("client profile parse error: {0}")]
    ClientProfileParse(String),
    #[error("unknown client profile: {0:?}")]
    UnknownClientProfile(ClientProfileId),
    #[error("client profile {0:?} is bundled and read-only")]
    ProfileImmutable(ClientProfileId),
    #[error("no active client profile")]
    NoActiveClientProfile,
    #[error("unknown torrent")]
    TorrentNotFound,
    #[error("torrent already added: {0}")]
    TorrentAlreadyExists(String),
    #[error("torrent is not active (must be downloading or seeding)")]
    TorrentNotActive,
    #[error("torrent has no metainfo (cannot announce)")]
    TorrentNoMetainfo,
    #[error("torrent has no HTTP announce trackers")]
    NoHttpTrackers,
    #[error("torrent has no announce URL")]
    MissingAnnounceUrl,
    #[error("seeding loop is already running")]
    SeedingAlreadyRunning,
    #[error("engine is shutting down")]
    EngineShuttingDown,
    #[error("announce HTTP: {0}")]
    AnnounceHttp(String),
    #[error("tracker failure: {0}")]
    TrackerFailure(String),
    #[error("tracker response bencode: {0}")]
    TrackerBencode(String),
    #[error("announce query: {0}")]
    PlaceholderBuild(String),
    #[error("target preset uses a different client profile; delete and re-add the torrent under the new preset to switch identity")]
    PresetClientMismatch,
}

impl SudoratioError {
    #[inline]
    pub fn api_code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Json(_) => "json",
            Self::ClientProfileParse(_) => "profile_parse",
            Self::UnknownClientProfile(_) => "unknown_profile",
            Self::ProfileImmutable(_) => "profile_immutable",
            Self::NoActiveClientProfile => "no_active_profile",
            Self::TorrentNotFound => "torrent_not_found",
            Self::TorrentAlreadyExists(_) => "torrent_already_exists",
            Self::TorrentNotActive => "torrent_not_active",
            Self::TorrentNoMetainfo => "torrent_no_metainfo",
            Self::NoHttpTrackers => "no_http_trackers",
            Self::MissingAnnounceUrl => "missing_announce_url",
            Self::SeedingAlreadyRunning => "seeding_already_running",
            Self::EngineShuttingDown => "engine_shutting_down",
            Self::AnnounceHttp(_) => "announce_http",
            Self::TrackerFailure(_) => "tracker_failure",
            Self::TrackerBencode(_) => "tracker_bencode",
            Self::PlaceholderBuild(_) => "placeholder_build",
            Self::PresetClientMismatch => "preset_client_mismatch",
        }
    }

    pub fn to_api_body(&self) -> ApiErrorBody {
        ApiErrorBody {
            code: self.api_code(),
            message: self.to_string(),
        }
    }
}
