//! Per-engine announce identity caches: two parallel [`IdCache`]s (one each for `peer_id` and
//! tracker `key`) plus the shared [`RandomPoolChecksumState`] used by the
//! `RANDOM_POOL_WITH_CHECKSUM` peer-id algorithm.
//!
//! The actual policy logic lives in [`crate::profile::refresh`].

use parking_lot::Mutex;

pub(crate) use crate::profile::refresh::{
    resolve_with_policy, IdCache, Policy, PERSISTENT_KEY_IDLE_TTL,
};
use crate::profile::RandomPoolChecksumState;

#[derive(Default)]
pub(crate) struct AnnounceIdentities {
    pub(crate) peer_id: IdCache,
    pub(crate) key: IdCache,
    /// Shared state for the `RANDOM_POOL_WITH_CHECKSUM` peer-id algorithm.
    pub(crate) peer_random_pool: Mutex<Option<RandomPoolChecksumState>>,
}

impl AnnounceIdentities {
    pub(crate) fn clear(&self) {
        self.peer_id.clear();
        self.key.clear();
        *self.peer_random_pool.lock() = None;
    }

    pub(crate) fn forget_info_hash(&self, info_hash: &[u8; 20]) {
        self.peer_id.forget_info_hash(info_hash);
        self.key.forget_info_hash(info_hash);
    }
}
