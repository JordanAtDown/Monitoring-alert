//! `GET /api/trend`: the drift data of one sensor at one load level.

use anyhow::Result;

use crate::application::clock::day_string;
use crate::application::ports::ReadModel;
use crate::domain::trends::{self, Trend};

/// The current day is left out: it is incomplete and would bias the daily median.
pub fn execute(read: &dyn ReadModel, hardware: &str, sensor: &str, load: &str) -> Result<Trend> {
    let stats = read.daily_stats(hardware, sensor, load, &day_string(0))?;
    Ok(trends::compute_trend(&stats))
}
