//! The pages of the dashboard, ready to display: everything the page prints is decided here, from the
//! daily figures of the sensors. Pure functions of the facts they are given; reading them from the
//! database is the use cases' job.

pub mod curves;
pub mod data;
pub mod overview;
pub mod sensor;

use crate::domain::trends::{DayStat, SensorSummary};

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
