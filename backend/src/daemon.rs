//! Composition root: plugs the adapters into the use cases and runs them.
//!
//! The collection loop and the dashboard live in one process, the daemon.

use anyhow::{Context, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::adapters::config::AppConfig;
use crate::adapters::http::{self, AppState};
use crate::adapters::lhm::LhmSource;
use crate::adapters::sqlite::{self, SqliteStore};
use crate::application::monitor::{run_monitor, sleep_interruptible, MonitorSettings};
use crate::application::ports::{ReadModel, TemperatureStore};
use crate::application::queries::AppInfo;
use crate::application::status::CollectorStatus;

/// Delay before the collector is restarted after an unexpected failure.
const COLLECTOR_RESTART_DELAY_SECS: u64 = 30;

/// How long the daemon waits for the database to become reachable at startup: at logon a
/// secondary or network drive may not be mounted yet, and nothing would restart the
/// process afterwards.
const DB_OPEN_ATTEMPTS: u32 = 30;
const DB_OPEN_RETRY_SECS: u64 = 10;

/// Opens (and migrates) the database, retrying while it is unreachable.
pub fn open_database_with_retries(
    db_path: &Path,
    attempts: u32,
    delay_secs: u64,
    stop: &AtomicBool,
) -> Result<()> {
    for attempt in 1..=attempts {
        match sqlite::init_db(db_path) {
            Ok(conn) => {
                drop(conn);
                return Ok(());
            }
            Err(e) if attempt < attempts => {
                tracing::warn!(
                    "Database not reachable ({e:#}) — attempt {attempt}/{attempts}, retrying in \
                     {delay_secs}s (is its drive ready?)"
                );
                if sleep_interruptible(delay_secs, stop) {
                    return Ok(());
                }
            }
            Err(e) => return Err(e).context("Failed to open database"),
        }
    }
    Ok(())
}

/// Runs the collection loop, restarting it if it ever fails or panics: with no
/// service manager around, nothing else would bring it back.
pub fn supervise_collector(
    cfg: &AppConfig,
    db_path: &Path,
    status: &CollectorStatus,
    stop: &AtomicBool,
) {
    let source = LhmSource::new(&cfg.lhm_host, cfg.lhm_port);
    let settings = MonitorSettings {
        interval_secs: cfg.collect_interval_secs,
        raw_retention_days: cfg.raw_retention_days,
    };
    let open_store = || -> Result<Box<dyn TemperatureStore>> {
        Ok(Box::new(SqliteStore::new(sqlite::init_db(db_path)?)))
    };

    while !stop.load(Ordering::Relaxed) {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            run_monitor(&open_store, &source, &settings, status, stop)
        }));
        match outcome {
            Ok(Ok(())) => return, // stop requested
            Ok(Err(e)) => {
                tracing::error!("Collector stopped: {e:#}");
                status.record_failure(&format!("{e:#}"));
            }
            Err(_) => {
                tracing::error!("Collector thread panicked");
                status.record_failure("collector thread panicked");
            }
        }
        tracing::info!("Restarting the collector in {COLLECTOR_RESTART_DELAY_SECS}s…");
        sleep_interruptible(COLLECTOR_RESTART_DELAY_SECS, stop);
    }
}

/// Starts the dashboard and the collection loop, and blocks until `stop` is set
/// (the daemon never sets it: it runs until the process is killed).
///
/// Listens first: if the port is already taken, another instance is running and
/// this one exits before touching the database.
pub fn run(cfg: &AppConfig, db_path: &Path, stop: Arc<AtomicBool>) -> Result<()> {
    let listener = http::bind(&cfg.web_host, cfg.web_port)?;
    // Opening the database also migrates it to the current schema.
    open_database_with_retries(db_path, DB_OPEN_ATTEMPTS, DB_OPEN_RETRY_SECS, &stop)?;

    let status = Arc::new(CollectorStatus::new(cfg.collect_interval_secs));
    let path = db_path.to_path_buf();
    let state = Arc::new(AppState {
        open_read_model: Box::new(move || {
            Ok(Box::new(sqlite::open_read_model(&path)?) as Box<dyn ReadModel>)
        }),
        status: status.clone(),
        info: AppInfo {
            lhm_host: cfg.lhm_host.clone(),
            lhm_port: cfg.lhm_port,
            raw_retention_days: cfg.raw_retention_days,
        },
        allowed_hosts: http::allowed_hosts_for(&cfg.web_host),
    });
    tracing::info!(
        "Dashboard available at http://{}:{}/",
        cfg.web_host,
        cfg.web_port
    );

    std::thread::scope(|scope| {
        scope.spawn(|| supervise_collector(cfg, db_path, &status, &stop));
        http::run(listener, state, stop.clone());
    });
    Ok(())
}
