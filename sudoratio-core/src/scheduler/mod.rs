//! Always-on orchestrator: scheduler loop + per-announce dispatch. Slot management lives in
//! [`slots`]; the deadline heap lives in [`delay_queue`].

pub(crate) mod delay_queue;
pub(crate) mod slots;

pub(crate) use slots::{
    remove_from_active, remove_torrent, request_slot_for_torrent, try_fill_slots,
    try_fill_slots_with_jitter, user_resume_torrent, user_stop_torrent,
};

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::state::Engine;
use crate::torrent::{
    AnnounceEvent, AnnounceQueryOverrides, StopReason, TorrentId, TorrentState, TransferPhase,
};

const MAX_CONSECUTIVE_FAILS: u32 = 5;

pub(crate) fn should_pause_tiny_swarm(inner: &Engine, tid: TorrentId) -> bool {
    let min = inner.config.load().min_swarm_seeders_to_seed;
    if min == 0 {
        return false;
    }
    let Some(e) = inner.torrents.get(&tid) else {
        return false;
    };
    let seeders = e.with_state(|s| s.last_seeders);
    matches!(seeders, Some(n) if n < min)
}

pub(crate) fn should_pause_no_leechers(inner: &Engine, tid: TorrentId) -> bool {
    let cfg = inner.config.load();
    if !cfg.pause_torrent_with_zero_leechers {
        return false;
    }
    let Some(e) = inner.torrents.get(&tid) else {
        return false;
    };
    let since = e.with_state(|s| s.zero_leechers_since);
    let Some(since_ms) = since else {
        return false;
    };
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let elapsed_secs = now_ms.saturating_sub(since_ms) / 1000;
    elapsed_secs >= cfg.pause_torrent_with_zero_leechers_grace
}

fn tracker_reschedule_delay_secs(
    out: &crate::torrent::AnnounceOutcome,
    jitter_max_secs: u32,
) -> u32 {
    let iv = out.announce_interval.unwrap_or(5);
    let mi = out.min_interval.unwrap_or(0);
    let base = iv.max(mi).max(1);
    if jitter_max_secs == 0 {
        return base;
    }
    let mut rng = rand::rng();
    let extra = rand::Rng::random_range(&mut rng, 0..=jitter_max_secs);
    base.saturating_add(extra)
}

async fn delay_schedule(
    inner: &Engine,
    torrent_id: TorrentId,
    event: AnnounceEvent,
    delay: Duration,
) {
    let mut st = inner.seeding_state.lock();
    st.delay.add_or_replace(torrent_id, event, delay);
    inner.orchestrator_notify.notify_one();
}

/// Wait until the next announce deadline or until the queue is rescheduled.
async fn orchestrator_wait(inner: &Engine, next_deadline: Option<Instant>) {
    match next_deadline {
        None => {
            inner.orchestrator_notify.notified().await;
        }
        Some(deadline) => {
            let now = Instant::now();
            if deadline <= now {
                tokio::task::yield_now().await;
                return;
            }
            let dur = deadline.saturating_duration_since(now);
            tokio::select! {
                _ = tokio::time::sleep(dur) => {}
                _ = inner.orchestrator_notify.notified() => {}
            }
        }
    }
}

pub(crate) async fn run_loop(inner: Arc<Engine>) {
    let sem = Arc::new(tokio::sync::Semaphore::new(3));
    let bw_ms = inner.config.load().bandwidth_tick_ms.max(1);
    let mut t_bw = tokio::time::interval(Duration::from_millis(bw_ms));
    t_bw.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        if inner.shutting_down.load(Ordering::SeqCst) {
            break;
        }

        loop {
            let due = {
                let mut st = inner.seeding_state.lock();
                st.delay.pop_due(Instant::now())
            };
            if due.is_empty() {
                break;
            }
            for (tid, ev) in due {
                let inner2 = inner.clone();
                let sem2 = sem.clone();
                tokio::spawn(async move {
                    let Ok(_permit) = sem2.acquire_owned().await else {
                        return;
                    };
                    handle_queued_announce(inner2, tid, ev).await;
                });
            }
        }

        let next_deadline = {
            let st = inner.seeding_state.lock();
            st.delay.next_deadline()
        };

        tokio::select! {
            _ = t_bw.tick() => {
                inner.transfer_tick().await;
            }
            _ = orchestrator_wait(&inner, next_deadline) => {}
        }
    }
}

/// Single auto-pause helper. Skips the `stopped` announce when the tracker
/// is already known unreachable (`TrackerFailed`).
pub(crate) async fn auto_pause(inner: &Engine, tid: TorrentId, reason: StopReason) {
    if let Some(e) = inner.torrents.get(&tid) {
        e.with_state(|s| {
            s.state = TorrentState::Stopped(reason);
            // Clear stale grace so resume / config-toggle starts a fresh 3h window.
            if reason == StopReason::NoLeechers {
                s.zero_leechers_since = None;
            }
        });
    }
    inner.bandwidth.unregister_torrent(tid, &inner.torrents);
    remove_from_active(inner, tid).await;
    if reason != StopReason::TrackerFailed {
        let _ = inner
            .exec_tracker_announce(
                tid,
                AnnounceEvent::Stopped,
                &AnnounceQueryOverrides::default(),
            )
            .await;
    }
    inner.emit_state_change(tid);
}

fn upload_ratio_applies(inner: &Engine, tid: TorrentId) -> bool {
    inner
        .torrents
        .get(&tid)
        .map(|r| !r.download_first || r.transfer_phase() == TransferPhase::Seeding)
        .unwrap_or(true)
}

/// Run the post-success policy checks (no-leechers / upload-ratio) for a
/// successful announce. Returns `true` if the torrent has been auto-paused
/// and the caller should NOT schedule the next announce.
async fn maybe_auto_pause(inner: &Engine, tid: TorrentId) -> bool {
    if should_pause_tiny_swarm(inner, tid) {
        auto_pause(inner, tid, StopReason::TinySwarm).await;
        return true;
    }
    if should_pause_no_leechers(inner, tid) {
        auto_pause(inner, tid, StopReason::NoLeechers).await;
        return true;
    }
    if upload_ratio_applies(inner, tid) && inner.has_reached_upload_ratio_limit(tid) {
        auto_pause(inner, tid, StopReason::UploadRatio).await;
        return true;
    }
    false
}

/// Tracker error path: bump fail count; pause as `TrackerFailed` when the
/// limit is hit (slot is freed but the torrent stays in the engine, requiring
/// user resume), otherwise reschedule the same event after the last interval.
async fn handle_announce_error(inner: &Engine, tid: TorrentId, retry_event: AnnounceEvent) {
    if inner.bump_consecutive_fails(tid) >= MAX_CONSECUTIVE_FAILS {
        auto_pause(inner, tid, StopReason::TrackerFailed).await;
        return;
    }
    let iv = Duration::from_secs(u64::from(inner.last_interval_secs(tid)));
    delay_schedule(inner, tid, retry_event, iv).await;
}

async fn handle_queued_announce(inner: Arc<Engine>, tid: TorrentId, ev: AnnounceEvent) {
    let res = inner
        .exec_tracker_announce(tid, ev, &AnnounceQueryOverrides::default())
        .await;
    match (ev, res) {
        (AnnounceEvent::Started | AnnounceEvent::None | AnnounceEvent::Completed, Ok(out)) => {
            if ev == AnnounceEvent::Started {
                inner.register_bandwidth_for_torrent(tid);
            }
            inner.post_announce_success(tid, &out);
            if maybe_auto_pause(&inner, tid).await {
                return;
            }
            inner.reset_consecutive_fails(tid);
            let jitter = inner.config.load().max_announce_jitter;
            let iv = Duration::from_secs(u64::from(tracker_reschedule_delay_secs(&out, jitter)));
            delay_schedule(&inner, tid, AnnounceEvent::None, iv).await;
        }
        (
            ev @ (AnnounceEvent::Started | AnnounceEvent::None | AnnounceEvent::Completed),
            Err(_),
        ) => {
            handle_announce_error(&inner, tid, ev).await;
        }
        (AnnounceEvent::Stopped, _) => {
            // Fire-and-forget: real clients never retry `stopped`.
            inner.bandwidth.unregister_torrent(tid, &inner.torrents);
            remove_from_active(&inner, tid).await;
        }
    }
}
