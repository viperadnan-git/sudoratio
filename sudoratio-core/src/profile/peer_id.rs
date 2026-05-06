//! Peer id algorithms: `regex` (charset pattern) and `random_pool_with_checksum` (with golden vectors).

use rand::rngs::StdRng;
use rand::Rng;
use rand::RngCore;
use rand::SeedableRng;
use regex::Regex;

use super::codec::expand_charset;
use crate::error::SudoratioError;

pub const PEER_ID_LEN: usize = 20;

/// Pattern `prefix[charset]{n}` — fills `n` randomly-chosen characters from the charset.
pub fn peer_id_from_regex_pattern(pattern: &str) -> Result<String, SudoratioError> {
    let re_brace =
        Regex::new(r"\{(\d+)\}$").map_err(|e| SudoratioError::PlaceholderBuild(e.to_string()))?;
    let cap = re_brace.captures(pattern).ok_or_else(|| {
        SudoratioError::PlaceholderBuild("peer id pattern: missing {n} suffix".into())
    })?;
    let n: usize = cap
        .get(1)
        .unwrap()
        .as_str()
        .parse()
        .map_err(|_| SudoratioError::PlaceholderBuild("peer id pattern: bad {n}".into()))?;
    let brace = cap.get(0).unwrap().as_str();
    let head = pattern
        .strip_suffix(brace)
        .ok_or_else(|| SudoratioError::PlaceholderBuild("peer id pattern: brace strip".into()))?;
    if !head.ends_with(']') {
        return Err(SudoratioError::PlaceholderBuild(
            "peer id pattern: expected ] before {n}".into(),
        ));
    }
    let head = &head[..head.len() - 1];
    let bracket_pos = head.rfind('[').ok_or_else(|| {
        SudoratioError::PlaceholderBuild("peer id pattern: missing '[' before charset".into())
    })?;
    let prefix = &head[..bracket_pos];
    let charset_src = &head[bracket_pos + 1..];
    let chars = expand_charset(charset_src)?;
    if chars.is_empty() || n == 0 {
        return Err(SudoratioError::PlaceholderBuild(
            "empty charset or n=0".into(),
        ));
    }
    let mut rng = rand::rng();
    let tail: String = (0..n)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect();
    Ok(format!("{prefix}{tail}"))
}

fn random_int_10_to_50() -> u32 {
    let mut rng = rand::rng();
    rng.random_range(10..50)
}

/// Mutable state for the `random_pool_with_checksum` peer-id algorithm.
#[derive(Debug)]
pub struct RandomPoolChecksumState {
    rng: StdRng,
    generation_count: u32,
    refresh_seed_after: u32,
    prefix: String,
    pool_source: String,
    pool: Vec<char>,
    base: u32,
}

impl RandomPoolChecksumState {
    pub fn new(prefix: String, characters_pool: &str, base: u32) -> Result<Self, SudoratioError> {
        if prefix.is_empty() {
            return Err(SudoratioError::PlaceholderBuild(
                "RANDOM_POOL_WITH_CHECKSUM: prefix empty".into(),
            ));
        }
        if characters_pool.is_empty() {
            return Err(SudoratioError::PlaceholderBuild(
                "RANDOM_POOL_WITH_CHECKSUM: charactersPool empty".into(),
            ));
        }
        if base == 0 {
            return Err(SudoratioError::PlaceholderBuild(
                "RANDOM_POOL_WITH_CHECKSUM: base must be > 0".into(),
            ));
        }
        let pool: Vec<char> = characters_pool.chars().collect();
        if (pool.len() as u32) < base {
            return Err(SudoratioError::PlaceholderBuild(
                "RANDOM_POOL_WITH_CHECKSUM: charactersPool shorter than base".into(),
            ));
        }
        let pool_source = characters_pool.to_string();
        let rng = StdRng::try_from_os_rng().map_err(|e| {
            SudoratioError::PlaceholderBuild(format!("RANDOM_POOL_WITH_CHECKSUM: rng: {e}"))
        })?;
        Ok(Self {
            rng,
            generation_count: 0,
            refresh_seed_after: random_int_10_to_50(),
            prefix,
            pool_source,
            pool,
            base,
        })
    }

    pub fn matches_config(&self, prefix: &str, characters_pool: &str, base: u32) -> bool {
        self.prefix == prefix && self.pool_source == characters_pool && self.base == base
    }

    fn reseed_rng(&mut self) {
        self.rng = StdRng::from_os_rng();
        self.refresh_seed_after = random_int_10_to_50();
    }

    fn next_random_bytes(&mut self, len: usize) -> Vec<u8> {
        if self.generation_count >= self.refresh_seed_after {
            self.generation_count = 0;
            self.reseed_rng();
        }
        self.generation_count += 1;
        let mut v = vec![0u8; len];
        self.rng.fill_bytes(&mut v);
        v
    }

    /// Generate one 20-byte peer id from the shared pool state.
    pub fn generate(&mut self) -> Result<String, SudoratioError> {
        let suffix_len = PEER_ID_LEN.saturating_sub(self.prefix.len());
        if suffix_len == 0 {
            return Err(SudoratioError::PlaceholderBuild(
                "RANDOM_POOL_WITH_CHECKSUM: prefix too long for 20-char peer id".into(),
            ));
        }
        let need = suffix_len.saturating_sub(1);
        let random_bytes = self.next_random_bytes(need);
        random_pool_peer_id_with_random_bytes(&self.prefix, &self.pool, self.base, &random_bytes)
    }
}

/// Pure form of `random_pool_with_checksum` given raw random bytes
/// (`suffix_len - 1` bytes). Used by [`RandomPoolChecksumState::generate`] and parity tests.
pub(crate) fn random_pool_peer_id_with_random_bytes(
    prefix: &str,
    pool: &[char],
    base: u32,
    random_bytes: &[u8],
) -> Result<String, SudoratioError> {
    let suffix_len = PEER_ID_LEN.saturating_sub(prefix.len());
    if suffix_len == 0 {
        return Err(SudoratioError::PlaceholderBuild(
            "RANDOM_POOL_WITH_CHECKSUM: prefix too long for 20-char peer id".into(),
        ));
    }
    let need = suffix_len.saturating_sub(1);
    if random_bytes.len() != need {
        return Err(SudoratioError::PlaceholderBuild(format!(
            "RANDOM_POOL_WITH_CHECKSUM: expected {need} random bytes, got {}",
            random_bytes.len()
        )));
    }
    if pool.len() < base as usize {
        return Err(SudoratioError::PlaceholderBuild(
            "RANDOM_POOL_WITH_CHECKSUM: charactersPool shorter than base".into(),
        ));
    }
    let b = base as i32;
    let mut buf: Vec<char> = vec!['\0'; suffix_len];
    let mut total: i32 = 0;
    for i in 0..suffix_len.saturating_sub(1) {
        let mut val = i32::from(random_bytes[i]);
        val %= b;
        total += val;
        let idx = val as usize;
        let ch = *pool.get(idx).ok_or_else(|| {
            SudoratioError::PlaceholderBuild(
                "RANDOM_POOL_WITH_CHECKSUM: charactersPool shorter than base".into(),
            )
        })?;
        buf[i] = ch;
    }
    let val = if (total % b) != 0 { b - (total % b) } else { 0 };
    let last = *pool.get(val as usize).ok_or_else(|| {
        SudoratioError::PlaceholderBuild(
            "RANDOM_POOL_WITH_CHECKSUM: checksum index out of pool".into(),
        )
    })?;
    buf[suffix_len - 1] = last;
    Ok(format!("{}{}", prefix, buf.into_iter().collect::<String>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Golden vectors for `random_pool_with_checksum_peer_id` — fixed correctness checks
    /// against the algorithm output.
    #[test]
    fn random_pool_with_checksum_known_vectors() {
        let prefix = "-TR2820-";
        let pool: Vec<char> = "0123456789abcdefghijklmnopqrstuvwxyz".chars().collect();
        let base = 36u32;
        let cases: &[(&[u8], &str)] = &[
            (&[250u8; 11], "-TR2820-yyyyyyyyyyym"),
            (&[0u8; 11], "-TR2820-000000000000"),
            (&[255u8; 11], "-TR2820-333333333333"),
            (&[128u8; 11], "-TR2820-kkkkkkkkkkkw"),
            (&[1u8; 11], "-TR2820-11111111111p"),
            (
                &[26, 200, 124, 39, 84, 248, 3, 159, 64, 239, 0],
                "-TR2820-qkg3cw3fsn02",
            ),
        ];
        for (bytes, want) in cases {
            let got = random_pool_peer_id_with_random_bytes(prefix, &pool, base, bytes).unwrap();
            assert_eq!(&got, want, "bytes={bytes:?}");
        }
    }
}
