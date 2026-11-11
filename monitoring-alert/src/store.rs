use anyhow::Result;

use crate::db::OverallStats;

// ──────────────────────────────────────────────────────────────
// Storage abstraction
// ──────────────────────────────────────────────────────────────

/// Abstraction over the temperature data store.
///
/// Implement this trait to support different storage backends
/// (SQLite, in-memory mock for tests, remote store, etc.).
pub trait TemperatureStore {
    fn insert_snapshot(
        &self,
        ts: &str,
        cpu_load: Option<f64>,
        gpu_load: Option<f64>,
        load_cat: &str,
    ) -> Result<i64>;

    fn insert_reading(
        &self,
        snapshot_id: i64,
        hardware: &str,
        sensor: &str,
        value: f64,
    ) -> Result<()>;

    fn get_overall_stats(&self) -> Result<OverallStats>;

    /// Delete snapshots (and their readings) older than `days` days.
    /// Returns the number of snapshots removed.
    fn purge_old_data(&self, days: u32) -> Result<usize>;
}

// ──────────────────────────────────────────────────────────────
// SQLite implementation
// ──────────────────────────────────────────────────────────────

/// SQLite-backed implementation of [`TemperatureStore`].
///
/// Wraps a `rusqlite::Connection`; callers never interact with
/// the connection directly, keeping the SQLite dependency internal.
pub struct SqliteStore(rusqlite::Connection);

impl SqliteStore {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self(conn)
    }
}

impl TemperatureStore for SqliteStore {
    fn insert_snapshot(
        &self,
        ts: &str,
        cpu_load: Option<f64>,
        gpu_load: Option<f64>,
        load_cat: &str,
    ) -> Result<i64> {
        crate::db::insert_snapshot(&self.0, ts, cpu_load, gpu_load, load_cat)
    }

    fn insert_reading(
        &self,
        snapshot_id: i64,
        hardware: &str,
        sensor: &str,
        value: f64,
    ) -> Result<()> {
        crate::db::insert_reading(&self.0, snapshot_id, hardware, sensor, value)
    }

    fn get_overall_stats(&self) -> Result<OverallStats> {
        crate::db::get_overall_stats(&self.0)
    }

    fn purge_old_data(&self, days: u32) -> Result<usize> {
        crate::db::purge_old_snapshots(&self.0, days)
    }
}
