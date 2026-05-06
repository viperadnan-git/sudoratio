<div align="center">

<img src="web/public/sudoratio.png" alt="sudoratio" width="128" height="128" />

# sudoratio

**A self-hosted BitTorrent tracker-protocol simulator and research toolkit, with a modern web UI.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.93+-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-ready-2496ED.svg)](./Dockerfile)

[Quick Start](#quick-start) · [Configuration](#configuration) · [Client Profiles](#client-profiles) · [Architecture](#architecture) · [API](#http-api) · [Development](#development)

</div>

---

sudoratio is a research-oriented reference implementation of the BitTorrent tracker-announce and peer-wire handshake protocols. It speaks to trackers on behalf of `.torrent` files you load in, exercising the full announce flow with a configurable bandwidth model — without ever transferring real piece data. It also runs a TCP listener that completes BEP-3 handshakes and dials outbound peers from announce responses, providing a controlled environment to study how trackers and peers observe a participant whose announce traffic is decoupled from actual data exchange.

It is intended for **protocol learners, tracker operators stress-testing their own detection heuristics, and security researchers studying BitTorrent client fingerprinting**. Built in Rust as a single static binary with an embedded SPA and a typed HTTP API.

## Highlights

- **Single binary, no runtime deps.** Rust workspace compiled to a stripped distroless image; the Vite/React SPA is baked into the binary at build time via `rust-embed`.
- **Realistic peer presence.** TCP listener that completes BEP-3 handshakes, paired with an outbound dialer fired against the tracker's `peers` response. Silent after handshake by design — no advertised pieces it won't serve.
- **Per-torrent bandwidth simulation.** Each torrent samples a cap inside `[min, max]` after every announce, with per-tick ±10% jitter for natural variance and hard zero-gating when the swarm has no seeders (download) or no leechers (upload).
- **Layered client profiles.** 24 bundled client/version pairs, plus a TOML schema for adding your own variants on top of any bundled client family.
- **Embeddable core.** `sudoratio-core` is a standalone crate; `sudoratio-server` is a thin axum wrapper. Build your own frontend or embed the engine in a desktop app.
- **Persistent sessions.** Torrent state, announce traces, and user-registered profiles survive restarts via SQLite + a small filesystem layout under `--config-dir`.
- **BEP-27 compliant.** Never participates in DHT, PEX, or LSD — they're forbidden for private torrents and adding them would be a fingerprint, not a feature.

## Quick Start

### Docker (recommended)

```bash
echo "SUDORATIO_PASSWORD=$(openssl rand -hex 24)" > .env
docker compose up -d
```

Open <http://localhost:8787> and sign in with the password you set.

The compose file pulls `viperadnan/sudoratio:latest` from Docker Hub by default. To pull from GitHub Container Registry instead, set `SUDORATIO_IMAGE=ghcr.io/viperadnan-git/sudoratio:latest` in `.env`. Either path also accepts a local rebuild via `docker compose up -d --build`.

### From source

```bash
SUDORATIO_BUILD_WEB=1 cargo build --release --bin sudoratio-server

./target/release/sudoratio-server \
  --config-dir ./data \
  --listen 127.0.0.1:8787 \
  --password changeme
```

## Configuration

All flags accept the matching `SUDORATIO_*` environment variable. The most useful knobs:

| Flag | Env | Default | Purpose |
| --- | --- | --- | --- |
| `--listen` | `SUDORATIO_LISTEN` | `0.0.0.0:8787` | HTTP API + UI bind address |
| `--config-dir` | `SUDORATIO_CONFIG_DIR` | `./data` | session.sqlite3, config.json, clients/*.toml live here |
| `--peer-listen` | `SUDORATIO_PEER_LISTEN` | `0.0.0.0:0` | BT peer listener (empty disables) |
| `--announce-port` | `SUDORATIO_ANNOUNCE_PORT` | bound port | Override the `port=` value sent to trackers |
| `--max-upload-speed` | `SUDORATIO_MAX_UPLOAD_SPEED` | profile | Cap on simulated upload (bytes/s) |
| `--upload-ratio-target` | `SUDORATIO_UPLOAD_RATIO_TARGET` | profile | Stop simulating uploads past this ratio |
| `--max-active-torrents` | `SUDORATIO_MAX_ACTIVE_TORRENTS` | profile | Concurrency ceiling |
| `--pause-torrent-with-zero-leechers` | `SUDORATIO_PAUSE_TORRENT_WITH_ZERO_LEECHERS` | `true` | Skip torrents nobody is leeching |
| `--max-concurrent-announces` | `SUDORATIO_MAX_CONCURRENT_ANNOUNCES` | `0` | Tracker HTTP fan-out limit |

Run `sudoratio-server --help` for the full set, including HTTP client tuning (timeouts, pool sizing, redirects, keepalive). Most of these knobs are also live-tunable through `PATCH /api/config` without a restart.

## Client Profiles

Profiles control how sudoratio emulates a real BitTorrent client on every announce — peer-id format, tracker key generator, query parameter order, HTTP headers, URL-encoding rules, and reserved handshake bits. They're declarative TOML files at `sudoratio-core/src/profile/bundled/files/*.toml`.

### Bundled

| Family | Variants |
| --- | --- |
| qBittorrent | 3.3.16, 4.0.4, 4.1.9, 4.2.5, 4.3.9, 4.4.5, 4.5.4, 4.6.7, 5.2.0 |
| Transmission | 2.82, 2.92, 2.93, 2.94, 3.00, 4.05, 4.1.1 |
| Deluge | 1.3.15, 2.0.3, 2.1.1, 2.2.0 |
| rTorrent | 0.9.6 / libtorrent 0.13.6, 0.16.11 |
| BiglyBT | 4.0.0.0 |
| Vuze | 5.7.5.0 (project frozen) |
| BitTorrent Classic | 7.10.3_44429 |
| uTorrent | 3.2.2_28500, 3.5.4_44498 |
| Leap | 2.6.0.1 (discontinued) |

24 variants across 8 client families. Each profile id is `client@version` (e.g. `qbittorrent@5.2.0`).

### Schema model

One TOML file per family. The file declares the **base** config (query template, header set, peer-id algorithm, key generator, URL encoder) plus N `[[variant]]` blocks — one per shipped version — that overlay deltas on top of the base via RFC 7396 JSON Merge Patch (with a strategic-merge `headers_patch` for header upserts). Adding a new qBittorrent version is a 5-line `[[variant]]` block that supplies just the fields that changed (peer-id pattern, User-Agent string).

Users can register their own client docs through `POST /api/clients`, or extend a bundled client by posting a doc that contains *only* `[[variant]]` blocks against an existing client name (for example, adding `qbittorrent@5.3.0` once it ships, without re-shipping the binary). User extensions live alongside bundled variants under the same client tile in the UI.

## Persistence

```
<config-dir>/
├── config.json              # EngineConfig snapshot, written on every PATCH /api/config
├── session.sqlite3          # torrent rows + last 64 announce traces per torrent
└── clients/
    └── <client>.toml        # one file per user-registered client doc
```

The Docker compose recipe mounts this directory as the `sudoratio-data` named volume — back it up to preserve torrent state, identity caches, and user profiles across container rebuilds.

## Architecture

```
┌──────────────────────────┐      ┌──────────────────────────────┐
│  web/  (React + Vite)    │  →   │  sudoratio-server  (axum)    │
│  embedded via rust-embed │      │  auth · routes · sqlite      │
└──────────────────────────┘      └────────────┬─────────────────┘
                                               │
                                  ┌────────────▼─────────────────┐
                                  │  sudoratio-core              │
                                  │  engine · scheduler · wire   │
                                  │  announce · bandwidth · BT   │
                                  └──────────────────────────────┘
```

- **`sudoratio-core`** — the engine. Owns the announce scheduler, bandwidth simulator, BT wire protocol (handshake responder, outbound dialer), profile registry, and torrent state. No HTTP, no UI, no global state.
- **`sudoratio-server`** — axum router exposing `/api/{torrents,clients,stats,config,health,auth}`, password-derived bearer auth, per-row SQLite persistence, and the embedded SPA.
- **`web`** — TanStack Router + React 19 + Tailwind v4, built with Vite and Biome, served from the binary in production.

## Protocol coverage

| BEP | Title | Status | Why |
| --- | --- | --- | --- |
| BEP-3 | The BitTorrent Protocol | implemented | Tracker announce, peer handshake, swarm gating. |
| BEP-7 | IPv6 tracker extension | partial | We parse `peers6` from announce responses; no v6 listener yet. |
| BEP-10 | Extension Protocol (LTEP) | declared | Reserved bit set on handshake to match real clients; we don't reciprocate extended messages. |
| BEP-12 | Multitracker Metadata | implemented | Per-session tier shuffle + success-promotion to head of tier. |
| BEP-23 | Tracker peers list (compact) | implemented | Both `peers` (compact 6-byte) and the BEP-3 dict form are decoded. |
| BEP-27 | Private Torrents | respected | We never participate in DHT, PEX, or LSD — running them on `private=1` torrents would be a fingerprint that no compliant client produces. |
| BEP-5 | Mainline DHT | out of scope | Forbidden by BEP-27 on private torrents (the whole target audience). |
| BEP-9 / BEP-11 | Metadata exchange / PEX | out of scope | Not useful without a real wire-protocol backend, and BEP-11 is forbidden on private torrents. |
| BEP-14 | LSD | out of scope | Forbidden by BEP-27. |
| BEP-15 / BEP-29 / BEP-52 | UDP tracker / uTP / v2 | out of scope | Outside the project's scope (HTTP-only, no piece exchange). |

## Behavioural-fidelity status

For research and self-testing purposes, sudoratio's announce-side behaviour matches the on-the-wire output of the emulated client closely enough to pass passive announce-log inspection on a typical private tracker. It is **not** a complete behavioural twin: known gaps that an active or sophisticated tracker can use to distinguish it from a real client include JA3/JA4 TLS Client Hello fingerprinting (the underlying `rustls` handshake differs from the libcurl/OpenSSL stacks real clients ship with) and per-tracker announce-timing entropy (exact-cadence announces over long windows are themselves a fingerprint).

Tracker operators evaluating their own detection posture should treat these gaps as the easiest tells and the highest-signal places to look. Researchers comparing tracker-side anti-cheat heuristics should not expect this implementation to evade adversarial trackers and should design experiments accordingly.

## HTTP API

JSON under `/api/v1`, behind a bearer token issued by `POST /api/v1/auth/login`.

| Method | Path | Notes |
| --- | --- | --- |
| `GET` | `/api/v1/health` | Liveness |
| `GET` / `PATCH` | `/api/v1/config` | Live engine config (PATCH applies in-process) |
| `GET` / `POST` | `/api/v1/clients` | List variants / register a client doc |
| `GET` | `/api/v1/clients/{client}/source` | Raw TOML for a client family |
| `DELETE` | `/api/v1/clients/{client}` | Remove user variants of a client (bundled stay) |
| `POST` | `/api/v1/clients/variants/{id}/activate` | Activate one variant by `client@version` |
| `GET` / `POST` | `/api/v1/torrents` | List with live stats / upload `.torrent` (multipart) |
| `GET` / `PATCH` / `DELETE` | `/api/v1/torrents/{info_hash}` | Single torrent lifecycle |
| `POST` | `/api/v1/torrents/{info_hash}/{pause,resume,announce}` | Manual control |
| `GET` | `/api/v1/torrents/{info_hash}/announces` | Announce trace history (`?limit=&offset=`) |
| `GET` | `/api/v1/stats` | Aggregate counters |

## Embeddable core

The engine is a separate crate. To embed it in your own program:

```rust
use sudoratio_core::{Engine, EngineConfig};

let engine: std::sync::Arc<Engine> = Engine::new(EngineConfig::default());
engine.register_builtin_client(QBT_TOML).await?;
engine.set_active_profile("qbittorrent@5.2.0".into()).await?;
engine.start_peer_listener("0.0.0.0:0".parse()?).await?;

// add / pause / resume / remove torrents:
let id = engine.add_torrent_metainfo(meta).await?;
engine.pause_torrent(id).await?;
engine.announce_torrent(id, AnnounceEvent::None).await?;

// subscribe to announce traces (for your own persistence layer):
let mut rx = engine.subscribe_announces();
while let Some((tid, trace)) = rx.recv().await {
    persist(tid, trace);
}
```

Public API surface lives on `Engine` — the crate's lib.rs re-exports the few types you need (`AnnounceEvent`, `MetainfoTorrent`, `Torrent`, `EngineConfig`, etc.). `parse_metainfo` and `parse_client_doc` are exposed for integrations that don't go through the engine handle.

## Development

```bash
# Backend
cargo run -p sudoratio-server -- --config-dir ./data --password dev

# Frontend (dev server proxies /api to :8787)
cd web && bun install && bun run dev
```

Verification:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && bun run tsc --noEmit
```

The frontend is rebuilt into the release binary by `sudoratio-server/build.rs` whenever `SUDORATIO_BUILD_WEB=1` is set or a release profile is in use.

## Disclaimer

sudoratio is published for **educational and research purposes only**. It exists to study how the BitTorrent tracker protocol, peer-wire handshake, and tracker-side anti-cheat heuristics behave when announce traffic is decoupled from real piece exchange — a useful reference implementation for protocol learners, tracker operators evaluating their own detection posture, and security researchers exploring BitTorrent client fingerprinting.

The author does not endorse using this software to cheat ratio on private trackers, evade account requirements, or otherwise violate the terms of service of any tracker, public or private. Ratio manipulation is against the rules of essentially every private tracker and can result in warnings, throttling, or permanent bans — sometimes across linked-tracker networks. No detection-avoidance guarantees are made or implied.

You are solely responsible for how you use this software and for ensuring your usage complies with the rules of any tracker you connect it to and the laws of your jurisdiction. The author accepts no liability for account bans, lost upload credit, legal consequences, or any other damages arising from use of this project.

## License

[MIT](LICENSE).

