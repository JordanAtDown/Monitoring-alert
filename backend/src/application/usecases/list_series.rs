//! `GET /api/series`: every (sensor, load level) that has data.

use anyhow::Result;

use crate::application::ports::{ReadModel, SeriesInfo};

pub fn execute(read: &dyn ReadModel) -> Result<Vec<SeriesInfo>> {
    read.series()
}
