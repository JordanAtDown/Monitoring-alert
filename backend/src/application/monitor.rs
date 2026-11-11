//! The collection loop: startup checks, then collect / roll up / purge for ever.

use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::application::clock::day_string;
use crate::application::collect::collect_and_store;
use crate::application::ports::{SensorSource, TemperatureStore};
use crate::application::status::CollectorStatus;
use crate::domain::trends::MIN_SPAN_DAYS;

/// Number of consecutive empty collections before escalating to an error.
/// At the default 300s interval this corresponds to ~1 hour.
const EMPTY_ALERT_THRESHOLD: u32 = 12;

/// A purge removing at least this many snapshots (~17 days at 5 min) is followed by a VACUUM.
const VACUUM_AFTER_PURGE_SNAPSHOTS: usize = 5000;

/// Retry interval (seconds) while waiting for the sensor source to become available at startup.
const SOURCE_RETRY_INTERVAL_SECS: u64 = 10;

/// Maximum number of startup retry attempts before giving up and entering the normal loop.
/// 10s × 30 = 5 min, matching the default collection interval.
const SOURCE_RETRY_MAX_ATTEMPTS: u32 = 30;

pub struct MonitorSettings {
    /// Seconds between two collections.
    pub interval_secs: u64,
    /// Days of raw readings kept; the daily aggregates are kept for ever.
    pub raw_retention_days: u32,
}

/// Opens the store; called on the monitor's own thread.
pub type StoreFactory<'a> = &'a (dyn Fn() -> Result<Box<dyn TemperatureStore>> + Sync);

/// Sleeps for `secs` seconds in 1-second increments, checking the stop signal between each.
/// Returns `true` if the stop signal fired before the full duration elapsed.
pub fn sleep_interruptible(secs: u64, stop: &AtomicBool) -> bool {
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

fn startup_checks(
    store: &dyn TemperatureStore,
    source: &dyn SensorSource,
    settings: &MonitorSettings,
    status: &CollectorStatus,
    stop: &AtomicBool,
) -> bool {
    match store.overall_stats() {
        Ok(s) => {
            // The history counts from the first daily aggregate: raw readings are purged.
            let days = s
                .history_start_day()
                .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .map(|first| (chrono::Local::now().date_naive() - first).num_days());
            let readiness = match days {
                None => "aucune donnée".to_string(),
                Some(d) if d < MIN_SPAN_DAYS => {
                    format!("{d} j — dérive affichée à partir de {MIN_SPAN_DAYS} j de recul")
                }
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

    let at = source.describe();
    match source.read() {
        Ok(data) if data.temperatures.is_empty() => {
            status.record_success(0);
            tracing::warn!(
                "Startup check — LHM reachable at {at} but 0 temperature sensors found. \
                 Ensure LibreHardwareMonitor is running as administrator with Remote Web Server enabled."
            );
        }
        Ok(data) => {
            status.record_success(data.temperatures.len());
            tracing::info!(
                "Startup check — LHM OK at {at} ({} sensor(s) detected)",
                data.temperatures.len()
            );
        }
        Err(e) => {
            status.record_failure(&format!("{e:#}"));
            tracing::warn!(
                "Startup check — LHM unreachable at {at}, waiting up to {} min for it to start…",
                SOURCE_RETRY_MAX_ATTEMPTS as u64 * SOURCE_RETRY_INTERVAL_SECS / 60,
            );
            let mut ready = false;
            for attempt in 1..=SOURCE_RETRY_MAX_ATTEMPTS {
                if sleep_interruptible(SOURCE_RETRY_INTERVAL_SECS, stop) {
                    return false;
                }
                match source.read() {
                    Ok(data) => {
                        status.record_success(data.temperatures.len());
                        tracing::info!(
                            "LHM ready after {} retry(ies) — {} sensor(s) detected",
                            attempt,
                            data.temperatures.len()
                        );
                        ready = true;
                        break;
                    }
                    Err(e) => {
                        status.record_failure(&format!("{e:#}"));
                        tracing::warn!(
                            "LHM not ready (attempt {}/{}), retrying in {}s…",
                            attempt,
                            SOURCE_RETRY_MAX_ATTEMPTS,
                            SOURCE_RETRY_INTERVAL_SECS,
                        );
                    }
                }
            }
            if !ready {
                tracing::error!(
                    "LHM still unreachable after {} retries (~{} min). \
                     The collector will continue and retry every {}s. \
                     Launch LibreHardwareMonitor as administrator and enable Options › Remote Web Server › Run.",
                    SOURCE_RETRY_MAX_ATTEMPTS,
                    SOURCE_RETRY_MAX_ATTEMPTS as u64 * SOURCE_RETRY_INTERVAL_SECS / 60,
                    settings.interval_secs,
                );
            }
        }
    }
    true
}

/// Runs until `stop` is set (then returns `Ok`), or fails if the store cannot be opened.
pub fn run_monitor(
    open_store: StoreFactory<'_>,
    source: &dyn SensorSource,
    settings: &MonitorSettings,
    status: &CollectorStatus,
    stop: &AtomicBool,
) -> Result<()> {
    let store = open_store().context("Failed to open database")?;
    let store = store.as_ref();
    tracing::info!(
        "Collector started — interval: {}s, raw readings kept {} days (daily aggregates kept forever).",
        settings.interval_secs,
        settings.raw_retention_days
    );

    if !startup_checks(store, source, settings, status, stop) {
        return Ok(());
    }

    // Roll up the raw readings into daily aggregates once at startup, then hourly.
    // Run purge once at startup, then every 24 h. The purge itself aggregates
    // whatever it is about to delete, so no history is lost.
    let rollup_every = (3600 / settings.interval_secs).max(1);
    let purge_every = (86400 / settings.interval_secs).max(1);
    let mut iterations: u64 = 0;
    let mut empty_streak: u32 = 0;
    let mut first_run = true;

    loop {
        if stop.load(Ordering::Relaxed) {
            tracing::info!("Stop signal received — exiting the collector.");
            return Ok(());
        }

        if iterations.is_multiple_of(rollup_every) {
            // Yesterday + today: yesterday may have been completed since the last pass.
            match store.rollup_since(&day_string(1)) {
                Ok(_) => status.record_maintenance_ok(),
                Err(e) => {
                    tracing::warn!("Daily roll-up error: {:#}", e);
                    status.record_maintenance_error(&format!("{e:#}"));
                }
            }
        }

        if iterations.is_multiple_of(purge_every) {
            match store.purge_old_data(settings.raw_retention_days) {
                Ok(0) => {}
                Ok(n) => {
                    tracing::info!(
                        "Purged {} snapshot(s) older than {} days (daily aggregates kept).",
                        n,
                        settings.raw_retention_days
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
                Err(e) => {
                    tracing::warn!("Purge error: {:#}", e);
                    status.record_maintenance_error(&format!("{e:#}"));
                }
            }
        }

        if first_run {
            tracing::info!("First collection starting…");
            first_run = false;
        }

        let outcome = collect_and_store(store, source);
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
                        EMPTY_ALERT_THRESHOLD as u64 * settings.interval_secs / 60
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
        if sleep_interruptible(settings.interval_secs, stop) {
            return Ok(());
        }
    }
}
