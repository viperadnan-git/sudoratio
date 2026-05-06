//! Inbound BT peer listener + outbound BT dialer. Both pin to the same silence-after-handshake
//! pattern. See `peer::handle_connection` (inbound) and `dialer::spawn_dials` (outbound).

mod dialer;
mod handshake;
mod listener;
mod peer;

pub use dialer::spawn_dials;
pub use listener::spawn_peer_listener;
