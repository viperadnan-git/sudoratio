//! Client profile parsing (TOML) and peer-id / tracker-key algorithms.
//!
//! Submodules: **schema** (serde TOML), **codec** (URL percent-encoding), **peer_id** / **key**
//! algorithms, **refresh** (generic [`RefreshOnPolicy`] cache resolver).

pub mod bundled;
mod client_doc;
pub(crate) mod codec;
mod generation;
mod key;
mod peer_id;
pub(crate) mod refresh;
pub(crate) mod schema;

pub use bundled::BUNDLED_CLIENTS;
pub use client_doc::{parse_client_doc, ClientDoc, HeaderPatch, PartialSpec, Variant};

pub use generation::generate_peer_id_once;
pub use key::{apply_key_case, generate_key_material};
pub use peer_id::{RandomPoolChecksumState, PEER_ID_LEN};
pub use schema::{
    ClientProfileSpec, KeyAlgorithmSpec, KeyGenerator, PeerAlgorithmSpec, PeerIdGenerator,
    RefreshOnPolicy, UrlEncoderCfg,
};
