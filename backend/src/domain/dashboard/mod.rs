//! The pages of the dashboard, ready to display: everything the page prints is decided here, from the
//! daily figures of the sensors. Pure functions of the facts they are given; reading them from the
//! database is the use cases' job.

pub mod curves;
pub mod data;
pub mod overview;
pub mod sensor;

use crate::domain::trends::{self, DayStat, SensorSummary};

/// What every page starts from: the daily figures of every sensor at one load level.
pub struct Facts {
    /// Daily aggregates per (hardware, sensor).
    pub groups: Vec<(String, String, Vec<DayStat>)>,
    /// The overview of each of those sensors.
    pub summaries: Vec<SensorSummary>,
    /// Every sensor the machine reports (the numbering of the disks must not depend on the load level).
    pub seen: Vec<(String, String)>,
    /// First day with daily aggregates.
    pub since: Option<String>,
}

impl Facts {
    /// The drift shared by the whole machine: the median of the drifts of the sensors that
    /// stand for each component. One voice per component, so a processor with twelve core
    /// sensors does not outweigh a disk that has one.
    pub fn machine_drift(&self) -> Option<f64> {
        let deltas: Vec<f64> = overview::components_with_data(&self.seen, &self.summaries)
            .into_iter()
            .filter_map(|(_, _, rows)| rows.first().and_then(|s| s.drift()))
            .collect();
        trends::common_drift(&deltas)
    }
}
