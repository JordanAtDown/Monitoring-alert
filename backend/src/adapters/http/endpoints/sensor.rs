//! `GET /api/sensor?hardware=&sensor=&load=`: one sensor explained, ready to display.

use anyhow::Result;

use crate::adapters::http::{load_param, param, AppState, Reply};
use crate::application::usecases::get_sensor;

pub fn handle(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let (Some(hardware), Some(sensor)) = (param(params, "hardware"), param(params, "sensor"))
    else {
        return Ok(Reply::error(400, "hardware and sensor are required"));
    };
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&get_sensor::execute(
        read.as_ref(),
        hardware,
        sensor,
        load,
    )?))
}
