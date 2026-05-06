//! SQLite session persistence: WAL, foreign keys, STRICT tables.
//!
//! The schema is keyed by `info_hash` (40-hex). Engine-only runtime fields live next to the
//! observable ones in one row to avoid the multi-table juggling of the previous layout. Trace
//! payloads (`request`, `response`) and the tracker list are stored as JSON text — straightforward
//! and tool-friendly. `PRAGMA user_version = 1` is the baseline; later schema bumps will add
//! migrations from this version.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use sudoratio_core::{
    AnnounceEvent, AnnounceRequestTrace, AnnounceResponseTrace, AnnounceTrace, StopReason, Torrent,
    TorrentId, TorrentRuntime, TorrentState, TrackersHttp,
};

const SCHEMA_VERSION: i32 = 1;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS torrent (
    info_hash TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    size INTEGER NOT NULL,
    left_bytes INTEGER NOT NULL,
    uploaded INTEGER NOT NULL,
    download_speed INTEGER NOT NULL,
    upload_speed INTEGER NOT NULL,
    seeders INTEGER,
    leechers INTEGER,
    state TEXT NOT NULL,
    download_before_seed INTEGER NOT NULL,
    trackers TEXT NOT NULL,
    announce_interval INTEGER,
    min_announce_interval INTEGER,
    last_announced_at INTEGER,
    tier_index INTEGER NOT NULL,
    intra_index INTEGER NOT NULL,
    announce_count INTEGER NOT NULL,
    last_announce_interval_seconds INTEGER NOT NULL,
    last_successful_announce_unix_ms INTEGER NOT NULL,
    consecutive_fails INTEGER NOT NULL,
    reason TEXT,
    queue_position INTEGER NOT NULL,
    added_at INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS announce (
    info_hash TEXT NOT NULL REFERENCES torrent(info_hash) ON DELETE CASCADE,
    seq INTEGER NOT NULL,
    tracker_index INTEGER NOT NULL,
    event TEXT NOT NULL,
    announced_at INTEGER NOT NULL,
    success INTEGER NOT NULL,
    error_code TEXT,
    error_message TEXT,
    request TEXT NOT NULL,
    response TEXT NOT NULL,
    PRIMARY KEY (info_hash, seq)
) STRICT;

CREATE INDEX IF NOT EXISTS announce_by_info_hash ON announce(info_hash, seq);
"#;

#[derive(Clone)]
pub struct Session {
    conn: Arc<Mutex<Connection>>,
}

impl Session {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("open session database {}", path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);
        if v > SCHEMA_VERSION {
            anyhow::bail!("session db user_version {v} is newer than supported {SCHEMA_VERSION}");
        }
        if v < SCHEMA_VERSION {
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Insert or update a single torrent row. Does **not** touch the announce table — announce
    /// rows are appended incrementally by [`Self::append_announce`] as the engine emits traces.
    pub fn upsert_torrent(&self, t: &Torrent) -> Result<()> {
        let info_hash = t
            .info_hash
            .as_ref()
            .context("upsert_torrent: missing info_hash")?;
        let mut conn = self.conn.lock().expect("session lock");
        let tx = conn.transaction()?;
        upsert_one(&tx, info_hash, t)?;
        tx.commit()?;
        Ok(())
    }

    /// Append one announce trace. `seq` is auto-assigned as `MAX(seq)+1` per torrent so traces
    /// stay ordered without requiring callers to track sequence numbers.
    pub fn append_announce(&self, info_hash: &str, a: &AnnounceTrace) -> Result<()> {
        let mut conn = self.conn.lock().expect("session lock");
        let tx = conn.transaction()?;
        let next_seq: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(seq), -1) + 1 FROM announce WHERE info_hash = ?1",
                params![info_hash],
                |r| r.get(0),
            )
            .unwrap_or(0);
        insert_announce_row(&tx, info_hash, next_seq, a)?;
        tx.commit()?;
        Ok(())
    }

    /// Page through announce history (newest first). `offset = 0` is the latest trace.
    /// Returns `(items, total)` where `total` is the full count for this torrent.
    pub fn read_announces(
        &self,
        info_hash: &str,
        limit: usize,
        offset: usize,
    ) -> Result<(Vec<AnnounceTrace>, usize)> {
        let conn = self.conn.lock().expect("session lock");
        let total: i64 = conn.query_row(
            "SELECT COUNT(*) FROM announce WHERE info_hash = ?1",
            params![info_hash],
            |r| r.get(0),
        )?;
        let mut stmt = conn.prepare(
            r#"SELECT tracker_index, event, announced_at, success, error_code, error_message,
                  request, response FROM announce WHERE info_hash = ?1
                  ORDER BY seq DESC LIMIT ?2 OFFSET ?3"#,
        )?;
        let rows = stmt.query_map(
            params![info_hash, limit as i64, offset as i64],
            map_announce_row,
        )?;
        let items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok((items, total as usize))
    }

    /// FK `ON DELETE CASCADE` on `announce.info_hash` removes the trace history along with it.
    pub fn delete_torrent(&self, info_hash: &str) -> Result<()> {
        let conn = self.conn.lock().expect("session lock");
        conn.execute(
            "DELETE FROM torrent WHERE info_hash = ?1",
            params![info_hash],
        )?;
        Ok(())
    }

    /// Bulk upsert (single transaction). Non-destructive: rows not in `snapshot` are preserved.
    /// Removal happens only via the explicit `delete_torrent` path (route handler).
    pub fn upsert_many(&self, snapshot: &[Torrent]) -> Result<()> {
        if snapshot.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("session lock");
        let tx = conn.transaction()?;
        for t in snapshot {
            let Some(info_hash) = t.info_hash.as_ref() else {
                continue;
            };
            upsert_one(&tx, info_hash, t)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn read_all(&self) -> Result<Vec<Torrent>> {
        let conn = self.conn.lock().expect("session lock");
        let mut stmt = conn.prepare(
            r#"SELECT info_hash, name, size, left_bytes, uploaded,
               download_speed, upload_speed, seeders, leechers, state, download_before_seed,
               trackers, announce_interval, min_announce_interval, last_announced_at,
               tier_index, intra_index, announce_count, last_announce_interval_seconds,
               last_successful_announce_unix_ms, consecutive_fails, reason, queue_position
               FROM torrent ORDER BY queue_position ASC, added_at ASC"#,
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let info_hash: String = row.get(0)?;
            let id = TorrentId::from_hex(&info_hash)
                .ok_or_else(|| anyhow::anyhow!("invalid info_hash hex: {info_hash}"))?;
            let trackers_json: String = row.get(11)?;
            let trackers: TrackersHttp =
                serde_json::from_str(&trackers_json).context("trackers json")?;
            let download_before_seed = row.get::<_, i64>(10)? != 0;
            let left = row.get::<_, i64>(3)? as u64;
            let reason = row
                .get::<_, Option<String>>(21)?
                .as_deref()
                .map(parse_stop_reason);
            let state = parse_state(&row.get::<_, String>(9)?, reason)?;
            let min_iv: Option<i64> = row.get(13)?;
            let info_hash_bytes = decode_info_hash(&info_hash);
            out.push(Torrent {
                id,
                info_hash: Some(info_hash),
                name: row.get(1)?,
                size: Some(row.get::<_, i64>(2)? as u64),
                downloaded: None,
                left: Some(left),
                uploaded: Some(row.get::<_, i64>(4)? as u64),
                download_speed: Some(row.get::<_, i64>(5)? as u64),
                upload_speed: Some(row.get::<_, i64>(6)? as u64),
                seeders: row
                    .get::<_, Option<i64>>(7)?
                    .and_then(|v| u32::try_from(v).ok()),
                leechers: row
                    .get::<_, Option<i64>>(8)?
                    .and_then(|v| u32::try_from(v).ok()),
                state,
                reason,
                download_before_seed,
                trackers,
                announce_interval: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
                min_announce_interval: min_iv.map(|v| v as u32),
                last_announced_at: row.get::<_, Option<i64>>(14)?.map(|v| v as u64),
                queue_position: row.get::<_, i64>(22)? as u32,
                runtime: TorrentRuntime {
                    info_hash_bytes,
                    tier_index: row.get::<_, i64>(15)? as usize,
                    intra_index: row.get::<_, i64>(16)? as usize,
                    announce_count: row.get::<_, i64>(17)? as u64,
                    last_announce_interval_seconds: row.get::<_, i64>(18)? as u32,
                    last_min_interval_seconds: min_iv.unwrap_or(0) as u32,
                    last_successful_announce_unix_ms: row.get::<_, i64>(19)? as u64,
                    consecutive_fails: row.get::<_, i64>(20)? as u32,
                },
            });
        }
        Ok(out)
    }
}

fn upsert_one(tx: &rusqlite::Transaction<'_>, info_hash: &str, t: &Torrent) -> Result<()> {
    let trackers_json = serde_json::to_string(&t.trackers)?;
    let added_at: i64 = tx
        .query_row(
            "SELECT added_at FROM torrent WHERE info_hash = ?1",
            params![info_hash],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        });
    tx.execute(
        r#"INSERT INTO torrent (
            info_hash, name, size, left_bytes, uploaded,
            download_speed, upload_speed, seeders, leechers, state, download_before_seed,
            trackers, announce_interval, min_announce_interval, last_announced_at,
            tier_index, intra_index, announce_count, last_announce_interval_seconds,
            last_successful_announce_unix_ms, consecutive_fails, reason,
            queue_position, added_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                  ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
        ON CONFLICT(info_hash) DO UPDATE SET
            name=excluded.name, size=excluded.size,
            left_bytes=excluded.left_bytes,
            uploaded=excluded.uploaded, download_speed=excluded.download_speed,
            upload_speed=excluded.upload_speed, seeders=excluded.seeders,
            leechers=excluded.leechers, state=excluded.state,
            download_before_seed=excluded.download_before_seed, trackers=excluded.trackers,
            announce_interval=excluded.announce_interval,
            min_announce_interval=excluded.min_announce_interval,
            last_announced_at=excluded.last_announced_at,
            tier_index=excluded.tier_index, intra_index=excluded.intra_index,
            announce_count=excluded.announce_count,
            last_announce_interval_seconds=excluded.last_announce_interval_seconds,
            last_successful_announce_unix_ms=excluded.last_successful_announce_unix_ms,
            consecutive_fails=excluded.consecutive_fails, reason=excluded.reason,
            queue_position=excluded.queue_position"#,
        params![
            info_hash,
            t.name,
            t.size.unwrap_or(0) as i64,
            t.left.unwrap_or(0) as i64,
            t.uploaded.unwrap_or(0) as i64,
            t.download_speed.unwrap_or(0) as i64,
            t.upload_speed.unwrap_or(0) as i64,
            t.seeders.map(|v| v as i64),
            t.leechers.map(|v| v as i64),
            t.state.as_str(),
            t.download_before_seed as i64,
            trackers_json,
            t.announce_interval.map(|v| v as i64),
            t.min_announce_interval.map(|v| v as i64),
            t.last_announced_at.map(|v| v as i64),
            t.runtime.tier_index as i64,
            t.runtime.intra_index as i64,
            t.runtime.announce_count as i64,
            t.runtime.last_announce_interval_seconds as i64,
            t.runtime.last_successful_announce_unix_ms as i64,
            t.runtime.consecutive_fails as i64,
            t.reason.map(|r| r.as_str()),
            t.queue_position as i64,
            added_at,
        ],
    )?;
    Ok(())
}

fn insert_announce_row(
    tx: &rusqlite::Transaction<'_>,
    info_hash: &str,
    seq: i64,
    a: &AnnounceTrace,
) -> Result<()> {
    let req = serde_json::to_string(&a.request)?;
    let resp = serde_json::to_string(&a.response)?;
    tx.execute(
        r#"INSERT INTO announce (info_hash, seq, tracker_index, event, announced_at, success,
              error_code, error_message, request, response)
              VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
        params![
            info_hash,
            seq,
            a.tracker_index as i64,
            event_str(a.event),
            a.announced_at as i64,
            a.success as i64,
            a.error_code,
            a.error_message,
            req,
            resp,
        ],
    )?;
    Ok(())
}

/// Decode one announce row into an `AnnounceTrace`. Column order must match the SELECT in
/// [`Session::read_announces`]: `tracker_index, event, announced_at, success, error_code,
/// error_message, request, response`.
fn map_announce_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<AnnounceTrace> {
    let req: String = r.get(6)?;
    let resp: String = r.get(7)?;
    let event: String = r.get(1)?;
    Ok(AnnounceTrace {
        tracker_index: r.get::<_, i64>(0)? as usize,
        event: parse_event(&event).unwrap_or(AnnounceEvent::None),
        announced_at: r.get::<_, i64>(2)? as u64,
        success: r.get::<_, i64>(3)? != 0,
        error_code: r.get::<_, Option<String>>(4)?,
        error_message: r.get::<_, Option<String>>(5)?,
        request: serde_json::from_str::<AnnounceRequestTrace>(&req)
            .unwrap_or_else(|_| panic!("announce request json corrupt")),
        response: serde_json::from_str::<AnnounceResponseTrace>(&resp)
            .unwrap_or_else(|_| panic!("announce response json corrupt")),
    })
}

fn parse_state(s: &str, reason: Option<StopReason>) -> Result<TorrentState> {
    match s {
        "downloading" => Ok(TorrentState::Downloading),
        "seeding" => Ok(TorrentState::Seeding),
        "queued" => Ok(TorrentState::Queued),
        "stopped" => Ok(TorrentState::Stopped(reason.unwrap_or(StopReason::User))),
        _ => anyhow::bail!("unknown state {s:?}"),
    }
}

fn parse_stop_reason(s: &str) -> StopReason {
    match s {
        "upload_ratio" => StopReason::UploadRatio,
        "no_leechers" => StopReason::NoLeechers,
        "tracker_failed" => StopReason::TrackerFailed,
        _ => StopReason::User,
    }
}

fn event_str(e: AnnounceEvent) -> &'static str {
    match e {
        AnnounceEvent::None => "none",
        AnnounceEvent::Started => "started",
        AnnounceEvent::Stopped => "stopped",
        AnnounceEvent::Completed => "completed",
    }
}

fn parse_event(s: &str) -> Result<AnnounceEvent> {
    match s {
        "started" => Ok(AnnounceEvent::Started),
        "stopped" => Ok(AnnounceEvent::Stopped),
        "completed" => Ok(AnnounceEvent::Completed),
        "none" => Ok(AnnounceEvent::None),
        _ => anyhow::bail!("unknown announce event {s:?}"),
    }
}

fn decode_info_hash(s: &str) -> Option<[u8; 20]> {
    if s.len() != 40 {
        return None;
    }
    let mut bytes = [0u8; 20];
    hex::decode_to_slice(s, &mut bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sudoratio_core::TrackersHttp;

    fn sample_torrent(hash: &str) -> Torrent {
        Torrent {
            id: TorrentId::from_hex(hash).expect("test info_hash hex"),
            info_hash: Some(hash.to_string()),
            name: "sample".into(),
            size: Some(1024),
            downloaded: Some(0),
            uploaded: Some(0),
            left: Some(1024),
            download_speed: Some(0),
            upload_speed: Some(0),
            seeders: Some(3),
            leechers: Some(2),
            state: TorrentState::Seeding,
            reason: None,
            download_before_seed: false,
            trackers: TrackersHttp {
                tiers: vec![vec!["http://t/announce".into()]],
            },
            announce_interval: Some(60),
            min_announce_interval: Some(30),
            last_announced_at: Some(1234567890),
            queue_position: 0,
            runtime: TorrentRuntime::default(),
        }
    }

    #[test]
    fn roundtrip_upsert_then_read() {
        let dir = tempdir_inside_target();
        let db_path = dir.join("session.sqlite");
        let s = Session::open(&db_path).unwrap();
        let hash = "a".repeat(40);
        s.upsert_torrent(&sample_torrent(&hash)).unwrap();
        let rows = s.read_all().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].info_hash.as_deref(), Some(hash.as_str()));
        assert_eq!(rows[0].size, Some(1024));
    }

    #[test]
    fn delete_removes_row_and_announces() {
        let dir = tempdir_inside_target();
        let db_path = dir.join("session2.sqlite");
        let s = Session::open(&db_path).unwrap();
        let hash = "b".repeat(40);
        s.upsert_torrent(&sample_torrent(&hash)).unwrap();
        s.delete_torrent(&hash).unwrap();
        assert!(s.read_all().unwrap().is_empty());
    }

    fn tempdir_inside_target() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("sudoratio-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }
}
