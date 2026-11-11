//! `GET /api/curves?load=&mode=temp|delta`: one line per component, ready to draw.

use anyhow::Result;

use crate::adapters::http::{load_param, param, AppState, Reply};
use crate::application::usecases::get_curves;
use crate::domain::dashboard::curves::Mode;

pub fn handle(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let Some(mode) = Mode::parse(param(params, "mode").unwrap_or("temp")) else {
        return Ok(Reply::error(400, "mode must be temp or delta"));
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&get_curves::execute(
        read.as_ref(),
        load,
        mode,
    )?))
}
