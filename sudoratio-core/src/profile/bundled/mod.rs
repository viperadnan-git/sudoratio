//! Bundled client docs, embedded at compile time via [`include_str!`].
//!
//! Each entry is `(client, toml)`: the client family name (e.g. `qbittorrent`) and the raw
//! TOML doc source. One doc per client, declaring base config plus N `[[variant]]` blocks (one
//! per known version). Servers iterate this and call
//! [`crate::Engine::register_builtin_profile`] for each entry to register every variant
//! (`client@version`) on startup.

#[rustfmt::skip]
pub const BUNDLED_CLIENTS: &[(&str, &str)] = &[
    ("biglybt",      include_str!("files/biglybt.toml")),
    ("bittorrent",   include_str!("files/bittorrent.toml")),
    ("deluge",       include_str!("files/deluge.toml")),
    ("leap",         include_str!("files/leap.toml")),
    ("qbittorrent",  include_str!("files/qbittorrent.toml")),
    ("rtorrent",     include_str!("files/rtorrent.toml")),
    ("transmission", include_str!("files/transmission.toml")),
    ("utorrent",     include_str!("files/utorrent.toml")),
    ("vuze",         include_str!("files/vuze.toml")),
];

#[cfg(test)]
mod tests {
    use super::BUNDLED_CLIENTS;
    use crate::profile::parse_client_doc;

    #[test]
    fn every_bundled_client_resolves() {
        assert!(!BUNDLED_CLIENTS.is_empty(), "no bundled clients");
        for (client, toml) in BUNDLED_CLIENTS {
            let doc = parse_client_doc(toml).unwrap_or_else(|e| panic!("parse {client}: {e}"));
            assert_eq!(&doc.client, client, "doc.client must match registry name");
            let specs = doc
                .resolve()
                .unwrap_or_else(|e| panic!("resolve {client}: {e}"));
            assert!(!specs.is_empty(), "{client} has zero variants");
        }
    }

    #[test]
    fn bundled_client_names_are_unique() {
        let mut names: Vec<&str> = BUNDLED_CLIENTS.iter().map(|(n, _)| *n).collect();
        names.sort_unstable();
        let len = names.len();
        names.dedup();
        assert_eq!(names.len(), len, "duplicate names in BUNDLED_CLIENTS");
    }
}
