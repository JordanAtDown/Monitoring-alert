use anyhow::{Context, Result};
use std::path::Path;

use crate::application::ports::{
    LatestSnapshot, OverallStats, ReadModel, SeriesInfo, TemperatureStore,
};
use crate::domain::model::{ExtraSeries, NewSnapshot};
use crate::domain::trends::DayStat;

/// SQLite-backed implementation of [`TemperatureStore`] and [`ReadModel`].
///
/// Wraps a `rusqlite::Connection`; callers never interact with the connection
/// directly, keeping the SQLite dependency internal.
pub struct SqliteStore(rusqlite::Connection);

impl SqliteStore {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// Direct access to the connection, for tests that inspect the stored rows.
    #[cfg(test)]
    pub fn connection(&self) -> &rusqlite::Connection {
        &self.0
    }
}

/// Opens an existing database for the dashboard: read-only, never creates or migrates it.
pub fn open_read_model(path: &Path) -> Result<SqliteStore> {
    Ok(SqliteStore(super::open_read_only(path)?))
}

impl TemperatureStore for SqliteStore {
    fn record_snapshot(&self, snapshot: &NewSnapshot<'_>) -> Result<i64> {
        // One transaction: a snapshot never exists without its readings (and is much faster).
        let tx = self
            .0
            .unchecked_transaction()
            .context("Failed to start the snapshot transaction")?;
        let id = super::insert_snapshot(
            &tx,
            snapshot.ts,
            snapshot.cpu_load,
            snapshot.gpu_load,
            snapshot.load_cat,
        )?;
        for reading in snapshot.readings {
            super::insert_reading(&tx, id, &reading.hardware, &reading.sensor, reading.value)?;
        }
        for extra in snapshot.extras {
            super::insert_extra(&tx, id, extra)?;
        }
        tx.commit().context("Failed to commit the snapshot")?;
        Ok(id)
    }

    fn overall_stats(&self) -> Result<OverallStats> {
        super::get_overall_stats(&self.0)
    }

    fn purge_old_data(&self, days: u32) -> Result<usize> {
        super::purge_old_snapshots(&self.0, days)
    }

    fn rollup_since(&self, from_day: &str) -> Result<usize> {
        super::rollup_since(&self.0, from_day)
    }

    fn vacuum(&self) -> Result<()> {
        super::vacuum(&self.0)
    }
}

impl ReadModel for SqliteStore {
    fn overall_stats(&self) -> Result<OverallStats> {
        super::get_overall_stats(&self.0)
    }

    fn latest_snapshot(&self) -> Result<Option<LatestSnapshot>> {
        super::get_latest_snapshot(&self.0)
    }

    fn series(&self) -> Result<Vec<SeriesInfo>> {
        super::get_series(&self.0)
    }

    fn daily_stats(
        &self,
        hardware: &str,
        sensor: &str,
        load_cat: &str,
        before_day: &str,
    ) -> Result<Vec<DayStat>> {
        super::get_daily_stats(&self.0, hardware, sensor, load_cat, before_day)
    }

    fn daily_stats_for_load(
        &self,
        load_cat: &str,
        before_day: &str,
    ) -> Result<Vec<(String, String, Vec<DayStat>)>> {
        super::get_daily_stats_for_load(&self.0, load_cat, before_day)
    }

    fn extra_daily_stats_for_load(
        &self,
        load_cat: &str,
        before_day: &str,
    ) -> Result<Vec<ExtraSeries>> {
        super::get_extra_daily_stats_for_load(&self.0, load_cat, before_day)
    }

    fn storage_bytes(&self) -> u64 {
        let Some(path) = self.0.path() else { return 0 };
        let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        size(Path::new(path)) + size(Path::new(&format!("{path}-wal")))
    }
}
