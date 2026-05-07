//! Lifecycle transitions: promote, pause, resume, remove.
//!
//! Active set and wait queue are derived from each torrent's `TorrentState`,
//! never stored separately.

use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::error::SudoratioError;
use crate::state::Engine;
use crate::torrent::{
    AnnounceEvent, AnnounceQueryOverrides, StopReason, TorrentId, TorrentState, TransferPhase,
};

/// Eligible to enter the queue: has trackers and is not stopped.
fn is_eligible(inner: &Engine, tid: TorrentId) -> bool {
    let Some(e) = inner.torrents.get(&tid) else {
        return false;
    };
    !e.tiers.read().iter().all(|t| t.is_empty()) && !e.lifecycle().is_stopped()
}

fn preset_id_of(inner: &Engine, tid: TorrentId) -> Option<String> {
    inner.torrents.get(&tid).map(|e| e.preset_id())
}

fn count_active_in_preset(inner: &Engine, preset_id: &str) -> usize {
    inner
        .torrents
        .iter()
        .filter(|r| r.lifecycle().is_active() && r.preset_id() == preset_id)
        .count()
}

fn cap_for_preset(inner: &Engine, preset_id: &str) -> usize {
    inner
        .presets
        .get(preset_id)
        .map(|p| p.policy.load().max_active_torrents.max(1))
        .unwrap_or(1)
}

/// Lowest-`queue_position` torrent currently `Queued` within `preset_id`.
fn next_queued_in_preset(inner: &Engine, preset_id: &str) -> Option<TorrentId> {
    inner
        .torrents
        .iter()
        .filter(|r| r.lifecycle() == TorrentState::Queued && r.preset_id() == preset_id)
        .min_by_key(|r| (r.queue_position(), r.key().0))
        .map(|r| *r.key())
}

fn all_preset_ids(inner: &Engine) -> Vec<String> {
    inner.presets.list().iter().map(|p| p.id.clone()).collect()
}

async fn promote_to_active(inner: &Engine, tid: TorrentId, delay: Duration) {
    match inner.torrents.get(&tid) {
        Some(e) => {
            let active_state = match e.transfer_phase() {
                TransferPhase::Downloading => TorrentState::Downloading,
                TransferPhase::Seeding => TorrentState::Seeding,
            };
            e.set_lifecycle(active_state);
        }
        None => return,
    }
    let mut st = inner.seeding_state.lock();
    st.delay.add_or_replace(tid, AnnounceEvent::Started, delay);
    drop(st);
    inner.orchestrator_notify.notify_one();
}

pub(crate) async fn try_fill_slots(inner: &Engine) {
    fill_slots(inner, 0).await;
}

/// Random per-torrent Started delay in `[0, max_secs)`; avoids a restart-time announce burst.
pub(crate) async fn try_fill_slots_with_jitter(inner: &Engine, max_secs: u64) {
    fill_slots(inner, max_secs).await;
}

async fn fill_slots(inner: &Engine, jitter_max_secs: u64) {
    for preset_id in all_preset_ids(inner) {
        let cap = cap_for_preset(inner, &preset_id);
        while count_active_in_preset(inner, &preset_id) < cap {
            let Some(tid) = next_queued_in_preset(inner, &preset_id) else {
                break;
            };
            let delay = sample_delay(jitter_max_secs);
            promote_to_active(inner, tid, delay).await;
        }
    }
}

fn sample_delay(jitter_max_secs: u64) -> Duration {
    if jitter_max_secs == 0 {
        return Duration::ZERO;
    }
    use rand::Rng;
    let mut rng = rand::rng();
    Duration::from_secs(rng.random_range(0..jitter_max_secs))
}

/// Auto-assign the next free queue position.
pub(crate) fn next_queue_position(inner: &Engine) -> u32 {
    inner
        .torrents
        .iter()
        .map(|r| r.queue_position())
        .max()
        .map(|m| m.saturating_add(1))
        .unwrap_or(0)
}

/// Insert a freshly added torrent into the queue and promote if a slot is free.
pub(crate) async fn request_slot_for_torrent(inner: &Engine, tid: TorrentId) {
    if inner.shutting_down.load(Ordering::SeqCst) {
        return;
    }
    if !is_eligible(inner, tid) {
        return;
    }
    if let Some(e) = inner.torrents.get(&tid) {
        if !e.lifecycle().is_active() {
            e.set_lifecycle(TorrentState::Queued);
        }
    }
    try_fill_slots(inner).await;
}

/// Remove from the active set and the announce delay queue. Does NOT change
/// lifecycle state — caller already set it (e.g. to `Stopped(_)` or `Queued`).
pub(crate) async fn remove_from_active(inner: &Engine, tid: TorrentId) {
    {
        let mut st = inner.seeding_state.lock();
        st.delay.remove_torrent(tid);
    }
    inner.orchestrator_notify.notify_one();
    try_fill_slots(inner).await;
}

/// User pause: mark `Stopped(User)`, send best-effort `stopped`, free slot.
pub(crate) async fn user_stop_torrent(
    inner: &Engine,
    tid: TorrentId,
) -> Result<(), SudoratioError> {
    let was_active = match inner.torrents.get(&tid) {
        None => return Err(SudoratioError::TorrentNotFound),
        Some(e) => {
            let was = e.lifecycle().is_active();
            e.set_lifecycle(TorrentState::Stopped(StopReason::User));
            was
        }
    };
    {
        let mut st = inner.seeding_state.lock();
        st.delay.remove_torrent(tid);
    }
    inner.orchestrator_notify.notify_one();
    if was_active {
        inner.bandwidth.unregister_torrent(tid, &inner.torrents);
        let _ = inner
            .exec_tracker_announce(
                tid,
                AnnounceEvent::Stopped,
                &AnnounceQueryOverrides::default(),
            )
            .await;
    }
    try_fill_slots(inner).await;
    Ok(())
}

/// User resume: → `Queued` + reset grace timer so 3h restarts after next announce.
pub(crate) async fn user_resume_torrent(
    inner: &Engine,
    tid: TorrentId,
) -> Result<(), SudoratioError> {
    let Some(e) = inner.torrents.get(&tid) else {
        return Err(SudoratioError::TorrentNotFound);
    };
    e.with_state(|s| {
        s.state = TorrentState::Queued;
        s.zero_leechers_since = None;
    });
    drop(e);
    request_slot_for_torrent(inner, tid).await;
    Ok(())
}

/// Move a torrent to a different preset. Frees the old preset's slot accounting and lets the
/// next fill_slots round promote from the new preset's queue if capacity allows.
pub(crate) async fn move_torrent_to_preset(
    inner: &Engine,
    tid: TorrentId,
    new_preset_id: &str,
) -> Result<(), SudoratioError> {
    let new_preset = inner
        .presets
        .get(new_preset_id)
        .ok_or(SudoratioError::TorrentNotFound)?;
    let prev_preset_id = preset_id_of(inner, tid).ok_or(SudoratioError::TorrentNotFound)?;
    if prev_preset_id == new_preset_id {
        return Ok(());
    }
    let was_active = if let Some(e) = inner.torrents.get(&tid) {
        e.set_preset(new_preset_id.to_string(), new_preset.policy.clone());
        e.lifecycle().is_active()
    } else {
        return Err(SudoratioError::TorrentNotFound);
    };
    inner
        .bandwidth
        .sync_torrent_to_policy(tid, &inner.torrents);
    if was_active {
        let cap = cap_for_preset(inner, new_preset_id);
        let active_in_new = count_active_in_preset(inner, new_preset_id);
        if active_in_new > cap {
            // New preset is over-cap: park the moved torrent at the head of its new queue.
            if let Some(e) = inner.torrents.get(&tid) {
                e.set_lifecycle(TorrentState::Queued);
            }
            {
                let mut st = inner.seeding_state.lock();
                st.delay.remove_torrent(tid);
            }
            inner.orchestrator_notify.notify_one();
            inner.bandwidth.unregister_torrent(tid, &inner.torrents);
        }
    }
    try_fill_slots(inner).await;
    Ok(())
}

/// Remove from the engine entirely.
pub(crate) async fn remove_torrent(inner: &Engine, tid: TorrentId) -> Result<(), SudoratioError> {
    let row = inner.torrents.get(&tid).map(|r| {
        (
            r.info_hash_bytes,
            r.lifecycle().is_active(),
            r.with_state(|s| s.last_successful_announce_unix_ms > 0),
        )
    });
    let Some((info_hash_bytes, was_active, had_successful_announce)) = row else {
        return Err(SudoratioError::TorrentNotFound);
    };
    {
        let mut st = inner.seeding_state.lock();
        st.delay.remove_torrent(tid);
    }
    inner.orchestrator_notify.notify_one();
    if was_active {
        inner.bandwidth.unregister_torrent(tid, &inner.torrents);
    }
    if was_active || had_successful_announce {
        let _ = inner
            .exec_tracker_announce(
                tid,
                AnnounceEvent::Stopped,
                &AnnounceQueryOverrides::default(),
            )
            .await;
    }
    inner.forget_torrent_announce_keys(info_hash_bytes);
    inner.torrents.remove(&tid);
    try_fill_slots(inner).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::config::EngineConfig;
    use crate::preset::{PresetPolicy, PresetPolicyUpdate, DEFAULT_PRESET_ID};
    use crate::state::Engine;
    use crate::torrent::MetainfoTorrent;

    fn engine_with_cap(cap: usize) -> std::sync::Arc<Engine> {
        let e = Engine::new_in_memory(EngineConfig::default());
        e.presets
            .apply_policy(
                DEFAULT_PRESET_ID,
                PresetPolicyUpdate {
                    max_active_torrents: Some(cap),
                    ..Default::default()
                },
            )
            .unwrap();
        e
    }

    fn meta(name: &str, hash: u8) -> MetainfoTorrent {
        MetainfoTorrent {
            name: name.into(),
            info_hash: hex::encode([hash; 20]),
            info_hash_bytes: [hash; 20],
            trackers: crate::torrent::TrackersHttp {
                tiers: vec![vec!["http://127.0.0.1:1/announce".into()]],
            },
            size: 1024,
            download_before_seed: false,
        }
    }

    #[tokio::test]
    async fn cap_one_keeps_extras_in_wait_queue() {
        let engine = engine_with_cap(1);
        let _ = engine.add_torrent_metainfo(meta("a", 1)).await;
        let _ = engine.add_torrent_metainfo(meta("b", 2)).await;
        let _ = engine.add_torrent_metainfo(meta("c", 3)).await;
        let stats = engine.stats().await;
        assert_eq!(stats.active_torrents, 1);
        assert_eq!(stats.waiting_torrents, 2);
        engine.shutdown().await;
    }

    #[tokio::test]
    async fn growing_cap_promotes_waiting_torrents() {
        let engine = engine_with_cap(1);
        for i in 1..=4 {
            let _ = engine.add_torrent_metainfo(meta(&format!("t{i}"), i)).await;
        }
        assert_eq!(engine.stats().await.waiting_torrents, 3);
        engine
            .update_preset_policy(
                DEFAULT_PRESET_ID,
                PresetPolicyUpdate {
                    max_active_torrents: Some(3),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let stats = engine.stats().await;
        assert_eq!(stats.active_torrents, 3);
        assert_eq!(stats.waiting_torrents, 1);
        engine.shutdown().await;
    }

    #[tokio::test]
    async fn paused_torrent_is_ineligible() {
        let engine = engine_with_cap(2);
        let id = engine.add_torrent_metainfo(meta("p", 7)).await.unwrap();
        engine.pause_torrent(id).await.unwrap();
        let stats = engine.stats().await;
        assert_eq!(stats.active_torrents, 0);
        assert_eq!(stats.waiting_torrents, 0);
        engine.resume_torrent(id).await.unwrap();
        assert_eq!(engine.stats().await.active_torrents, 1);
        engine.shutdown().await;
    }

    // Silences unused-import warning when not all helpers consumed in this file.
    #[allow(dead_code)]
    fn _phantom(_: PresetPolicy) {}
}
