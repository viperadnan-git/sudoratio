//! In-memory core state: torrent rows, profiles, HTTP client, bandwidth, seeding bookkeeping.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicUsize};
use std::sync::Arc;

use arc_swap::ArcSwap;
use dashmap::DashMap;
use parking_lot::Mutex;
use tokio::sync::{mpsc, Mutex as AsyncMutex, Notify, RwLock, Semaphore};

use crate::announce::identity::{
    resolve_with_policy, AnnounceIdentities, Policy, PERSISTENT_KEY_IDLE_TTL,
};
use crate::announce::public_ip::TrackerReportedIpCache;
use crate::bandwidth::BandwidthDispatcher;
use crate::config::EngineConfig;
use crate::profile::{
    apply_key_case, generate_key_material, generate_peer_id_once, ClientProfileSpec,
};
use crate::scheduler::delay_queue::DelayQueue;
use crate::torrent::TorrentEntry;
use crate::torrent::{
    AnnounceEvent, AnnounceTrace, ClientProfileId, Torrent, TorrentId, TorrentRuntime,
    TorrentState, TrackersHttp,
};
use crate::SudoratioError;

pub(crate) struct ClientProfileRecord {
    pub(crate) spec: Arc<ClientProfileSpec>,
    /// Client family name (e.g. `qbittorrent`); same across all variants from one doc.
    pub(crate) client: Arc<str>,
    /// Raw TOML of the client doc the variant was registered from. Shared (Arc-cloned) by
    /// every variant of the same doc.
    pub(crate) source: Arc<str>,
    /// Bundled clients ship inside the binary and cannot be edited or deleted.
    pub(crate) bundled: bool,
}

pub struct Engine {
    pub(crate) config: ArcSwap<EngineConfig>,
    pub(crate) profiles: DashMap<ClientProfileId, Arc<ClientProfileRecord>>,
    pub(crate) active_profile: RwLock<Option<ClientProfileId>>,
    pub(crate) torrents: DashMap<TorrentId, TorrentEntry>,
    pub(crate) http: reqwest::Client,
    pub(crate) bandwidth: Arc<BandwidthDispatcher>,
    /// Set once on shutdown so the orchestrator loop can break out cleanly.
    pub(crate) shutting_down: AtomicBool,
    pub(crate) seeding_state: Mutex<SeedingState>,
    pub(crate) seeding_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    /// Limits concurrent tracker HTTP announces; `None` = unlimited.
    pub(crate) announce_concurrency: Option<Arc<Semaphore>>,
    pub(crate) announce_inflight: AtomicUsize,
    /// Wakes the seeding orchestrator when the announce delay queue gains or advances a deadline.
    pub(crate) orchestrator_notify: Arc<Notify>,
    /// All `peer_id` and tracker `key` caches; honour `RefreshOnPolicy` per profile.
    pub(crate) identities: AnnounceIdentities,
    /// Resolved public IP for announce `{ip}` / `{ipv6}` placeholders (HTTP providers + cache).
    pub(crate) tracker_reported_ip: Mutex<TrackerReportedIpCache>,
    /// Optional fan-out for announce traces (set by the embedder to persist or stream history).
    /// `None` means traces are dropped after the engine reads what it needs from them.
    pub(crate) announce_sink: Mutex<Option<mpsc::UnboundedSender<(TorrentId, AnnounceTrace)>>>,
    /// Doorbell for engine-driven state transitions (auto-pause, sim-download completion).
    /// Consumer pattern: `engine.state_change_notify.notified().await` then drain via `take_dirty_ids`.
    pub state_change_notify: Arc<Notify>,
    /// BT peer-listener bound port; `0` = not running.
    pub(crate) listening_port: AtomicU16,
    pub(crate) peer_listener_task: AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
    pub(crate) dial_global_sem: Arc<Semaphore>,
}

#[derive(Default)]
pub(crate) struct SeedingState {
    pub delay: DelayQueue,
}

impl Engine {
    /// Public IP for announce query placeholders, resolved when the active profile's `query`
    /// contains `{ip}` or `{ipv6}`.
    pub(crate) async fn resolve_tracker_reported_ip_for_query(
        &self,
        query: &str,
    ) -> Option<std::net::IpAddr> {
        crate::announce::public_ip::resolve_tracker_reported_ip(
            &self.http,
            &self.tracker_reported_ip,
            query,
        )
        .await
    }

    /// Drop all cached `peer_id` / `key` material; call when switching the active profile.
    pub(crate) fn clear_announce_identity_caches(&self) {
        self.identities.clear();
    }

    /// Resolve `peer_id` honouring the profile's `peer_id_generator.refresh_on` policy.
    pub(crate) fn resolve_announce_peer_id(
        &self,
        client: &ClientProfileSpec,
        info_hash: &[u8; 20],
        event: AnnounceEvent,
    ) -> Result<String, SudoratioError> {
        let pool = &self.identities.peer_random_pool;
        let alg = &client.peer_id_generator.algorithm;
        resolve_with_policy(
            &self.identities.peer_id,
            &Policy {
                policy: client.peer_id_generator.refresh_on,
                refresh_every_secs: client.peer_id_generator.refresh_every_secs,
                persistent_idle_ttl: None,
            },
            info_hash,
            event,
            "peerIdGenerator",
            || generate_peer_id_once(alg, pool),
        )
    }

    /// Resolve tracker `key` honoring `.client` `keyGenerator.refreshOn`.
    pub(crate) fn resolve_announce_key(
        &self,
        client: &ClientProfileSpec,
        info_hash: &[u8; 20],
        event: AnnounceEvent,
    ) -> Result<Option<String>, SudoratioError> {
        let Some(kg) = client.key_generator.as_ref() else {
            return Ok(None);
        };
        let alg = &kg.algorithm;
        let key_case = kg.key_case.clone();
        resolve_with_policy(
            &self.identities.key,
            &Policy {
                policy: kg.refresh_on,
                refresh_every_secs: kg.refresh_every_secs,
                persistent_idle_ttl: Some(PERSISTENT_KEY_IDLE_TTL),
            },
            info_hash,
            event,
            "keyGenerator",
            || {
                let raw = generate_key_material(alg)?;
                Ok(apply_key_case(&raw, key_case.as_deref()))
            },
        )
        .map(Some)
    }

    pub(crate) fn torrent_uploaded(&self, tid: TorrentId) -> u64 {
        self.torrents.get(&tid).map(|e| e.uploaded()).unwrap_or(0)
    }

    pub(crate) fn emit_state_change(&self, tid: TorrentId) {
        let _ = tid;
        self.state_change_notify.notify_one();
    }

    /// Forward an announce trace to the embedder (e.g. SQLite writer).
    pub(crate) fn emit_announce_trace(&self, tid: TorrentId, trace: AnnounceTrace) {
        if let Some(tx) = self.announce_sink.lock().as_ref() {
            if tx.send((tid, trace)).is_err() {
                tracing::warn!(?tid, "announce trace dropped: subscriber closed");
            }
        }
    }

    pub(crate) fn torrent_snapshot(&self, id: TorrentId, e: &TorrentEntry) -> Torrent {
        let snap = e.snapshot();
        let uploaded_total = e.uploaded();
        let n = snap.announce_count;
        Torrent {
            id,
            info_hash: e.info_hash.as_ref().map(|s| s.to_string()),
            name: e.name.to_string(),
            size: Some(e.size),
            downloaded: Some(e.downloaded()),
            uploaded: Some(uploaded_total),
            left: Some(snap.left),
            download_speed: Some(self.bandwidth.download_speed_for(id)),
            upload_speed: Some(self.bandwidth.upload_speed_for(id)),
            seeders: snap.last_seeders,
            leechers: snap.last_leechers,
            state: snap.state,
            reason: snap.state.reason(),
            download_before_seed: e.download_first,
            trackers: TrackersHttp {
                tiers: e.tiers.read().clone(),
            },
            announce_interval: (n > 0).then_some(snap.last_announce_interval_seconds),
            min_announce_interval: (n > 0 && snap.last_min_interval_seconds > 0)
                .then_some(snap.last_min_interval_seconds),
            last_announced_at: (snap.last_successful_announce_unix_ms > 0)
                .then_some(snap.last_successful_announce_unix_ms),
            queue_position: snap.queue_position,
            runtime: TorrentRuntime {
                info_hash_bytes: e.info_hash_bytes,
                tier_index: snap.tier_index,
                intra_index: snap.intra_index,
                announce_count: snap.announce_count,
                last_announce_interval_seconds: snap.last_announce_interval_seconds,
                last_min_interval_seconds: snap.last_min_interval_seconds,
                last_successful_announce_unix_ms: snap.last_successful_announce_unix_ms,
                consecutive_fails: snap.consecutive_fails,
            },
        }
    }

    pub fn export_torrent(&self, id: TorrentId) -> Option<Torrent> {
        self.torrents
            .get(&id)
            .map(|e| self.torrent_snapshot(id, e.value()))
    }

    /// Clears tracker identity caches keyed by this torrent’s info hash (after remove).
    pub(crate) fn forget_torrent_announce_keys(&self, info_hash_bytes: Option<[u8; 20]>) {
        if let Some(ih) = info_hash_bytes {
            self.identities.forget_info_hash(&ih);
        }
    }

    /// Bandwidth tick: peer-aware speeds, upload accumulation, simulated download bytes, download→seed transition.
    pub(crate) async fn transfer_tick(&self) {
        self.bandwidth.tick_with_torrent_rows(&self.torrents);
        let tick_ms = self.bandwidth.tick_ms().max(1);
        let mut completed: Vec<TorrentId> = Vec::new();
        for (tid, dps) in self.bandwidth.download_speed_pairs() {
            let Some(row) = self.torrents.get(&tid) else {
                continue;
            };
            if !row.download_first {
                continue;
            }
            let delta = dps.saturating_mul(tick_ms) / 1000;
            if delta == 0 {
                continue;
            }
            // Detect the `left>0 → 0` edge; phase is derived elsewhere.
            let just_completed = row.with_state(|s| {
                if s.left == 0 {
                    return false;
                }
                let applied = delta.min(s.left);
                s.left = s.left.saturating_sub(applied);
                s.left == 0
            });
            if just_completed {
                if row.lifecycle() == TorrentState::Downloading {
                    row.set_lifecycle(TorrentState::Seeding);
                }
                completed.push(tid);
            }
        }
        if !completed.is_empty() {
            let mut st = self.seeding_state.lock();
            for tid in &completed {
                st.delay
                    .add_or_replace(*tid, AnnounceEvent::Completed, std::time::Duration::ZERO);
            }
            drop(st);
            self.orchestrator_notify.notify_one();
            self.bandwidth
                .resample_many(completed.iter().copied(), &self.torrents);
            for tid in &completed {
                self.emit_state_change(*tid);
            }
        }

        // Catch ratio overshoot or grace expiry within one bandwidth tick.
        let active: Vec<TorrentId> = self
            .torrents
            .iter()
            .filter(|r| r.lifecycle().is_active())
            .map(|r| *r.key())
            .collect();
        for tid in active {
            if crate::scheduler::should_pause_no_leechers(self, tid) {
                crate::scheduler::auto_pause(self, tid, crate::torrent::StopReason::NoLeechers)
                    .await;
                continue;
            }
            if self.has_reached_upload_ratio_limit(tid) {
                crate::scheduler::auto_pause(self, tid, crate::torrent::StopReason::UploadRatio)
                    .await;
            }
        }
    }
}
