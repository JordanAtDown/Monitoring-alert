use anyhow::Result;

use crate::db::{CategoryCount, OverallStats, SensorKey};

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

    fn get_distinct_sensors(&self) -> Result<Vec<SensorKey>>;

    fn get_avg_for_window(
        &self,
        hardware: &str,
        sensor: &str,
        load_cat: &str,
        days: u32,
        offset_days: u32,
    ) -> Result<Option<f64>>;

    fn get_overall_stats(&self) -> Result<OverallStats>;

    fn get_category_distribution(&self, days: u32) -> Result<Vec<CategoryCount>>;

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

    fn get_distinct_sensors(&self) -> Result<Vec<SensorKey>> {
        crate::db::get_distinct_sensors(&self.0)
    }

    fn get_avg_for_window(
        &self,
        hardware: &str,
        sensor: &str,
        load_cat: &str,
        days: u32,
        offset_days: u32,
    ) -> Result<Option<f64>> {
        crate::db::get_avg_for_window(&self.0, hardware, sensor, load_cat, days, offset_days)
    }

    fn get_overall_stats(&self) -> Result<OverallStats> {
        crate::db::get_overall_stats(&self.0)
    }

    fn get_category_distribution(&self, days: u32) -> Result<Vec<CategoryCount>> {
        crate::db::get_category_distribution(&self.0, days)
    }

    fn purge_old_data(&self, days: u32) -> Result<usize> {
        crate::db::purge_old_snapshots(&self.0, days)
    }
}
