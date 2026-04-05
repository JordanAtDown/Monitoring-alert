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

pub struct CategoryCount {
    pub load_cat: String,
    pub count: i64,
}

pub fn get_category_distribution(conn: &Connection, days: u32) -> Result<Vec<CategoryCount>> {
    let mut stmt = conn.prepare(
        "SELECT load_cat, COUNT(*) as cnt FROM snapshots
         WHERE datetime(ts) >= datetime('now', ?1)
         GROUP BY load_cat",
    )?;
    let days_str = format!("-{} days", days);
    let rows = stmt
        .query_map(params![days_str], |row| {
            Ok(CategoryCount {
                load_cat: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .context("Failed to query category distribution")?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read category row")?);
    }
    Ok(result)
}

pub struct SensorKey {
    pub hardware: String,
    pub sensor: String,
}

pub fn get_distinct_sensors(conn: &Connection) -> Result<Vec<SensorKey>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT hardware, sensor FROM readings ORDER BY hardware, sensor")?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SensorKey {
                hardware: row.get(0)?,
                sensor: row.get(1)?,
            })
        })
        .context("Failed to query distinct sensors")?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.context("Failed to read sensor row")?);
    }
    Ok(result)
}

pub fn get_avg_for_window(
    conn: &Connection,
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    days: u32,
    offset_days: u32,
) -> Result<Option<f64>> {
    let start = format!("-{} days", days + offset_days);
    let end = format!("-{} days", offset_days);
    let result = conn
        .query_row(
            "SELECT AVG(r.value)
             FROM readings r
             JOIN snapshots s ON r.snapshot_id = s.id
             WHERE r.hardware = ?1 AND r.sensor = ?2 AND s.load_cat = ?3
               AND datetime(s.ts) >= datetime('now', ?4)
               AND datetime(s.ts) < datetime('now', ?5)",
            params![hardware, sensor, load_cat, start, end],
            |row| row.get::<_, Option<f64>>(0),
        )
        .context("Failed to query average for window")?;
    Ok(result)
}
