//! Accept loop. Cancel via `JoinHandle::abort` at engine shutdown.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::peer::handle_connection;
use crate::state::Engine;

pub struct PeerListenerHandle {
    pub bound_port: u16,
    pub task: JoinHandle<()>,
}

pub async fn spawn_peer_listener(
    engine: Arc<Engine>,
    bind_addr: SocketAddr,
) -> std::io::Result<PeerListenerHandle> {
    let listener = TcpListener::bind(bind_addr).await?;
    let bound = listener.local_addr()?;
    let bound_port = bound.port();
    tracing::info!(%bound, "BT peer listener bound");
    let task = tokio::spawn(async move {
        loop {
            let (stream, peer_addr) = match listener.accept().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "peer accept failed");
                    continue;
                }
            };
            tracing::trace!(%peer_addr, "incoming peer connection");
            let engine = engine.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, engine).await {
                    tracing::trace!(%peer_addr, error = %e, "peer handler ended");
                }
            });
        }
    });
    Ok(PeerListenerHandle { bound_port, task })
}
