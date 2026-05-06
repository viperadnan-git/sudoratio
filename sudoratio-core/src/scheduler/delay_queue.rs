//! Per-torrent delay queue: unique entries, ordered by release time.

use crate::torrent::{AnnounceEvent, TorrentId};
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct Item {
    release_at: Instant,
    torrent_id: TorrentId,
    event: AnnounceEvent,
}

impl Eq for Item {}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.release_at == other.release_at && self.torrent_id == other.torrent_id
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        // Earliest time first in max-heap → reverse Instant
        other.release_at.cmp(&self.release_at)
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Default)]
pub struct DelayQueue {
    heap: BinaryHeap<Item>,
}

impl DelayQueue {
    pub fn add_or_replace(&mut self, torrent_id: TorrentId, event: AnnounceEvent, delay: Duration) {
        let release_at = Instant::now() + delay;
        self.retain(|i| i.torrent_id != torrent_id);
        self.heap.push(Item {
            release_at,
            torrent_id,
            event,
        });
    }

    pub fn remove_torrent(&mut self, torrent_id: TorrentId) {
        self.retain(|i| i.torrent_id != torrent_id);
    }

    fn retain(&mut self, mut pred: impl FnMut(&Item) -> bool) {
        let drained: Vec<_> = self.heap.drain().collect();
        for i in drained {
            if pred(&i) {
                self.heap.push(i);
            }
        }
    }

    /// Earliest scheduled `release_at`, if any (for deadline-driven wakeups).
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.heap.peek().map(|i| i.release_at)
    }

    /// Ready items with `release_at <= now`, earliest batch first (drains all due in one poll).
    pub fn pop_due(&mut self, now: Instant) -> Vec<(TorrentId, AnnounceEvent)> {
        let mut out = Vec::new();
        while let Some(top) = self.heap.peek() {
            if top.release_at > now {
                break;
            }
            let Item {
                torrent_id, event, ..
            } = self.heap.pop().unwrap();
            out.push((torrent_id, event));
        }
        out
    }
}
