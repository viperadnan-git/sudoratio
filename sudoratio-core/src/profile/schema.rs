//! Client profile schema (TOML).
//!
//! A client profile defines how sudoratio emulates a real BitTorrent client when announcing
//! to trackers: the announce query template, peer-id and tracker-key generation, URL encoding
//! rules, and HTTP headers. Native TOML; legacy `.client` JSON is not supported.

use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RefreshOnPolicy {
    #[default]
    Always,
    Never,
    Timed,
    /// Rotate on a `started` announce or after `refresh_every_secs`. Tracker `key` only.
    TimedOrAfterStartedAnnounce,
    /// One value per torrent, kept across announces (with idle eviction for keys).
    TorrentPersistent,
    /// One value per torrent, dropped on `stopped`.
    TorrentVolatile,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientProfileSpec {
    /// Stable primary key (e.g. `qbittorrent-4.6.7`). Must be unique across all registered
    /// profiles; the API uses this as the path parameter for activation / lookup.
    pub id: String,
    /// Human-readable label shown in the UI. Defaults to [`Self::id`] when absent.
    #[serde(default)]
    pub name: Option<String>,
    /// Optional display version (e.g. `"4.6.7"`). Not interpreted; cosmetic only.
    #[serde(default)]
    pub version: Option<String>,
    /// Announce URL query template (placeholders: `{infohash}`, `{peerid}`, `{port}`, `{uploaded}`,
    /// `{downloaded}`, `{left}`, `{event}`, `{numwant}`, `{key}`, `{ip}`, `{ipv6}`).
    pub query: String,
    /// Default `numwant=` value for periodic / started announces.
    pub numwant: i64,
    /// `numwant=` value when announcing `stopped` (real clients usually send `0`).
    pub numwant_on_stop: i64,
    /// Extra HTTP headers sent on every announce request.
    #[serde(default)]
    pub request_headers: Vec<ClientProfileHeader>,
    pub peer_id_generator: PeerIdGenerator,
    pub url_encoder: UrlEncoderCfg,
    #[serde(default)]
    pub key_generator: Option<KeyGenerator>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientProfileHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerIdGenerator {
    pub algorithm: PeerAlgorithmSpec,
    #[serde(default)]
    pub should_url_encode: bool,
    #[serde(default, rename = "refresh_on")]
    pub refresh_on: RefreshOnPolicy,
    #[serde(default)]
    pub refresh_every_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PeerAlgorithmSpec {
    /// Pattern of the form `prefix[charset]{n}`.
    Regex { pattern: String },
    /// Generates a 20-byte peer-id whose suffix is drawn from `characters_pool` (size at least
    /// `base`) with a checksum byte appended.
    RandomPoolWithChecksum {
        prefix: String,
        characters_pool: String,
        base: u32,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct UrlEncoderCfg {
    /// Regex matching characters that must be passed through *unencoded* (everything else is
    /// percent-encoded). Defaults to RFC 3986 unreserved characters.
    pub encoding_exclusion_pattern: String,
    /// `"upper"` or `"lower"` — case for percent-encoded hex digits.
    pub encoded_hex_case: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyGenerator {
    pub algorithm: KeyAlgorithmSpec,
    #[serde(default)]
    pub key_case: Option<String>,
    #[serde(default, rename = "refresh_on")]
    pub refresh_on: RefreshOnPolicy,
    #[serde(default)]
    pub refresh_every_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyAlgorithmSpec {
    /// `length` random hex digits.
    Hash { length: usize },
    /// `length` random hex digits with all leading zeros stripped (and a trailing `1` if the
    /// remainder is empty).
    HashNoLeadingZero { length: usize },
    /// Charset regex of the form `[charset]{n}`.
    Regex { pattern: String },
    /// Random integer in `[lower, upper]` rendered as lowercase hex without leading zeros.
    DigitRangeHex { lower: i64, upper: i64 },
}
