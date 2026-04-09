use anyhow::{Context, Result};
use chrono::Local;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::store::{SqliteStore, TemperatureStore};
use crate::{db, sensors};

/// Number of consecutive empty collections before escalating to an error.
/// At the default 300s interval this corresponds to ~1 hour.
const EMPTY_ALERT_THRESHOLD: u32 = 12;

/// Retry interval (seconds) while waiting for LHM to become available at startup.
#[cfg(windows)]
const LHM_RETRY_INTERVAL_SECS: u64 = 10;

/// Maximum number of startup retry attempts before giving up and entering the normal loop.
/// 10s × 30 = 5 min, matching the default collection interval.
#[cfg(windows)]
const LHM_RETRY_MAX_ATTEMPTS: u32 = 30;

/// Sleeps for `secs` seconds in 1-second increments, checking the stop signal between each.
/// Returns `true` if the stop signal fired before the full duration elapsed.
fn sleep_interruptible(secs: u64, stop: &Arc<AtomicBool>) -> bool {
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
    retention_days: u32,
    lhm_host: &str,
    lhm_port: u16,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let conn = db::init_db(db_path).context("Failed to open database")?;
    let store = SqliteStore::new(conn);
    tracing::info!(
        "Watch loop started — interval: {}s, retention: {} days.",
        interval_secs,
        retention_days
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
                Some(d) if d < 2 => format!("{d} j — aucune comparaison disponible"),
                Some(d) if d < 14 => format!("{d} j — 24h OK, 7j/30j/90j/180j en attente"),
                Some(d) if d < 60 => format!(
                    "{d} j — 24h+7j OK, 30j/90j/180j en attente (notifications: dans {} j)",
                    60 - d
                ),
                Some(d) if d < 180 => {
                    format!("{d} j — 24h+7j+30j OK (notifications actives), 90j/180j en attente")
                }
                Some(d) if d < 360 => format!("{d} j — 24h+7j+30j+90j OK, 180j en attente"),
                Some(d) => format!("{d} j — toutes les fenêtres disponibles"),
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
    #[cfg(windows)]
    match sensors::read_sensors(lhm_host, lhm_port) {
        Ok(data) if data.temperatures.is_empty() => {
            tracing::warn!(
                "Startup check — LHM reachable at {}:{} but 0 temperature sensors found. \
                 Ensure LibreHardwareMonitor is running as administrator with Remote Web Server enabled.",
                lhm_host, lhm_port
            );
        }
        Ok(data) => {
            tracing::info!(
                "Startup check — LHM OK at {}:{} ({} sensor(s) detected)",
                lhm_host,
                lhm_port,
                data.temperatures.len()
            );
        }
        Err(_) => {
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
                        tracing::info!(
                            "LHM ready after {} retry(ies) — {} sensor(s) detected",
                            attempt,
                            data.temperatures.len()
                        );
                        lhm_ready = true;
                        break;
                    }
                    Err(_) => {
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

    // Run purge once at startup, then every 24 h.
    let purge_every = (86400 / interval_secs).max(1);
    let mut iterations: u64 = 0;
    let mut empty_streak: u32 = 0;
    let mut first_run = true;

    loop {
        if stop.load(Ordering::Relaxed) {
            tracing::info!("Stop signal received — exiting watch loop.");
            break;
        }

        if iterations.is_multiple_of(purge_every) {
            match store.purge_old_data(retention_days) {
                Ok(0) => {}
                Ok(n) => tracing::info!(
                    "Purged {} snapshot(s) older than {} days.",
                    n,
                    retention_days
                ),
                Err(e) => tracing::warn!("Purge error: {:#}", e),
            }
        }

        if first_run {
            tracing::info!("First collection starting…");
            first_run = false;
        }

        match collect_and_store(&store, lhm_host, lhm_port) {
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
