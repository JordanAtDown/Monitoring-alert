//! `GET /api/cooling?load=`: the processor's cooling, ready to display.

use anyhow::Result;

use crate::adapters::http::{load_param, AppState, Reply};
use crate::application::usecases::get_cooling;

pub fn handle(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&get_cooling::execute(read.as_ref(), load)?))
}
