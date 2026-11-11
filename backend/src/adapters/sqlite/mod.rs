//! SQLite adapter: schema and migrations, the SQL behind the ports, `SqliteStore`.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use rusqlite::{params, Connection};
use std::path::Path;

use crate::application::clock::day_string;
use crate::application::ports::{LatestReading, LatestSnapshot, OverallStats, SeriesInfo};
use crate::domain::trends::DayStat;

mod store;
pub use store::{open_read_model, SqliteStore};

/// Current schema version, stored in `PRAGMA user_version`.
///
/// * 0 / 1 — v1.x databases: `snapshots` + `readings` only.
/// * 2     — adds `daily_stats`, the never-purged daily aggregates.
pub const SCHEMA_VERSION: i64 = 2;

pub fn init_db(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
    }
    let conn = Connection::open(path)
        .with_context(|| format!("Failed to open database at: {}", path.display()))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;
         CREATE TABLE IF NOT EXISTS snapshots (
             id        INTEGER PRIMARY KEY AUTOINCREMENT,
             ts        TEXT NOT NULL,
             cpu_load  REAL,
             gpu_load  REAL,
             load_cat  TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS readings (
             id          INTEGER PRIMARY KEY AUTOINCREMENT,
             snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
             hardware    TEXT NOT NULL,
             sensor      TEXT NOT NULL,
             value       REAL NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_snapshots_ts ON snapshots(ts);
         CREATE INDEX IF NOT EXISTS idx_readings_snapshot ON readings(snapshot_id);
         CREATE INDEX IF NOT EXISTS idx_readings_hw_sensor ON readings(hardware, sensor);
         CREATE TABLE IF NOT EXISTS daily_stats (
             day      TEXT    NOT NULL,
             hardware TEXT    NOT NULL,
             sensor   TEXT    NOT NULL,
             load_cat TEXT    NOT NULL,
             n        INTEGER NOT NULL,
             min      REAL    NOT NULL,
             median   REAL    NOT NULL,
             p95      REAL    NOT NULL,
             max      REAL    NOT NULL,
             avg      REAL    NOT NULL,
             PRIMARY KEY (day, hardware, sensor, load_cat)
         ) WITHOUT ROWID;",
    )
    .context("Failed to initialize database schema")?;
    migrate(&conn, path)?;
    Ok(conn)
}

// ──────────────────────────────────────────────────────────────
// Schema migration
// ──────────────────────────────────────────────────────────────

fn migrate(conn: &Connection, path: &Path) -> Result<()> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .context("Failed to read schema version")?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    backup_before_migration(conn, path)?;

    // Aggregate the whole existing history before anything can be purged.
    let tx = conn
        .unchecked_transaction()
        .context("Failed to start migration transaction")?;
    let days = rollup_inner(&tx, None, None).context("Failed to backfill daily_stats")?;
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
        .context("Failed to set schema version")?;
    tx.commit().context("Failed to commit migration")?;
    if days > 0 {
        tracing::info!(
            "Database migrated to schema v{SCHEMA_VERSION}: {days} daily aggregate(s) computed."
        );
    }
    Ok(())
}

/// Keeps a consistent copy of a pre-v2 database next to it (`<db>.v1.bak`).
///
/// Skipped for in-memory databases, empty databases and when a backup already exists.
fn backup_before_migration(conn: &Connection, path: &Path) -> Result<()> {
    if path.as_os_str() == ":memory:" {
        return Ok(());
    }
    let snapshots: i64 = conn.query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))?;
    if snapshots == 0 {
        return Ok(());
    }
    let mut backup = path.as_os_str().to_owned();
    backup.push(".v1.bak");
    let backup = std::path::PathBuf::from(backup);
    if backup.exists() {
        return Ok(());
    }
    // VACUUM INTO gives a consistent copy even if another connection is open.
    conn.execute("VACUUM INTO ?1", params![backup.to_string_lossy()])
        .with_context(|| format!("Failed to back up database to {}", backup.display()))?;
    tracing::info!("Pre-migration backup written to {}", backup.display());
    Ok(())
}

// ──────────────────────────────────────────────────────────────
// Daily aggregates
// ──────────────────────────────────────────────────────────────

/// Linear-interpolation quantile of an ascending-sorted, non-empty slice.
fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    let pos = q * (sorted.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// Recomputes `daily_stats` for the days in `[from_day, to_day)` (`YYYY-MM-DD`,
/// either bound optional). Must run inside the caller's transaction.
///
/// A day's aggregates are only ever recomputed while its raw readings are
/// complete: raw data is purged by whole days, and only after being aggregated.
fn rollup_inner(conn: &Connection, from_day: Option<&str>, to_day: Option<&str>) -> Result<usize> {
    let mut sql = String::from(
        "SELECT substr(s.ts, 1, 10), r.hardware, r.sensor, s.load_cat, r.value
         FROM readings r JOIN snapshots s ON s.id = r.snapshot_id WHERE 1 = 1",
    );
    let mut args: Vec<&str> = Vec::new();
    if let Some(from) = from_day {
        args.push(from);
        sql.push_str(&format!(" AND s.ts >= ?{}", args.len()));
    }
    if let Some(to) = to_day {
        args.push(to);
        sql.push_str(&format!(" AND s.ts < ?{}", args.len()));
    }
    // Sorted values per group make median / p95 a simple index lookup.
    sql.push_str(" ORDER BY 1, r.hardware, r.sensor, s.load_cat, r.value");

    let mut select = conn.prepare(&sql)?;
    let mut insert = conn.prepare(
        "INSERT OR REPLACE INTO daily_stats
             (day, hardware, sensor, load_cat, n, min, median, p95, max, avg)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )?;

    let mut rows = select.query(rusqlite::params_from_iter(args.iter()))?;
    let mut key: Option<(String, String, String, String)> = None;
    let mut values: Vec<f64> = Vec::new();
    let mut written = 0usize;

    let mut flush = |key: &(String, String, String, String), values: &mut Vec<f64>| -> Result<()> {
        if values.is_empty() {
            return Ok(());
        }
        let n = values.len();
        let sum: f64 = values.iter().sum();
        insert.execute(params![
            key.0,
            key.1,
            key.2,
            key.3,
            n as i64,
            values[0],
            quantile_sorted(values, 0.5),
            quantile_sorted(values, 0.95),
            values[n - 1],
            sum / n as f64,
        ])?;
        values.clear();
        Ok(())
    };

    while let Some(row) = rows.next()? {
        let current = (
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        );
        if key.as_ref() != Some(&current) {
            if let Some(previous) = &key {
                flush(previous, &mut values)?;
                written += 1;
            }
            key = Some(current);
        }
        values.push(row.get::<_, f64>(4)?);
    }
    if let Some(previous) = &key {
        flush(previous, &mut values)?;
        written += 1;
    }
    Ok(written)
}

/// Recomputes the daily aggregates of every day from `from_day` (`YYYY-MM-DD`,
/// inclusive) onwards. Idempotent. Returns the number of aggregates written.
pub fn rollup_since(conn: &Connection, from_day: &str) -> Result<usize> {
    let tx = conn.unchecked_transaction()?;
    let written = rollup_inner(&tx, Some(from_day), None).context("Failed to roll up readings")?;
    tx.commit()?;
    Ok(written)
}

pub fn insert_snapshot(
    conn: &Connection,
    ts: &str,
    cpu_load: Option<f64>,
    gpu_load: Option<f64>,
    load_cat: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO snapshots (ts, cpu_load, gpu_load, load_cat) VALUES (?1, ?2, ?3, ?4)",
        params![ts, cpu_load, gpu_load, load_cat],
    )
    .context("Failed to insert snapshot")?;
    Ok(conn.last_insert_rowid())
}

pub fn insert_reading(
    conn: &Connection,
    snapshot_id: i64,
    hardware: &str,
    sensor: &str,
    value: f64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO readings (snapshot_id, hardware, sensor, value) VALUES (?1, ?2, ?3, ?4)",
        params![snapshot_id, hardware, sensor, value],
    )
    .context("Failed to insert reading")?;
    Ok(())
}

pub fn get_overall_stats(conn: &Connection) -> Result<OverallStats> {
    let row = conn
        .query_row(
            "SELECT COUNT(*), MIN(ts), MAX(ts) FROM snapshots",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            },
        )
        .context("Failed to query overall stats")?;
    Ok(OverallStats {
        total_snapshots: row.0,
        first_ts: row.1,
        last_ts: row.2,
    })
}

/// Compacts the database file, reclaiming space freed by DELETE operations.
///
/// Must be called on an idle connection (no active transactions).
pub fn vacuum(conn: &Connection) -> Result<()> {
    conn.execute_batch("VACUUM")
        .context("Failed to vacuum database")?;
    Ok(())
}

/// Deletes the raw snapshots and readings of every whole day older than `days` days.
///
/// The days about to disappear are aggregated into `daily_stats` first, in the
/// same transaction, so the long-term history is never lost.
/// Returns the number of snapshots removed.
pub fn purge_old_snapshots(conn: &Connection, days: u32) -> Result<usize> {
    let cutoff = day_string(days);
    let tx = conn.unchecked_transaction()?;
    rollup_inner(&tx, None, Some(&cutoff)).context("Failed to roll up before purge")?;
    tx.execute(
        "DELETE FROM readings WHERE snapshot_id IN (SELECT id FROM snapshots WHERE ts < ?1)",
        params![cutoff],
    )
    .context("Failed to purge old readings")?;
    let removed = tx
        .execute("DELETE FROM snapshots WHERE ts < ?1", params![cutoff])
        .context("Failed to purge old snapshots")?;
    tx.commit()?;
    Ok(removed)
}

// ──────────────────────────────────────────────────────────────
// Dashboard queries
// ──────────────────────────────────────────────────────────────

pub fn get_series(conn: &Connection) -> Result<Vec<SeriesInfo>> {
    let mut stmt = conn.prepare(
        "SELECT hardware, sensor, load_cat, COUNT(*), MIN(day), MAX(day)
         FROM daily_stats GROUP BY hardware, sensor, load_cat
         ORDER BY hardware, sensor, load_cat",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SeriesInfo {
                hardware: r.get(0)?,
                sensor: r.get(1)?,
                load_cat: r.get(2)?,
                days: r.get(3)?,
                first_day: r.get(4)?,
                last_day: r.get(5)?,
            })
        })
        .context("Failed to query the series list")?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to read a series row")
}

fn day_stat_from_row(r: &rusqlite::Row<'_>, first_col: usize) -> rusqlite::Result<Option<DayStat>> {
    let day: String = r.get(first_col)?;
    let Ok(day) = NaiveDate::parse_from_str(&day, "%Y-%m-%d") else {
        return Ok(None);
    };
    Ok(Some(DayStat {
        day,
        n: r.get(first_col + 1)?,
        min: r.get(first_col + 2)?,
        median: r.get(first_col + 3)?,
        p95: r.get(first_col + 4)?,
        max: r.get(first_col + 5)?,
    }))
}

/// Daily aggregates of one series, oldest first, for the days strictly before
/// `before_day` (`YYYY-MM-DD` — pass today to leave out the incomplete current day).
pub fn get_daily_stats(
    conn: &Connection,
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    before_day: &str,
) -> Result<Vec<DayStat>> {
    let mut stmt = conn.prepare(
        "SELECT day, n, min, median, p95, max FROM daily_stats
         WHERE hardware = ?1 AND sensor = ?2 AND load_cat = ?3 AND day < ?4
         ORDER BY day",
    )?;
    let rows = stmt
        .query_map(params![hardware, sensor, load_cat, before_day], |r| {
            day_stat_from_row(r, 0)
        })
        .context("Failed to query daily stats")?;
    let mut out = Vec::new();
    for row in rows {
        out.extend(row.context("Failed to read a daily stats row")?);
    }
    Ok(out)
}

/// Daily aggregates of every sensor at one load category, grouped per
/// (hardware, sensor), same day rule as [`get_daily_stats`].
pub fn get_daily_stats_for_load(
    conn: &Connection,
    load_cat: &str,
    before_day: &str,
) -> Result<Vec<(String, String, Vec<DayStat>)>> {
    let mut stmt = conn.prepare(
        "SELECT hardware, sensor, day, n, min, median, p95, max FROM daily_stats
         WHERE load_cat = ?1 AND day < ?2
         ORDER BY hardware, sensor, day",
    )?;
    let rows = stmt
        .query_map(params![load_cat, before_day], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                day_stat_from_row(r, 2)?,
            ))
        })
        .context("Failed to query daily stats")?;
    let mut out: Vec<(String, String, Vec<DayStat>)> = Vec::new();
    for row in rows {
        let (hardware, sensor, stat) = row.context("Failed to read a daily stats row")?;
        let Some(stat) = stat else { continue };
        match out.last_mut() {
            Some((h, s, stats)) if *h == hardware && *s == sensor => stats.push(stat),
            _ => out.push((hardware, sensor, vec![stat])),
        }
    }
    Ok(out)
}

pub fn get_latest_snapshot(conn: &Connection) -> Result<Option<LatestSnapshot>> {
    let head = conn.query_row(
        "SELECT id, ts, cpu_load, gpu_load, load_cat FROM snapshots ORDER BY ts DESC, id DESC LIMIT 1",
        [],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<f64>>(2)?,
                r.get::<_, Option<f64>>(3)?,
                r.get::<_, String>(4)?,
            ))
        },
    );
    let (id, ts, cpu_load, gpu_load, load_cat) = match head {
        Ok(head) => head,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e).context("Failed to query the latest snapshot"),
    };
    let mut stmt = conn.prepare(
        "SELECT hardware, sensor, value FROM readings WHERE snapshot_id = ?1
         ORDER BY hardware, sensor",
    )?;
    let readings = stmt
        .query_map(params![id], |r| {
            Ok(LatestReading {
                hardware: r.get(0)?,
                sensor: r.get(1)?,
                value: r.get(2)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("Failed to read the latest readings")?;
    Ok(Some(LatestSnapshot {
        ts,
        cpu_load,
        gpu_load,
        load_cat,
        readings,
    }))
}

/// Opens an existing database for reading only (the dashboard), without
/// creating it or touching its schema.
pub fn open_read_only(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("Failed to open database read-only: {}", path.display()))?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")?;
    Ok(conn)
}
