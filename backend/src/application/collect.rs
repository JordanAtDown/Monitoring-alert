//! One collection: read the sensors, classify the load, store the snapshot.

use anyhow::{Context, Result};

use crate::application::clock::now_string;
use crate::application::ports::{SensorSource, TemperatureStore};
use crate::domain::load::{effective_load, load_category};
use crate::domain::model::NewSnapshot;

/// Collects one snapshot and persists it.
/// Returns the number of temperature readings stored (0 means no sensors detected).
pub fn collect_and_store(store: &dyn TemperatureStore, source: &dyn SensorSource) -> Result<usize> {
    let data = source.read().context("Failed to read sensor data")?;
    let ts = now_string();
    let cat = effective_load(data.cpu_load, data.gpu_load)
        .map(load_category)
        .unwrap_or("idle");

    let snapshot_id = store
        .record_snapshot(&NewSnapshot {
            ts: &ts,
            cpu_load: data.cpu_load,
            gpu_load: data.gpu_load,
            load_cat: cat,
            readings: &data.temperatures,
        })
        .context("Failed to store snapshot")?;

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
