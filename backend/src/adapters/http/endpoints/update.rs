//! `GET /api/update`: is a newer version available, and where to get it.

use anyhow::Result;

use crate::adapters::http::{AppState, Reply};
use crate::application::usecases::get_update;

pub fn handle(state: &AppState) -> Result<Reply> {
    Ok(Reply::json(&get_update::execute(&state.updates)))
}
