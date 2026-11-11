//! `GET /api/health`: is everything working? Gathers the facts and runs the checks.

use anyhow::Result;
use std::collections::BTreeSet;

use crate::application::clock::{day_string, now_string};
use crate::application::health::{evaluate, Health, HealthInputs};
use crate::application::ports::ReadModel;
use crate::application::status::CollectorStatus;
use crate::application::updates::UpdateStatus;

pub fn execute(
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
