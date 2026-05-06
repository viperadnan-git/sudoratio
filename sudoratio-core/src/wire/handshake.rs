//! BEP-3 handshake: `[19][BitTorrent protocol][reserved:8][info_hash:20][peer_id:20]`.
//! Reserved byte 5 LSB `0x10` = BEP-10 extended messaging; mainstream clients set it.

use std::io;

pub const PROTOCOL: &[u8; 19] = b"BitTorrent protocol";
pub const PROTOCOL_LEN: u8 = 19;
pub const HANDSHAKE_LEN: usize = 68;

#[derive(Debug, Clone)]
pub struct Handshake {
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        let mut reserved = [0u8; 8];
        reserved[5] = 0x10;
        Self {
            reserved,
            info_hash,
            peer_id,
        }
    }

    pub fn encode(&self) -> [u8; HANDSHAKE_LEN] {
        let mut out = [0u8; HANDSHAKE_LEN];
        out[0] = PROTOCOL_LEN;
        out[1..20].copy_from_slice(PROTOCOL);
        out[20..28].copy_from_slice(&self.reserved);
        out[28..48].copy_from_slice(&self.info_hash);
        out[48..68].copy_from_slice(&self.peer_id);
        out
    }

    pub fn decode(buf: &[u8; HANDSHAKE_LEN]) -> io::Result<Self> {
        if buf[0] != PROTOCOL_LEN || &buf[1..20] != PROTOCOL.as_slice() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "not a BitTorrent handshake",
            ));
        }
        let mut reserved = [0u8; 8];
        reserved.copy_from_slice(&buf[20..28]);
        let mut info_hash = [0u8; 20];
        info_hash.copy_from_slice(&buf[28..48]);
        let mut peer_id = [0u8; 20];
        peer_id.copy_from_slice(&buf[48..68]);
        Ok(Self {
            reserved,
            info_hash,
            peer_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let h = Handshake::new([1u8; 20], [2u8; 20]);
        let encoded = h.encode();
        assert_eq!(encoded.len(), HANDSHAKE_LEN);
        assert_eq!(encoded[0], 19);
        assert_eq!(&encoded[1..20], PROTOCOL.as_slice());
        assert_eq!(encoded[25], 0x10, "BEP-10 bit set");
        let back = Handshake::decode(&encoded).unwrap();
        assert_eq!(back.info_hash, [1u8; 20]);
        assert_eq!(back.peer_id, [2u8; 20]);
    }

    #[test]
    fn rejects_garbage() {
        let mut buf = [0u8; HANDSHAKE_LEN];
        buf[0] = 19;
        buf[1..20].copy_from_slice(b"NotBitTorrent      ");
        assert!(Handshake::decode(&buf).is_err());
    }

    #[test]
    fn rejects_wrong_pstr_len() {
        let mut buf = [0u8; HANDSHAKE_LEN];
        buf[0] = 18;
        buf[1..20].copy_from_slice(PROTOCOL);
        assert!(Handshake::decode(&buf).is_err());
    }
}
