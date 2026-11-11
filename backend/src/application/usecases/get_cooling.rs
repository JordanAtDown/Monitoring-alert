//! `GET /api/cooling`: is the processor's cooling getting worse, at one load level.

use anyhow::Result;

use crate::application::clock::day_string;
use crate::application::ports::ReadModel;
use crate::application::usecases::gather_facts;
use crate::domain::dashboard::cooling::{self, Cooling};

pub fn execute(read: &dyn ReadModel, load: &str) -> Result<Cooling> {
    let facts = gather_facts(read, load)?;
    // The current day is left out: it is incomplete.
    let extras = read.extra_daily_stats_for_load(load, &day_string(0))?;
    Ok(cooling::build(&facts, &extras, load))
}
