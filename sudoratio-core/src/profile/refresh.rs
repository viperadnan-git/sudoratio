//! Generic resolver for [`RefreshOnPolicy`] (peer-id / tracker-key cache lookup).
//!
//! The five [`RefreshOnPolicy`] variants map to:
//!
//! All caches are keyed by info-hash so each torrent gets a distinct identity
//! (matches real qBT/libtorrent: one peer_id per torrent_handle).
//!
//! - `Never`               → cached forever per info-hash.
//! - `Always`              → freshly generated each call (no cache).
//! - `Timed`               → cached per info-hash with a wall-clock TTL.
//! - `TimedOrAfterStartedAnnounce` → cached per info-hash; rotates on TTL OR
//!   on a `started` announce. (Tracker `key` only — never used for peer_id.)
//! - `TorrentPersistent`   → cached per info-hash; evicted after `persistent_idle_ttl` of
//!   inactivity (set the field to `None` to disable eviction, e.g. for peer_id).
//! - `TorrentVolatile`     → cached per info-hash; entry is removed on `stopped`.

use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::error::SudoratioError;
use crate::profile::RefreshOnPolicy;
use crate::torrent::AnnounceEvent;

/// Default idle TTL for `TorrentPersistent` tracker keys (2h of inactivity → evicted).
pub(crate) const PERSISTENT_KEY_IDLE_TTL: Duration = Duration::from_secs(120 * 60);

struct Entry {
    value: String,
    /// Last time the value was (re)generated — used by `Timed` TTL.
    refreshed_at: Instant,
    /// Last time the entry was read — used by `TorrentPersistent` idle eviction.
    accessed_at: Instant,
}

#[derive(Default)]
pub(crate) struct IdCache {
    by_info_hash: DashMap<[u8; 20], Entry>,
}

impl IdCache {
    pub(crate) fn clear(&self) {
        self.by_info_hash.clear();
    }

    pub(crate) fn forget_info_hash(&self, info_hash: &[u8; 20]) {
        self.by_info_hash.remove(info_hash);
    }
}

pub(crate) struct Policy {
    pub policy: RefreshOnPolicy,
    pub refresh_every_secs: Option<u64>,
    pub persistent_idle_ttl: Option<Duration>,
}

pub(crate) fn resolve_with_policy(
    cache: &IdCache,
    spec: &Policy,
    info_hash: &[u8; 20],
    event: AnnounceEvent,
    name: &'static str,
    mut generate: impl FnMut() -> Result<String, SudoratioError>,
) -> Result<String, SudoratioError> {
    if matches!(spec.policy, RefreshOnPolicy::Always) {
        return generate();
    }
    let now = Instant::now();
    let needs_refresh = |e: &Entry| -> bool {
        match spec.policy {
            RefreshOnPolicy::Always => true,
            RefreshOnPolicy::Never => false,
            RefreshOnPolicy::Timed => match spec.refresh_every_secs {
                Some(s) => now.duration_since(e.refreshed_at) >= Duration::from_secs(s),
                None => false,
            },
            RefreshOnPolicy::TimedOrAfterStartedAnnounce => {
                if matches!(event, AnnounceEvent::Started) {
                    return true;
                }
                match spec.refresh_every_secs {
                    Some(s) => now.duration_since(e.refreshed_at) >= Duration::from_secs(s),
                    None => false,
                }
            }
            RefreshOnPolicy::TorrentPersistent | RefreshOnPolicy::TorrentVolatile => false,
        }
    };
    if matches!(
        spec.policy,
        RefreshOnPolicy::Timed | RefreshOnPolicy::TimedOrAfterStartedAnnounce
    ) && spec.refresh_every_secs.is_none()
    {
        return Err(SudoratioError::PlaceholderBuild(format!(
            "{name}: TIMED policy requires refreshEvery"
        )));
    }
    if matches!(spec.policy, RefreshOnPolicy::TorrentPersistent) {
        if let Some(ttl) = spec.persistent_idle_ttl {
            evict_stale_persistent(cache, ttl);
        }
    }
    let value = if let Some(mut row) = cache.by_info_hash.get_mut(info_hash) {
        if needs_refresh(row.value()) {
            let v = generate()?;
            row.value = v.clone();
            row.refreshed_at = now;
            row.accessed_at = now;
            v
        } else {
            row.accessed_at = now;
            row.value.clone()
        }
    } else {
        let v = generate()?;
        cache.by_info_hash.insert(
            *info_hash,
            Entry {
                value: v.clone(),
                refreshed_at: now,
                accessed_at: now,
            },
        );
        v
    };
    if matches!(spec.policy, RefreshOnPolicy::TorrentVolatile)
        && matches!(event, AnnounceEvent::Stopped)
    {
        cache.by_info_hash.remove(info_hash);
    }
    Ok(value)
}

fn evict_stale_persistent(cache: &IdCache, ttl: Duration) {
    let now = Instant::now();
    let stale: Vec<[u8; 20]> = cache
        .by_info_hash
        .iter()
        .filter_map(|e| (now.duration_since(e.value().accessed_at) > ttl).then_some(*e.key()))
        .collect();
    for k in stale {
        cache.by_info_hash.remove(&k);
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_with_policy, IdCache, Policy};
    use crate::error::SudoratioError;
    use crate::profile::RefreshOnPolicy;
    use crate::torrent::AnnounceEvent;

    fn pol(p: RefreshOnPolicy) -> Policy {
        Policy {
            policy: p,
            refresh_every_secs: None,
            persistent_idle_ttl: None,
        }
    }

    fn timed(secs: u64) -> Policy {
        Policy {
            policy: RefreshOnPolicy::Timed,
            refresh_every_secs: Some(secs),
            persistent_idle_ttl: None,
        }
    }

    fn started_or_timed(secs: u64) -> Policy {
        Policy {
            policy: RefreshOnPolicy::TimedOrAfterStartedAnnounce,
            refresh_every_secs: Some(secs),
            persistent_idle_ttl: None,
        }
    }

    #[test]
    fn always_generates_each_call() {
        let cache = IdCache::default();
        let mut n = 0u32;
        let mk = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = pol(RefreshOnPolicy::Always);
        let mut gen = mk;
        let a = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen)
            .unwrap();
        let b = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen)
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn never_keys_per_info_hash() {
        let cache = IdCache::default();
        let mut n = 0;
        let mut gen = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = pol(RefreshOnPolicy::Never);
        let h1 = [1u8; 20];
        let h2 = [2u8; 20];
        let a = resolve_with_policy(&cache, &p, &h1, AnnounceEvent::None, "t", &mut gen).unwrap();
        let b = resolve_with_policy(&cache, &p, &h2, AnnounceEvent::None, "t", &mut gen).unwrap();
        assert_ne!(a, b, "Never must give each torrent its own value");
    }

    #[test]
    fn never_caches_forever() {
        let cache = IdCache::default();
        let mut n = 0;
        let mut gen = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = pol(RefreshOnPolicy::Never);
        let a = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen)
            .unwrap();
        let b = resolve_with_policy(
            &cache,
            &p,
            &[0u8; 20],
            AnnounceEvent::Started,
            "t",
            &mut gen,
        )
        .unwrap();
        assert_eq!(a, b, "never policy should hand back the same value");
    }

    #[test]
    fn torrent_persistent_keys_per_info_hash() {
        let cache = IdCache::default();
        let mut n = 0;
        let mut gen = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = pol(RefreshOnPolicy::TorrentPersistent);
        let h1 = [1u8; 20];
        let h2 = [2u8; 20];
        let a = resolve_with_policy(&cache, &p, &h1, AnnounceEvent::None, "t", &mut gen).unwrap();
        let b = resolve_with_policy(&cache, &p, &h2, AnnounceEvent::None, "t", &mut gen).unwrap();
        let a2 = resolve_with_policy(&cache, &p, &h1, AnnounceEvent::None, "t", &mut gen).unwrap();
        assert_ne!(a, b);
        assert_eq!(a, a2);
    }

    #[test]
    fn torrent_volatile_drops_on_stopped() {
        let cache = IdCache::default();
        let mut n = 0;
        let mut gen = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = pol(RefreshOnPolicy::TorrentVolatile);
        let h = [9u8; 20];
        let a = resolve_with_policy(&cache, &p, &h, AnnounceEvent::Started, "t", &mut gen).unwrap();
        let a2 = resolve_with_policy(&cache, &p, &h, AnnounceEvent::None, "t", &mut gen).unwrap();
        assert_eq!(a, a2);
        // Stopped still returns the cached value but evicts the entry afterwards.
        let _ = resolve_with_policy(&cache, &p, &h, AnnounceEvent::Stopped, "t", &mut gen).unwrap();
        let b = resolve_with_policy(&cache, &p, &h, AnnounceEvent::Started, "t", &mut gen).unwrap();
        assert_ne!(a, b, "after Stopped, next call must regenerate");
    }

    #[test]
    fn timed_caches_within_window() {
        let cache = IdCache::default();
        let mut n = 0;
        let mut gen = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = timed(3600);
        let a = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen)
            .unwrap();
        let b = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen)
            .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn timed_or_after_started_rotates_on_started() {
        let cache = IdCache::default();
        let mut n = 0;
        let mut gen = || {
            n += 1;
            Ok(format!("v{n}"))
        };
        let p = started_or_timed(3600);
        let a = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen)
            .unwrap();
        let b = resolve_with_policy(
            &cache,
            &p,
            &[0u8; 20],
            AnnounceEvent::Started,
            "t",
            &mut gen,
        )
        .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn timed_without_refresh_every_errors() {
        let cache = IdCache::default();
        let p = pol(RefreshOnPolicy::Timed);
        let mut gen = || Ok::<_, SudoratioError>(String::from("v"));
        let r = resolve_with_policy(&cache, &p, &[0u8; 20], AnnounceEvent::None, "t", &mut gen);
        assert!(r.is_err());
    }
}
