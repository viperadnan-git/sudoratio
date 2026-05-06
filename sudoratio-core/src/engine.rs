//! Public-API impl block for [`Engine`]. The struct itself lives in [`crate::state`].

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use parking_lot::Mutex as PlMutex;
use tokio::sync::{Mutex, RwLock};

use crate::announce::public_ip::TrackerReportedIpCache;
use crate::bandwidth::BandwidthDispatcher;
use crate::config::EngineConfig;
use crate::error::SudoratioError;
use crate::profile;
use crate::scheduler;
use crate::state::{ClientProfileRecord, Engine, SeedingState};
use crate::torrent::{
    AnnounceEvent, AnnounceOutcome, AnnounceQueryOverrides, AnnounceTrace, ClientProfileId,
    ClientProfileSummary, HealthStatus, MetainfoTorrent, SeedingStatus, Torrent, TorrentEntry,
    TorrentId, TorrentMutable, TorrentState,
};

impl Engine {
    /// Construct an engine handle and start the always-on orchestrator loop.
    /// Returned `Arc<Engine>` is the cloneable handle.
    pub fn new(config: EngineConfig) -> Arc<Self> {
        let http = config.http_tracker.build_reqwest_client();
        let bandwidth = Arc::new(BandwidthDispatcher::new(config.bandwidth_tick_ms.max(1)));
        let announce_concurrency = if config.max_concurrent_announces > 0 {
            Some(Arc::new(tokio::sync::Semaphore::new(
                config.max_concurrent_announces.max(1),
            )))
        } else {
            None
        };
        let dial_cap = config.outbound_dial_max_concurrent_global.max(1);
        let inner = Arc::new(Self {
            config: arc_swap::ArcSwap::from_pointee(config),
            profiles: DashMap::new(),
            active_profile: RwLock::new(None),
            torrents: DashMap::new(),
            http,
            bandwidth,
            shutting_down: AtomicBool::new(false),
            seeding_state: PlMutex::new(SeedingState::default()),
            seeding_task: Mutex::new(None),
            announce_concurrency,
            announce_inflight: AtomicUsize::new(0),
            orchestrator_notify: Arc::new(tokio::sync::Notify::new()),
            identities: Default::default(),
            tracker_reported_ip: PlMutex::new(TrackerReportedIpCache::default()),
            announce_sink: PlMutex::new(None),
            state_change_notify: Arc::new(tokio::sync::Notify::new()),
            listening_port: AtomicU16::new(0),
            peer_listener_task: Mutex::new(None),
            dial_global_sem: Arc::new(tokio::sync::Semaphore::new(dial_cap)),
        });
        let loop_inner = inner.clone();
        let task = tokio::spawn(async move { scheduler::run_loop(loop_inner).await });
        if let Ok(mut g) = inner.seeding_task.try_lock() {
            *g = Some(task);
        }
        inner
    }

    /// Snapshot of the live engine config. Held by `Arc`; cheap to clone.
    pub fn current_config(&self) -> Arc<EngineConfig> {
        self.config.load_full()
    }

    /// Bound peer-listener port; `0` if not bound.
    pub fn peer_listener_port(&self) -> u16 {
        self.listening_port.load(Ordering::Relaxed)
    }

    /// Port the engine announces: `cfg.announce_port` → bound peer-port → [`crate::DEFAULT_ANNOUNCE_PORT`].
    pub fn resolved_announce_port(&self) -> u16 {
        if let Some(p) = self.config.load().announce_port {
            return p;
        }
        let bound = self.listening_port.load(Ordering::Relaxed);
        if bound != 0 {
            bound
        } else {
            crate::DEFAULT_ANNOUNCE_PORT
        }
    }

    /// Apply a new config atomically. The orchestrator and bandwidth simulator pick up new
    /// values on their next iteration; if `max_active_torrents` increased, idle torrents are
    /// promoted into freshly opened slots before this returns.
    ///
    /// Note: HTTP tracker client knobs (`HttpTrackerConfig.*`) are read at startup only.
    pub async fn update_config(&self, cfg: EngineConfig) {
        let prev = self.config.load();
        let prev_active_cap = prev.max_active_torrents;
        let speed_bounds_changed = prev.min_upload_speed != cfg.min_upload_speed
            || prev.max_upload_speed != cfg.max_upload_speed
            || prev.min_download_speed != cfg.min_download_speed
            || prev.max_download_speed != cfg.max_download_speed;
        let new_active_cap = cfg.max_active_torrents;
        let (min_dl, max_dl, min_ul, max_ul) = (
            cfg.min_download_speed,
            cfg.max_download_speed,
            cfg.min_upload_speed,
            cfg.max_upload_speed,
        );
        drop(prev);
        self.config.store(Arc::new(cfg));
        if speed_bounds_changed {
            // New min/max bounds take effect immediately for every registered torrent —
            // no waiting for the next tick or next announce.
            self.bandwidth
                .apply_bounds(min_dl, max_dl, min_ul, max_ul, &self.torrents);
        }
        self.orchestrator_notify.notify_one();
        if new_active_cap > prev_active_cap {
            scheduler::try_fill_slots(self).await;
        }
    }

    /// Register a **user** client doc. Resolves every `[[variant]]` into a profile id of
    /// `client@version`. Replaces any existing variants registered for the same client.
    /// Refuses if the doc's `client` name is bundled.
    pub async fn register_client(
        &self,
        toml_str: &str,
    ) -> Result<Vec<ClientProfileId>, SudoratioError> {
        self.insert_client_doc(toml_str, false)
    }

    /// Register a bundled (read-only) client doc. Server's startup seeder uses this.
    pub async fn register_builtin_client(
        &self,
        toml_str: &str,
    ) -> Result<Vec<ClientProfileId>, SudoratioError> {
        self.insert_client_doc(toml_str, true)
    }

    fn insert_client_doc(
        &self,
        toml_str: &str,
        bundled: bool,
    ) -> Result<Vec<ClientProfileId>, SudoratioError> {
        let doc = profile::parse_client_doc(toml_str)?;
        let client_name: Arc<str> = Arc::from(doc.client.as_str());
        let source: Arc<str> = Arc::from(toml_str);

        // For user docs whose client name matches a bundled client, we accept *only*
        // extension docs (no base fields, only `[[variant]]` blocks) — the variants are
        // resolved against the bundled doc's base. Full overrides are rejected.
        let bundled_overlap: Option<Arc<str>> = if !bundled {
            self.profiles
                .iter()
                .find(|r| r.value().client.as_ref() == client_name.as_ref() && r.value().bundled)
                .map(|r| r.value().source.clone())
        } else {
            None
        };

        let specs = if let Some(bundled_src) = bundled_overlap.as_ref() {
            if !doc.is_extension() {
                return Err(SudoratioError::ProfileImmutable(ClientProfileId(
                    doc.client.clone(),
                )));
            }
            let bundled_doc = profile::parse_client_doc(bundled_src)?;
            for v in &doc.variants {
                if bundled_doc
                    .variants
                    .iter()
                    .any(|bv| bv.version == v.version)
                {
                    return Err(SudoratioError::ClientProfileParse(format!(
                        "client {}: version {} already exists in bundled — use a different version",
                        doc.client, v.version
                    )));
                }
            }
            doc.resolve_with_base(&bundled_doc.base, bundled_doc.display_name.as_deref())?
        } else {
            doc.resolve()?
        };

        // Drop prior variants for this client at the same mutability layer (bundled→bundled,
        // user→user). Bundled and user variants of the same client coexist (extension model).
        let stale: Vec<ClientProfileId> = self
            .profiles
            .iter()
            .filter(|r| {
                r.value().client.as_ref() == client_name.as_ref() && r.value().bundled == bundled
            })
            .map(|r| r.key().clone())
            .collect();
        for id in &stale {
            self.profiles.remove(id);
        }

        let mut ids = Vec::with_capacity(specs.len());
        for spec in specs {
            let id = ClientProfileId(spec.id.clone());
            self.profiles.insert(
                id.clone(),
                Arc::new(ClientProfileRecord {
                    spec: Arc::new(spec),
                    client: client_name.clone(),
                    source: source.clone(),
                    bundled,
                }),
            );
            ids.push(id);
        }
        tracing::info!(
            client = %doc.client,
            variants = ids.len(),
            bundled,
            extension = bundled_overlap.is_some(),
            "registered client doc"
        );
        Ok(ids)
    }

    /// Remove every **user** variant of a client (extension docs and standalone user docs).
    /// Bundled variants stay in place. Errors if the client is bundled-only or unknown.
    /// If the active variant was user-owned for this client, it's cleared.
    pub async fn remove_client(&self, client: &str) -> Result<usize, SudoratioError> {
        let user_ids: Vec<ClientProfileId> = self
            .profiles
            .iter()
            .filter(|r| r.value().client.as_ref() == client && !r.value().bundled)
            .map(|r| r.key().clone())
            .collect();
        if user_ids.is_empty() {
            let exists = self
                .profiles
                .iter()
                .any(|r| r.value().client.as_ref() == client);
            return Err(if exists {
                SudoratioError::ProfileImmutable(ClientProfileId(client.to_string()))
            } else {
                SudoratioError::UnknownClientProfile(ClientProfileId(client.to_string()))
            });
        }
        let mut active = self.active_profile.write().await;
        let active_in_user = active.as_ref().is_some_and(|id| user_ids.contains(id));
        for id in &user_ids {
            self.profiles.remove(id);
        }
        if active_in_user {
            *active = None;
            self.clear_announce_identity_caches();
        }
        Ok(user_ids.len())
    }

    /// Raw client-doc TOML the variant was registered from. Returns `None` if unknown.
    pub fn profile_source(&self, id: &ClientProfileId) -> Option<String> {
        self.profiles.get(id).map(|r| r.source.to_string())
    }

    /// Raw client-doc TOML for a client family. Returns user (extension or standalone) doc
    /// if registered, otherwise the bundled doc.
    pub fn client_source(&self, client: &str) -> Option<String> {
        self.client_source_layer(client, /* prefer_user = */ true)
    }

    /// Layered source lookup. `prefer_user`: return user doc if it exists, else bundled.
    /// Pass `false` to explicitly fetch the bundled doc when both layers are registered.
    pub fn client_source_layer(&self, client: &str, prefer_user: bool) -> Option<String> {
        let mut user: Option<String> = None;
        let mut bundled: Option<String> = None;
        for r in self.profiles.iter() {
            if r.value().client.as_ref() != client {
                continue;
            }
            let s = r.value().source.to_string();
            if r.value().bundled {
                if bundled.is_none() {
                    bundled = Some(s);
                }
            } else if user.is_none() {
                user = Some(s);
            }
            if user.is_some() && bundled.is_some() {
                break;
            }
        }
        if prefer_user {
            user.or(bundled)
        } else {
            bundled.or(user)
        }
    }

    pub async fn set_active_profile(&self, id: ClientProfileId) -> Result<(), SudoratioError> {
        if !self.profiles.contains_key(&id) {
            return Err(SudoratioError::UnknownClientProfile(id));
        }
        let id_log = id.clone();
        *self.active_profile.write().await = Some(id);
        self.clear_announce_identity_caches();
        tracing::info!(profile_id = %id_log, "set active client profile");
        Ok(())
    }

    pub async fn current_active_profile(&self) -> Option<ClientProfileId> {
        self.active_profile.read().await.clone()
    }

    pub async fn list_profiles(&self) -> Vec<ClientProfileSummary> {
        let active = self.active_profile.read().await.clone();
        self.profiles
            .iter()
            .map(|r| {
                let id = r.key().clone();
                let rec = r.value();
                let spec = &rec.spec;
                let version = spec.version.clone().unwrap_or_default();
                ClientProfileSummary {
                    active: Some(&id) == active.as_ref(),
                    client: rec.client.to_string(),
                    version: version.clone(),
                    name: spec
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("{} {}", rec.client, version)),
                    editable: !rec.bundled,
                    id: id.0,
                }
            })
            .collect()
    }

    pub async fn add_torrent_metainfo(
        &self,
        meta: MetainfoTorrent,
    ) -> Result<TorrentId, SudoratioError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(SudoratioError::EngineShuttingDown);
        }
        let id = TorrentId(meta.info_hash_bytes);
        let name: Arc<str> = meta.name.into();
        let ih: Arc<str> = meta.info_hash.clone().into();
        let mut tiers = meta.trackers.tiers.clone();
        shuffle_tiers(&mut tiers);
        let primary = tiers.iter().flatten().next().cloned();
        let total_trackers: usize = tiers.iter().map(|t| t.len()).sum();
        let initial_left = if meta.download_before_seed && meta.size > 0 {
            meta.size
        } else {
            0
        };
        // Build the entry before claiming the shard: `entry()` holds a write-lock that would
        // self-deadlock if held across `next_queue_position` (which iterates the same map).
        let queue_position = scheduler::slots::next_queue_position(self);
        let entry = TorrentEntry {
            name: name.clone(),
            info_hash: Some(ih.clone()),
            info_hash_bytes: Some(meta.info_hash_bytes),
            tiers: parking_lot::RwLock::new(tiers),
            size: meta.size,
            download_first: meta.download_before_seed,
            uploaded: AtomicU64::new(0),
            state: PlMutex::new(TorrentMutable {
                left: initial_left,
                state: TorrentState::Queued,
                queue_position,
                tier_index: 0,
                intra_index: 0,
                consecutive_fails: 0,
                last_announce_interval_seconds: 5,
                last_min_interval_seconds: 0,
                last_successful_announce_unix_ms: 0,
                announce_count: 0,
                last_seeders: None,
                last_leechers: None,
                zero_leechers_since: None,
            }),
            dirty: AtomicBool::new(true),
        };
        match self.torrents.entry(id) {
            dashmap::Entry::Occupied(_) => {
                return Err(SudoratioError::TorrentAlreadyExists(meta.info_hash));
            }
            dashmap::Entry::Vacant(slot) => {
                slot.insert(entry);
            }
        }
        tracing::info!(
            torrent_id = %id,
            name = %name,
            info_hash = %ih,
            primary_tracker = ?primary,
            trackers = total_trackers,
            initial_left,
            download_before_seed = meta.download_before_seed,
            "added metainfo torrent"
        );
        scheduler::request_slot_for_torrent(self, id).await;
        Ok(id)
    }

    pub async fn announce_torrent(
        self: &Arc<Self>,
        torrent_id: TorrentId,
        event: AnnounceEvent,
    ) -> Result<AnnounceOutcome, SudoratioError> {
        self.announce_torrent_with_overrides(torrent_id, event, AnnounceQueryOverrides::default())
            .await
    }

    pub async fn announce_torrent_with_overrides(
        self: &Arc<Self>,
        torrent_id: TorrentId,
        event: AnnounceEvent,
        overrides: AnnounceQueryOverrides,
    ) -> Result<AnnounceOutcome, SudoratioError> {
        match self.torrents.get(&torrent_id) {
            None => return Err(SudoratioError::TorrentNotFound),
            Some(e) if !e.lifecycle().is_active() => {
                return Err(SudoratioError::TorrentNotActive);
            }
            _ => {}
        }
        let out = self
            .exec_tracker_announce(torrent_id, event, &overrides)
            .await?;
        self.post_announce_success(torrent_id, &out);
        Ok(out)
    }

    /// Post-announce-success bookkeeping shared by the orchestrator's announce arms and the
    /// public manual-announce API. Updates the bandwidth dispatcher's swarm counts (which
    /// repositions the cap inside its swarm-aware band) and dispatches the outbound BT dial
    /// burst against peers in the response.
    pub(crate) fn post_announce_success(self: &Arc<Self>, tid: TorrentId, out: &AnnounceOutcome) {
        self.bandwidth.update_torrent_peers(
            tid,
            out.seeders.unwrap_or(0) as i64,
            out.leechers.unwrap_or(0) as i64,
            &self.torrents,
        );
        crate::wire::spawn_dials(self.clone(), tid.0, out.peers.clone());
    }

    /// Bind the BT peer listener. Idempotent.
    pub async fn start_peer_listener(
        self: &Arc<Self>,
        bind_addr: SocketAddr,
    ) -> std::io::Result<u16> {
        let mut slot = self.peer_listener_task.lock().await;
        if slot.is_some() {
            return Ok(self.listening_port.load(Ordering::Relaxed));
        }
        let handle = crate::wire::spawn_peer_listener(self.clone(), bind_addr).await?;
        self.listening_port
            .store(handle.bound_port, Ordering::Relaxed);
        *slot = Some(handle.task);
        Ok(handle.bound_port)
    }

    /// peer_id for an inbound BT handshake — same per-info-hash slot as outbound announces.
    pub(crate) async fn resolve_inbound_peer_id(&self, info_hash: &[u8; 20]) -> Option<String> {
        let active = self.active_profile.read().await.clone()?;
        let record = self.profiles.get(&active)?;
        let spec = record.spec.clone();
        drop(record);
        self.resolve_announce_peer_id(&spec, info_hash, AnnounceEvent::None)
            .ok()
    }

    /// Stop orchestrator, drain inflight, send parallel `stopped`, abort peer listener.
    pub async fn shutdown(self: &Arc<Self>) {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(t) = self.peer_listener_task.lock().await.take() {
            t.abort();
        }
        let active_ids: Vec<TorrentId> = self
            .torrents
            .iter()
            .filter(|r| r.lifecycle().is_active())
            .map(|r| *r.key())
            .collect();
        {
            let mut st = self.seeding_state.lock();
            st.delay = scheduler::delay_queue::DelayQueue::default();
        }
        self.orchestrator_notify.notify_one();
        if let Some(h) = self.seeding_task.lock().await.take() {
            let _ = h.await;
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        while self.announce_inflight.load(Ordering::Relaxed) > 0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        // Fire `stopped` for every active torrent in parallel; real clients don't space them out.
        let mut set = tokio::task::JoinSet::new();
        for tid in active_ids {
            let inner = self.clone();
            set.spawn(async move {
                let _ = inner
                    .exec_tracker_announce(
                        tid,
                        AnnounceEvent::Stopped,
                        &AnnounceQueryOverrides::default(),
                    )
                    .await;
                inner.bandwidth.unregister_torrent(tid, &inner.torrents);
            });
        }
        while set.join_next().await.is_some() {}
    }

    pub async fn stats(&self) -> SeedingStatus {
        let mut active = 0usize;
        let mut waiting = 0usize;
        let mut metainfo_count = 0usize;
        for r in self.torrents.iter() {
            metainfo_count += 1;
            match r.lifecycle() {
                TorrentState::Downloading | TorrentState::Seeding => active += 1,
                TorrentState::Queued => waiting += 1,
                TorrentState::Stopped(_) => {}
            }
        }
        SeedingStatus {
            running: !self.shutting_down.load(Ordering::SeqCst),
            upload_speed: self.bandwidth.total_upload_speed(),
            download_speed: self.bandwidth.global_download_speed(),
            max_active_torrents: self.config.load().max_active_torrents,
            active_torrents: active,
            waiting_torrents: waiting,
            tracked_metainfo_torrents: metainfo_count,
        }
    }

    pub async fn get_torrent(&self, id: TorrentId) -> Option<Torrent> {
        self.torrents
            .get(&id)
            .map(|r| self.torrent_snapshot(*r.key(), r.value()))
    }

    pub async fn list_torrents(&self) -> Vec<Torrent> {
        let mut v: Vec<Torrent> = self
            .torrents
            .iter()
            .map(|r| {
                let id = *r.key();
                self.torrent_snapshot(id, r.value())
            })
            .collect();
        v.sort_by_key(|t| t.id);
        v
    }

    /// Per-torrent pause: leave registered, free a slot, send `stopped` if it was active.
    pub async fn pause_torrent(&self, id: TorrentId) -> Result<(), SudoratioError> {
        scheduler::user_stop_torrent(self, id).await
    }

    /// Resume after [`Self::pause_torrent`]; re-enters the wait queue or starts immediately if a slot is free.
    pub async fn resume_torrent(&self, id: TorrentId) -> Result<(), SudoratioError> {
        scheduler::user_resume_torrent(self, id).await
    }

    /// Remove the torrent from the engine (and session export); `stopped` if it had contacted the tracker.
    pub async fn remove_torrent(&self, id: TorrentId) -> Result<(), SudoratioError> {
        scheduler::remove_torrent(self, id).await
    }

    /// Subscribe to the engine's announce trace stream. Each successful or failed announce produces
    /// one trace; the embedder is responsible for persisting/forwarding them. Replaces any previous
    /// sink. Drop the returned receiver to stop receiving traces.
    pub fn subscribe_announces(
        &self,
    ) -> tokio::sync::mpsc::UnboundedReceiver<(TorrentId, AnnounceTrace)> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        *self.announce_sink.lock() = Some(tx);
        rx
    }

    pub fn list_torrent_ids(&self) -> Vec<TorrentId> {
        self.torrents.iter().map(|r| *r.key()).collect()
    }

    /// Decode a 40-hex info-hash into the canonical [`TorrentId`].
    pub fn torrent_id_by_info_hash(&self, info_hash_hex: &str) -> Option<TorrentId> {
        TorrentId::from_hex(info_hash_hex).filter(|id| self.torrents.contains_key(id))
    }

    /// IDs of torrents whose state changed since the last call. Clears the dirty flag.
    pub fn take_dirty_ids(&self) -> Vec<TorrentId> {
        self.torrents
            .iter()
            .filter_map(|r| r.value().take_dirty().then(|| *r.key()))
            .collect()
    }

    /// Clear the dirty flag for one torrent (idempotent).
    pub fn clear_dirty(&self, id: TorrentId) {
        if let Some(e) = self.torrents.get(&id) {
            e.take_dirty();
        }
    }

    /// Restore one torrent (e.g. from SQLite session). Skips if already present.
    pub async fn restore_torrent(&self, b: Torrent) -> Result<(), SudoratioError> {
        let slot = match self.torrents.entry(b.id) {
            dashmap::Entry::Occupied(_) => return Ok(()),
            dashmap::Entry::Vacant(slot) => slot,
        };
        let r = b.runtime.clone();
        let name: Arc<str> = b.name.into();
        let ih = b.info_hash.as_ref().map(|s| Arc::from(s.as_str()));
        let entry = TorrentEntry {
            name,
            info_hash: ih.clone(),
            info_hash_bytes: r.info_hash_bytes,
            tiers: parking_lot::RwLock::new({
                let mut t = b.trackers.tiers.clone();
                shuffle_tiers(&mut t);
                t
            }),
            size: b.size.unwrap_or(0),
            download_first: b.download_before_seed,
            uploaded: AtomicU64::new(b.uploaded.unwrap_or(0)),
            state: PlMutex::new(TorrentMutable {
                left: b.left.unwrap_or(0),
                state: match b.state {
                    TorrentState::Stopped(reason) => TorrentState::Stopped(reason),
                    _ => TorrentState::Queued,
                },
                queue_position: b.queue_position,
                tier_index: r.tier_index,
                intra_index: r.intra_index,
                consecutive_fails: r.consecutive_fails,
                last_announce_interval_seconds: r.last_announce_interval_seconds.max(1),
                last_min_interval_seconds: r.last_min_interval_seconds,
                last_successful_announce_unix_ms: r.last_successful_announce_unix_ms,
                announce_count: r.announce_count,
                last_seeders: b.seeders,
                last_leechers: b.leechers,
                zero_leechers_since: None,
            }),
            dirty: AtomicBool::new(false),
        };
        slot.insert(entry);
        Ok(())
    }

    pub const RESTORE_STARTED_JITTER_SECS: u64 = 30;

    /// Finalize a batch of [`Self::restore_torrent`] calls: jittered slot promotion.
    /// Bandwidth registration happens later, on the post-Started handler.
    pub async fn finish_restore(&self, jitter_max_secs: u64) {
        scheduler::try_fill_slots_with_jitter(self, jitter_max_secs).await;
    }

    pub fn health(&self) -> HealthStatus {
        tracing::debug!("health check");
        HealthStatus {
            ok: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub async fn update_torrent_transfer(
        &self,
        id: TorrentId,
        downloaded: Option<u64>,
        left: Option<u64>,
        uploaded: Option<u64>,
    ) -> Result<(), SudoratioError> {
        let Some(ent) = self.torrents.get(&id) else {
            return Err(SudoratioError::TorrentNotFound);
        };
        let size = ent.size;
        let download_first = ent.download_first;
        // PATCH operates on `left`; `downloaded` is accepted as alias (`left` wins on conflict).
        let new_left = match (left, downloaded) {
            (Some(l), _) => Some(l.min(size)),
            (None, Some(d)) if download_first => Some(size.saturating_sub(d.min(size))),
            _ => None,
        };
        let just_completed = ent.with_state(|s| {
            let was_left = s.left;
            if let Some(l) = new_left {
                s.left = l;
            }
            download_first && was_left > 0 && s.left == 0
        });
        if let Some(u) = uploaded {
            ent.store_uploaded(u);
        }
        if just_completed && ent.lifecycle() == TorrentState::Downloading {
            ent.set_lifecycle(TorrentState::Seeding);
        }
        drop(ent);
        if just_completed {
            // Mirror the simulator-completion path: fire `completed`, reroll the cap.
            {
                let mut st = self.seeding_state.lock();
                st.delay
                    .add_or_replace(id, AnnounceEvent::Completed, std::time::Duration::ZERO);
            }
            self.orchestrator_notify.notify_one();
            self.bandwidth.resample_torrent_cap(id, &self.torrents);
        }
        if !self.has_reached_upload_ratio_limit(id) {
            if let Some(ent) = self.torrents.get(&id) {
                if ent.lifecycle() == TorrentState::Stopped(crate::torrent::StopReason::UploadRatio)
                {
                    ent.set_lifecycle(TorrentState::Queued);
                }
            }
        }
        scheduler::request_slot_for_torrent(self, id).await;
        Ok(())
    }
}

/// Shuffle each BEP-12 tier in place (per-session randomization).
fn shuffle_tiers(tiers: &mut [Vec<String>]) {
    use rand::seq::SliceRandom;
    let mut rng = rand::rng();
    for tier in tiers.iter_mut() {
        tier.shuffle(&mut rng);
    }
}
