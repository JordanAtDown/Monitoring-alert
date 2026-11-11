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

// ──────────────────────────────────────────────────────────────
// The host system: installation, start-up, cleanup of a v1.x installation
// ──────────────────────────────────────────────────────────────

/// What is left of a v1.x installation (Windows service + scheduled report tasks + toast registration).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LegacyFindings {
    /// The `MonitoringAlert` Windows service is registered.
    pub service: bool,
    /// Scheduled tasks of the old reports that still exist.
    pub tasks: Vec<String>,
    /// The toast-notification registration (AUMID) exists.
    pub aumid: bool,
    /// Old scripts left in the data directory.
    pub scripts: Vec<String>,
}

impl LegacyFindings {
    pub fn is_empty(&self) -> bool {
        !self.service && self.tasks.is_empty() && !self.aumid && self.scripts.is_empty()
    }
}

/// Outcome of the configuration file migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigMigration {
    /// No config file, or it already is in the current format.
    Current,
    /// The file used obsolete keys: it was rewritten, the old one kept as `backup`.
    Rewritten {
        backup: std::path::PathBuf,
        removed_keys: Vec<String>,
    },
}

/// Text that identifies `scripts/Register-Tasks.ps1` as the migration shim shipped since 2.0
/// (and not the v1.x script of the same name, which created the report tasks).
pub const MIGRATION_SHIM_MARKER: &str = "monitoring-alert-migration-shim";

/// File name of the background executable.
pub const DAEMON_EXE_NAME: &str = "monitoring-daemon.exe";

/// Where the files of an installation go.
#[derive(Debug, Clone)]
pub struct DeployPlan {
    /// The executable being run (the one of the downloaded archive, or the installed one).
    pub source_exe: std::path::PathBuf,
    /// Directory of the executables.
    pub install_dir: std::path::PathBuf,
    /// Directory of `config.toml`, the database and the logs.
    pub data_dir: std::path::PathBuf,
    /// Database chosen by the installer; `None` keeps what the configuration says.
    pub db_path: Option<std::path::PathBuf>,
}

impl DeployPlan {
    pub fn cli_exe(&self) -> std::path::PathBuf {
        self.install_dir.join("monitoring-alert.exe")
    }

    /// The background copy of the executable (no console window).
    pub fn daemon_exe(&self) -> std::path::PathBuf {
        self.install_dir.join(DAEMON_EXE_NAME)
    }
}

/// Everything the installer needs from the operating system.
pub trait HostSystem {
    fn legacy_findings(&self, data_dir: &std::path::Path) -> Result<LegacyFindings>;
    /// Stops and unregisters the v1.x Windows service. Fails if it cannot (e.g. not administrator).
    fn remove_legacy_service(&self) -> Result<()>;
    fn remove_legacy_tasks(&self) -> Result<()>;
    fn remove_legacy_aumid(&self) -> Result<()>;
    fn remove_legacy_scripts(&self, data_dir: &std::path::Path) -> Result<Vec<String>>;

    /// Rewrites a v1.x `config.toml` without its obsolete keys.
    fn migrate_config(&self, config_path: &std::path::Path) -> Result<ConfigMigration>;

    /// Puts the executables, the default config and the helper scripts in place.
    /// Returns what was written, one line per file.
    fn deploy(&self, plan: &DeployPlan) -> Result<Vec<String>>;

    /// Points `config.toml` at the database `db_path` (creating the file if needed).
    fn set_db_path(&self, config_path: &std::path::Path, db_path: &std::path::Path) -> Result<()>;

    /// Stops the background process; `false` if it was not running.
    fn stop_daemon(&self) -> Result<bool>;
    fn start_daemon(&self, daemon_exe: &std::path::Path) -> Result<()>;
    fn autostart_enable(&self, daemon_exe: &std::path::Path) -> Result<()>;
    /// Removes the start-up entry; `false` if there was none.
    fn autostart_disable(&self) -> Result<bool>;
    fn autostart_enabled(&self) -> Result<bool>;
}
