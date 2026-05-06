//! Outbound silence-after-handshake. Same wire shape as `peer.rs`, just initiating side.

use std::io;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::handshake::{Handshake, HANDSHAKE_LEN};
use crate::state::Engine;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
const IDLE_CAP: Duration = Duration::from_secs(180);

pub fn spawn_dials(engine: Arc<Engine>, info_hash: [u8; 20], peers: Vec<SocketAddr>) {
    let cfg = engine.config.load();
    if !cfg.outbound_dial_enabled {
        return;
    }
    let allow_loopback = cfg.outbound_dial_allow_loopback;
    let take_n = cfg.outbound_dial_max_per_announce;
    drop(cfg);
    let our_port = engine.listening_port.load(Ordering::Relaxed);

    let candidates: Vec<SocketAddr> = peers
        .into_iter()
        .filter(|a| !a.ip().is_unspecified() && a.port() != 0)
        .filter(|a| allow_loopback || !a.ip().is_loopback())
        .filter(|a| !(our_port != 0 && a.port() == our_port && a.ip().is_loopback()))
        .take(take_n)
        .collect();

    for addr in candidates {
        let engine = engine.clone();
        tokio::spawn(async move {
            let Ok(Ok(permit)) = timeout(
                Duration::from_secs(1),
                engine.dial_global_sem.clone().acquire_owned(),
            )
            .await
            else {
                return;
            };
            if let Err(e) = dial_one(&engine, addr, info_hash).await {
                tracing::trace!(%addr, error = %e, "outbound dial ended");
            }
            drop(permit);
        });
    }
}

async fn dial_one(engine: &Arc<Engine>, addr: SocketAddr, info_hash: [u8; 20]) -> io::Result<()> {
    let mut stream = timeout(CONNECT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "connect timeout"))??;
    let _ = stream.set_nodelay(true);

    let our_peer_id = engine
        .resolve_inbound_peer_id(&info_hash)
        .await
        .ok_or_else(|| io::Error::other("no active profile"))?;
    let our_peer_id_bytes: [u8; 20] = our_peer_id
        .as_bytes()
        .try_into()
        .map_err(|_| io::Error::other("peer_id not 20 bytes"))?;
    let our_hs = Handshake::new(info_hash, our_peer_id_bytes);
    stream.write_all(&our_hs.encode()).await?;

    let mut hs_buf = [0u8; HANDSHAKE_LEN];
    timeout(HANDSHAKE_TIMEOUT, stream.read_exact(&mut hs_buf))
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "handshake timeout"))??;
    let peer_hs = Handshake::decode(&hs_buf)?;
    if peer_hs.info_hash != info_hash {
        return Ok(());
    }

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
