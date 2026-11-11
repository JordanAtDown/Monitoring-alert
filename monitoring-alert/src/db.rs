use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::Path;

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
         CREATE INDEX IF NOT EXISTS idx_readings_hw_sensor ON readings(hardware, sensor);",
    )
    .context("Failed to initialize database schema")?;
    Ok(conn)
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

/// Deletes snapshots (and their readings) older than `days` days.
/// Returns the number of snapshots removed.
pub fn purge_old_snapshots(conn: &Connection, days: u32) -> Result<usize> {
    let cutoff = format!("-{} days", days);
    conn.execute(
        "DELETE FROM readings WHERE snapshot_id IN (
             SELECT id FROM snapshots WHERE datetime(ts) < datetime('now', ?1)
         )",
        params![cutoff],
    )
    .context("Failed to purge old readings")?;
    let removed = conn
        .execute(
            "DELETE FROM snapshots WHERE datetime(ts) < datetime('now', ?1)",
            params![cutoff],
        )
        .context("Failed to purge old snapshots")?;
    Ok(removed)
}
