//! `GET /api/status`: the collector, the database and the latest readings.

use anyhow::Result;

use crate::adapters::http::{AppState, Reply};
use crate::application::usecases::get_status;

pub fn handle(state: &AppState) -> Result<Reply> {
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&get_status::execute(
        read.as_ref(),
        &state.status,
        &state.info,
    )?))
}
