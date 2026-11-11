//! Local web server: a small read-only JSON API and the dashboard files.
//!
//! * `GET /api/status`  — collector health, database figures, latest readings
//! * `GET /api/series`  — every (sensor, load category) with data
//! * `GET /api/health`  — is everything working, check by check
//! * `GET /api/update`  — is a newer version available, and where to get it
//! * `GET /api/trend?hardware=&sensor=&load=` — drift chart data of one sensor
//! * `GET /api/summary?load=` — drift overview of all the sensors
//! * `GET /api/overview?load=` — the home of the dashboard: headline and components, ready to display
//! * `GET /api/curves?load=&mode=temp|delta` — one line per component, ready to draw
//! * `GET /api/sensor?hardware=&sensor=&load=` — one sensor: its story, curve and months, ready to display
//!
//! The server only reads. It listens on the loopback interface by default and
//! refuses requests whose `Host` header is not a loopback name, which blocks
//! DNS-rebinding attacks from web pages open in the browser.

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::application::curves::Mode;
use crate::application::ports::ReadModel;
use crate::application::queries::{self, AppInfo};
use crate::application::status::CollectorStatus;
use crate::application::updates::UpdateStatus;

mod assets;

/// Number of threads answering requests.
const WORKERS: usize = 2;

/// Opens a read model for one request.
pub type ReadModelFactory = Box<dyn Fn() -> Result<Box<dyn ReadModel>> + Send + Sync>;

pub struct AppState {
    pub open_read_model: ReadModelFactory,
    pub status: Arc<CollectorStatus>,
    pub updates: Arc<UpdateStatus>,
    /// Whether the start-up entry exists (`None` where that does not apply).
    pub autostart: Box<dyn Fn() -> Option<bool> + Send + Sync>,
    pub info: AppInfo,
    /// Accepted `Host` header names; `None` accepts any (when listening beyond loopback).
    pub allowed_hosts: Option<Vec<String>>,
}

/// Host names accepted when the server only listens on the loopback interface.
pub fn allowed_hosts_for(bind_host: &str) -> Option<Vec<String>> {
    let loopback = matches!(bind_host, "127.0.0.1" | "localhost" | "::1" | "[::1]");
    loopback.then(|| {
        ["127.0.0.1", "localhost", "[::1]"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    })
}

// ──────────────────────────────────────────────────────────────
// Routing (pure: no socket involved, so it is unit-testable)
// ──────────────────────────────────────────────────────────────

pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Reply {
    fn json<T: Serialize>(value: &T) -> Reply {
        match serde_json::to_vec(value) {
            Ok(body) => Reply {
                status: 200,
                content_type: "application/json; charset=utf-8",
                body,
            },
            Err(e) => {
                tracing::error!("JSON serialization failed: {e}");
                Reply::error(500, "internal error")
            }
        }
    }

    fn error(status: u16, message: &str) -> Reply {
        Reply {
            status,
            content_type: "application/json; charset=utf-8",
            body: serde_json::to_vec(&serde_json::json!({ "error": message }))
                .unwrap_or_else(|_| b"{}".to_vec()),
        }
    }
}

/// Splits `Host: name:port` into `name` (keeps the brackets of an IPv6 literal).
fn host_name(header: &str) -> &str {
    if header.starts_with('[') {
        return header.split_inclusive(']').next().unwrap_or(header);
    }
    match header.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) => name,
        _ => header,
    }
}

fn host_allowed(state: &AppState, host: Option<&str>) -> bool {
    match &state.allowed_hosts {
        None => true,
        Some(allowed) => host
            .map(|h| allowed.iter().any(|a| a.eq_ignore_ascii_case(host_name(h))))
            .unwrap_or(false),
    }
}

pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                    Some(b) => {
                        out.push(b);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|kv| !kv.is_empty())
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
            (percent_decode(k), percent_decode(v))
        })
        .collect()
}

fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty())
}

/// Answers one request.
pub fn route(state: &AppState, method: &str, url: &str, host: Option<&str>) -> Reply {
    if method != "GET" && method != "HEAD" {
        return Reply::error(405, "method not allowed");
    }
    if !host_allowed(state, host) {
        return Reply::error(403, "forbidden host");
    }
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let params = parse_query(query);

    let result = match path {
        "/api/status" => api_status(state),
        "/api/series" => api_series(state),
        "/api/health" => api_health(state),
        "/api/update" => Ok(Reply::json(&queries::update(&state.updates))),
        "/api/trend" => api_trend(state, &params),
        "/api/summary" => api_summary(state, &params),
        "/api/overview" => api_overview(state, &params),
        "/api/curves" => api_curves(state, &params),
        "/api/sensor" => api_sensor(state, &params),
        p if p.starts_with("/api/") => Ok(Reply::error(404, "unknown endpoint")),
        p => Ok(static_file(p)),
    };
    result.unwrap_or_else(|e| {
        tracing::error!("Request {url} failed: {e:#}");
        Reply::error(500, "internal error")
    })
}

fn static_file(path: &str) -> Reply {
    match assets::get(path) {
        Some(asset) => Reply {
            status: 200,
            content_type: asset.mime,
            body: asset.body.to_vec(),
        },
        None => Reply {
            status: 404,
            content_type: "text/plain; charset=utf-8",
            body: b"not found".to_vec(),
        },
    }
}

fn load_param(params: &[(String, String)]) -> std::result::Result<&str, Reply> {
    let load = param(params, "load").unwrap_or("idle");
    if queries::is_load_category(load) {
        Ok(load)
    } else {
        Err(Reply::error(
            400,
            "load must be idle, light, moderate or heavy",
        ))
    }
}

// ──────────────────────────────────────────────────────────────
// Endpoints: parse the request, run a use case, serialize the answer
// ──────────────────────────────────────────────────────────────

fn api_status(state: &AppState) -> Result<Reply> {
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::status(
        read.as_ref(),
        &state.status,
        &state.info,
    )?))
}

fn api_health(state: &AppState) -> Result<Reply> {
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::health(
        read.as_ref(),
        &state.status,
        &state.updates,
        (state.autostart)(),
    )?))
}

fn api_series(state: &AppState) -> Result<Reply> {
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::series(read.as_ref())?))
}

fn api_trend(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let (Some(hardware), Some(sensor)) = (param(params, "hardware"), param(params, "sensor"))
    else {
        return Ok(Reply::error(400, "hardware and sensor are required"));
    };
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::trend(
        read.as_ref(),
        hardware,
        sensor,
        load,
    )?))
}

fn api_summary(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::summary(read.as_ref(), load)?))
}

fn api_overview(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::overview(read.as_ref(), load)?))
}

fn api_curves(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let Some(mode) = Mode::parse(param(params, "mode").unwrap_or("temp")) else {
        return Ok(Reply::error(400, "mode must be temp or delta"));
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::curves(read.as_ref(), load, mode)?))
}

fn api_sensor(state: &AppState, params: &[(String, String)]) -> Result<Reply> {
    let (Some(hardware), Some(sensor)) = (param(params, "hardware"), param(params, "sensor"))
    else {
        return Ok(Reply::error(400, "hardware and sensor are required"));
    };
    let load = match load_param(params) {
        Ok(load) => load,
        Err(reply) => return Ok(reply),
    };
    let read = (state.open_read_model)()?;
    Ok(Reply::json(&queries::sensor(
        read.as_ref(),
        hardware,
        sensor,
        load,
    )?))
}

// ──────────────────────────────────────────────────────────────
// Socket handling
// ──────────────────────────────────────────────────────────────

/// Whether a dashboard answers on `host:port` (used to tell if the daemon is running).
pub fn ping(host: &str, port: u16) -> bool {
    // 0.0.0.0 means "all interfaces": the loopback one always works.
    let host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build()
        .get(&format!("http://{host}:{port}/api/series"))
        .call()
        .is_ok()
}

/// Starts listening. Fails fast (e.g. port already taken) so that a second
/// instance of the daemon never starts collecting.
pub fn bind(host: &str, port: u16) -> Result<Arc<tiny_http::Server>> {
    tiny_http::Server::http((host, port))
        .map(Arc::new)
        .map_err(|e| anyhow!("Cannot listen on {host}:{port}: {e}"))
}

fn header(name: &str, value: &str) -> Result<tiny_http::Header> {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes())
        .map_err(|_| anyhow!("invalid header {name}"))
}

fn respond(request: tiny_http::Request, reply: Reply) -> Result<()> {
    // File names are not content-hashed: always revalidate, so an upgraded
    // executable never serves a script that does not match its API.
    let response = tiny_http::Response::from_data(reply.body)
        .with_status_code(reply.status)
        .with_header(header("Content-Type", reply.content_type)?)
        .with_header(header("Cache-Control", "no-store")?)
        .with_header(header("X-Content-Type-Options", "nosniff")?)
        .with_header(header(
            "Content-Security-Policy",
            "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:",
        )?);
    request.respond(response).context("Failed to send response")
}

fn worker(server: &tiny_http::Server, state: &AppState, stop: &AtomicBool) {
    while !stop.load(Ordering::Relaxed) {
        match server.recv_timeout(Duration::from_millis(500)) {
            Ok(Some(request)) => {
                let host = request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv("Host"))
                    .map(|h| h.value.as_str().to_string());
                let reply = route(
                    state,
                    request.method().as_str(),
                    request.url(),
                    host.as_deref(),
                );
                if let Err(e) = respond(request, reply) {
                    tracing::debug!("Response not delivered: {e:#}");
                }
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("Web server receive error: {e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

/// Answers requests until `stop` is set. Blocks the calling thread.
pub fn run(server: Arc<tiny_http::Server>, state: Arc<AppState>, stop: Arc<AtomicBool>) {
    std::thread::scope(|scope| {
        for _ in 0..WORKERS {
            scope.spawn(|| worker(&server, &state, &stop));
        }
    });
}
