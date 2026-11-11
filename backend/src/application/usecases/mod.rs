//! One file per thing the dashboard can ask, named after what it does. Each use case reads what it
//! needs through the ports and hands it to the business code of the domain; the web server only
//! calls them (one endpoint, one use case).

pub mod get_curves;
pub mod get_health;
pub mod get_overview;
pub mod get_sensor;
pub mod get_status;
pub mod get_summary;
pub mod get_trend;
pub mod get_update;
pub mod list_series;

use anyhow::Result;

use crate::application::clock::day_string;
use crate::application::ports::ReadModel;
use crate::domain::dashboard::Facts;
use crate::domain::trends;

/// The daily figures of every sensor at one load level, the starting point of the page use cases.
/// The current day is left out: it is incomplete and would bias the daily median.
pub(crate) fn gather_facts(read: &dyn ReadModel, load: &str) -> Result<Facts> {
    let groups = read.daily_stats_for_load(load, &day_string(0))?;
    let summaries = groups
        .iter()
        .map(|(h, s, stats)| trends::summarize(h, s, load, stats))
        .collect();
    let seen = read
        .latest_snapshot()?
        .map(|l| {
            l.readings
                .into_iter()
                .map(|r| (r.hardware, r.sensor))
                .collect()
        })
        .unwrap_or_default();
    Ok(Facts {
        groups,
        summaries,
        seen,
        since: read
            .overall_stats()?
            .history_start_day()
            .map(str::to_string),
    })
}
