//! One-shot generation from profile algorithm specs (uses shared pool mutex for checksum peer ids).

use parking_lot::Mutex;

use super::peer_id::{peer_id_from_regex_pattern, RandomPoolChecksumState, PEER_ID_LEN};
use super::schema::PeerAlgorithmSpec;
use crate::error::SudoratioError;

pub fn generate_peer_id_once(
    alg: &PeerAlgorithmSpec,
    pool_slot: &Mutex<Option<RandomPoolChecksumState>>,
) -> Result<String, SudoratioError> {
    match alg {
        PeerAlgorithmSpec::Regex { pattern } => peer_id_from_regex_pattern(pattern),
        PeerAlgorithmSpec::RandomPoolWithChecksum {
            prefix,
            characters_pool,
            base,
        } => {
            let mut guard = pool_slot.lock();
            let need_init = guard
                .as_ref()
                .map(|s| !s.matches_config(prefix, characters_pool, *base))
                .unwrap_or(true);
            if need_init {
                *guard = Some(RandomPoolChecksumState::new(
                    prefix.clone(),
                    characters_pool,
                    *base,
                )?);
            }
            let out = guard.as_mut().unwrap().generate()?;
            if out.len() != PEER_ID_LEN {
                return Err(SudoratioError::PlaceholderBuild(format!(
                    "peer id length must be {PEER_ID_LEN}, got {}",
                    out.len()
                )));
            }
            Ok(out)
        }
    }
}
