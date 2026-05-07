//! Per-preset policy: bandwidth caps, slot count, ratio target, auto-pause rules.
//! Engine carries one `PresetRegistry`; each torrent holds an `Arc<ArcSwap<PresetPolicy>>`
//! shared with its preset, so PATCHing a preset is a single store seen by all its torrents.

use std::sync::Arc;

use arc_swap::ArcSwap;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

pub const DEFAULT_PRESET_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetPolicy {
    pub min_upload_speed: u64,
    pub max_upload_speed: u64,
    pub min_download_speed: u64,
    pub max_download_speed: u64,
    pub max_active_torrents: usize,
    pub upload_ratio_target: f32,
    pub pause_torrent_with_zero_leechers: bool,
    pub pause_torrent_with_zero_leechers_grace: u64,
    pub min_swarm_seeders_to_seed: u32,
    pub max_announce_jitter: u32,
    /// Client profile variant id (`client@version`). `None` = inherit engine default.
    #[serde(default)]
    pub client_profile_id: Option<String>,
}

impl Default for PresetPolicy {
    fn default() -> Self {
        Self {
            min_upload_speed: 27,
            max_upload_speed: 183,
            min_download_speed: 800,
            max_download_speed: 1200,
            max_active_torrents: 5,
            upload_ratio_target: 3.0,
            pause_torrent_with_zero_leechers: false,
            pause_torrent_with_zero_leechers_grace: 3 * 60 * 60,
            min_swarm_seeders_to_seed: 0,
            max_announce_jitter: 8,
            client_profile_id: None,
        }
    }
}

/// Patch type for `Engine::update_preset`. Each `Some(_)` overwrites; bounds are normalized
/// (min ≤ max) by `PresetRegistry::update`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PresetPolicyUpdate {
    pub min_upload_speed: Option<u64>,
    pub max_upload_speed: Option<u64>,
    pub min_download_speed: Option<u64>,
    pub max_download_speed: Option<u64>,
    pub max_active_torrents: Option<usize>,
    pub upload_ratio_target: Option<f32>,
    pub pause_torrent_with_zero_leechers: Option<bool>,
    pub pause_torrent_with_zero_leechers_grace: Option<u64>,
    pub min_swarm_seeders_to_seed: Option<u32>,
    pub max_announce_jitter: Option<u32>,
    /// Outer Option = "field present in patch"; inner = `None` (clear) vs `Some(id)` (set).
    #[serde(default)]
    pub client_profile_id: Option<Option<String>>,
}

impl PresetPolicyUpdate {
    pub fn merge(self, p: &mut PresetPolicy) {
        if let Some(v) = self.min_upload_speed {
            p.min_upload_speed = v;
        }
        if let Some(v) = self.max_upload_speed {
            p.max_upload_speed = v;
        }
        if let Some(v) = self.min_download_speed {
            p.min_download_speed = v;
        }
        if let Some(v) = self.max_download_speed {
            p.max_download_speed = v;
        }
        if let Some(v) = self.max_active_torrents {
            p.max_active_torrents = v.max(1);
        }
        if let Some(v) = self.upload_ratio_target {
            p.upload_ratio_target = v;
        }
        if let Some(v) = self.pause_torrent_with_zero_leechers {
            p.pause_torrent_with_zero_leechers = v;
        }
        if let Some(v) = self.pause_torrent_with_zero_leechers_grace {
            p.pause_torrent_with_zero_leechers_grace = v;
        }
        if let Some(v) = self.min_swarm_seeders_to_seed {
            p.min_swarm_seeders_to_seed = v;
        }
        if let Some(v) = self.max_announce_jitter {
            p.max_announce_jitter = v;
        }
        if let Some(v) = self.client_profile_id {
            p.client_profile_id = v;
        }
        if p.max_upload_speed < p.min_upload_speed {
            std::mem::swap(&mut p.min_upload_speed, &mut p.max_upload_speed);
        }
        if p.max_download_speed < p.min_download_speed {
            std::mem::swap(&mut p.min_download_speed, &mut p.max_download_speed);
        }
    }
}

#[derive(Debug)]
pub struct Preset {
    pub id: String,
    pub name: parking_lot::RwLock<String>,
    pub color: parking_lot::RwLock<String>,
    pub is_default: bool,
    pub policy: Arc<ArcSwap<PresetPolicy>>,
    pub created_at_ms: u64,
    pub updated_at_ms: parking_lot::RwLock<u64>,
}

impl Preset {
    pub fn snapshot(&self) -> PresetSnapshot {
        PresetSnapshot {
            id: self.id.clone(),
            name: self.name.read().clone(),
            color: self.color.read().clone(),
            is_default: self.is_default,
            policy: (*self.policy.load_full()).clone(),
            created_at_ms: self.created_at_ms,
            updated_at_ms: *self.updated_at_ms.read(),
        }
    }
}

/// Per-preset rollup (counts + summed live speeds). Recomputed on read.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PresetRollup {
    pub torrent_count: usize,
    pub active_count: usize,
    pub queued_count: usize,
    pub upload_speed_bps: u64,
    pub download_speed_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSnapshot {
    pub id: String,
    pub name: String,
    pub color: String,
    pub is_default: bool,
    pub policy: PresetPolicy,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug)]
pub struct PresetRegistry {
    inner: DashMap<String, Arc<Preset>>,
}

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("preset {0:?} not found")]
    NotFound(String),
    #[error("preset id {0:?} already exists")]
    AlreadyExists(String),
    #[error("preset id {0:?} is invalid: must match [a-z0-9_-]{{1,32}}")]
    InvalidId(String),
    #[error("cannot delete the default preset")]
    DeleteDefault,
    #[error("preset {0:?} has torrents; specify reassign_to or move them first")]
    HasTorrents(String),
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn validate_id(id: &str) -> Result<(), PresetError> {
    if id.is_empty() || id.len() > 32 {
        return Err(PresetError::InvalidId(id.to_string()));
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(PresetError::InvalidId(id.to_string()));
    }
    Ok(())
}

pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        let lc = c.to_ascii_lowercase();
        if lc.is_ascii_lowercase() || lc.is_ascii_digit() {
            out.push(lc);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("preset");
    }
    out.truncate(32);
    out
}

impl Default for PresetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PresetRegistry {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    pub fn get(&self, id: &str) -> Option<Arc<Preset>> {
        self.inner.get(id).map(|r| r.value().clone())
    }

    pub fn list(&self) -> Vec<Arc<Preset>> {
        let mut v: Vec<Arc<Preset>> = self.inner.iter().map(|r| r.value().clone()).collect();
        v.sort_by(|a, b| match (a.is_default, b.is_default) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.id.cmp(&b.id),
        });
        v
    }

    pub fn count(&self) -> usize {
        self.inner.len()
    }

    /// Insert a fully-built preset (used by session restore). Replaces existing entry.
    pub fn insert(&self, preset: Arc<Preset>) {
        self.inner.insert(preset.id.clone(), preset);
    }

    /// Ensure a `default` preset exists; creates one with `PresetPolicy::default()` if absent.
    pub fn ensure_default(&self) -> Arc<Preset> {
        if let Some(p) = self.inner.get(DEFAULT_PRESET_ID) {
            return p.value().clone();
        }
        let now = now_ms();
        let preset = Arc::new(Preset {
            id: DEFAULT_PRESET_ID.to_string(),
            name: parking_lot::RwLock::new("Default".into()),
            color: parking_lot::RwLock::new("#64748b".into()),
            is_default: true,
            policy: Arc::new(ArcSwap::from_pointee(PresetPolicy::default())),
            created_at_ms: now,
            updated_at_ms: parking_lot::RwLock::new(now),
        });
        self.inner.insert(DEFAULT_PRESET_ID.into(), preset.clone());
        preset
    }

    pub fn create(
        &self,
        id: Option<String>,
        name: String,
        color: String,
        policy: PresetPolicy,
    ) -> Result<Arc<Preset>, PresetError> {
        let id = id.unwrap_or_else(|| slugify(&name));
        validate_id(&id)?;
        if self.inner.contains_key(&id) {
            return Err(PresetError::AlreadyExists(id));
        }
        let now = now_ms();
        let preset = Arc::new(Preset {
            id: id.clone(),
            name: parking_lot::RwLock::new(name),
            color: parking_lot::RwLock::new(color),
            is_default: false,
            policy: Arc::new(ArcSwap::from_pointee(policy)),
            created_at_ms: now,
            updated_at_ms: parking_lot::RwLock::new(now),
        });
        self.inner.insert(id, preset.clone());
        Ok(preset)
    }

    pub fn update_name_color(
        &self,
        id: &str,
        name: Option<String>,
        color: Option<String>,
    ) -> Result<Arc<Preset>, PresetError> {
        let preset = self
            .inner
            .get(id)
            .map(|r| r.value().clone())
            .ok_or_else(|| PresetError::NotFound(id.into()))?;
        if let Some(n) = name {
            *preset.name.write() = n;
        }
        if let Some(c) = color {
            *preset.color.write() = c;
        }
        *preset.updated_at_ms.write() = now_ms();
        Ok(preset)
    }

    /// Live-apply a policy patch by storing a new snapshot in the preset's `ArcSwap`.
    pub fn apply_policy(
        &self,
        id: &str,
        patch: PresetPolicyUpdate,
    ) -> Result<Arc<Preset>, PresetError> {
        let preset = self
            .inner
            .get(id)
            .map(|r| r.value().clone())
            .ok_or_else(|| PresetError::NotFound(id.into()))?;
        let mut next = (*preset.policy.load_full()).clone();
        patch.merge(&mut next);
        preset.policy.store(Arc::new(next));
        *preset.updated_at_ms.write() = now_ms();
        Ok(preset)
    }

    /// Removes the preset row. Caller must verify `is_default == false` and reassign torrents.
    pub fn remove(&self, id: &str) -> Result<(), PresetError> {
        let preset = self
            .inner
            .get(id)
            .map(|r| r.value().clone())
            .ok_or_else(|| PresetError::NotFound(id.into()))?;
        if preset.is_default {
            return Err(PresetError::DeleteDefault);
        }
        self.inner.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_handles_punctuation() {
        assert_eq!(slugify("Alpha Tracker!"), "alpha-tracker");
        assert_eq!(slugify("  foo--bar  "), "foo-bar");
        assert_eq!(slugify(""), "preset");
    }

    #[test]
    fn validate_id_rejects_uppercase() {
        assert!(validate_id("Alpha").is_err());
        assert!(validate_id("alpha").is_ok());
        assert!(validate_id("alpha_42").is_ok());
        assert!(validate_id("a-b").is_ok());
        assert!(validate_id("").is_err());
    }

    #[test]
    fn ensure_default_is_idempotent() {
        let r = PresetRegistry::new();
        let a = r.ensure_default();
        let b = r.ensure_default();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn cannot_delete_default() {
        let r = PresetRegistry::new();
        r.ensure_default();
        assert!(matches!(
            r.remove(DEFAULT_PRESET_ID),
            Err(PresetError::DeleteDefault)
        ));
    }

    #[test]
    fn apply_policy_swaps_inverted_bounds() {
        let r = PresetRegistry::new();
        let p = r
            .create(
                Some("alpha".into()),
                "Alpha".into(),
                "#000".into(),
                PresetPolicy::default(),
            )
            .unwrap();
        r.apply_policy(
            "alpha",
            PresetPolicyUpdate {
                min_upload_speed: Some(500),
                max_upload_speed: Some(100),
                ..Default::default()
            },
        )
        .unwrap();
        let pol = (*p.policy.load_full()).clone();
        assert_eq!(pol.min_upload_speed, 100);
        assert_eq!(pol.max_upload_speed, 500);
    }
}
