//! Simulated upload and download bandwidth ([`BandwidthDispatcher`]).
//!
//! All speed values stored in this module are in **bytes/second**. Config knobs are exposed in
//! decimal **KB/s** (same units a typical desktop client reports) and converted at the boundary
//! by [`kb_to_bytes_per_sec`]. Swarm gating uses [`SwarmSpeedDerivation`] to zero out a direction
//! when no peer can support it.

use crate::torrent::TorrentEntry;
use crate::torrent::{TorrentId, TransferPhase};
use dashmap::DashMap;
use rand::Rng;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Effective caps for one scrape: each direction is either **0** or the full `reference_speed`
/// passed in, depending on leechers / seeders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmSpeedDerivation {
    /// Simulated upload budget for this scrape: full reference when `leechers > 0`, else `0`.
    pub upload_cap: u64,
    /// Simulated download budget for this scrape: full reference when `seeders > 0`, else `0`.
    pub download_cap: u64,
}

impl SwarmSpeedDerivation {
    /// Derive caps from a `reference_speed` (bytes/s) and scrape counts.
    #[must_use]
    pub fn from_scrape(reference_speed: u64, seeders: i64, leechers: i64) -> Self {
        Self {
            upload_cap: if leechers > 0 { reference_speed } else { 0 },
            download_cap: if seeders > 0 { reference_speed } else { 0 },
        }
    }

    #[must_use]
    pub const fn upload_blocked(self) -> bool {
        self.upload_cap == 0
    }

    #[must_use]
    pub const fn download_blocked(self) -> bool {
        self.download_cap == 0
    }
}

#[inline]
fn kb_to_bytes_per_sec(kb: u64) -> u64 {
    kb.saturating_mul(1000)
}

fn sample_speed_range(min: u64, max: u64) -> u64 {
    let max = max.max(min);
    if min == max {
        max
    } else {
        let mut rng = rand::rng();
        rng.random_range(min..=max)
    }
}

/// Multiplicative noise applied per-tick to the held cap so reported throughput fluctuates
/// instead of plateauing at a constant value. Guarantees:
/// 1. Sign is randomly chosen (up or down).
/// 2. Magnitude is uniform in \[1%, 10%\] of the cap — the dead zone around zero is excluded.
/// 3. Result is clamped to \[`min`, `max`\] so a bound-tight cap (e.g. `min == max`) never
///    exceeds the configured ceiling or drops below the floor.
/// 4. If integer truncation would collapse the result back onto `cap`, the value is nudged by
///    one byte/sec in the rolled direction so the returned value differs from `cap` whenever
///    the bounds permit.
fn jittered(cap: u64, min: u64, max: u64) -> u64 {
    if cap == 0 {
        return 0;
    }
    let max = max.max(min);
    let mut rng = rand::rng();
    // Pick magnitude in [1%, 10%] then randomize sign — avoids the near-zero dead zone where
    // (cap * factor) as u64 collapses back to cap.
    let magnitude: f64 = rng.random_range(0.01_f64..0.10_f64);
    let direction: bool = rng.random_bool(0.5);
    let factor = if direction {
        1.0 + magnitude
    } else {
        1.0 - magnitude
    };
    let mut raw = ((cap as f64) * factor).max(0.0) as u64;
    // Truncation can still land on `cap` for small values (e.g. cap=10, factor=1.05 → 10.5 → 10).
    // Nudge by one in the rolled direction so the value is guaranteed to change before clamping.
    if raw == cap {
        raw = if direction {
            cap.saturating_add(1)
        } else {
            cap.saturating_sub(1)
        };
    }
    raw.clamp(min, max)
}

#[derive(Debug)]
struct TorrentEntryBw {
    upload_speed: AtomicU64,
    download_speed: AtomicU64,
    seeders: AtomicI64,
    leechers: AtomicI64,
    min_download_speed: AtomicU64,
    max_download_speed: AtomicU64,
    min_upload_speed: AtomicU64,
    max_upload_speed: AtomicU64,
    /// Current per-torrent download cap; resampled on event triggers (announce response,
    /// phase flip, config update) — NOT on a wall-clock timer.
    download_cap: AtomicU64,
    /// Current per-torrent upload cap; resampled alongside `download_cap`.
    upload_cap: AtomicU64,
}

/// Per-torrent speeds; each tick adds upload bytes to [`TorrentEntry::uploaded`] on the core row.
///
/// Caps re-sample on events that would naturally shift a real client's throughput:
/// 1. Announce response (peer counts changed)
/// 2. Download → Seeding phase flip
/// 3. Config update changing min/max bounds
/// 4. Initial registration
///
/// Per-tick a ±10% jitter envelope is applied to the held cap so the *reported* speed in the
/// announce request fluctuates naturally instead of being a flat plateau.
#[derive(Debug)]
pub struct BandwidthDispatcher {
    tick_ms: u64,
    torrents: DashMap<TorrentId, TorrentEntryBw>,
}

impl BandwidthDispatcher {
    pub fn new(tick_ms: u64) -> Self {
        Self {
            tick_ms,
            torrents: DashMap::new(),
        }
    }

    pub fn tick_ms(&self) -> u64 {
        self.tick_ms
    }

    /// Register a torrent reading its bandwidth bounds from its preset policy.
    pub fn register_torrent(
        &self,
        id: TorrentId,
        torrent_rows: &DashMap<TorrentId, TorrentEntry>,
    ) {
        let Some(row) = torrent_rows.get(&id) else {
            return;
        };
        let policy = row.policy_snapshot();
        drop(row);
        let min_download_speed = kb_to_bytes_per_sec(policy.min_download_speed);
        let max_download_speed =
            kb_to_bytes_per_sec(policy.max_download_speed).max(min_download_speed);
        let download_cap = sample_speed_range(min_download_speed, max_download_speed);

        let min_upload_speed = kb_to_bytes_per_sec(policy.min_upload_speed);
        let max_upload_speed = kb_to_bytes_per_sec(policy.max_upload_speed).max(min_upload_speed);
        let upload_cap = sample_speed_range(min_upload_speed, max_upload_speed);

        let e = TorrentEntryBw {
            upload_speed: AtomicU64::new(0),
            download_speed: AtomicU64::new(0),
            seeders: AtomicI64::new(0),
            leechers: AtomicI64::new(0),
            min_download_speed: AtomicU64::new(min_download_speed),
            max_download_speed: AtomicU64::new(max_download_speed),
            min_upload_speed: AtomicU64::new(min_upload_speed),
            max_upload_speed: AtomicU64::new(max_upload_speed),
            download_cap: AtomicU64::new(download_cap),
            upload_cap: AtomicU64::new(upload_cap),
        };
        self.torrents.insert(id, e);
        self.recompute_with_torrent_rows(torrent_rows);
    }

    /// Resample one torrent's caps and recompute reported speeds.
    pub fn resample_torrent_cap(
        &self,
        id: TorrentId,
        torrent_rows: &DashMap<TorrentId, TorrentEntry>,
    ) {
        self.resample_one_inplace(id);
        self.recompute_with_torrent_rows(torrent_rows);
    }

    /// Resample many torrents' caps with a single trailing recompute.
    pub fn resample_many(
        &self,
        ids: impl IntoIterator<Item = TorrentId>,
        torrent_rows: &DashMap<TorrentId, TorrentEntry>,
    ) {
        for id in ids {
            self.resample_one_inplace(id);
        }
        self.recompute_with_torrent_rows(torrent_rows);
    }

    fn resample_one_inplace(&self, id: TorrentId) {
        let Some(e) = self.torrents.get(&id) else {
            return;
        };
        let down = sample_speed_range(
            e.min_download_speed.load(Ordering::Relaxed),
            e.max_download_speed.load(Ordering::Relaxed),
        );
        let up = sample_speed_range(
            e.min_upload_speed.load(Ordering::Relaxed),
            e.max_upload_speed.load(Ordering::Relaxed),
        );
        e.download_cap.store(down, Ordering::Relaxed);
        e.upload_cap.store(up, Ordering::Relaxed);
    }

    /// Sync one torrent's bandwidth bounds to its current preset policy. Used after
    /// `move_torrent` and after a preset PATCH that touches speed bounds.
    pub fn sync_torrent_to_policy(
        &self,
        id: TorrentId,
        torrent_rows: &DashMap<TorrentId, TorrentEntry>,
    ) {
        let Some(row) = torrent_rows.get(&id) else {
            return;
        };
        let policy = row.policy_snapshot();
        let min_dl = kb_to_bytes_per_sec(policy.min_download_speed);
        let max_dl = kb_to_bytes_per_sec(policy.max_download_speed).max(min_dl);
        let min_ul = kb_to_bytes_per_sec(policy.min_upload_speed);
        let max_ul = kb_to_bytes_per_sec(policy.max_upload_speed).max(min_ul);
        if let Some(r) = self.torrents.get(&id) {
            r.min_download_speed.store(min_dl, Ordering::Relaxed);
            r.max_download_speed.store(max_dl, Ordering::Relaxed);
            r.min_upload_speed.store(min_ul, Ordering::Relaxed);
            r.max_upload_speed.store(max_ul, Ordering::Relaxed);
            r.download_cap
                .store(sample_speed_range(min_dl, max_dl), Ordering::Relaxed);
            r.upload_cap
                .store(sample_speed_range(min_ul, max_ul), Ordering::Relaxed);
        }
        self.recompute_with_torrent_rows(torrent_rows);
    }

    /// Remove the torrent and recompute remaining-row totals.
    pub fn unregister_torrent(
        &self,
        id: TorrentId,
        torrent_rows: &DashMap<TorrentId, TorrentEntry>,
    ) {
        let _ = self.torrents.remove(&id);
        self.recompute_with_torrent_rows(torrent_rows);
    }

    /// Per-announce hook: store swarm counts, resample the cap, recompute reported speeds.
    pub fn on_announce_success(
        &self,
        id: TorrentId,
        seeders: i64,
        leechers: i64,
        torrent_rows: &DashMap<TorrentId, TorrentEntry>,
    ) {
        if let Some(e) = self.torrents.get_mut(&id) {
            e.seeders.store(seeders, Ordering::Relaxed);
            e.leechers.store(leechers, Ordering::Relaxed);
        }
        self.resample_one_inplace(id);
        self.recompute_with_torrent_rows(torrent_rows);
    }

    pub fn upload_speed_for(&self, id: TorrentId) -> u64 {
        self.torrents
            .get(&id)
            .map(|e| e.upload_speed.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    pub fn download_speed_for(&self, id: TorrentId) -> u64 {
        self.torrents
            .get(&id)
            .map(|e| e.download_speed.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    pub(crate) fn download_speed_pairs(&self) -> Vec<(TorrentId, u64)> {
        self.torrents
            .iter()
            .map(|r| (*r.key(), r.download_speed.load(Ordering::Relaxed)))
            .collect()
    }

    /// One scheduler tick: recompute per-torrent speeds (with per-tick jitter), then add the
    /// resulting upload bytes to each torrent's running total.
    pub fn tick_with_torrent_rows(&self, torrent_rows: &DashMap<TorrentId, TorrentEntry>) {
        self.recompute_with_torrent_rows(torrent_rows);
        let tick = self.tick_ms;
        for r in self.torrents.iter() {
            let tid = *r.key();
            let speed = r.upload_speed.load(Ordering::Relaxed);
            let delta = speed.saturating_mul(tick) / 1000;
            if delta > 0 {
                if let Some(ent) = torrent_rows.get(&tid) {
                    ent.uploaded.fetch_add(delta, Ordering::Relaxed);
                }
            }
        }
    }

    /// Recompute the *reported* speed for each torrent: held cap × per-call jitter, then gated by
    /// swarm presence (no leechers → no upload, no seeders → no download). Called every tick and
    /// after each event trigger; jitter rolls each call so consecutive ticks fluctuate naturally.
    pub fn recompute_with_torrent_rows(&self, torrent_rows: &DashMap<TorrentId, TorrentEntry>) {
        for r in self.torrents.iter() {
            let tid = *r.key();
            let (down, up) = match torrent_rows.get(&tid) {
                None => (0u64, 0u64),
                Some(row) => {
                    let seeders = r.seeders.load(Ordering::Relaxed);
                    let leechers = r.leechers.load(Ordering::Relaxed);
                    let phase = row.transfer_phase();

                    // Raw caps — gating happens once at the bottom of the loop.
                    let raw_down = if row.download_first && phase == TransferPhase::Downloading {
                        jittered(
                            r.download_cap.load(Ordering::Relaxed),
                            r.min_download_speed.load(Ordering::Relaxed),
                            r.max_download_speed.load(Ordering::Relaxed),
                        )
                    } else {
                        0
                    };

                    let raw_up = {
                        let cap = jittered(
                            r.upload_cap.load(Ordering::Relaxed),
                            r.min_upload_speed.load(Ordering::Relaxed),
                            r.max_upload_speed.load(Ordering::Relaxed),
                        );
                        // While leeching, scale upload by piece-availability:
                        // a real BT client can only upload pieces it already has,
                        // so reported upload ramps from ~0% → 100% as the download
                        // progresses. A 5% floor (once any byte is downloaded)
                        // avoids the dead zone where new leechers would report 0.
                        match phase {
                            TransferPhase::Seeding => cap,
                            TransferPhase::Downloading => {
                                let downloaded = row.downloaded();
                                let size = row.size;
                                if downloaded == 0 || size == 0 {
                                    0
                                } else {
                                    let progress =
                                        (downloaded as f64 / size as f64).clamp(0.0, 1.0);
                                    let scaled = (cap as f64 * progress) as u64;
                                    scaled.max((cap as f64 * 0.05) as u64)
                                }
                            }
                        }
                    };

                    // SINGLE SWARM-GATE POINT — every code path that materializes a speed
                    // value passes through here. No upload without leechers, no download
                    // without seeders. Adding new branches above is safe by construction.
                    let down = if seeders > 0 { raw_down } else { 0 };
                    let up = if leechers > 0 { raw_up } else { 0 };
                    (down, up)
                }
            };
            r.download_speed.store(down, Ordering::Relaxed);
            r.upload_speed.store(up, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swarm_caps_zero_when_no_seeders_or_leechers() {
        let d = SwarmSpeedDerivation::from_scrape(1000, 0, 0);
        assert_eq!(d.upload_cap, 0);
        assert_eq!(d.download_cap, 0);
        assert!(d.upload_blocked());
        assert!(d.download_blocked());
    }

    #[test]
    fn swarm_caps_full_when_peers_present() {
        let d = SwarmSpeedDerivation::from_scrape(1000, 5, 3);
        assert_eq!(d.upload_cap, 1000);
        assert_eq!(d.download_cap, 1000);
        assert!(!d.upload_blocked());
        assert!(!d.download_blocked());
    }

    #[test]
    fn swarm_upload_zero_with_only_seeders() {
        // No leechers means we cannot upload.
        let d = SwarmSpeedDerivation::from_scrape(1000, 8, 0);
        assert_eq!(d.upload_cap, 0);
        assert!(d.upload_blocked());
        assert_eq!(d.download_cap, 1000);
    }

    #[test]
    fn swarm_download_zero_with_only_leechers() {
        let d = SwarmSpeedDerivation::from_scrape(1000, 0, 8);
        assert_eq!(d.download_cap, 0);
        assert!(d.download_blocked());
        assert_eq!(d.upload_cap, 1000);
    }

    #[test]
    fn kb_to_bytes_per_sec_is_decimal() {
        assert_eq!(kb_to_bytes_per_sec(0), 0);
        assert_eq!(kb_to_bytes_per_sec(1), 1_000);
        assert_eq!(kb_to_bytes_per_sec(150), 150_000);
    }

    #[test]
    fn sample_speed_range_clamps_to_min_when_max_smaller() {
        // When max < min, the function treats max as min.
        let v = sample_speed_range(500, 100);
        assert_eq!(v, 500);
    }

    #[test]
    fn sample_speed_range_returns_min_when_equal() {
        assert_eq!(sample_speed_range(42, 42), 42);
    }

    #[test]
    fn jittered_zero_cap_returns_zero() {
        assert_eq!(jittered(0, 0, 1000), 0);
    }

    #[test]
    fn jittered_respects_clamp_when_bounds_tight() {
        // Cap at the ceiling: jitter direction up should still clamp to max.
        for _ in 0..200 {
            let v = jittered(1000, 1000, 1000);
            assert_eq!(v, 1000, "tight bounds must collapse to that value");
        }
    }

    #[test]
    fn jittered_changes_value_within_loose_bounds() {
        // With wide bounds, every roll must differ from the cap.
        for _ in 0..200 {
            let v = jittered(10_000, 0, 100_000);
            assert_ne!(v, 10_000, "jittered must differ from cap");
        }
    }

    #[test]
    fn jittered_clamps_to_min_and_max() {
        // Cap at the floor with no headroom below: clamp keeps result == min.
        for _ in 0..200 {
            let v = jittered(100, 100, 200);
            assert!((100..=200).contains(&v));
        }
    }
}
