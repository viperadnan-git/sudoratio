//! Client doc: top-level TOML carrying base config + N `[[variant]]` blocks.
//!
//! Resolution applies RFC 7396 JSON Merge Patch semantics:
//!   - scalar in variant replaces base
//!   - table in variant recurses into base's table
//!   - array in variant replaces base's array (no smart merge)
//!   - the dedicated `headers_patch` array upserts/removes by `name`
//!
//! Output: one [`ClientProfileSpec`] per variant with id `client@version`.
//!
//! End-user authoring is the same TOML shape as bundled docs. To extend a bundled client
//! with a new version, the user GETs the bundled source, appends a `[[variant]]`, and POSTs
//! the modified doc as a user-owned client (under a new `client = ...` name). Bundled
//! client docs are immutable.

use serde::Deserialize;

use crate::error::SudoratioError;
use crate::profile::schema::{
    ClientProfileHeader, ClientProfileSpec, KeyAlgorithmSpec, KeyGenerator, PeerAlgorithmSpec,
    PeerIdGenerator, RefreshOnPolicy, UrlEncoderCfg,
};

/// Top-level shape of a client doc TOML. Base fields live next to the `[[variant]]` array.
#[derive(Debug, Clone, Deserialize)]
pub struct ClientDoc {
    /// Stable family identifier (e.g. `qbittorrent`). Combined with each variant's
    /// `version` to form the per-variant profile id `client@version`.
    pub client: String,
    /// Optional human-readable label (defaults to `client` capitalized).
    #[serde(default)]
    pub display_name: Option<String>,

    #[serde(flatten)]
    pub base: PartialSpec,

    #[serde(default, rename = "variant")]
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Variant {
    /// Version string (free-form; combined into `id = "{client}@{version}"`).
    pub version: String,

    #[serde(flatten)]
    pub overrides: PartialSpec,

    /// Optional header upserts (applied AFTER base+overrides merge).
    /// Set `remove = true` to delete a header inherited from base.
    #[serde(default)]
    pub headers_patch: Vec<HeaderPatch>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeaderPatch {
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub remove: bool,
}

// ─── partial mirrors of the spec types: every field optional, tables recurse ────────────

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialSpec {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub numwant: Option<i64>,
    #[serde(default)]
    pub numwant_on_stop: Option<i64>,
    /// Replaced as a whole; merge granularity is per-header via `headers_patch`.
    #[serde(default)]
    pub request_headers: Option<Vec<ClientProfileHeader>>,
    #[serde(default)]
    pub peer_id_generator: Option<PartialPeerIdGenerator>,
    #[serde(default)]
    pub url_encoder: Option<PartialUrlEncoderCfg>,
    /// `Some(None)` (toml literal `key_generator = null` not supported by toml-rs) cannot
    /// express deletion; absent = inherit, present = replace/merge.
    #[serde(default)]
    pub key_generator: Option<PartialKeyGenerator>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialPeerIdGenerator {
    #[serde(default)]
    pub algorithm: Option<PartialPeerAlgorithm>,
    #[serde(default)]
    pub should_url_encode: Option<bool>,
    #[serde(default)]
    pub refresh_on: Option<RefreshOnPolicy>,
    #[serde(default)]
    pub refresh_every_secs: Option<u64>,
}

/// Flat partial: tag (`type`) and every field optional; merged field-wise. Variants can omit
/// `type` to inherit the algorithm shape from base while overriding individual fields.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialPeerAlgorithm {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub characters_pool: Option<String>,
    #[serde(default)]
    pub base: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialKeyGenerator {
    #[serde(default)]
    pub algorithm: Option<PartialKeyAlgorithm>,
    #[serde(default)]
    pub key_case: Option<String>,
    #[serde(default)]
    pub refresh_on: Option<RefreshOnPolicy>,
    #[serde(default)]
    pub refresh_every_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialKeyAlgorithm {
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub length: Option<usize>,
    #[serde(default)]
    pub pattern: Option<String>,
    #[serde(default)]
    pub lower: Option<i64>,
    #[serde(default)]
    pub upper: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PartialUrlEncoderCfg {
    #[serde(default)]
    pub encoding_exclusion_pattern: Option<String>,
    #[serde(default)]
    pub encoded_hex_case: Option<String>,
}

// ─── parsing ────────────────────────────────────────────────────────────────────────────

pub fn parse_client_doc(toml_str: &str) -> Result<ClientDoc, SudoratioError> {
    toml::from_str(toml_str).map_err(|e| SudoratioError::ClientProfileParse(format!("toml: {e}")))
}

// ─── merge + finalize ───────────────────────────────────────────────────────────────────

impl PartialSpec {
    /// RFC 7396 merge: variant fields overlay base.
    fn merge(&mut self, other: PartialSpec) {
        if let Some(v) = other.query {
            self.query = Some(v);
        }
        if let Some(v) = other.numwant {
            self.numwant = Some(v);
        }
        if let Some(v) = other.numwant_on_stop {
            self.numwant_on_stop = Some(v);
        }
        if let Some(v) = other.request_headers {
            self.request_headers = Some(v);
        }
        match (&mut self.peer_id_generator, other.peer_id_generator) {
            (Some(a), Some(b)) => a.merge(b),
            (slot, Some(b)) => *slot = Some(b),
            _ => {}
        }
        match (&mut self.url_encoder, other.url_encoder) {
            (Some(a), Some(b)) => a.merge(b),
            (slot, Some(b)) => *slot = Some(b),
            _ => {}
        }
        match (&mut self.key_generator, other.key_generator) {
            (Some(a), Some(b)) => a.merge(b),
            (slot, Some(b)) => *slot = Some(b),
            _ => {}
        }
    }
}

impl PartialPeerIdGenerator {
    fn merge(&mut self, other: PartialPeerIdGenerator) {
        match (&mut self.algorithm, other.algorithm) {
            (Some(a), Some(b)) => a.merge(b),
            (slot, Some(b)) => *slot = Some(b),
            _ => {}
        }
        if let Some(v) = other.should_url_encode {
            self.should_url_encode = Some(v);
        }
        if let Some(v) = other.refresh_on {
            self.refresh_on = Some(v);
        }
        if let Some(v) = other.refresh_every_secs {
            self.refresh_every_secs = Some(v);
        }
    }
}

impl PartialPeerAlgorithm {
    fn merge(&mut self, other: PartialPeerAlgorithm) {
        if other.kind.is_some() {
            self.kind = other.kind;
        }
        if other.pattern.is_some() {
            self.pattern = other.pattern;
        }
        if other.prefix.is_some() {
            self.prefix = other.prefix;
        }
        if other.characters_pool.is_some() {
            self.characters_pool = other.characters_pool;
        }
        if other.base.is_some() {
            self.base = other.base;
        }
    }
}

impl PartialKeyGenerator {
    fn merge(&mut self, other: PartialKeyGenerator) {
        match (&mut self.algorithm, other.algorithm) {
            (Some(a), Some(b)) => a.merge(b),
            (slot, Some(b)) => *slot = Some(b),
            _ => {}
        }
        if let Some(v) = other.key_case {
            self.key_case = Some(v);
        }
        if let Some(v) = other.refresh_on {
            self.refresh_on = Some(v);
        }
        if let Some(v) = other.refresh_every_secs {
            self.refresh_every_secs = Some(v);
        }
    }
}

impl PartialKeyAlgorithm {
    fn merge(&mut self, other: PartialKeyAlgorithm) {
        if other.kind.is_some() {
            self.kind = other.kind;
        }
        if other.length.is_some() {
            self.length = other.length;
        }
        if other.pattern.is_some() {
            self.pattern = other.pattern;
        }
        if other.lower.is_some() {
            self.lower = other.lower;
        }
        if other.upper.is_some() {
            self.upper = other.upper;
        }
    }
}

impl PartialUrlEncoderCfg {
    fn merge(&mut self, other: PartialUrlEncoderCfg) {
        if let Some(v) = other.encoding_exclusion_pattern {
            self.encoding_exclusion_pattern = Some(v);
        }
        if let Some(v) = other.encoded_hex_case {
            self.encoded_hex_case = Some(v);
        }
    }
}

fn missing_at(id: &str, field: &str) -> SudoratioError {
    SudoratioError::ClientProfileParse(format!("{id}: missing required `{field}`"))
}

fn apply_headers_patch(headers: &mut Vec<ClientProfileHeader>, patches: &[HeaderPatch]) {
    for p in patches {
        if p.remove {
            headers.retain(|h| h.name != p.name);
            continue;
        }
        let value = p.value.clone().unwrap_or_default();
        if let Some(slot) = headers.iter_mut().find(|h| h.name == p.name) {
            slot.value = value;
        } else {
            headers.push(ClientProfileHeader {
                name: p.name.clone(),
                value,
            });
        }
    }
}

impl PartialSpec {
    fn finalize(self, id: String, version: String) -> Result<ClientProfileSpec, SudoratioError> {
        let m = |f: &str| missing_at(&id, f);
        let query = self.query.ok_or_else(|| m("query"))?;
        let numwant = self.numwant.ok_or_else(|| m("numwant"))?;
        let numwant_on_stop = self.numwant_on_stop.ok_or_else(|| m("numwant_on_stop"))?;
        let request_headers = self.request_headers.unwrap_or_default();
        let peer_id_generator = self
            .peer_id_generator
            .ok_or_else(|| m("peer_id_generator"))?
            .finalize(&id)?;
        let url_encoder = self
            .url_encoder
            .ok_or_else(|| m("url_encoder"))?
            .finalize(&id)?;
        let key_generator = match self.key_generator {
            Some(k) => Some(k.finalize(&id)?),
            None => None,
        };
        Ok(ClientProfileSpec {
            id,
            name: None,
            version: Some(version),
            query,
            numwant,
            numwant_on_stop,
            request_headers,
            peer_id_generator,
            url_encoder,
            key_generator,
        })
    }
}

impl PartialPeerIdGenerator {
    fn finalize(self, id: &str) -> Result<PeerIdGenerator, SudoratioError> {
        let algorithm = self
            .algorithm
            .ok_or_else(|| missing_at(id, "peer_id_generator.algorithm"))?
            .finalize(id)?;
        Ok(PeerIdGenerator {
            algorithm,
            should_url_encode: self.should_url_encode.unwrap_or(false),
            refresh_on: self.refresh_on.unwrap_or_default(),
            refresh_every_secs: self.refresh_every_secs,
        })
    }
}

impl PartialPeerAlgorithm {
    fn finalize(self, id: &str) -> Result<PeerAlgorithmSpec, SudoratioError> {
        let kind = self
            .kind
            .as_deref()
            .ok_or_else(|| missing_at(id, "peer_id_generator.algorithm.type"))?;
        match kind {
            "regex" => Ok(PeerAlgorithmSpec::Regex {
                pattern: self
                    .pattern
                    .ok_or_else(|| missing_at(id, "peer_id_generator.algorithm.pattern"))?,
            }),
            "random_pool_with_checksum" => Ok(PeerAlgorithmSpec::RandomPoolWithChecksum {
                prefix: self
                    .prefix
                    .ok_or_else(|| missing_at(id, "peer_id_generator.algorithm.prefix"))?,
                characters_pool: self
                    .characters_pool
                    .ok_or_else(|| missing_at(id, "peer_id_generator.algorithm.characters_pool"))?,
                base: self
                    .base
                    .ok_or_else(|| missing_at(id, "peer_id_generator.algorithm.base"))?,
            }),
            other => Err(SudoratioError::ClientProfileParse(format!(
                "{id}: peer_id_generator.algorithm.type: unknown {other:?}"
            ))),
        }
    }
}

impl PartialKeyGenerator {
    fn finalize(self, id: &str) -> Result<KeyGenerator, SudoratioError> {
        let algorithm = self
            .algorithm
            .ok_or_else(|| missing_at(id, "key_generator.algorithm"))?
            .finalize(id)?;
        Ok(KeyGenerator {
            algorithm,
            key_case: self.key_case,
            refresh_on: self.refresh_on.unwrap_or_default(),
            refresh_every_secs: self.refresh_every_secs,
        })
    }
}

impl PartialKeyAlgorithm {
    fn finalize(self, id: &str) -> Result<KeyAlgorithmSpec, SudoratioError> {
        let kind = self
            .kind
            .as_deref()
            .ok_or_else(|| missing_at(id, "key_generator.algorithm.type"))?;
        match kind {
            "hash" => Ok(KeyAlgorithmSpec::Hash {
                length: self
                    .length
                    .ok_or_else(|| missing_at(id, "key_generator.algorithm.length"))?,
            }),
            "hash_no_leading_zero" => Ok(KeyAlgorithmSpec::HashNoLeadingZero {
                length: self
                    .length
                    .ok_or_else(|| missing_at(id, "key_generator.algorithm.length"))?,
            }),
            "regex" => Ok(KeyAlgorithmSpec::Regex {
                pattern: self
                    .pattern
                    .ok_or_else(|| missing_at(id, "key_generator.algorithm.pattern"))?,
            }),
            "digit_range_hex" => Ok(KeyAlgorithmSpec::DigitRangeHex {
                lower: self
                    .lower
                    .ok_or_else(|| missing_at(id, "key_generator.algorithm.lower"))?,
                upper: self
                    .upper
                    .ok_or_else(|| missing_at(id, "key_generator.algorithm.upper"))?,
            }),
            other => Err(SudoratioError::ClientProfileParse(format!(
                "{id}: key_generator.algorithm.type: unknown {other:?}"
            ))),
        }
    }
}

impl PartialUrlEncoderCfg {
    fn finalize(self, id: &str) -> Result<UrlEncoderCfg, SudoratioError> {
        Ok(UrlEncoderCfg {
            encoding_exclusion_pattern: self
                .encoding_exclusion_pattern
                .ok_or_else(|| missing_at(id, "url_encoder.encoding_exclusion_pattern"))?,
            encoded_hex_case: self
                .encoded_hex_case
                .ok_or_else(|| missing_at(id, "url_encoder.encoded_hex_case"))?,
        })
    }
}

impl PartialSpec {
    /// True iff every base field is unset — used to detect "extension docs" that overlay a
    /// bundled client's base.
    pub fn is_empty(&self) -> bool {
        self.query.is_none()
            && self.numwant.is_none()
            && self.numwant_on_stop.is_none()
            && self.request_headers.is_none()
            && self.peer_id_generator.is_none()
            && self.url_encoder.is_none()
            && self.key_generator.is_none()
    }
}

impl ClientDoc {
    /// True iff this doc supplies no base fields. Such docs are valid only as overlays on a
    /// bundled client of the same name (`Engine::register_client` enforces).
    pub fn is_extension(&self) -> bool {
        self.base.is_empty()
    }

    /// Resolve every variant in this doc into a concrete spec, using `self.base` as the base.
    pub fn resolve(&self) -> Result<Vec<ClientProfileSpec>, SudoratioError> {
        self.resolve_with_base(&self.base, None)
    }

    /// Resolve variants against an explicit base (e.g. inherited from a bundled doc when this
    /// doc is an extension). `display_name_fallback` supplies the spec name when the doc itself
    /// omits `display_name`.
    pub fn resolve_with_base(
        &self,
        base: &PartialSpec,
        display_name_fallback: Option<&str>,
    ) -> Result<Vec<ClientProfileSpec>, SudoratioError> {
        if self.client.is_empty() {
            return Err(SudoratioError::ClientProfileParse(
                "client doc: `client` must not be empty".into(),
            ));
        }
        if self.variants.is_empty() {
            return Err(SudoratioError::ClientProfileParse(format!(
                "client doc {}: needs at least one [[variant]]",
                self.client
            )));
        }
        let display = self
            .display_name
            .clone()
            .or_else(|| display_name_fallback.map(str::to_string));
        let mut out = Vec::with_capacity(self.variants.len());
        let mut seen_versions: std::collections::HashSet<&str> = Default::default();
        for v in &self.variants {
            if v.version.is_empty() {
                return Err(SudoratioError::ClientProfileParse(format!(
                    "client {}: variant.version must not be empty",
                    self.client
                )));
            }
            if !seen_versions.insert(v.version.as_str()) {
                return Err(SudoratioError::ClientProfileParse(format!(
                    "client {}: duplicate variant version {}",
                    self.client, v.version
                )));
            }
            let mut merged = base.clone();
            merged.merge(v.overrides.clone());
            let mut hdrs = merged.request_headers.take().unwrap_or_default();
            apply_headers_patch(&mut hdrs, &v.headers_patch);
            merged.request_headers = Some(hdrs);
            let id = format!("{}@{}", self.client, v.version);
            let mut spec = merged.finalize(id, v.version.clone())?;
            spec.name = display
                .clone()
                .or_else(|| Some(format!("{} {}", self.client, v.version)));
            out.push(spec);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QBITTORRENT: &str = include_str!("bundled/files/qbittorrent.toml");

    #[test]
    fn parses_qbittorrent_doc() {
        let doc = parse_client_doc(QBITTORRENT).unwrap();
        assert_eq!(doc.client, "qbittorrent");
        assert_eq!(doc.variants.len(), 9);
    }

    #[test]
    fn resolves_variants_to_unique_ids() {
        let doc = parse_client_doc(QBITTORRENT).unwrap();
        let specs = doc.resolve().unwrap();
        assert_eq!(specs.len(), 9);
        let mut ids: Vec<_> = specs.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len);
        assert!(specs.iter().any(|s| s.id == "qbittorrent@4.6.7"));
    }

    #[test]
    fn variant_overrides_propagate_to_spec() {
        let doc = parse_client_doc(QBITTORRENT).unwrap();
        let specs = doc.resolve().unwrap();
        let v467 = specs.iter().find(|s| s.id == "qbittorrent@4.6.7").unwrap();
        let ua = v467
            .request_headers
            .iter()
            .find(|h| h.name == "User-Agent")
            .unwrap();
        assert_eq!(ua.value, "qBittorrent/4.6.7");
        if let PeerAlgorithmSpec::Regex { pattern } = &v467.peer_id_generator.algorithm {
            assert!(pattern.contains("-qB4670-"));
        } else {
            panic!("expected regex peer_id");
        }
    }

    #[test]
    fn headers_patch_remove_drops_header() {
        let toml = r#"
client = "x"
query = "?{infohash}"
numwant = 1
numwant_on_stop = 0

[[request_headers]]
name = "User-Agent"
value = "x/1"
[[request_headers]]
name = "Accept-Language"
value = "en"

[peer_id_generator]
[peer_id_generator.algorithm]
type = "regex"
pattern = "-X-[A-Za-z0-9]{16}"

[url_encoder]
encoding_exclusion_pattern = "."
encoded_hex_case = "lower"

[[variant]]
version = "1"

[[variant]]
version = "2"
headers_patch = [
    { name = "Accept-Language", remove = true },
]
"#;
        let doc = parse_client_doc(toml).unwrap();
        let specs = doc.resolve().unwrap();
        let v1 = specs.iter().find(|s| s.id == "x@1").unwrap();
        let v2 = specs.iter().find(|s| s.id == "x@2").unwrap();
        assert_eq!(v1.request_headers.len(), 2);
        assert_eq!(v2.request_headers.len(), 1);
        assert!(v2
            .request_headers
            .iter()
            .all(|h| h.name != "Accept-Language"));
    }

    #[test]
    fn duplicate_variant_versions_error() {
        let toml = r#"
client = "x"
query = "?"
numwant = 1
numwant_on_stop = 0
[peer_id_generator]
[peer_id_generator.algorithm]
type = "regex"
pattern = "x"
[url_encoder]
encoding_exclusion_pattern = "."
encoded_hex_case = "lower"
[[variant]]
version = "1"
[[variant]]
version = "1"
"#;
        let doc = parse_client_doc(toml).unwrap();
        assert!(doc.resolve().is_err());
    }

    #[test]
    fn missing_required_field_after_merge_errors() {
        let toml = r#"
client = "x"
numwant = 1
numwant_on_stop = 0
[peer_id_generator]
[peer_id_generator.algorithm]
type = "regex"
pattern = "x"
[url_encoder]
encoding_exclusion_pattern = "."
encoded_hex_case = "lower"
[[variant]]
version = "1"
"#;
        let doc = parse_client_doc(toml).unwrap();
        let err = doc.resolve().unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("query"), "msg = {msg}");
    }

    #[test]
    fn extension_doc_resolves_against_borrowed_base() {
        let bundled = parse_client_doc(QBITTORRENT).unwrap();
        let extension_toml = r#"
client = "qbittorrent"

[[variant]]
version = "5.0.0"
headers_patch = [
    { name = "User-Agent", value = "qBittorrent/5.0.0" },
]

[variant.peer_id_generator.algorithm]
pattern = "-qB5000-[A-Za-z0-9_~\\(\\)\\!\\.\\*-]{12}"
"#;
        let extension = parse_client_doc(extension_toml).unwrap();
        assert!(extension.is_extension(), "doc has no base fields");
        let specs = extension
            .resolve_with_base(&bundled.base, None)
            .expect("resolve extension");
        assert_eq!(specs.len(), 1);
        let v = &specs[0];
        assert_eq!(v.id, "qbittorrent@5.0.0");
        // Inherited from bundled base:
        assert_eq!(v.numwant, 200);
        assert!(v.query.contains("compact=1"));
        // From the variant:
        let ua = v
            .request_headers
            .iter()
            .find(|h| h.name == "User-Agent")
            .expect("ua present");
        assert_eq!(ua.value, "qBittorrent/5.0.0");
        if let PeerAlgorithmSpec::Regex { pattern } = &v.peer_id_generator.algorithm {
            assert!(pattern.contains("-qB5000-"));
        } else {
            panic!("expected regex peer_id");
        }
    }

    #[test]
    fn doc_with_base_fields_is_not_an_extension() {
        let toml = r#"
client = "qbittorrent"
numwant = 1
[[variant]]
version = "x"
"#;
        let doc = parse_client_doc(toml).unwrap();
        assert!(!doc.is_extension());
    }
}
