//! Silence-after-handshake. Read peer's BEP-3 handshake, send ours, then drain reads but
//! never write. Peer disconnects us at libtorrent `peer_timeout` (120s) →
//! `errors::timed_out_inactivity` (organic-idle bucket), not the 10s `handshake_timeout`
//! "dead client" bucket. Refs: libtorrent `src/peer_connection.cpp:4953`, `src/settings_pack.cpp`.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::handshake::{Handshake, HANDSHAKE_LEN};
use crate::state::Engine;
use crate::torrent::TorrentId;

/// 8s — under libtorrent's 10s `handshake_timeout`, with margin for slow peers.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
/// 180s — past `peer_timeout=120s`, so the peer disconnects us first.
const IDLE_CAP: Duration = Duration::from_secs(180);

pub async fn handle_connection(mut stream: TcpStream, engine: Arc<Engine>) -> io::Result<()> {
    let _ = stream.set_nodelay(true);

    let mut hs_buf = [0u8; HANDSHAKE_LEN];
    timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut hs_buf))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "handshake timeout"))??;
    let peer_hs = Handshake::decode(&hs_buf)?;

    let tid = TorrentId(peer_hs.info_hash);
    if !engine.torrents.contains_key(&tid) {
        return Ok(());
    }

    let Some(our_peer_id) = engine.resolve_inbound_peer_id(&peer_hs.info_hash).await else {
        return Ok(());
    };
    let our_peer_id_bytes: [u8; 20] = our_peer_id
        .as_bytes()
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "peer_id not 20 bytes"))?;

    let our_hs = Handshake::new(peer_hs.info_hash, our_peer_id_bytes);
    stream.write_all(&our_hs.encode()).await?;

    // Drain reads (avoid TCP backpressure) but never write again.
    let (mut rd, _wr) = stream.into_split();
    let mut scratch = vec![0u8; 16 * 1024];
    let _ = timeout(IDLE_CAP, async {
        loop {
            match rd.read(&mut scratch).await {
                Ok(0) | Err(_) => break,
                Ok(_) => continue,
            }
        }
    })
    .await;
    Ok(())
}
