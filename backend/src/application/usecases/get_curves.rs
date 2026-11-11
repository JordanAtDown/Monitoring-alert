//! `GET /api/curves`: one line per component, in temperatures or as the change since the beginning.

use anyhow::Result;

use crate::application::ports::ReadModel;
use crate::application::usecases::gather_facts;
use crate::domain::dashboard::curves::{self, Curves, Mode};

pub fn execute(read: &dyn ReadModel, load: &str, mode: Mode) -> Result<Curves> {
    Ok(curves::build(&gather_facts(read, load)?, mode))
}
