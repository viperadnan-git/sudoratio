//! SQLite session persistence for presets, torrents, and announce traces.
//!
//! Owned by the engine: [`Engine::new`] opens the DB, runs the schema, restores presets and
//! torrents, and routes write-throughs from every state-changing engine method.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};

use crate::preset::{PresetPolicy, PresetSnapshot};
use crate::torrent::{
    AnnounceEvent, AnnounceRequestTrace, AnnounceResponseTrace, AnnounceTrace, StopReason,
    Torrent, TorrentId, TorrentRuntime, TorrentState, TrackersHttp,
};

const SCHEMA_VERSION: i32 = 1;

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS preset (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    color TEXT NOT NULL,
    is_default INTEGER NOT NULL,
    policy_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
) STRICT;

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
    added_at INTEGER NOT NULL,
    preset_id TEXT NOT NULL DEFAULT 'default'
) STRICT;

CREATE INDEX IF NOT EXISTS torrent_by_preset ON torrent(preset_id);

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
pub(crate) struct Persistence {
    conn: Arc<Mutex<Connection>>,
}

impl Persistence {
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

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA_SQL)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn save_preset(&self, p: &PresetSnapshot) -> Result<()> {
        let conn = self.conn.lock().expect("session lock");
        let policy_json = serde_json::to_string(&p.policy)?;
        conn.execute(
            r#"INSERT INTO preset (id, name, color, is_default, policy_json, created_at, updated_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(id) DO UPDATE SET
                 name = excluded.name,
                 color = excluded.color,
                 is_default = excluded.is_default,
                 policy_json = excluded.policy_json,
                 updated_at = excluded.updated_at"#,
            params![
                p.id,
                p.name,
                p.color,
                p.is_default as i64,
                policy_json,
                p.created_at_ms as i64,
                p.updated_at_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn delete_preset(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("session lock");
        conn.execute("DELETE FROM preset WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn read_presets(&self) -> Result<Vec<PresetSnapshot>> {
        let conn = self.conn.lock().expect("session lock");
        let mut stmt = conn.prepare(
            r#"SELECT id, name, color, is_default, policy_json, created_at, updated_at
               FROM preset ORDER BY is_default DESC, id ASC"#,
        )?;
        let mut rows = stmt.query([])?;
        let mut out = Vec::new();
        while let Some(row) = rows.next()? {
            let policy_json: String = row.get(4)?;
            let policy: PresetPolicy = serde_json::from_str(&policy_json)
                .with_context(|| "preset policy_json corrupt")?;
            out.push(PresetSnapshot {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                is_default: row.get::<_, i64>(3)? != 0,
                policy,
                created_at_ms: row.get::<_, i64>(5)? as u64,
                updated_at_ms: row.get::<_, i64>(6)? as u64,
            });
        }
        Ok(out)
    }

    pub fn save_torrent(&self, t: &Torrent) -> Result<()> {
        let info_hash = t
            .info_hash
            .as_ref()
            .context("save_torrent: missing info_hash")?;
        let mut conn = self.conn.lock().expect("session lock");
        let tx = conn.transaction()?;
        upsert_one(&tx, info_hash, t)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_torrent(&self, info_hash: &str) -> Result<()> {
        let conn = self.conn.lock().expect("session lock");
        conn.execute(
            "DELETE FROM torrent WHERE info_hash = ?1",
            params![info_hash],
        )?;
        Ok(())
    }

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

    pub fn read_torrents(&self) -> Result<Vec<Torrent>> {
        let conn = self.conn.lock().expect("session lock");
        let mut stmt = conn.prepare(
            r#"SELECT info_hash, name, size, left_bytes, uploaded,
               download_speed, upload_speed, seeders, leechers, state, download_before_seed,
               trackers, announce_interval, min_announce_interval, last_announced_at,
               tier_index, intra_index, announce_count, last_announce_interval_seconds,
               last_successful_announce_unix_ms, consecutive_fails, reason, queue_position,
               preset_id
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
            let info_hash_bytes = Some(id.0);
            let preset_id: String = row.get(23)?;
            out.push(Torrent {
                id,
                info_hash: Some(info_hash),
                preset_id,
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
            queue_position, added_at, preset_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                  ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
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
            queue_position=excluded.queue_position,
            preset_id=excluded.preset_id"#,
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
            t.preset_id,
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
        "tiny_swarm" => StopReason::TinySwarm,
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

