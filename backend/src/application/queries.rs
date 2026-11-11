//! What the dashboard asks: collector status, the series, a sensor's trend, the overview.

use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeSet;

use crate::application::clock::{day_string, now_string};
use crate::application::health::{evaluate, Health, HealthInputs};
use crate::application::ports::{LatestSnapshot, ReadModel, SeriesInfo};
use crate::application::status::{CollectorStatus, StatusSnapshot};
use crate::application::updates::{UpdateSnapshot, UpdateStatus};
use crate::domain::trends::{self, SensorSummary, Trend};

/// Static facts about the running instance.
pub struct AppInfo {
    /// Where the database is, as shown to the user.
    pub db_path: String,
    pub lhm_host: String,
    pub lhm_port: u16,
    pub raw_retention_days: u32,
}

#[derive(Serialize)]
pub struct LhmInfo {
    pub host: String,
    pub port: u16,
}

#[derive(Serialize)]
pub struct DbInfo {
    pub path: String,
    /// First day with daily aggregates: the history the trends are computed on.
    pub history_since: Option<String>,
    pub size_bytes: u64,
    pub total_snapshots: i64,
    pub first_ts: Option<String>,
    pub last_ts: Option<String>,
    pub raw_retention_days: u32,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub version: &'static str,
    pub now: String,
    pub collector: StatusSnapshot,
    pub lhm: LhmInfo,
    pub db: DbInfo,
    pub latest: Option<LatestSnapshot>,
}

pub fn is_load_category(load: &str) -> bool {
    trends::LOAD_CATEGORIES.contains(&load)
}

pub fn status(
    read: &dyn ReadModel,
    collector: &CollectorStatus,
    info: &AppInfo,
) -> Result<StatusResponse> {
    let stats = read.overall_stats()?;
    let history_since = stats.history_start_day().map(str::to_string);
    Ok(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        now: now_string(),
        collector: collector.snapshot(),
        lhm: LhmInfo {
            host: info.lhm_host.clone(),
            port: info.lhm_port,
        },
        db: DbInfo {
            path: info.db_path.clone(),
            history_since,
            size_bytes: read.storage_bytes(),
            total_snapshots: stats.total_snapshots,
            first_ts: stats.first_ts,
            last_ts: stats.last_ts,
            raw_retention_days: info.raw_retention_days,
        },
        latest: read.latest_snapshot()?,
    })
}

/// What the dashboard shows about updates.
pub fn update(status: &UpdateStatus) -> UpdateSnapshot {
    status.snapshot()
}

/// Is everything working? Gathers the facts and runs the checks.
pub fn health(
    read: &dyn ReadModel,
    collector: &CollectorStatus,
    updates: &UpdateStatus,
    autostart: Option<bool>,
) -> Result<Health> {
    let stats = read.overall_stats()?;
    let latest = read.latest_snapshot()?;
    // Sensors that reported lately: those with a daily aggregate in the last days.
    let since = day_string(3);
    let recent: BTreeSet<(String, String)> = read
        .series()?
        .into_iter()
        .filter(|s| s.last_day >= since)
        .map(|s| (s.hardware, s.sensor))
        .collect();
    let recent: Vec<(String, String)> = recent.into_iter().collect();
    let snapshot = collector.snapshot();
    let update = updates.snapshot();
    Ok(evaluate(&HealthInputs {
        now: &now_string(),
        collector: &snapshot,
        latest: latest.as_ref(),
        recent_sensors: &recent,
        db_size_bytes: read.storage_bytes(),
        db_snapshots: stats.total_snapshots,
        updates: &update,
        autostart,
    }))
}

pub fn series(read: &dyn ReadModel) -> Result<Vec<SeriesInfo>> {
    read.series()
}

/// Drift chart data of one sensor. The current day is left out: it is incomplete
/// and would bias the daily median.
pub fn trend(read: &dyn ReadModel, hardware: &str, sensor: &str, load: &str) -> Result<Trend> {
    let stats = read.daily_stats(hardware, sensor, load, &day_string(0))?;
    Ok(trends::compute_trend(&stats))
}

/// Drift overview of every sensor at one load level, largest drift first.
pub fn summary(read: &dyn ReadModel, load: &str) -> Result<Vec<SensorSummary>> {
    let groups = read.daily_stats_for_load(load, &day_string(0))?;
    let mut summaries: Vec<_> = groups
        .iter()
        .map(|(hardware, sensor, stats)| trends::summarize(hardware, sensor, load, stats))
        .collect();
    trends::sort_summaries(&mut summaries);
    Ok(summaries)
}
