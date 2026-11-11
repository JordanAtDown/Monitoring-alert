//! The long-running process: collection loop + dashboard web server.

use anyhow::{Context, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::collector::{self, CollectorStatus};
use crate::config::AppConfig;
use crate::db;
use crate::server::{self, AppState};

/// Delay before the collector is restarted after an unexpected failure.
const COLLECTOR_RESTART_DELAY_SECS: u64 = 30;

/// Runs the collection loop, restarting it if it ever fails or panics: with no
/// service manager around, nothing else would bring it back.
fn supervise_collector(
    cfg: &AppConfig,
    db_path: &Path,
    status: &CollectorStatus,
    stop: &Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Relaxed) {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            collector::watch(
                db_path,
                cfg.collect_interval_secs,
                cfg.raw_retention_days,
                &cfg.lhm_host,
                cfg.lhm_port,
                status,
                stop.clone(),
            )
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
        collector::sleep_interruptible(COLLECTOR_RESTART_DELAY_SECS, stop);
    }
}

/// Starts the dashboard and the collection loop, and blocks until `stop` is set
/// (the daemon never sets it: it runs until the process is killed).
///
/// Listens first: if the port is already taken, another instance is running and
/// this one exits before touching the database.
pub fn run(cfg: &AppConfig, db_path: &Path, stop: Arc<AtomicBool>) -> Result<()> {
    let listener = server::bind(&cfg.web_host, cfg.web_port)?;
    // Opening the database also migrates it to the current schema.
    drop(db::init_db(db_path).context("Failed to open database")?);

    let status = Arc::new(CollectorStatus::new(cfg.collect_interval_secs));
    let state = Arc::new(AppState {
        db_path: db_path.to_path_buf(),
        status: status.clone(),
        lhm_host: cfg.lhm_host.clone(),
        lhm_port: cfg.lhm_port,
        raw_retention_days: cfg.raw_retention_days,
        allowed_hosts: server::allowed_hosts_for(&cfg.web_host),
    });
    tracing::info!(
        "Dashboard available at http://{}:{}/",
        cfg.web_host,
        cfg.web_port
    );

    std::thread::scope(|scope| {
        scope.spawn(|| supervise_collector(cfg, db_path, &status, &stop));
        server::run(listener, state, stop.clone());
    });
    Ok(())
}
