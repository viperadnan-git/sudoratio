//! Public-API impl block for [`Engine`]. The struct itself lives in [`crate::state`].

use std::net::SocketAddr;
use std::path::PathBuf;
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
use crate::persistence::Persistence;
use crate::preset::{
    Preset, PresetError, PresetPolicy, PresetPolicyUpdate, PresetRegistry, PresetRollup,
};
use crate::profile;
use crate::scheduler;
use crate::state::{ClientProfileRecord, Engine, SeedingState};
use crate::torrent::{
    AnnounceEvent, AnnounceOutcome, AnnounceQueryOverrides, AnnounceTrace, ClientProfileId,
    ClientProfileSummary, HealthStatus, MetainfoTorrent, SeedingStatus, Torrent, TorrentEntry,
    TorrentId, TorrentMutable, TorrentState,
};

impl Engine {
    /// Open or create persistence at `data_dir/session.sqlite3`, restore presets and
    /// torrents, register bundled + user client profiles, and start the orchestrator.
    /// `data_dir = None` returns an in-memory-only engine (tests).
    pub async fn new(
        config: EngineConfig,
        data_dir: Option<PathBuf>,
    ) -> anyhow::Result<Arc<Self>> {
        let db = match data_dir.as_ref() {
            Some(dir) => {
                std::fs::create_dir_all(dir)?;
                tokio::fs::create_dir_all(dir.join(crate::profile_store::SUBDIR)).await?;
                let p = dir.join("session.sqlite3");
                let p_clone = p.clone();
                let db =
                    tokio::task::spawn_blocking(move || Persistence::open(p_clone.as_path()))
                        .await??;
                tracing::info!(path = %p.display(), "session sqlite ready");
                db
            }
            None => Persistence::open_in_memory()?,
        };
        let inner = build_engine(config, db, data_dir)?;
        inner.bootstrap().await?;
        let loop_inner = inner.clone();
        let task = tokio::spawn(async move { scheduler::run_loop(loop_inner).await });
        if let Ok(mut g) = inner.seeding_task.try_lock() {
            *g = Some(task);
        }
        Ok(inner)
    }

    /// Pure in-memory engine. No persistence; suitable for tests.
    pub fn new_in_memory(config: EngineConfig) -> Arc<Self> {
        let db = Persistence::open_in_memory().expect(":memory: SQLite open");
        let inner = build_engine(config, db, None).expect("in-memory engine bootstrap");
        inner.presets.ensure_default();
        let snap = inner.presets.list()[0].snapshot();
        let _ = inner.db.save_preset(&snap);
        let loop_inner = inner.clone();
        let task = tokio::spawn(async move { scheduler::run_loop(loop_inner).await });
        if let Ok(mut g) = inner.seeding_task.try_lock() {
            *g = Some(task);
        }
        inner
    }

    /// Restore presets + torrents from SQLite, register bundled and user client docs.
    async fn bootstrap(self: &Arc<Self>) -> anyhow::Result<()> {
        let presets = {
            let db = self.db.clone();
            tokio::task::spawn_blocking(move || db.read_presets()).await??
        };
        if presets.is_empty() {
            self.presets.ensure_default();
            let snap = self.presets.list()[0].snapshot();
            let db = self.db.clone();
            tokio::task::spawn_blocking(move || db.save_preset(&snap)).await??;
        } else {
            for p in presets {
                self.presets.insert(crate::preset::Preset::from_snapshot(p));
            }
            self.presets.ensure_default();
        }

        for (client, toml) in profile::BUNDLED_CLIENTS {
            if let Err(e) = self.register_builtin_client(toml).await {
                tracing::warn!(client = %client, error = %e, "bundled client load failed");
            }
        }
        if let Some(dir) = self.data_dir.as_ref() {
            match crate::profile_store::read_all(dir).await {
                Ok(items) => {
                    for (path, toml) in items {
                        if let Err(e) = self.register_client(&toml).await {
                            tracing::warn!(path = %path.display(), error = %e, "skipping invalid user client doc");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "failed to scan user client profiles"),
            }
        }
        if let Err(e) = self.activate_default_profile_or_first().await {
            tracing::warn!(error = %e, "no client profile activated");
        }

        let torrents = {
            let db = self.db.clone();
            tokio::task::spawn_blocking(move || db.read_torrents()).await??
        };
        for t in torrents {
            let preset_id = t.preset_id.clone();
            if let Err(e) = self.restore_torrent_with_preset(t, Some(&preset_id)).await {
                tracing::warn!(error = %e, "restore_torrent skipped");
            }
        }
        Ok(())
    }

    /// Try `deluge@2.2.0` first, then any registered profile.
    async fn activate_default_profile_or_first(&self) -> anyhow::Result<()> {
        let pref = ClientProfileId::from("deluge@2.2.0");
        if self.set_active_profile(pref).await.is_ok() {
            return Ok(());
        }
        let rows = self.list_profiles().await;
        let Some(first) = rows.first() else {
            anyhow::bail!("no client profile available to activate");
        };
        self.set_active_profile(ClientProfileId::from(first.id.as_str()))
            .await
            .map_err(|e| anyhow::anyhow!("set active profile: {e}"))?;
        Ok(())
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

    /// Apply a new infra config and persist `config.json`. Per-preset policy lives separately.
    pub async fn update_config(&self, cfg: EngineConfig) -> anyhow::Result<()> {
        self.config.store(Arc::new(cfg.clone()));
        self.orchestrator_notify.notify_one();
        if let Some(dir) = self.data_dir.clone() {
            tokio::task::spawn_blocking(move || {
                crate::config_io::save(&dir.join("config.json"), &cfg)
            })
            .await??;
        }
        Ok(())
    }

    /// Snapshot list of all presets (sorted: default first, then by id).
    pub fn list_presets(&self) -> Vec<Arc<Preset>> {
        self.presets.list()
    }

    pub fn get_preset(&self, id: &str) -> Option<Arc<Preset>> {
        self.presets.get(id)
    }

    pub async fn create_preset(
        &self,
        id: Option<String>,
        name: String,
        color: String,
        policy: PresetPolicy,
    ) -> Result<Arc<Preset>, PresetError> {
        let preset = self.presets.create(id, name, color, policy)?;
        self.persist_preset(&preset);
        Ok(preset)
    }

    pub async fn rename_preset(
        &self,
        id: &str,
        name: Option<String>,
        color: Option<String>,
    ) -> Result<Arc<Preset>, PresetError> {
        let preset = self.presets.update_name_color(id, name, color)?;
        self.persist_preset(&preset);
        Ok(preset)
    }

    /// Fire-and-forget DB write on the blocking pool. Logs and swallows errors —
    /// safe because every state change has an in-memory source-of-truth.
    fn spawn_db<F>(&self, label: &'static str, f: F)
    where
        F: FnOnce(&Persistence) -> anyhow::Result<()> + Send + 'static,
    {
        let db = self.db.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = f(&db) {
                tracing::warn!(error = %e, op = label, "db write failed");
            }
        });
    }

    fn persist_preset(&self, preset: &Arc<Preset>) {
        let snap = preset.snapshot();
        self.spawn_db("save_preset", move |db| db.save_preset(&snap));
    }

    fn persist_torrent(&self, tid: TorrentId) {
        let Some(t) = self.export_torrent(tid) else {
            return;
        };
        self.clear_dirty(tid);
        self.spawn_db("save_torrent", move |db| db.save_torrent(&t));
    }

    /// Live-apply a policy patch. All torrents using this preset see the new caps on next
    /// bandwidth tick; if upload/download bounds changed, the bandwidth dispatcher is also
    /// re-synced for those torrents immediately. If `max_active_torrents` decreased, the
    /// scheduler will demote excess; if it increased, queued torrents may promote.
    pub async fn update_preset_policy(
        &self,
        id: &str,
        patch: PresetPolicyUpdate,
    ) -> Result<Arc<Preset>, PresetError> {
        let prev_policy = self
            .presets
            .get(id)
            .map(|p| (*p.policy.load_full()).clone());
        let preset = self.presets.apply_policy(id, patch)?;
        let new_policy = (*preset.policy.load_full()).clone();
        let bounds_changed = prev_policy.as_ref().is_some_and(|p| {
            p.min_upload_speed != new_policy.min_upload_speed
                || p.max_upload_speed != new_policy.max_upload_speed
                || p.min_download_speed != new_policy.min_download_speed
                || p.max_download_speed != new_policy.max_download_speed
        });
        let profile_changed = prev_policy
            .as_ref()
            .map(|p| p.client_profile_id != new_policy.client_profile_id)
            .unwrap_or(false);
        if bounds_changed {
            let ids: Vec<TorrentId> = self
                .torrents
                .iter()
                .filter(|r| r.value().preset_id() == id)
                .map(|r| *r.key())
                .collect();
            for tid in ids {
                self.bandwidth.sync_torrent_to_policy(tid, &self.torrents);
            }
        }
        if profile_changed {
            self.forget_preset_identities(id);
        }
        self.orchestrator_notify.notify_one();
        scheduler::try_fill_slots(self).await;
        self.persist_preset(&preset);
        Ok(preset)
    }

    /// Delete a non-default preset. Torrents previously assigned to it are reassigned to
    /// `reassign_to` (or to `default` when `reassign_to` is `None`).
    pub async fn delete_preset(
        &self,
        id: &str,
        reassign_to: Option<&str>,
    ) -> Result<(), PresetError> {
        if id == crate::preset::DEFAULT_PRESET_ID {
            return Err(PresetError::DeleteDefault);
        }
        let target_id = reassign_to.unwrap_or(crate::preset::DEFAULT_PRESET_ID);
        if self.presets.get(target_id).is_none() {
            return Err(PresetError::NotFound(target_id.into()));
        }
        let affected: Vec<TorrentId> = self
            .torrents
            .iter()
            .filter(|r| r.value().preset_id() == id)
            .map(|r| *r.key())
            .collect();
        for tid in &affected {
            let _ = scheduler::slots::move_torrent_to_preset(self, *tid, target_id).await;
            self.persist_torrent(*tid);
        }
        self.presets.remove(id)?;
        let id_owned = id.to_string();
        self.spawn_db("delete_preset", move |db| db.delete_preset(&id_owned));
        Ok(())
    }

    /// Move a single torrent to a different preset.
    pub async fn move_torrent_to_preset(
        &self,
        tid: TorrentId,
        new_preset_id: &str,
    ) -> Result<(), SudoratioError> {
        scheduler::slots::move_torrent_to_preset(self, tid, new_preset_id).await?;
        self.persist_torrent(tid);
        Ok(())
    }

    /// Register a **user** client doc. Resolves every `[[variant]]` into a profile id of
    /// `client@version`. Replaces any existing variants registered for the same client.
    /// Refuses if the doc's `client` name is bundled. Does NOT persist to disk; use
    /// [`Self::register_user_client_doc`] for the API write-through.
    pub async fn register_client(
        &self,
        toml_str: &str,
    ) -> Result<Vec<ClientProfileId>, SudoratioError> {
        self.insert_client_doc(toml_str, false)
    }

    /// Register a user client doc AND persist it to `<data_dir>/clients/<client>.toml`.
    pub async fn register_user_client_doc(
        &self,
        toml_str: &str,
    ) -> anyhow::Result<Vec<ClientProfileId>> {
        let doc = profile::parse_client_doc(toml_str)
            .map_err(|e| anyhow::anyhow!("parse client doc: {e}"))?;
        let client = doc.client.clone();
        let ids = self
            .insert_client_doc(toml_str, false)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(dir) = self.data_dir.clone() {
            crate::profile_store::save(&dir, &client, toml_str).await?;
        }
        Ok(ids)
    }

    /// Remove a user client doc AND delete the on-disk file. Cascade-resets every preset
    /// whose `client_profile_id` pointed to a variant of this client family back to
    /// `None` (engine default) — Option A.
    pub async fn remove_user_client_doc(&self, client: &str) -> anyhow::Result<usize> {
        // Snapshot variant ids being removed BEFORE removing them, so we can find
        // presets pointing to any of them.
        let removed_variant_ids: std::collections::HashSet<String> = self
            .profiles
            .iter()
            .filter(|r| r.value().client.as_ref() == client && !r.value().bundled)
            .map(|r| r.key().0.clone())
            .collect();

        let removed = self
            .remove_client(client)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(dir) = self.data_dir.clone() {
            if let Err(e) = crate::profile_store::delete(&dir, client).await {
                tracing::warn!(error = %e, "client file delete failed");
            }
        }

        if !removed_variant_ids.is_empty() {
            self.cascade_reset_preset_profiles(&removed_variant_ids).await;
        }
        Ok(removed)
    }

    /// Reset every preset whose `client_profile_id` is in `dead_ids` back to `None`.
    /// Persists each touched preset and invalidates the matching torrents' identity caches.
    async fn cascade_reset_preset_profiles(
        &self,
        dead_ids: &std::collections::HashSet<String>,
    ) {
        let affected: Vec<String> = self
            .presets
            .list()
            .into_iter()
            .filter(|p| {
                p.policy
                    .load()
                    .client_profile_id
                    .as_deref()
                    .is_some_and(|id| dead_ids.contains(id))
            })
            .map(|p| p.id.clone())
            .collect();
        for preset_id in affected {
            let _ = self
                .presets
                .apply_policy(
                    &preset_id,
                    PresetPolicyUpdate {
                        client_profile_id: Some(None),
                        ..Default::default()
                    },
                )
                .map(|preset| {
                    self.persist_preset(&preset);
                    self.forget_preset_identities(&preset_id);
                });
        }
    }

    /// Read announce trace history (newest-first) for a torrent.
    pub fn read_announces(
        &self,
        info_hash: &str,
        limit: usize,
        offset: usize,
    ) -> anyhow::Result<(Vec<AnnounceTrace>, usize)> {
        self.db.read_announces(info_hash, limit, offset)
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

    /// Centralized: resolve the profile a torrent should use.
    /// Order: torrent's preset.client_profile_id → engine default.
    pub(crate) async fn resolve_profile_for_torrent(
        &self,
        tid: TorrentId,
    ) -> Option<ClientProfileId> {
        let preset_pid = self
            .torrents
            .get(&tid)
            .and_then(|e| e.policy_snapshot().client_profile_id.clone())
            .map(ClientProfileId);
        if preset_pid.is_some() {
            return preset_pid;
        }
        self.active_profile.read().await.clone()
    }

    /// Drop cached `peer_id` / `key` material for every torrent currently in `preset_id`.
    /// Called after a preset's `client_profile_id` changes so the next announce regenerates
    /// identity using the new profile's algorithm + refresh policy.
    pub(crate) fn forget_preset_identities(&self, preset_id: &str) {
        for r in self.torrents.iter() {
            if r.value().preset_id() != preset_id {
                continue;
            }
            if let Some(ih) = r.value().info_hash_bytes {
                self.identities.forget_info_hash(&ih);
            }
        }
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
        self.add_torrent_metainfo_with_preset(meta, None).await
    }

    pub async fn add_torrent_metainfo_with_preset(
        &self,
        meta: MetainfoTorrent,
        preset_id: Option<&str>,
    ) -> Result<TorrentId, SudoratioError> {
        if self.shutting_down.load(Ordering::SeqCst) {
            return Err(SudoratioError::EngineShuttingDown);
        }
        let preset_id = preset_id.unwrap_or(crate::preset::DEFAULT_PRESET_ID);
        let preset = self
            .presets
            .get(preset_id)
            .ok_or(SudoratioError::TorrentNotFound)?;
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
            preset_id: parking_lot::RwLock::new(preset.id.clone()),
            policy: parking_lot::RwLock::new(preset.policy.clone()),
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
        self.persist_torrent(id);
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
        self.bandwidth.on_announce_success(
            tid,
            out.seeders.unwrap_or(0) as i64,
            out.leechers.unwrap_or(0) as i64,
            &self.torrents,
        );
        crate::wire::spawn_dials(self.clone(), tid.0, out.peers.clone());
    }

    /// Register the torrent with the bandwidth dispatcher reading bounds from its preset policy.
    pub(crate) fn register_bandwidth_for_torrent(&self, tid: TorrentId) {
        self.bandwidth.register_torrent(tid, &self.torrents);
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
    /// Resolves via the preset of the matching torrent if any, falling back to engine default.
    pub(crate) async fn resolve_inbound_peer_id(&self, info_hash: &[u8; 20]) -> Option<String> {
        let tid = TorrentId(*info_hash);
        let pid = self.resolve_profile_for_torrent(tid).await?;
        let record = self.profiles.get(&pid)?;
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
                scheduler::slots::stop_torrent(&inner, tid, scheduler::slots::StopMode::Announce)
                    .await;
            });
        }
        while set.join_next().await.is_some() {}
    }

    /// Fold the torrent map once with an optional preset filter. Single source of truth
    /// for `preset_rollups`, `preset_rollup`, `stats`, and `stats_for_preset`.
    fn fold_torrents(&self, filter: Option<&str>) -> PresetRollup {
        let mut acc = PresetRollup::default();
        for r in self.torrents.iter() {
            if filter.is_some_and(|f| r.value().preset_id() != f) {
                continue;
            }
            acc.torrent_count += 1;
            match r.lifecycle() {
                TorrentState::Downloading | TorrentState::Seeding => {
                    acc.active_count += 1;
                    let id = *r.key();
                    acc.upload_speed_bps = acc
                        .upload_speed_bps
                        .saturating_add(self.bandwidth.upload_speed_for(id));
                    acc.download_speed_bps = acc
                        .download_speed_bps
                        .saturating_add(self.bandwidth.download_speed_for(id));
                }
                TorrentState::Queued => acc.queued_count += 1,
                TorrentState::Stopped(_) => {}
            }
        }
        acc
    }

    /// Per-preset rollups for ALL presets (single O(N) walk). Used by `/presets` list.
    pub fn preset_rollups(&self) -> std::collections::HashMap<String, PresetRollup> {
        let mut out: std::collections::HashMap<String, PresetRollup> = self
            .presets
            .list()
            .iter()
            .map(|p| (p.id.clone(), PresetRollup::default()))
            .collect();
        for r in self.torrents.iter() {
            let entry = out.entry(r.value().preset_id()).or_default();
            entry.torrent_count += 1;
            match r.lifecycle() {
                TorrentState::Downloading | TorrentState::Seeding => {
                    entry.active_count += 1;
                    let id = *r.key();
                    entry.upload_speed_bps = entry
                        .upload_speed_bps
                        .saturating_add(self.bandwidth.upload_speed_for(id));
                    entry.download_speed_bps = entry
                        .download_speed_bps
                        .saturating_add(self.bandwidth.download_speed_for(id));
                }
                TorrentState::Queued => entry.queued_count += 1,
                TorrentState::Stopped(_) => {}
            }
        }
        out
    }

    /// Cheap single-preset rollup. Walks once, filtered. Used by `/presets/{id}`.
    pub fn preset_rollup(&self, preset_id: &str) -> PresetRollup {
        self.fold_torrents(Some(preset_id))
    }

    /// Scoped stats: when `preset_id = Some(id)`, only torrents in that preset are counted.
    pub async fn stats_for_preset(&self, preset_id: Option<&str>) -> SeedingStatus {
        let Some(filter) = preset_id else {
            return self.stats().await;
        };
        let r = self.fold_torrents(Some(filter));
        let cap = self
            .presets
            .get(filter)
            .map(|p| p.policy.load().max_active_torrents.max(1))
            .unwrap_or(1);
        SeedingStatus {
            running: !self.shutting_down.load(Ordering::SeqCst),
            upload_speed: r.upload_speed_bps,
            download_speed: r.download_speed_bps,
            max_active_torrents: cap,
            active_torrents: r.active_count,
            waiting_torrents: r.queued_count,
            tracked_metainfo_torrents: r.torrent_count,
        }
    }

    pub async fn stats(&self) -> SeedingStatus {
        let r = self.fold_torrents(None);
        let max_active_total: usize = self
            .presets
            .list()
            .iter()
            .map(|p| p.policy.load().max_active_torrents.max(1))
            .sum();
        SeedingStatus {
            running: !self.shutting_down.load(Ordering::SeqCst),
            upload_speed: r.upload_speed_bps,
            download_speed: r.download_speed_bps,
            max_active_torrents: max_active_total,
            active_torrents: r.active_count,
            waiting_torrents: r.queued_count,
            tracked_metainfo_torrents: r.torrent_count,
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

    /// Paginated list. `preset_id = Some(id)` filters; `None` returns all.
    /// Returns `(items, total_after_filter)`. Sort: by `queue_position`, then `id`.
    pub async fn list_torrents_paginated(
        &self,
        preset_id: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> (Vec<Torrent>, usize) {
        let mut all: Vec<Torrent> = self
            .torrents
            .iter()
            .filter(|r| match preset_id {
                Some(id) => r.value().preset_id() == id,
                None => true,
            })
            .map(|r| self.torrent_snapshot(*r.key(), r.value()))
            .collect();
        all.sort_by(|a, b| a.queue_position.cmp(&b.queue_position).then(a.id.cmp(&b.id)));
        let total = all.len();
        let items: Vec<Torrent> = all.into_iter().skip(offset).take(limit).collect();
        (items, total)
    }

    /// Per-torrent pause: leave registered, free a slot, send `stopped` if it was active.
    pub async fn pause_torrent(&self, id: TorrentId) -> Result<(), SudoratioError> {
        scheduler::user_stop_torrent(self, id).await?;
        self.persist_torrent(id);
        Ok(())
    }

    /// Resume after [`Self::pause_torrent`]; re-enters the wait queue or starts immediately if a slot is free.
    pub async fn resume_torrent(&self, id: TorrentId) -> Result<(), SudoratioError> {
        scheduler::user_resume_torrent(self, id).await?;
        self.persist_torrent(id);
        Ok(())
    }

    /// Remove the torrent from the engine (and session export); `stopped` if it had contacted the tracker.
    pub async fn remove_torrent(&self, id: TorrentId) -> Result<(), SudoratioError> {
        let info_hash = self.torrents.get(&id).and_then(|e| {
            e.info_hash.as_ref().map(|s| s.to_string())
        });
        scheduler::remove_torrent(self, id).await?;
        if let Some(h) = info_hash {
            self.spawn_db("delete_torrent", move |db| db.delete_torrent(&h));
        }
        Ok(())
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
        self.restore_torrent_with_preset(b, None).await
    }

    pub async fn restore_torrent_with_preset(
        &self,
        b: Torrent,
        preset_id: Option<&str>,
    ) -> Result<(), SudoratioError> {
        let slot = match self.torrents.entry(b.id) {
            dashmap::Entry::Occupied(_) => return Ok(()),
            dashmap::Entry::Vacant(slot) => slot,
        };
        let preset = self
            .presets
            .get(preset_id.unwrap_or(crate::preset::DEFAULT_PRESET_ID))
            .unwrap_or_else(|| self.presets.ensure_default());
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
            preset_id: parking_lot::RwLock::new(preset.id.clone()),
            policy: parking_lot::RwLock::new(preset.policy.clone()),
        };
        slot.insert(entry);
        Ok(())
    }

    pub const RESTORE_STARTED_JITTER_SECS: u64 = 30;

    /// After a batch of restore_torrent calls, promote with jitter.
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

fn build_engine(
    config: EngineConfig,
    db: Persistence,
    data_dir: Option<PathBuf>,
) -> anyhow::Result<Arc<Engine>> {
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
    let presets = Arc::new(PresetRegistry::new());
    Ok(Arc::new(Engine {
        config: arc_swap::ArcSwap::from_pointee(config),
        presets,
        db,
        data_dir,
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
    }))
}
