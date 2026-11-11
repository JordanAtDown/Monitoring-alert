//! `GET /api/series`: every (sensor, load level) that has data.

use anyhow::Result;

use crate::adapters::http::{AppState, Reply};
use crate::application::usecases::list_series;

pub fn handle(state: &AppState) -> Result<Reply> {
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&list_series::execute(read.as_ref())?))
}
