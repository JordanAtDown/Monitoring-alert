//! `GET /api/sensor`: one sensor explained: its story, its curve, its months.

use anyhow::Result;

use crate::application::ports::ReadModel;
use crate::application::usecases::gather_facts;
use crate::domain::dashboard::sensor::{self, SensorPage};

pub fn execute(
    read: &dyn ReadModel,
    hardware: &str,
    sensor: &str,
    load: &str,
) -> Result<SensorPage> {
    Ok(sensor::build(
        &gather_facts(read, load)?,
        hardware,
        sensor,
        load,
    ))
}
