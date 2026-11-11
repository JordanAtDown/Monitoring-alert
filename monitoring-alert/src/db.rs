use anyhow::{Context, Result};
use chrono::{Duration, Local};
use rusqlite::{params, Connection};
use std::path::Path;

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

/// Returns today's local date and the one of `days_back` days ago, as `YYYY-MM-DD`.
pub fn day_string(days_back: u32) -> String {
    (Local::now().date_naive() - Duration::days(i64::from(days_back)))
        .format("%Y-%m-%d")
        .to_string()
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

pub struct OverallStats {
    pub total_snapshots: i64,
    pub first_ts: Option<String>,
    pub last_ts: Option<String>,
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
