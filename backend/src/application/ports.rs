//! The interfaces the application needs from the outside world.

use anyhow::Result;
use serde::Serialize;

use crate::domain::model::{NewSnapshot, SnapshotData};
use crate::domain::trends::DayStat;

/// Where the temperatures come from (LibreHardwareMonitor today).
pub trait SensorSource: Send + Sync {
    /// Polls every sensor once.
    fn read(&self) -> Result<SnapshotData>;
    /// Where the source lives, for log messages (e.g. `127.0.0.1:8085`).
    fn describe(&self) -> String;
}

/// Figures about the raw snapshots.
#[derive(Debug, Clone)]
pub struct OverallStats {
    pub total_snapshots: i64,
    pub first_ts: Option<String>,
    pub last_ts: Option<String>,
}

/// One (sensor, load category) series present in the daily aggregates.
#[derive(Debug, Serialize)]
pub struct SeriesInfo {
    pub hardware: String,
    pub sensor: String,
    pub load_cat: String,
    pub days: i64,
    pub first_day: String,
    pub last_day: String,
}

#[derive(Debug, Serialize)]
pub struct LatestReading {
    pub hardware: String,
    pub sensor: String,
    pub value: f64,
}

/// The most recent snapshot with all its readings.
#[derive(Debug, Serialize)]
pub struct LatestSnapshot {
    pub ts: String,
    pub cpu_load: Option<f64>,
    pub gpu_load: Option<f64>,
    pub load_cat: String,
    pub readings: Vec<LatestReading>,
}

/// Write side: what the collector needs to persist and maintain the history.
pub trait TemperatureStore {
    /// Stores a snapshot and its readings atomically; returns the snapshot id.
    fn record_snapshot(&self, snapshot: &NewSnapshot<'_>) -> Result<i64>;

    fn overall_stats(&self) -> Result<OverallStats>;

    /// Deletes the raw snapshots and readings of every whole day older than `days`
    /// days, after aggregating them: the daily aggregates are kept. Returns the
    /// number of snapshots removed.
    fn purge_old_data(&self, days: u32) -> Result<usize>;

    /// Recomputes the daily aggregates of every day from `from_day` (`YYYY-MM-DD`) onwards.
    fn rollup_since(&self, from_day: &str) -> Result<usize>;

    /// Compacts the storage and releases the free space.
    fn vacuum(&self) -> Result<()>;
}

/// Read side: what the dashboard needs to show.
pub trait ReadModel {
    fn overall_stats(&self) -> Result<OverallStats>;
    fn latest_snapshot(&self) -> Result<Option<LatestSnapshot>>;
    fn series(&self) -> Result<Vec<SeriesInfo>>;

    /// Daily aggregates of one series, oldest first, for the days strictly before
    /// `before_day` (`YYYY-MM-DD` — pass today to leave out the incomplete current day).
    fn daily_stats(
        &self,
        hardware: &str,
        sensor: &str,
        load_cat: &str,
        before_day: &str,
    ) -> Result<Vec<DayStat>>;

    /// Daily aggregates of every sensor at one load category, grouped per
    /// (hardware, sensor), same day rule as [`ReadModel::daily_stats`].
    fn daily_stats_for_load(
        &self,
        load_cat: &str,
        before_day: &str,
    ) -> Result<Vec<(String, String, Vec<DayStat>)>>;

    /// Size of the storage, in bytes.
    fn storage_bytes(&self) -> u64;
}
