use crate::adapters::sqlite::{self as db, SqliteStore};
use crate::application::collect::collect_and_store;
use crate::application::monitor::{run_monitor, MonitorSettings, StoreFactory};
use crate::application::ports::{SensorSource, TemperatureStore};
use crate::application::status::CollectorStatus;
use crate::domain::load::LoadTracker;
use crate::domain::model::{SnapshotData, TemperatureReading};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

/// A sensor source that replays scripted answers and can raise a stop flag.
struct FakeSource {
    answers: Mutex<Vec<Result<SnapshotData, String>>>,
    reads: AtomicUsize,
    stop_after: Option<(usize, &'static AtomicBool)>,
}

fn reading(sensor: &str, value: f64) -> TemperatureReading {
    TemperatureReading {
        hardware: "CPU".into(),
        sensor: sensor.into(),
        value,
    }
}

fn data(cpu: Option<f64>, gpu: Option<f64>, temps: &[(&str, f64)]) -> SnapshotData {
    SnapshotData {
        temperatures: temps.iter().map(|(s, v)| reading(s, *v)).collect(),
        extras: Vec::new(),
        cpu_load: cpu,
        gpu_load: gpu,
    }
}

impl FakeSource {
    fn new(answers: Vec<Result<SnapshotData, String>>) -> Self {
        Self {
            answers: Mutex::new(answers),
            reads: AtomicUsize::new(0),
            stop_after: None,
        }
    }
}

impl SensorSource for FakeSource {
    fn read(&self) -> anyhow::Result<SnapshotData> {
        let n = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some((limit, stop)) = self.stop_after {
            if n >= limit {
                stop.store(true, Ordering::SeqCst);
            }
        }
        let mut answers = self.answers.lock().unwrap();
        // The last scripted answer is repeated.
        let answer = if answers.len() > 1 {
            answers.remove(0)
        } else {
            answers[0].as_ref().map(clone_data).map_err(Clone::clone)
        };
        answer.map_err(|e| anyhow::anyhow!(e))
    }

    fn describe(&self) -> String {
        "fake".into()
    }
}

fn clone_data(d: &SnapshotData) -> SnapshotData {
    SnapshotData {
        extras: d.extras.clone(),
        ..data(
            d.cpu_load,
            d.gpu_load,
            &d.temperatures
                .iter()
                .map(|r| (r.sensor.as_str(), r.value))
                .collect::<Vec<_>>(),
        )
    }
}

fn memory_store() -> SqliteStore {
    SqliteStore::new(db::init_db(std::path::Path::new(":memory:")).unwrap())
}

fn category_and_load(store: &SqliteStore) -> (String, Option<f64>, Option<f64>) {
    store
        .connection()
        .query_row(
            "SELECT load_cat, cpu_load, gpu_load FROM snapshots",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
}

#[test]
fn a_collection_stores_the_snapshot_and_its_readings() {
    let store = memory_store();
    let source = FakeSource::new(vec![Ok(data(
        Some(12.0),
        None,
        &[("Package", 49.0), ("Core 1", 47.5)],
    ))]);

    assert_eq!(
        collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap(),
        2
    );

    let readings: i64 = store
        .connection()
        .query_row("SELECT COUNT(*) FROM readings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(readings, 2);
    assert_eq!(category_and_load(&store), ("idle".into(), Some(12.0), None));
}

#[test]
fn the_load_category_follows_the_busiest_of_cpu_and_gpu() {
    let store = memory_store();
    // Gaming: GPU 90 %, CPU 15 % → heavy, not light.
    let source = FakeSource::new(vec![Ok(data(Some(15.0), Some(90.0), &[("Package", 70.0)]))]);
    collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap();
    assert_eq!(category_and_load(&store).0, "heavy");
}

#[test]
fn a_snapshot_without_any_load_is_idle() {
    let store = memory_store();
    let source = FakeSource::new(vec![Ok(data(None, None, &[("Package", 35.0)]))]);
    collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap();
    assert_eq!(category_and_load(&store).0, "idle");
}

#[test]
fn no_temperature_sensor_is_reported_as_zero_readings() {
    let store = memory_store();
    let source = FakeSource::new(vec![Ok(data(Some(5.0), None, &[]))]);
    assert_eq!(
        collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap(),
        0
    );
}

#[test]
fn a_failing_source_stores_nothing() {
    let store = memory_store();
    let source = FakeSource::new(vec![Err("LHM is down".into())]);

    let error = collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap_err();
    assert!(format!("{error:#}").contains("LHM is down"));
    assert_eq!(store.overall_stats().unwrap().total_snapshots, 0);
}

#[test]
fn a_snapshot_is_stored_atomically() {
    // A reading that violates the schema must not leave a snapshot without readings behind.
    let store = memory_store();
    let bad = TemperatureReading {
        hardware: "CPU".into(),
        sensor: "Package".into(),
        value: f64::NAN, // NaN is stored as NULL, which readings.value (NOT NULL) refuses
    };
    let snapshot = crate::domain::model::NewSnapshot {
        ts: "2024-01-01T10:00:00",
        cpu_load: None,
        gpu_load: None,
        load_cat: "idle",
        readings: &[bad],
        extras: &[],
    };
    assert!(store.record_snapshot(&snapshot).is_err());
    assert_eq!(store.overall_stats().unwrap().total_snapshots, 0);
}

// ── Collector status ──────────────────────────────────────────

#[test]
fn the_status_tracks_successes_and_failures() {
    let status = CollectorStatus::new(300);
    assert!(status.snapshot().last_attempt.is_none());

    status.record_failure("boom");
    status.record_failure("boom");
    let s = status.snapshot();
    assert_eq!(s.consecutive_failures, 2);
    assert_eq!(s.last_error.as_deref(), Some("boom"));
    assert!(s.last_attempt.is_some() && s.last_success.is_none());

    status.record_success(7);
    let s = status.snapshot();
    assert_eq!((s.consecutive_failures, s.last_sensor_count), (0, 7));
    assert!(s.last_error.is_none() && s.last_success.is_some());
}

// ── The collection loop ───────────────────────────────────────

static STOP: AtomicBool = AtomicBool::new(false);

#[test]
fn the_monitor_collects_then_stops_when_asked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    STOP.store(false, Ordering::SeqCst);

    // read #1 = startup check, read #2 = first collection, which raises the stop flag.
    let mut source = FakeSource::new(vec![Ok(data(Some(3.0), None, &[("Package", 40.0)]))]);
    source.stop_after = Some((2, &STOP));
    let open_store = || -> anyhow::Result<Box<dyn TemperatureStore>> {
        Ok(Box::new(SqliteStore::new(db::init_db(&path)?)))
    };
    let factory: StoreFactory<'_> = &open_store;
    let status = CollectorStatus::new(60);
    let settings = MonitorSettings {
        interval_secs: 60,
        raw_retention_days: 90,
    };

    run_monitor(factory, &source, &settings, &status, &STOP).unwrap();

    assert_eq!(source.reads.load(Ordering::SeqCst), 2);
    let stored = SqliteStore::new(db::init_db(&path).unwrap());
    assert_eq!(stored.overall_stats().unwrap().total_snapshots, 1);
    let s = status.snapshot();
    assert_eq!((s.consecutive_failures, s.last_sensor_count), (0, 1));
}

#[test]
fn the_monitor_refuses_to_start_when_the_store_cannot_be_opened() {
    let stop = AtomicBool::new(true);
    let source = FakeSource::new(vec![Ok(data(None, None, &[]))]);
    let open_store =
        || -> anyhow::Result<Box<dyn TemperatureStore>> { anyhow::bail!("disk on fire") };
    let settings = MonitorSettings {
        interval_secs: 60,
        raw_retention_days: 90,
    };
    let error = run_monitor(
        &open_store,
        &source,
        &settings,
        &CollectorStatus::new(60),
        &stop,
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("disk on fire"));
}

// ── Waiting for the database at startup ───────────────────────

mod database_at_startup {
    use crate::daemon::open_database_with_retries;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn an_available_database_opens_at_once() {
        let dir = tempfile::tempdir().unwrap();
        let stop = AtomicBool::new(false);
        open_database_with_retries(&dir.path().join("t.db"), 3, 0, &stop).unwrap();
    }

    #[test]
    fn a_database_that_never_appears_is_an_error_after_the_last_attempt() {
        let dir = tempfile::tempdir().unwrap();
        // The "folder" of the database is a regular file: it can never be created.
        let blocker = dir.path().join("E-drive");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let stop = AtomicBool::new(false);

        let error =
            open_database_with_retries(&blocker.join("db").join("t.db"), 2, 0, &stop).unwrap_err();
        assert!(format!("{error:#}").contains("Failed to open database"));
    }

    #[test]
    fn a_drive_that_becomes_ready_later_is_picked_up() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("E-drive");
        std::fs::write(&blocker, b"not ready").unwrap();
        let db = blocker.join("t.db");

        // The "drive" appears 300 ms in, i.e. before the second attempt (1 s later).
        let unblock = {
            let blocker = blocker.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(300));
                std::fs::remove_file(&blocker).unwrap();
                std::fs::create_dir(&blocker).unwrap();
            })
        };
        let stop = AtomicBool::new(false);
        open_database_with_retries(&db, 5, 1, &stop).unwrap();
        unblock.join().unwrap();
        assert!(db.is_file());
    }

    #[test]
    fn a_stop_request_ends_the_wait() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("E-drive");
        std::fs::write(&blocker, b"x").unwrap();
        let stop = AtomicBool::new(true);
        // Would otherwise wait 10 x 10 s.
        open_database_with_retries(&blocker.join("t.db"), 10, 10, &stop).unwrap();
    }
}

#[test]
fn a_reading_right_after_an_effort_is_not_recorded_as_resting() {
    let store = memory_store();
    let mut tracker = LoadTracker::new();
    let gaming = FakeSource::new(vec![Ok(data(Some(20.0), Some(95.0), &[("Package", 80.0)]))]);
    let resting = FakeSource::new(vec![Ok(data(Some(2.0), Some(1.0), &[("Package", 60.0)]))]);

    collect_and_store(&store, &gaming, &mut tracker).unwrap();
    assert_eq!(category_and_load(&store).0, "heavy");

    // Next collection: the load is 2 %, but the machine has just stopped working.
    collect_and_store(&store, &resting, &mut tracker).unwrap();
    let categories: Vec<String> = store
        .connection()
        .prepare("SELECT load_cat FROM snapshots ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(categories, ["heavy", "cooldown"]);
}

#[test]
fn the_monitor_keeps_its_memory_from_one_collection_to_the_next() {
    static STOP_AFTER_REST: AtomicBool = AtomicBool::new(false);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");

    // read #1 = startup check, #2 = an effort, #3 = the machine back to rest, which stops the loop.
    let mut source = FakeSource::new(vec![
        Ok(data(Some(3.0), None, &[("Package", 40.0)])),
        Ok(data(Some(90.0), None, &[("Package", 80.0)])),
        Ok(data(Some(2.0), None, &[("Package", 55.0)])),
    ]);
    source.stop_after = Some((3, &STOP_AFTER_REST));
    let open_store = || -> anyhow::Result<Box<dyn TemperatureStore>> {
        Ok(Box::new(SqliteStore::new(db::init_db(&path)?)))
    };
    let status = CollectorStatus::new(1);
    let settings = MonitorSettings {
        interval_secs: 1,
        raw_retention_days: 90,
    };

    run_monitor(&open_store, &source, &settings, &status, &STOP_AFTER_REST).unwrap();

    let stored = SqliteStore::new(db::init_db(&path).unwrap());
    let categories: Vec<String> = stored
        .connection()
        .prepare("SELECT load_cat FROM snapshots ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // The loop carries what it saw from one collection to the next: the second reading is not "idle".
    assert_eq!(categories, ["heavy", "cooldown"]);
}

fn fan(sensor: &str, value: f64) -> crate::domain::model::ExtraReading {
    crate::domain::model::ExtraReading {
        hardware: "Board".into(),
        sensor: sensor.into(),
        kind: crate::domain::model::ExtraKind::Fan,
        value,
    }
}

#[test]
fn fan_speeds_and_power_draws_are_stored_with_their_snapshot() {
    let store = memory_store();
    let mut with_extras = data(Some(40.0), None, &[("Package", 60.0)]);
    with_extras.extras = vec![
        fan("CPU Fan", 1293.0),
        crate::domain::model::ExtraReading {
            hardware: "CPU".into(),
            sensor: "Package".into(),
            kind: crate::domain::model::ExtraKind::Power,
            value: 36.2,
        },
    ];
    let source = FakeSource::new(vec![Ok(with_extras)]);
    // The count returned is the one of temperatures: extras do not change what "no sensors" means.
    assert_eq!(
        collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap(),
        1
    );

    let rows: Vec<(String, String, f64)> = store
        .connection()
        .prepare("SELECT sensor, kind, value FROM extra_readings ORDER BY kind")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [
            ("CPU Fan".to_string(), "fan".to_string(), 1293.0),
            ("Package".to_string(), "power".to_string(), 36.2)
        ]
    );
}

#[test]
fn a_stopped_fan_is_a_reading_not_a_missing_one() {
    let store = memory_store();
    let mut stopped = data(Some(2.0), None, &[("Package", 40.0)]);
    stopped.extras = vec![fan("GPU Fan", 0.0)];
    let source = FakeSource::new(vec![Ok(stopped)]);
    collect_and_store(&store, &source, &mut LoadTracker::new()).unwrap();
    let value: f64 = store
        .connection()
        .query_row("SELECT value FROM extra_readings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(value, 0.0);
}

#[test]
fn a_snapshot_is_still_atomic_with_its_extras() {
    let store = memory_store();
    let mut broken = data(Some(2.0), None, &[("Package", 40.0)]);
    broken.extras = vec![fan("CPU Fan", f64::NAN)]; // NULL, which extra_readings.value refuses
    let source = FakeSource::new(vec![Ok(broken)]);
    assert!(collect_and_store(&store, &source, &mut LoadTracker::new()).is_err());
    // Neither the snapshot nor its temperatures survive a failed extra.
    assert_eq!(store.overall_stats().unwrap().total_snapshots, 0);
    let readings: i64 = store
        .connection()
        .query_row("SELECT COUNT(*) FROM readings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(readings, 0);
}
