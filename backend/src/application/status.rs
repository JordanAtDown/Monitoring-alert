//! Health of the collection loop, shared with the dashboard.

use serde::Serialize;
use std::sync::Mutex;

use crate::application::clock::now_string;

#[derive(Default)]
struct StatusInner {
    last_attempt: Option<String>,
    last_success: Option<String>,
    last_error: Option<String>,
    consecutive_failures: u32,
    last_sensor_count: usize,
    last_rollup: Option<String>,
    maintenance_error: Option<String>,
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
    /// When the daily summaries were last computed.
    pub last_rollup: Option<String>,
    /// Why the last daily-summary / purge pass failed, if it did.
    pub maintenance_error: Option<String>,
}

/// Health of the collection loop, written by the collector and read by the web server.
pub struct CollectorStatus {
    started_at: String,
    interval_secs: u64,
    inner: Mutex<StatusInner>,
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

    /// The daily summaries were computed (and old readings purged) without error.
    pub fn record_maintenance_ok(&self) {
        let mut s = self.lock();
        s.last_rollup = Some(now_string());
        s.maintenance_error = None;
    }

    /// A daily-summary or purge pass failed.
    pub fn record_maintenance_error(&self, error: &str) {
        self.lock().maintenance_error = Some(error.to_string());
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
            last_rollup: s.last_rollup.clone(),
            maintenance_error: s.maintenance_error.clone(),
        }
    }
}
