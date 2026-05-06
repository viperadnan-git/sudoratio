//! Announce HTTP trace types: structured snapshots of the request and response stored on each
//! torrent's bounded ring buffer for inspection in the API.

use serde::{Deserialize, Serialize};

use crate::torrent::AnnounceEvent;

/// One HTTP header line sent with the announce (emulated client profile + `Host`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceHttpHeader {
    pub name: String,
    pub value: String,
}

/// BEP-3 query fields mirrored in announce traces (`request.params`).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AnnounceRequestParams {
    pub port: u16,
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
    pub event: AnnounceEvent,
}

/// Structured snapshot of the announce HTTP request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceRequestTrace {
    pub method: String,
    /// `http` or `https`, derived from the announce URL scheme.
    pub protocol: String,
    pub url: String,
    pub headers: Vec<AnnounceHttpHeader>,
    pub params: AnnounceRequestParams,
}

/// Tracker HTTP response snapshot: status line, headers, and parsed or diagnostic body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceResponseTrace {
    pub status: u16,
    pub headers: Vec<AnnounceHttpHeader>,
    pub body: serde_json::Value,
}

/// One tracker announce attempt (success or failure), like a client session log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceTrace {
    pub tracker_index: usize,
    pub event: AnnounceEvent,
    pub announced_at: u64,
    pub success: bool,
    pub request: AnnounceRequestTrace,
    pub response: AnnounceResponseTrace,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
