use anyhow::{Context, Result};
use chrono::Local;
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::store::{SqliteStore, TemperatureStore};
use crate::{db, sensors};

/// Number of consecutive empty collections before escalating to an error.
/// At the default 300s interval this corresponds to ~1 hour.
const EMPTY_ALERT_THRESHOLD: u32 = 12;

/// A purge removing at least this many snapshots (~17 days at 5 min) is followed by a VACUUM.
const VACUUM_AFTER_PURGE_SNAPSHOTS: usize = 5000;

/// Retry interval (seconds) while waiting for LHM to become available at startup.
const LHM_RETRY_INTERVAL_SECS: u64 = 10;

/// Maximum number of startup retry attempts before giving up and entering the normal loop.
/// 10s × 30 = 5 min, matching the default collection interval.
const LHM_RETRY_MAX_ATTEMPTS: u32 = 30;

// ──────────────────────────────────────────────────────────────
// Collector health, shared with the dashboard
// ──────────────────────────────────────────────────────────────

#[derive(Default)]
struct StatusInner {
    last_attempt: Option<String>,
    last_success: Option<String>,
    last_error: Option<String>,
    consecutive_failures: u32,
    last_sensor_count: usize,
}

/// What the collector is doing, as shown by the dashboard.
#[derive(Debug, Serialize)]
pub struct StatusSnapshot {
    pub started_at: String,
    pub interval_secs: u64,
    pub last_attempt: Option<String>,
    pub last_success: Option<String>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub last_sensor_count: usize,
}

/// Health of the collection loop, written by the collector and read by the web server.
pub struct CollectorStatus {
    started_at: String,
    interval_secs: u64,
    inner: Mutex<StatusInner>,
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

impl CollectorStatus {
    pub fn new(interval_secs: u64) -> Self {
        Self {
            started_at: now_string(),
            interval_secs,
            inner: Mutex::new(StatusInner::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StatusInner> {
        // A poisoned lock only means another thread panicked mid-update: the data is plain.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A collection reached LibreHardwareMonitor (`sensors` temperature readings stored).
    pub fn record_success(&self, sensors: usize) {
        let now = now_string();
        let mut s = self.lock();
        s.last_attempt = Some(now.clone());
        s.last_success = Some(now);
        s.last_error = None;
        s.consecutive_failures = 0;
        s.last_sensor_count = sensors;
    }

    /// A collection failed (LibreHardwareMonitor unreachable, database error…).
    pub fn record_failure(&self, error: &str) {
        let mut s = self.lock();
        s.last_attempt = Some(now_string());
        s.last_error = Some(error.to_string());
        s.consecutive_failures = s.consecutive_failures.saturating_add(1);
    }

    pub fn snapshot(&self) -> StatusSnapshot {
        let s = self.lock();
        StatusSnapshot {
            started_at: self.started_at.clone(),
            interval_secs: self.interval_secs,
            last_attempt: s.last_attempt.clone(),
            last_success: s.last_success.clone(),
            last_error: s.last_error.clone(),
            consecutive_failures: s.consecutive_failures,
            last_sensor_count: s.last_sensor_count,
        }
    }
}

/// Sleeps for `secs` seconds in 1-second increments, checking the stop signal between each.
/// Returns `true` if the stop signal fired before the full duration elapsed.
pub fn sleep_interruptible(secs: u64, stop: &Arc<AtomicBool>) -> bool {
    let mut elapsed = 0u64;
    while elapsed < secs {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
        elapsed += 1;
    }
    false
}

pub fn load_category(load: f64) -> &'static str {
    match load as u32 {
        0..=14 => "idle",
        15..=39 => "light",
        40..=74 => "moderate",
        75..=100 => "heavy",
        _ => "heavy",
    }
}

/// Returns the effective load to use for categorisation.
///
/// Takes the **maximum** of CPU and GPU load so that GPU-intensive workloads
/// (e.g. gaming: GPU 90 %, CPU 15 %) are classified as `heavy` rather than
/// `light`. This ensures GPU temperatures recorded during gaming sessions are
/// compared against other gaming sessions, not against idle-CPU periods.
pub fn effective_load(cpu: Option<f64>, gpu: Option<f64>) -> Option<f64> {
    [cpu, gpu].into_iter().flatten().reduce(f64::max)
}

/// Collects one snapshot and persists it.
/// Returns the number of temperature readings stored (0 means no sensors detected).
pub fn collect_and_store(
    store: &dyn TemperatureStore,
    lhm_host: &str,
    lhm_port: u16,
) -> Result<usize> {
    let data = sensors::read_sensors(lhm_host, lhm_port).context("Failed to read sensor data")?;
    let ts = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let cat = effective_load(data.cpu_load, data.gpu_load)
        .map(load_category)
        .unwrap_or("idle")
        .to_string();

    let snapshot_id = store
        .insert_snapshot(&ts, data.cpu_load, data.gpu_load, &cat)
        .context("Failed to insert snapshot")?;

    for reading in &data.temperatures {
        store
            .insert_reading(
                snapshot_id,
                &reading.hardware,
                &reading.sensor,
                reading.value,
            )
            .context("Failed to insert reading")?;
    }

    let n = data.temperatures.len();
    if n == 0 {
        tracing::warn!(
            "Snapshot #{} — no temperature sensors detected. \
             Is LibreHardwareMonitor running with Remote Web Server enabled (Options → Remote Web Server → Run)?",
            snapshot_id
        );
    } else {
        tracing::info!(
            "Snapshot #{} — CPU: {:.1}%, GPU: {:.1}%, cat: {}, sensors: {}",
            snapshot_id,
            data.cpu_load.unwrap_or(0.0),
            data.gpu_load.unwrap_or(0.0),
            cat,
            n
        );
    }
    Ok(n)
}

pub fn watch(
    db_path: &Path,
    interval_secs: u64,
    raw_retention_days: u32,
    lhm_host: &str,
    lhm_port: u16,
    status: &CollectorStatus,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let conn = db::init_db(db_path).context("Failed to open database")?;
    let store = SqliteStore::new(conn);
    tracing::info!(
        "Watch loop started — interval: {}s, raw readings kept {} days (daily aggregates kept forever).",
        interval_secs,
        raw_retention_days
    );

    // ── Startup diagnostics ───────────────────────────────────
    tracing::info!("Startup check — DB: {}", db_path.display());
    match store.get_overall_stats() {
        Ok(s) => {
            let days = s
                .first_ts
                .as_deref()
                .and_then(|ts| ts.get(..10))
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .map(|first| (chrono::Local::now().date_naive() - first).num_days());
            let readiness = match days {
                None => "aucune donnée".to_string(),
                Some(d) if d < crate::trends::MIN_SPAN_DAYS => format!(
                    "{d} j — dérive affichée à partir de {} j de recul",
                    crate::trends::MIN_SPAN_DAYS
                ),
                Some(d) => format!("{d} j — suivi de dérive disponible"),
            };
            tracing::info!(
                "Startup check — DB OK ({} snapshot(s), last: {}, données: {})",
                s.total_snapshots,
                s.last_ts.as_deref().unwrap_or("none"),
                readiness
            );
        }
        Err(e) => tracing::error!("Startup check — DB error: {:#}", e),
    }
    match sensors::read_sensors(lhm_host, lhm_port) {
        Ok(data) if data.temperatures.is_empty() => {
            status.record_success(0);
            tracing::warn!(
                "Startup check — LHM reachable at {}:{} but 0 temperature sensors found. \
                 Ensure LibreHardwareMonitor is running as administrator with Remote Web Server enabled.",
                lhm_host, lhm_port
            );
        }
        Ok(data) => {
            status.record_success(data.temperatures.len());
            tracing::info!(
                "Startup check — LHM OK at {}:{} ({} sensor(s) detected)",
                lhm_host,
                lhm_port,
                data.temperatures.len()
            );
        }
        Err(e) => {
            status.record_failure(&format!("{e:#}"));
            tracing::warn!(
                "Startup check — LHM unreachable at {}:{}, waiting up to {} min for it to start…",
                lhm_host,
                lhm_port,
                LHM_RETRY_MAX_ATTEMPTS as u64 * LHM_RETRY_INTERVAL_SECS / 60,
            );
            let mut lhm_ready = false;
            for attempt in 1..=LHM_RETRY_MAX_ATTEMPTS {
                if sleep_interruptible(LHM_RETRY_INTERVAL_SECS, &stop) {
                    return Ok(());
                }
                match sensors::read_sensors(lhm_host, lhm_port) {
                    Ok(data) => {
                        status.record_success(data.temperatures.len());
                        tracing::info!(
                            "LHM ready after {} retry(ies) — {} sensor(s) detected",
                            attempt,
                            data.temperatures.len()
                        );
                        lhm_ready = true;
                        break;
                    }
                    Err(e) => {
                        status.record_failure(&format!("{e:#}"));
                        tracing::warn!(
                            "LHM not ready (attempt {}/{}), retrying in {}s…",
                            attempt,
                            LHM_RETRY_MAX_ATTEMPTS,
                            LHM_RETRY_INTERVAL_SECS,
                        );
                    }
                }
            }
            if !lhm_ready {
                tracing::error!(
                    "LHM still unreachable after {} retries (~{} min). \
                     The service will continue and retry every {}s. \
                     Launch LibreHardwareMonitor as administrator and enable Options › Remote Web Server › Run.",
                    LHM_RETRY_MAX_ATTEMPTS,
                    LHM_RETRY_MAX_ATTEMPTS as u64 * LHM_RETRY_INTERVAL_SECS / 60,
                    interval_secs,
                );
            }
        }
    }
    // ─────────────────────────────────────────────────────────

    // Roll up the raw readings into daily aggregates once at startup, then hourly.
    // Run purge once at startup, then every 24 h. The purge itself aggregates
    // whatever it is about to delete, so no history is lost.
    let rollup_every = (3600 / interval_secs).max(1);
    let purge_every = (86400 / interval_secs).max(1);
    let mut iterations: u64 = 0;
    let mut empty_streak: u32 = 0;
    let mut first_run = true;

    loop {
        if stop.load(Ordering::Relaxed) {
            tracing::info!("Stop signal received — exiting watch loop.");
            break;
        }

        if iterations.is_multiple_of(rollup_every) {
            // Yesterday + today: yesterday may have been completed since the last pass.
            if let Err(e) = store.rollup_since(&db::day_string(1)) {
                tracing::warn!("Daily roll-up error: {:#}", e);
            }
        }

        if iterations.is_multiple_of(purge_every) {
            match store.purge_old_data(raw_retention_days) {
                Ok(0) => {}
                Ok(n) => {
                    tracing::info!(
                        "Purged {} snapshot(s) older than {} days (daily aggregates kept).",
                        n,
                        raw_retention_days
                    );
                    // A large purge (e.g. right after the v2 migration) leaves a lot of
                    // free pages: give the space back to the disk.
                    if n >= VACUUM_AFTER_PURGE_SNAPSHOTS {
                        match store.vacuum() {
                            Ok(()) => tracing::info!("Database compacted after purge."),
                            Err(e) => tracing::warn!("Vacuum error: {:#}", e),
                        }
                    }
                }
                Err(e) => tracing::warn!("Purge error: {:#}", e),
            }
        }

        if first_run {
            tracing::info!("First collection starting…");
            first_run = false;
        }

        let outcome = collect_and_store(&store, lhm_host, lhm_port);
        match &outcome {
            Ok(n) => status.record_success(*n),
            Err(e) => status.record_failure(&format!("{e:#}")),
        }
        match outcome {
            Ok(0) => {
                empty_streak += 1;
                if empty_streak == EMPTY_ALERT_THRESHOLD {
                    tracing::error!(
                        "No temperature readings for {} consecutive collections \
                         (~{} min) — ensure LibreHardwareMonitor is running.",
                        EMPTY_ALERT_THRESHOLD,
                        EMPTY_ALERT_THRESHOLD as u64 * interval_secs / 60
                    );
                    // Reset so the error re-fires after another full streak, not every loop.
                    empty_streak = 0;
                }
            }
            Ok(_) => {
                if empty_streak > 0 {
                    tracing::info!(
                        "Temperature readings restored after {} empty collection(s).",
                        empty_streak
                    );
                }
                empty_streak = 0;
            }
            Err(e) => tracing::error!("Collection error: {:#}", e),
        }

        iterations += 1;
        if sleep_interruptible(interval_secs, &stop) {
            return Ok(());
        }
    }
    Ok(())
}
