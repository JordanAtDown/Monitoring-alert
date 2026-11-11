//! `GET /api/health`: is everything working, check by check.

use anyhow::Result;

use crate::adapters::http::{AppState, Reply};
use crate::application::usecases::get_health;

pub fn handle(state: &AppState) -> Result<Reply> {
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&get_health::execute(
        read.as_ref(),
        &state.status,
        &state.updates,
        (state.autostart)(),
    )?))
}
