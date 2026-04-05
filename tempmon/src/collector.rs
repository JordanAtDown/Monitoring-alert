use anyhow::{Context, Result};
use chrono::Local;
use rusqlite::Connection;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::{db, sensors};

pub fn load_category(cpu_load: f64) -> &'static str {
    match cpu_load as u32 {
        0..=14 => "idle",
        15..=39 => "light",
        40..=74 => "moderate",
        75..=100 => "heavy",
        _ => "heavy",
    }
}

pub fn collect_and_store(conn: &Connection) -> Result<()> {
    let data = sensors::read_sensors().context("Failed to read sensor data")?;
    let ts = Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let cat = data
        .cpu_load
        .map(load_category)
        .unwrap_or("idle")
        .to_string();

    let snapshot_id =
        db::insert_snapshot(conn, &ts, data.cpu_load, data.gpu_load, &cat)
            .context("Failed to insert snapshot")?;

    for reading in &data.temperatures {
        db::insert_reading(conn, snapshot_id, &reading.hardware, &reading.sensor, reading.value)
            .context("Failed to insert reading")?;
    }

    println!(
        "[{}] Snapshot #{} — CPU: {:.1}%, GPU: {:.1}%, cat: {}, temps: {}",
        ts,
        snapshot_id,
        data.cpu_load.unwrap_or(0.0),
        data.gpu_load.unwrap_or(0.0),
        cat,
        data.temperatures.len()
    );
    Ok(())
}

pub fn watch(db_path: &Path, interval_secs: u64, stop: Arc<AtomicBool>) -> Result<()> {
    let conn = db::init_db(db_path).context("Failed to open database")?;
    println!(
        "Starting watch loop every {}s. Press Ctrl+C to stop.",
        interval_secs
    );
    loop {
        if stop.load(Ordering::SeqCst) {
            println!("Stop signal received, exiting watch loop.");
            break;
        }
        if let Err(e) = collect_and_store(&conn) {
            eprintln!("Collection error: {:#}", e);
        }
        // Sleep in small increments to remain responsive to stop signals
        let mut elapsed = 0u64;
        while elapsed < interval_secs {
            if stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            elapsed += 1;
        }
    }
    Ok(())
}
