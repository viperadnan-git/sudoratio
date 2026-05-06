//! Map [`SudoratioError`] to HTTP responses.

use axum::http::StatusCode;
use axum::Json;
use sudoratio_core::{ApiErrorBody, SudoratioError};

pub type ApiErrorResponse = (StatusCode, Json<ApiErrorBody>);

pub fn status_for(e: &SudoratioError) -> StatusCode {
    use SudoratioError::*;
    match e {
        UnknownClientProfile(_) | TorrentNotFound => StatusCode::NOT_FOUND,
        ProfileImmutable(_)
        | SeedingAlreadyRunning
        | EngineShuttingDown
        | TorrentNotActive
        | TorrentAlreadyExists(_) => StatusCode::CONFLICT,
        Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        Json(_)
        | ClientProfileParse(_)
        | NoActiveClientProfile
        | TorrentNoMetainfo
        | NoHttpTrackers
        | MissingAnnounceUrl
        | AnnounceHttp(_)
        | TrackerFailure(_)
        | TrackerBencode(_)
        | PlaceholderBuild(_) => StatusCode::BAD_REQUEST,
    }
}

pub fn api_error(e: SudoratioError) -> ApiErrorResponse {
    let status = status_for(&e);
    tracing::warn!(code = e.api_code(), error = %e, "request rejected");
    (status, Json(e.to_api_body()))
}
