//! BEP-3 `.torrent` parser. Extracts only what sudoratio needs: name, info-hash, total length,
//! and HTTP(S) announce URLs. The peer wire protocol is out of scope, so we deliberately skip
//! pieces verification beyond a sanity check on length.
//!
//! Info-hash is SHA-1 of the *original* bytes of the `info` dictionary, located in-place by a
//! lightweight bencode value scanner. This avoids a re-encode round-trip (which can drift if a
//! producer used non-canonical key order or unknown extra fields).

use serde::Deserialize;
use serde_bencode::value::Value as Bv;
use sha1::{Digest, Sha1};

use crate::error::SudoratioError;
use crate::torrent::{MetainfoTorrent, TrackersHttp};

#[derive(Debug, Deserialize)]
struct Top {
    #[serde(default)]
    announce: Option<serde_bytes::ByteBuf>,
    #[serde(default, rename = "announce-list")]
    announce_list: Option<Vec<Vec<serde_bytes::ByteBuf>>>,
    info: Bv,
}

#[derive(Debug, Deserialize)]
struct InfoSingle {
    #[serde(default)]
    name: Option<serde_bytes::ByteBuf>,
    #[serde(rename = "piece length")]
    piece_length: u64,
    pieces: serde_bytes::ByteBuf,
    #[serde(default)]
    length: Option<u64>,
    #[serde(default)]
    files: Option<Vec<InfoFile>>,
}

#[derive(Debug, Deserialize)]
struct InfoFile {
    length: u64,
}

pub fn parse(bytes: &[u8]) -> Result<MetainfoTorrent, SudoratioError> {
    let top: Top = serde_bencode::from_bytes(bytes)
        .map_err(|e| SudoratioError::PlaceholderBuild(format!("bencode metainfo: {e}")))?;

    let info_bytes = locate_info_dict_bytes(bytes).ok_or_else(|| {
        SudoratioError::PlaceholderBuild("metainfo: 'info' dict not found".into())
    })?;
    let info_hash_bytes: [u8; 20] = Sha1::digest(info_bytes).into();
    let info_hash = hex::encode(info_hash_bytes);

    let info: InfoSingle = decode_value(&top.info)?;
    let size: u64 = match (info.length, info.files.as_ref()) {
        (Some(n), _) => n,
        (None, Some(fs)) => fs.iter().map(|f| f.length).sum(),
        _ => 0,
    };
    if size == 0 {
        return Err(SudoratioError::PlaceholderBuild(
            "metainfo: zero total size".into(),
        ));
    }
    let piece_len = info.piece_length.max(1);
    let total_pieces = size.div_ceil(piece_len) as usize;
    if info.pieces.len() != total_pieces * 20 {
        return Err(SudoratioError::PlaceholderBuild(format!(
            "metainfo: pieces length {} != {} pieces × 20",
            info.pieces.len(),
            total_pieces
        )));
    }

    let name = info
        .name
        .as_ref()
        .map(|b| String::from_utf8_lossy(b).into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(unnamed)".to_string());

    let is_http = |s: &str| s.starts_with("http://") || s.starts_with("https://");
    let already_seen =
        |tiers: &[Vec<String>], s: &str| tiers.iter().any(|t| t.iter().any(|u| u == s));
    let mut tiers: Vec<Vec<String>> = Vec::new();
    if let Some(announce_list) = top.announce_list {
        for raw_tier in announce_list {
            let mut tier: Vec<String> = Vec::new();
            for u in raw_tier {
                let s = String::from_utf8_lossy(&u).into_owned();
                if is_http(&s) && !already_seen(&tiers, &s) && !tier.contains(&s) {
                    tier.push(s);
                }
            }
            if !tier.is_empty() {
                tiers.push(tier);
            }
        }
    }
    if let Some(a) = top.announce {
        let s = String::from_utf8_lossy(&a).into_owned();
        if is_http(&s) && !already_seen(&tiers, &s) {
            tiers.push(vec![s]);
        }
    }

    Ok(MetainfoTorrent {
        name,
        info_hash,
        info_hash_bytes,
        trackers: TrackersHttp { tiers },
        size,
        download_before_seed: false,
    })
}

fn decode_value<T: serde::de::DeserializeOwned>(v: &Bv) -> Result<T, SudoratioError> {
    let buf = serde_bencode::to_bytes(v)
        .map_err(|e| SudoratioError::PlaceholderBuild(format!("bencode reencode: {e}")))?;
    serde_bencode::from_bytes(&buf)
        .map_err(|e| SudoratioError::PlaceholderBuild(format!("bencode info: {e}")))
}

/// Find the byte range of the top-level `info` dict in a bencoded `.torrent` file.
fn locate_info_dict_bytes(bytes: &[u8]) -> Option<&[u8]> {
    // The torrent file is one outer dict whose keys are sorted lexicographically; `info` lives at
    // the top level. We walk the outer dict, looking for the `4:info` key, and then return the
    // next bencoded value's slice.
    let mut p = 0usize;
    if bytes.first() != Some(&b'd') {
        return None;
    }
    p += 1;
    while p < bytes.len() && bytes[p] != b'e' {
        let key_end = bencode_value_end(bytes, p)?;
        let key = &bytes[p..key_end];
        p = key_end;
        let val_end = bencode_value_end(bytes, p)?;
        // bencode strings are `<len>:<bytes>`; check this key against `4:info`.
        if key == b"4:info" || strip_string(key) == Some(b"info".as_slice()) {
            return Some(&bytes[p..val_end]);
        }
        p = val_end;
    }
    None
}

fn strip_string(b: &[u8]) -> Option<&[u8]> {
    let colon = b.iter().position(|c| *c == b':')?;
    let n: usize = std::str::from_utf8(&b[..colon]).ok()?.parse().ok()?;
    let start = colon + 1;
    if start + n != b.len() {
        return None;
    }
    Some(&b[start..])
}

fn bencode_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    let c = *bytes.get(start)?;
    match c {
        b'i' => {
            let e = bytes[start + 1..].iter().position(|c| *c == b'e')?;
            Some(start + 1 + e + 1)
        }
        b'l' | b'd' => {
            let mut p = start + 1;
            while p < bytes.len() && bytes[p] != b'e' {
                p = bencode_value_end(bytes, p)?;
            }
            (p < bytes.len()).then_some(p + 1)
        }
        b'0'..=b'9' => {
            let colon = bytes[start..].iter().position(|c| *c == b':')? + start;
            let n: usize = std::str::from_utf8(&bytes[start..colon])
                .ok()?
                .parse()
                .ok()?;
            Some(colon + 1 + n)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_torrent(name: &str, length: u64, piece_length: u64, announce: &str) -> Vec<u8> {
        let pieces_len = (length.div_ceil(piece_length) as usize) * 20;
        let pieces = vec![0u8; pieces_len];
        let info = format!(
            "d6:lengthi{length}e4:name{name_len}:{name}12:piece lengthi{piece_length}e6:pieces{pieces_len}:",
            length = length,
            name_len = name.len(),
            name = name,
            piece_length = piece_length,
            pieces_len = pieces_len,
        );
        let mut info_bytes = info.into_bytes();
        info_bytes.extend_from_slice(&pieces);
        info_bytes.push(b'e');

        let mut out = Vec::new();
        out.extend_from_slice(b"d");
        out.extend_from_slice(format!("8:announce{}:{}", announce.len(), announce).as_bytes());
        out.extend_from_slice(b"4:info");
        out.extend_from_slice(&info_bytes);
        out.push(b'e');
        out
    }

    #[test]
    fn parses_single_file_torrent() {
        let raw = build_torrent("hello.bin", 32, 16, "http://tracker.example/announce");
        let m = parse(&raw).unwrap();
        assert_eq!(m.name, "hello.bin");
        assert_eq!(m.size, 32);
        assert_eq!(
            m.trackers.tiers,
            vec![vec!["http://tracker.example/announce".to_string()]]
        );
        assert_eq!(m.info_hash.len(), 40);
    }

    #[test]
    fn rejects_zero_size() {
        let raw = build_torrent("empty", 0, 16, "http://t/announce");
        assert!(parse(&raw).is_err());
    }
}
