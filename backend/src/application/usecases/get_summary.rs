//! `GET /api/summary`: the drift overview of every sensor at one load level, largest drift first.

use anyhow::Result;

use crate::application::clock::day_string;
use crate::application::ports::ReadModel;
use crate::domain::trends::{self, SensorSummary};

pub fn execute(read: &dyn ReadModel, load: &str) -> Result<Vec<SensorSummary>> {
    let groups = read.daily_stats_for_load(load, &day_string(0))?;
    let mut summaries: Vec<_> = groups
        .iter()
        .map(|(hardware, sensor, stats)| trends::summarize(hardware, sensor, load, stats))
        .collect();
    trends::sort_summaries(&mut summaries);
    Ok(summaries)
}
