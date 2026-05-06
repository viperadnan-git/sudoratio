//! Tracker `key` algorithms used by bundled client profiles.

use rand::Rng;
use regex::Regex;

use super::codec::expand_charset;
use super::schema::KeyAlgorithmSpec;
use crate::error::SudoratioError;

pub fn apply_key_case(s: &str, key_case: Option<&str>) -> String {
    match key_case.map(|c| c.to_ascii_lowercase()).as_deref() {
        Some("upper") => s.to_ascii_uppercase(),
        Some("lower") => s.to_ascii_lowercase(),
        Some("none") | None => s.to_string(),
        _ => s.to_string(),
    }
}

fn random_hex_string(len: usize) -> String {
    const HEX: &[u8] = b"0123456789ABCDEF";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| HEX[rng.random_range(0..HEX.len())] as char)
        .collect()
}

/// Random hex digits of fixed length.
pub fn key_hash(length: usize) -> String {
    random_hex_string(length)
}

/// Random hex digits of fixed length, with leading zeros trimmed.
pub fn key_hash_no_leading_zero(length: usize) -> String {
    let mut s = random_hex_string(length);
    s = s.trim_start_matches('0').to_string();
    if s.is_empty() {
        s.push('1');
    }
    s
}

/// Random integer in `[lower, upper]` rendered as lowercase hex without leading zeros.
pub fn key_digit_range_hex(lower: i64, upper: i64) -> Result<String, SudoratioError> {
    if upper < lower {
        return Err(SudoratioError::PlaceholderBuild(
            "DIGIT_RANGE key: upper < lower".into(),
        ));
    }
    let mut rng = rand::rng();
    let v = rng.random_range(lower..=upper);
    Ok(format!("{v:x}"))
}

/// Charset regex pattern of the form `[charset]{n}` (e.g. `[A-Z0-9]{8}`).
pub fn key_from_charset_regex_pattern(pattern: &str) -> Result<String, SudoratioError> {
    let re = Regex::new(r"^\[(.+)\]\{(\d+)\}$")
        .map_err(|e| SudoratioError::PlaceholderBuild(e.to_string()))?;
    let cap = re.captures(pattern.trim()).ok_or_else(|| {
        SudoratioError::PlaceholderBuild(format!(
            "REGEX key: unsupported pattern (need [charset]{{n}} like bundled Vuze): {pattern}"
        ))
    })?;
    let charset_src = cap.get(1).unwrap().as_str();
    let n: usize = cap
        .get(2)
        .unwrap()
        .as_str()
        .parse()
        .map_err(|_| SudoratioError::PlaceholderBuild("REGEX key: bad {n}".into()))?;
    let chars = expand_charset(charset_src)?;
    if chars.is_empty() || n == 0 {
        return Err(SudoratioError::PlaceholderBuild(
            "REGEX key: empty charset or n=0".into(),
        ));
    }
    let mut rng = rand::rng();
    let s: String = (0..n)
        .map(|_| chars[rng.random_range(0..chars.len())])
        .collect();
    Ok(s)
}

pub fn generate_key_material(alg: &KeyAlgorithmSpec) -> Result<String, SudoratioError> {
    match alg {
        KeyAlgorithmSpec::Hash { length } => Ok(key_hash(*length)),
        KeyAlgorithmSpec::HashNoLeadingZero { length } => Ok(key_hash_no_leading_zero(*length)),
        KeyAlgorithmSpec::Regex { pattern } => key_from_charset_regex_pattern(pattern),
        KeyAlgorithmSpec::DigitRangeHex { lower, upper } => key_digit_range_hex(*lower, *upper),
    }
}
