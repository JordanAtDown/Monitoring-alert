use crate::db;
use std::path::Path;

fn make_conn() -> rusqlite::Connection {
    db::init_db(Path::new(":memory:")).expect("in-memory db")
}

// ── Insertion / lecture de base ───────────────────────────────

#[test]
fn insert_and_query_snapshot() {
    let conn = make_conn();
    let id = db::insert_snapshot(&conn, "2024-01-01T12:00:00", Some(10.0), None, "idle")
        .expect("insert snapshot");
    assert!(id > 0);

    let stats = db::get_overall_stats(&conn).expect("stats");
    assert_eq!(stats.total_snapshots, 1);
    assert_eq!(stats.first_ts.as_deref(), Some("2024-01-01T12:00:00"));
}

#[test]
fn insert_reading_is_linked_to_its_snapshot() {
    let conn = make_conn();
    let snap_id =
        db::insert_snapshot(&conn, "2024-01-01T12:00:00", Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, snap_id, "AMDCPU", "CPU Package", 42.5).unwrap();

    let (hw, sensor, value): (String, String, f64) = conn
        .query_row(
            "SELECT hardware, sensor, value FROM readings WHERE snapshot_id = ?1",
            [snap_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (hw.as_str(), sensor.as_str(), value),
        ("AMDCPU", "CPU Package", 42.5)
    );
}

#[test]
fn stats_on_empty_db() {
    let conn = make_conn();
    let stats = db::get_overall_stats(&conn).expect("stats");
    assert_eq!(stats.total_snapshots, 0);
    assert!(stats.first_ts.is_none());
}

#[test]
fn init_db_creates_schema() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("test.db");
    let conn = db::init_db(&path).expect("init db");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
        .expect("query");
    assert_eq!(count, 0);
}

// ── Purge ─────────────────────────────────────────────────────

#[test]
fn purge_removes_old_snapshots() {
    let conn = make_conn();
    // Snapshot très ancien
    let sid = db::insert_snapshot(&conn, "2000-01-01T00:00:00", Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, sid, "CPU", "Package", 40.0).unwrap();

    let removed = db::purge_old_snapshots(&conn, 30).expect("purge");
    assert_eq!(removed, 1);

    let stats = db::get_overall_stats(&conn).unwrap();
    assert_eq!(stats.total_snapshots, 0);
}

#[test]
fn purge_preserves_recent_snapshots() {
    let conn = make_conn();
    // Snapshot futur → ne doit jamais être purgé
    let sid = db::insert_snapshot(&conn, "2030-06-01T00:00:00", Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, sid, "CPU", "Package", 45.0).unwrap();

    let removed = db::purge_old_snapshots(&conn, 30).unwrap();
    assert_eq!(removed, 0);

    let stats = db::get_overall_stats(&conn).unwrap();
    assert_eq!(stats.total_snapshots, 1);
}

#[test]
fn purge_removes_orphaned_readings() {
    let conn = make_conn();
    let sid = db::insert_snapshot(&conn, "2000-01-01T00:00:00", Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, sid, "CPU", "Package", 40.0).unwrap();
    db::insert_reading(&conn, sid, "GPU", "Junction", 75.0).unwrap();

    db::purge_old_snapshots(&conn, 30).unwrap();

    // Aucune lecture orpheline ne doit subsister
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM readings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn purge_on_empty_db_does_nothing() {
    let conn = make_conn();
    let removed = db::purge_old_snapshots(&conn, 30).expect("purge on empty db");
    assert_eq!(removed, 0);
}

#[test]
fn purge_old_kept_recent_mix() {
    let conn = make_conn();
    // Ancien
    db::insert_snapshot(&conn, "2000-01-01T00:00:00", Some(5.0), None, "idle").unwrap();
    // Récent
    db::insert_snapshot(&conn, "2030-06-01T00:00:00", Some(5.0), None, "idle").unwrap();

    let removed = db::purge_old_snapshots(&conn, 30).unwrap();
    assert_eq!(removed, 1);

    let stats = db::get_overall_stats(&conn).unwrap();
    assert_eq!(stats.total_snapshots, 1);
    assert_eq!(stats.first_ts.as_deref(), Some("2030-06-01T00:00:00"));
}

// ── Vacuum ────────────────────────────────────────────────────

#[test]
fn vacuum_runs_without_error() {
    let conn = make_conn();
    db::insert_snapshot(&conn, "2000-01-01T00:00:00", Some(5.0), None, "idle").unwrap();
    db::purge_old_snapshots(&conn, 30).unwrap();
    db::vacuum(&conn).expect("vacuum should not fail");
}

// ── Journal mode ──────────────────────────────────────────────

#[test]
fn file_database_uses_wal_so_readers_do_not_block_the_writer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wal.db");
    let writer = db::init_db(&path).unwrap();

    let mode: String = writer
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");

    // A second connection (the dashboard) can read while the first holds a write transaction.
    writer.execute_batch("BEGIN IMMEDIATE").unwrap();
    db::insert_snapshot(&writer, "2024-01-01T12:00:00", None, None, "idle").unwrap();
    let reader = db::init_db(&path).unwrap();
    let stats = db::get_overall_stats(&reader).unwrap();
    assert_eq!(stats.total_snapshots, 0);
    writer.execute_batch("COMMIT").unwrap();
}

// ── Daily aggregates ──────────────────────────────────────────

/// Inserts one snapshot with one reading of `hardware`/`sensor`.
fn seed(conn: &rusqlite::Connection, ts: &str, cat: &str, sensor: &str, value: f64) {
    let id = db::insert_snapshot(conn, ts, Some(10.0), None, cat).unwrap();
    db::insert_reading(conn, id, "CPU", sensor, value).unwrap();
}

struct Stat {
    n: i64,
    min: f64,
    median: f64,
    p95: f64,
    max: f64,
    avg: f64,
}

fn stat(conn: &rusqlite::Connection, day: &str, sensor: &str, cat: &str) -> Option<Stat> {
    conn.query_row(
        "SELECT n, min, median, p95, max, avg FROM daily_stats
         WHERE day = ?1 AND sensor = ?2 AND load_cat = ?3",
        rusqlite::params![day, sensor, cat],
        |r| {
            Ok(Stat {
                n: r.get(0)?,
                min: r.get(1)?,
                median: r.get(2)?,
                p95: r.get(3)?,
                max: r.get(4)?,
                avg: r.get(5)?,
            })
        },
    )
    .ok()
}

fn stats_rows(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM daily_stats", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn rollup_computes_the_daily_statistics() {
    let conn = make_conn();
    for (i, v) in [43.0, 40.0, 50.0, 41.0, 42.0].into_iter().enumerate() {
        seed(
            &conn,
            &format!("2024-03-10T10:0{i}:00"),
            "idle",
            "Package",
            v,
        );
    }

    let written = db::rollup_since(&conn, "2024-03-10").unwrap();
    assert_eq!(written, 1);

    let s = stat(&conn, "2024-03-10", "Package", "idle").expect("row");
    assert_eq!(s.n, 5);
    assert_eq!((s.min, s.median, s.max), (40.0, 42.0, 50.0));
    assert!((s.avg - 43.2).abs() < 1e-9);
    // p95 of [40,41,42,43,50] by linear interpolation = 43 + 0.8 * 7
    assert!((s.p95 - 48.6).abs() < 1e-9);
}

#[test]
fn rollup_median_of_an_even_count_interpolates() {
    let conn = make_conn();
    for (i, v) in [40.0, 42.0, 44.0, 46.0].into_iter().enumerate() {
        seed(
            &conn,
            &format!("2024-03-10T10:0{i}:00"),
            "idle",
            "Package",
            v,
        );
    }
    db::rollup_since(&conn, "2024-03-10").unwrap();
    assert_eq!(
        stat(&conn, "2024-03-10", "Package", "idle").unwrap().median,
        43.0
    );
}

#[test]
fn rollup_keeps_days_sensors_and_load_categories_apart() {
    let conn = make_conn();
    seed(&conn, "2024-03-10T10:00:00", "idle", "Package", 40.0);
    seed(&conn, "2024-03-10T11:00:00", "heavy", "Package", 80.0);
    seed(&conn, "2024-03-10T12:00:00", "idle", "Core 1", 41.0);
    seed(&conn, "2024-03-11T10:00:00", "idle", "Package", 45.0);

    assert_eq!(db::rollup_since(&conn, "2024-03-10").unwrap(), 4);
    assert_eq!(
        stat(&conn, "2024-03-10", "Package", "idle").unwrap().median,
        40.0
    );
    assert_eq!(
        stat(&conn, "2024-03-10", "Package", "heavy")
            .unwrap()
            .median,
        80.0
    );
    assert_eq!(
        stat(&conn, "2024-03-10", "Core 1", "idle").unwrap().median,
        41.0
    );
    assert_eq!(
        stat(&conn, "2024-03-11", "Package", "idle").unwrap().median,
        45.0
    );
}

#[test]
fn rollup_is_idempotent_and_picks_up_new_readings() {
    let conn = make_conn();
    seed(&conn, "2024-03-10T10:00:00", "idle", "Package", 40.0);
    db::rollup_since(&conn, "2024-03-10").unwrap();
    db::rollup_since(&conn, "2024-03-10").unwrap();
    assert_eq!(stats_rows(&conn), 1);

    seed(&conn, "2024-03-10T10:05:00", "idle", "Package", 44.0);
    db::rollup_since(&conn, "2024-03-10").unwrap();
    assert_eq!(stats_rows(&conn), 1);
    let s = stat(&conn, "2024-03-10", "Package", "idle").unwrap();
    assert_eq!((s.n, s.median), (2, 42.0));
}

#[test]
fn rollup_since_leaves_earlier_days_untouched() {
    let conn = make_conn();
    seed(&conn, "2024-03-09T10:00:00", "idle", "Package", 40.0);
    seed(&conn, "2024-03-10T10:00:00", "idle", "Package", 50.0);
    db::rollup_since(&conn, "2024-03-09").unwrap();

    // Raw data of the 9th disappears; a later roll-up of the 10th must not touch the 9th.
    conn.execute_batch("DELETE FROM readings WHERE value = 40.0")
        .unwrap();
    db::rollup_since(&conn, "2024-03-10").unwrap();
    assert_eq!(
        stat(&conn, "2024-03-09", "Package", "idle").unwrap().median,
        40.0
    );
}

#[test]
fn rollup_on_empty_db_writes_nothing() {
    let conn = make_conn();
    assert_eq!(db::rollup_since(&conn, "2000-01-01").unwrap(), 0);
}

#[test]
fn purge_aggregates_before_deleting_and_keeps_the_aggregates() {
    let conn = make_conn();
    seed(&conn, "2000-01-01T10:00:00", "idle", "Package", 40.0);
    seed(&conn, "2000-01-01T10:05:00", "idle", "Package", 42.0);

    assert_eq!(db::purge_old_snapshots(&conn, 30).unwrap(), 2);

    let raw: i64 = conn
        .query_row("SELECT COUNT(*) FROM readings", [], |r| r.get(0))
        .unwrap();
    assert_eq!(raw, 0);
    let s = stat(&conn, "2000-01-01", "Package", "idle").expect("aggregate survives the purge");
    assert_eq!((s.n, s.median), (2, 41.0));
}

#[test]
fn purge_removes_whole_days_only() {
    let conn = make_conn();
    let cutoff = db::day_string(30);
    let before = db::day_string(31);
    seed(
        &conn,
        &format!("{before}T23:59:59"),
        "idle",
        "Package",
        40.0,
    );
    seed(
        &conn,
        &format!("{cutoff}T00:00:01"),
        "idle",
        "Package",
        41.0,
    );

    assert_eq!(db::purge_old_snapshots(&conn, 30).unwrap(), 1);
    // The cutoff day itself is still complete in the raw data.
    let kept: String = conn
        .query_row("SELECT ts FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert!(kept.starts_with(&cutoff));
}

// ── Migration v1 → v2 ─────────────────────────────────────────

/// Creates a database as written by monitoring-alert 1.x (no `daily_stats`).
fn create_v1_db(path: &Path, snapshots: &[(&str, &str, f64)]) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE snapshots (
             id INTEGER PRIMARY KEY AUTOINCREMENT, ts TEXT NOT NULL,
             cpu_load REAL, gpu_load REAL, load_cat TEXT NOT NULL);
         CREATE TABLE readings (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
             hardware TEXT NOT NULL, sensor TEXT NOT NULL, value REAL NOT NULL);",
    )
    .unwrap();
    for (ts, cat, value) in snapshots {
        seed(&conn, ts, cat, "Package", *value);
    }
}

fn user_version(conn: &rusqlite::Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn migrating_a_v1_database_backfills_the_aggregates_and_keeps_a_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    create_v1_db(
        &path,
        &[
            ("2024-01-01T10:00:00", "idle", 40.0),
            ("2024-01-01T10:05:00", "idle", 42.0),
            ("2024-01-02T10:00:00", "heavy", 80.0),
        ],
    );

    let conn = db::init_db(&path).unwrap();

    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
    assert_eq!(stats_rows(&conn), 2);
    assert_eq!(
        stat(&conn, "2024-01-01", "Package", "idle").unwrap().median,
        41.0
    );
    assert_eq!(
        stat(&conn, "2024-01-02", "Package", "heavy").unwrap().max,
        80.0
    );

    // The raw data is untouched and a consistent copy of the old database exists.
    assert_eq!(db::get_overall_stats(&conn).unwrap().total_snapshots, 3);
    let backup = dir.path().join("temperatures.db.v1.bak");
    let old = rusqlite::Connection::open(&backup).unwrap();
    let n: i64 = old
        .query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3);
}

#[test]
fn migration_runs_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    create_v1_db(&path, &[("2024-01-01T10:00:00", "idle", 40.0)]);
    drop(db::init_db(&path).unwrap());

    // Raw data added behind the migration's back is not aggregated by a second open:
    // only the hourly roll-up does that, never the (one-shot) migration.
    let conn = db::init_db(&path).unwrap();
    seed(&conn, "2024-02-01T10:00:00", "idle", "Package", 50.0);
    drop(conn);
    let conn = db::init_db(&path).unwrap();
    assert_eq!(stats_rows(&conn), 1);
    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
}

#[test]
fn a_fresh_or_empty_database_needs_no_backup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    let conn = db::init_db(&path).unwrap();
    assert_eq!(user_version(&conn), db::SCHEMA_VERSION);
    assert!(!dir.path().join("temperatures.db.v1.bak").exists());
}

#[test]
fn an_existing_backup_is_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    let backup = dir.path().join("temperatures.db.v1.bak");
    std::fs::write(&backup, b"precious").unwrap();
    create_v1_db(&path, &[("2024-01-01T10:00:00", "idle", 40.0)]);

    drop(db::init_db(&path).unwrap());
    assert_eq!(std::fs::read(&backup).unwrap(), b"precious");
}
