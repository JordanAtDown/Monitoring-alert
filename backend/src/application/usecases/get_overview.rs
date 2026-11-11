//! `GET /api/overview`: the home of the dashboard at one load level.

use anyhow::Result;

use crate::application::ports::ReadModel;
use crate::application::usecases::gather_facts;
use crate::domain::dashboard::overview::{self, Overview};

pub fn execute(read: &dyn ReadModel, load: &str) -> Result<Overview> {
    Ok(overview::build(&gather_facts(read, load)?, load))
}
