//! Percent-encoding for tracker query components.

use regex::Regex;

use crate::error::SudoratioError;

/// The exclusion pattern is compiled like a single-char regex match.
/// An **empty** pattern encodes every character (no single-character `matches()` hit).
pub fn compile_url_encoder_exclusion(pattern: &str) -> Result<Regex, SudoratioError> {
    if pattern.is_empty() {
        Regex::new(r"\A\z").map_err(|e| SudoratioError::PlaceholderBuild(e.to_string()))
    } else {
        Regex::new(pattern).map_err(|e| SudoratioError::PlaceholderBuild(e.to_string()))
    }
}

pub fn expand_charset(src: &str) -> Result<Vec<char>, SudoratioError> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 2 < bytes.len() && bytes[i + 1] == b'-' {
            let a = bytes[i] as char;
            let b = bytes[i + 2] as char;
            if a <= b {
                for c in a..=b {
                    out.push(c);
                }
            } else {
                for c in b..=a {
                    out.push(c);
                }
            }
            i += 3;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(out)
}

pub fn url_encode_query_component(s: &str, exclusion: &Regex, hex_upper: bool) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        let mut buf = [0u8; 4];
        let enc = ch.encode_utf8(&mut buf);
        for &byte in enc.as_bytes() {
            if exclusion.is_match(&(byte as char).to_string()) {
                out.push(byte as char);
            } else if hex_upper {
                out.push_str(&format!("%{:02X}", byte));
            } else {
                out.push_str(&format!("%{:02x}", byte));
            }
        }
    }
    out
}

pub fn url_encode_query_bytes(bytes: &[u8], exclusion: &Regex, hex_upper: bool) -> String {
    let mut out = String::new();
    for &byte in bytes {
        let ch = byte as char;
        if exclusion.is_match(&format!("{ch}")) {
            out.push(ch);
        } else if hex_upper {
            out.push_str(&format!("%{:02X}", byte));
        } else {
            out.push_str(&format!("%{:02x}", byte));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_exclusion_uppercase_encodes_every_byte() {
        let ex = compile_url_encoder_exclusion("").unwrap();
        assert_eq!(url_encode_query_bytes(b"a", &ex, true), "%61");
    }

    #[test]
    fn dashes_and_letters_are_all_encoded_when_excluded_pattern_is_empty() {
        let ex = compile_url_encoder_exclusion("").unwrap();
        let peer = "-AA-aaaaaaaaaaaaaaaa";
        let enc = url_encode_query_component(peer, &ex, true);
        assert_eq!(
            enc,
            "%2D%41%41%2D%61%61%61%61%61%61%61%61%61%61%61%61%61%61%61%61"
        );
    }
}
