use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// String-keyed identity of a registered client profile (the TOML `id` field).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClientProfileId(pub String);

impl ClientProfileId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ClientProfileId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ClientProfileId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for ClientProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Torrent identity = BitTorrent info-hash (20 bytes). Serializes as 40-hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TorrentId(pub [u8; 20]);

impl TorrentId {
    pub fn from_hex(s: &str) -> Option<Self> {
        let v: [u8; 20] = hex::decode(s).ok()?.try_into().ok()?;
        Some(Self(v))
    }
}

impl std::fmt::Display for TorrentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for TorrentId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TorrentId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_hex(&s).ok_or_else(|| serde::de::Error::custom("invalid info_hash hex"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientProfileSummary {
    /// Per-variant primary key, formatted `client@version`.
    pub id: String,
    /// Client family name (e.g. `qbittorrent`); the part before `@`. UIs group by this.
    pub client: String,
    /// Variant version (the part after `@`).
    pub version: String,
    pub active: bool,
    /// Display label. Falls back to `"{client} {version}"`.
    pub name: String,
    /// `false` for bundled profiles (compiled into the binary, read-only).
    pub editable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SeedingStatus {
    pub running: bool,
    /// Sum of per-torrent simulated upload rates (bytes/s).
    pub upload_speed: u64,
    /// Sum of per-torrent simulated download rates (bytes/s).
    pub download_speed: u64,
    /// Configured cap on concurrent orchestrated torrents.
    pub max_active_torrents: usize,
    /// Currently in the active set (announces + bandwidth).
    pub active_torrents: usize,
    /// Waiting for a free slot (FIFO).
    pub waiting_torrents: usize,
    pub tracked_metainfo_torrents: usize,
}

/// Internal BT-protocol phase. Drives `event=completed` and the `left=`
/// query value; not part of the public state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferPhase {
    #[default]
    Downloading,
    Seeding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    User,
    UploadRatio,
    NoLeechers,
    TinySwarm,
    TrackerFailed,
}

impl StopReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::UploadRatio => "upload_ratio",
            Self::NoLeechers => "no_leechers",
            Self::TinySwarm => "tiny_swarm",
            Self::TrackerFailed => "tracker_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TorrentState {
    Downloading,
    Seeding,
    #[default]
    Queued,
    Stopped(StopReason),
}

impl TorrentState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Downloading => "downloading",
            Self::Seeding => "seeding",
            Self::Queued => "queued",
            Self::Stopped(_) => "stopped",
        }
    }

    pub const fn reason(self) -> Option<StopReason> {
        match self {
            Self::Stopped(r) => Some(r),
            _ => None,
        }
    }

    /// Active: in the announce loop and the bandwidth dispatcher.
    pub const fn is_active(self) -> bool {
        matches!(self, Self::Downloading | Self::Seeding)
    }

    pub const fn is_stopped(self) -> bool {
        matches!(self, Self::Stopped(_))
    }
}

impl Serialize for TorrentState {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TorrentState {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "downloading" => Ok(Self::Downloading),
            "seeding" => Ok(Self::Seeding),
            "queued" => Ok(Self::Queued),
            // Sibling `reason` field carries the actual reason.
            "stopped" => Ok(Self::Stopped(StopReason::User)),
            other => Err(serde::de::Error::custom(format!(
                "invalid torrent state: {other}"
            ))),
        }
    }
}

/// HTTP tracker list grouped by BEP-12 tier (each inner Vec is one tier).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackersHttp {
    pub tiers: Vec<Vec<String>>,
}

impl TrackersHttp {
    pub fn flat(&self) -> Vec<String> {
        self.tiers.iter().flatten().cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tiers.iter().all(|t| t.is_empty())
    }
}

/// Payload for adding a torrent from decoded `.torrent` metainfo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetainfoTorrent {
    pub name: String,
    /// 40-character lowercase hex SHA-1 of the `info` dictionary.
    pub info_hash: String,
    pub info_hash_bytes: [u8; 20],
    pub trackers: TrackersHttp,
    /// Total length of the torrent payload (all files), in bytes.
    pub size: u64,
    /// When true, simulate a full download (peer-gated), then seed; when false, seed-only path
    /// (BEP 3: from-storage seeder, announces `left=0` from the first `started`).
    #[serde(default)]
    pub download_before_seed: bool,
}

/// Optional values for a single announce HTTP query (override core state for this request only).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnounceQueryOverrides {
    pub uploaded: Option<u64>,
    pub downloaded: Option<u64>,
    pub left: Option<u64>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnounceEvent {
    None,
    Started,
    Stopped,
    Completed,
}

pub use crate::announce::trace::{
    AnnounceHttpHeader, AnnounceRequestParams, AnnounceRequestTrace, AnnounceResponseTrace,
    AnnounceTrace,
};

/// Engine-only runtime row (skipped from public JSON; persisted as a child row).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TorrentRuntime {
    pub info_hash_bytes: Option<[u8; 20]>,
    pub tier_index: usize,
    pub intra_index: usize,
    pub announce_count: u64,
    pub last_announce_interval_seconds: u32,
    /// BEP-3 `min interval` from last success (`0` = tracker omitted it).
    pub last_min_interval_seconds: u32,
    pub last_successful_announce_unix_ms: u64,
    pub consecutive_fails: u32,
}

impl Default for TorrentRuntime {
    fn default() -> Self {
        Self {
            info_hash_bytes: None,
            tier_index: 0,
            intra_index: 0,
            announce_count: 0,
            last_announce_interval_seconds: 5,
            last_min_interval_seconds: 0,
            last_successful_announce_unix_ms: 0,
            consecutive_fails: 0,
        }
    }
}

/// Single torrent model: API + persistence + SQLite (`SCHEMA.md` field order).
/// `runtime` is omitted from JSON (`#[serde(skip)]`); `announces` is capped to 32 newest when serializing for the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Torrent {
    pub id: TorrentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloaded: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uploaded: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_speed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upload_speed: Option<u64>,
    /// Last known tracker swarm `complete` count (seeders), if the tracker reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeders: Option<u32>,
    /// Last known tracker swarm `incomplete` count (leechers), if the tracker reported it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leechers: Option<u32>,
    pub state: TorrentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<StopReason>,
    pub download_before_seed: bool,
    pub trackers: TrackersHttp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announce_interval: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_announce_interval: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_announced_at: Option<u64>,
    /// Lower runs first when promoting from `Queued`; stable across pause/resume.
    pub queue_position: u32,
    #[serde(skip, default)]
    pub runtime: TorrentRuntime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransferStats {
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnounceOutcome {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub announce_interval: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_interval: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seeders: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leechers: Option<u32>,
    /// Peers from the announce response (`peers` BEP-23/BEP-3 + `peers6` BEP-7). Consumed by `wire::spawn_dials`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<std::net::SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub ok: bool,
    pub version: String,
}
// --- in-memory torrent state owned by the engine ----------------------------

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;

/// Identity (`name`, `info_hash`, `http_tracker_urls`, `size`, `download_first`) is immutable for
/// the lifetime of the torrent. The high-frequency `uploaded` counter stays atomic because the
/// bandwidth simulator increments it on every tick. Everything else lives behind a single
/// `parking_lot::Mutex<TorrentMutable>`.
pub(crate) struct TorrentEntry {
    pub(crate) name: Arc<str>,
    pub(crate) info_hash: Option<Arc<str>>,
    pub(crate) info_hash_bytes: Option<[u8; 20]>,
    /// BEP-12 tiers. The intra-tier order is the per-session shuffled order; on success the
    /// working tracker is rotated to the head of its tier (libtorrent / qBT semantics).
    pub(crate) tiers: parking_lot::RwLock<Vec<Vec<String>>>,
    pub(crate) size: u64,
    pub(crate) download_first: bool,
    /// Total bytes uploaded; written by the bandwidth tick on every interval, hence atomic.
    pub(crate) uploaded: AtomicU64,
    pub(crate) state: Mutex<TorrentMutable>,
    /// Set on every state mutation; cleared by the periodic persist task.
    pub(crate) dirty: AtomicBool,
}

#[derive(Debug, Clone)]
pub(crate) struct TorrentMutable {
    pub left: u64,
    pub state: TorrentState,
    pub queue_position: u32,
    /// Active tier and offset within that tier's shuffled order.
    pub tier_index: usize,
    pub intra_index: usize,
    pub consecutive_fails: u32,
    pub last_announce_interval_seconds: u32,
    pub last_min_interval_seconds: u32,
    pub last_successful_announce_unix_ms: u64,
    pub announce_count: u64,
    pub last_seeders: Option<u32>,
    pub last_leechers: Option<u32>,
    /// Unix-ms when leechers first hit zero; `None` while leechers > 0.
    pub zero_leechers_since: Option<u64>,
}

impl TorrentEntry {
    pub(crate) fn uploaded(&self) -> u64 {
        self.uploaded.load(Ordering::Relaxed)
    }

    pub(crate) fn store_uploaded(&self, v: u64) {
        self.uploaded.store(v, Ordering::Relaxed);
        self.mark_dirty();
    }

    pub(crate) fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::Release);
    }

    /// Returns prior dirty state and clears it.
    pub(crate) fn take_dirty(&self) -> bool {
        self.dirty.swap(false, Ordering::AcqRel)
    }

    /// Derived from `(download_first, left)` — single source of truth, no drift.
    pub(crate) fn transfer_phase(&self) -> TransferPhase {
        if self.download_first && self.state.lock().left > 0 {
            TransferPhase::Downloading
        } else {
            TransferPhase::Seeding
        }
    }

    pub(crate) fn lifecycle(&self) -> TorrentState {
        self.state.lock().state
    }

    pub(crate) fn set_lifecycle(&self, s: TorrentState) {
        self.state.lock().state = s;
        self.mark_dirty();
    }

    pub(crate) fn queue_position(&self) -> u32 {
        self.state.lock().queue_position
    }

    /// Currently-selected tracker URL per BEP-12 (`tiers[tier_index][intra_index]`).
    pub(crate) fn current_tracker(&self) -> Option<String> {
        let s = self.state.lock();
        let tier = s.tier_index;
        let intra = s.intra_index;
        drop(s);
        let tiers = self.tiers.read();
        tiers.get(tier).and_then(|t| t.get(intra)).cloned()
    }

    /// (tier_index, intra_index) — for trace/persistence. Encodes as a flat index for legacy code.
    pub(crate) fn tracker_position(&self) -> (usize, usize) {
        let s = self.state.lock();
        (s.tier_index, s.intra_index)
    }

    /// Flatten (tier, intra) to a single sequential index for traces / display.
    pub(crate) fn flat_tracker_index(&self) -> usize {
        let (tier, intra) = self.tracker_position();
        let tiers = self.tiers.read();
        let prefix: usize = tiers.iter().take(tier).map(|t| t.len()).sum();
        prefix + intra
    }

    /// BEP-12 failover: advance within tier, then to next tier; wrap from last tier to (0,0).
    pub(crate) fn advance_tracker(&self) {
        let tiers = self.tiers.read();
        if tiers.is_empty() {
            return;
        }
        let mut s = self.state.lock();
        let tier_len = tiers.get(s.tier_index).map(|t| t.len()).unwrap_or(0);
        s.intra_index += 1;
        if s.intra_index >= tier_len {
            s.intra_index = 0;
            s.tier_index = (s.tier_index + 1) % tiers.len();
        }
        drop(s);
        drop(tiers);
        self.mark_dirty();
    }

    /// BEP-12 success: rotate the working tracker to the head of its tier, reset intra to 0.
    pub(crate) fn promote_current_tracker(&self) {
        let (tier, intra) = self.tracker_position();
        if intra == 0 {
            return;
        }
        {
            let mut tiers = self.tiers.write();
            if let Some(t) = tiers.get_mut(tier) {
                if intra < t.len() {
                    let url = t.remove(intra);
                    t.insert(0, url);
                }
            }
        }
        let mut s = self.state.lock();
        s.intra_index = 0;
        drop(s);
        self.mark_dirty();
    }

    pub(crate) fn reset_consecutive_fails(&self) {
        self.state.lock().consecutive_fails = 0;
        self.mark_dirty();
    }

    pub(crate) fn bump_consecutive_fails(&self) -> u32 {
        let mut s = self.state.lock();
        s.consecutive_fails = s.consecutive_fails.saturating_add(1);
        let v = s.consecutive_fails;
        drop(s);
        self.mark_dirty();
        v
    }

    pub(crate) fn last_announce_interval_seconds(&self) -> u32 {
        self.state.lock().last_announce_interval_seconds
    }

    pub(crate) fn last_min_interval_seconds(&self) -> u32 {
        self.state.lock().last_min_interval_seconds
    }

    /// Derived: `0` for from-storage seeders, `size - left` otherwise.
    pub(crate) fn downloaded(&self) -> u64 {
        if self.download_first {
            self.size.saturating_sub(self.state.lock().left)
        } else {
            0
        }
    }

    pub(crate) fn left(&self) -> u64 {
        self.state.lock().left
    }

    /// Atomically apply `f` to the mutable state, marking the row dirty.
    pub(crate) fn with_state<R>(&self, f: impl FnOnce(&mut TorrentMutable) -> R) -> R {
        let r = f(&mut self.state.lock());
        self.mark_dirty();
        r
    }

    pub(crate) fn snapshot(&self) -> TorrentMutable {
        self.state.lock().clone()
    }
}
