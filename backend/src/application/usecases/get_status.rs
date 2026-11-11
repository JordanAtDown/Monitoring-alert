//! `GET /api/status`: the collector, the database and the latest readings.

use anyhow::Result;
use serde::Serialize;

use crate::application::app_info::AppInfo;
use crate::application::clock::now_string;
use crate::application::ports::{LatestSnapshot, ReadModel};
use crate::application::status::{CollectorStatus, StatusSnapshot};
use crate::domain::dashboard::data::{self, DataFact, DataFigures};

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
    /// What is kept, for how long and where, in words.
    pub data: Vec<DataFact>,
}

pub fn execute(
    read: &dyn ReadModel,
    collector: &CollectorStatus,
    info: &AppInfo,
) -> Result<StatusResponse> {
    let stats = read.overall_stats()?;
    let history_since = stats.history_start_day().map(str::to_string);
    let size = read.storage_bytes();
    let snapshot = collector.snapshot();
    let data = data::facts(&DataFigures {
        history_since: history_since.as_deref(),
        interval_secs: snapshot.interval_secs,
        raw_retention_days: info.raw_retention_days,
        total_snapshots: stats.total_snapshots,
        db_path: &info.db_path,
        db_size_bytes: size,
    });
    Ok(StatusResponse {
        data,
        version: env!("CARGO_PKG_VERSION"),
        now: now_string(),
        collector: snapshot,
        lhm: LhmInfo {
            host: info.lhm_host.clone(),
            port: info.lhm_port,
        },
        db: DbInfo {
            path: info.db_path.clone(),
            history_since,
            size_bytes: size,
            total_snapshots: stats.total_snapshots,
            first_ts: stats.first_ts,
            last_ts: stats.last_ts,
            raw_retention_days: info.raw_retention_days,
        },
        latest: read.latest_snapshot()?,
    })
}
