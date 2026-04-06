use crate::db;
use crate::tests::common::ts_days_ago;
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
fn insert_reading_and_query_sensors() {
    let conn = make_conn();
    let snap_id =
        db::insert_snapshot(&conn, "2024-01-01T12:00:00", Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, snap_id, "AMDCPU", "CPU Package", 42.5).unwrap();

    let sensors = db::get_distinct_sensors(&conn).expect("sensors");
    assert_eq!(sensors.len(), 1);
    assert_eq!(sensors[0].hardware, "AMDCPU");
    assert_eq!(sensors[0].sensor, "CPU Package");
}

#[test]
fn stats_on_empty_db() {
    let conn = make_conn();
    let stats = db::get_overall_stats(&conn).expect("stats");
    assert_eq!(stats.total_snapshots, 0);
    assert!(stats.first_ts.is_none());
}

#[test]
fn category_distribution_empty() {
    let conn = make_conn();
    let dist = db::get_category_distribution(&conn, 90).expect("distribution");
    assert!(dist.is_empty());
}

#[test]
fn category_distribution_counts_correctly() {
    let conn = make_conn();
    let ts = "2030-01-01T00:00:00"; // future = always within any window
    db::insert_snapshot(&conn, ts, Some(5.0), None, "idle").unwrap();
    db::insert_snapshot(&conn, ts, Some(5.0), None, "idle").unwrap();
    db::insert_snapshot(&conn, ts, Some(80.0), None, "heavy").unwrap();

    let dist = db::get_category_distribution(&conn, 90).expect("distribution");
    let map: std::collections::HashMap<_, _> =
        dist.into_iter().map(|c| (c.load_cat, c.count)).collect();
    assert_eq!(map.get("idle").copied().unwrap_or(0), 2);
    assert_eq!(map.get("heavy").copied().unwrap_or(0), 1);
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

// ── get_avg_for_window ────────────────────────────────────────

#[test]
fn get_avg_for_window_basic_average() {
    let conn = make_conn();
    // Insère 3 lectures dans les 30 derniers jours (hier, avant-hier, il y a 3j)
    for (day, temp) in [(1i64, 60.0_f64), (2, 70.0), (3, 80.0)] {
        let ts = ts_days_ago(day);
        let sid = db::insert_snapshot(&conn, &ts, Some(5.0), None, "idle").unwrap();
        db::insert_reading(&conn, sid, "CPU", "Package", temp).unwrap();
    }
    let avg = db::get_avg_for_window(&conn, "CPU", "Package", "idle", 30, 0).expect("avg query");
    assert_eq!(avg, Some(70.0)); // (60+70+80)/3
}

#[test]
fn get_avg_for_window_no_data_returns_none() {
    let conn = make_conn();
    let avg = db::get_avg_for_window(&conn, "CPU", "Package", "idle", 30, 0).unwrap();
    assert_eq!(avg, None);
}

#[test]
fn get_avg_for_window_wrong_load_cat_returns_none() {
    let conn = make_conn();
    let sid = db::insert_snapshot(&conn, "2030-01-01T00:00:00", Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, sid, "CPU", "Package", 55.0).unwrap();

    // Même capteur, mais on demande la catégorie "heavy" alors que le snapshot est "idle"
    let avg = db::get_avg_for_window(&conn, "CPU", "Package", "heavy", 3650, 0).unwrap();
    assert_eq!(avg, None);
}

#[test]
fn get_avg_for_window_offset_isolates_reference_period() {
    let conn = make_conn();
    // Fenêtre courante [0..30 j] : 80 °C
    let s1 = db::insert_snapshot(&conn, &ts_days_ago(1), Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, s1, "CPU", "Package", 80.0).unwrap();
    // Fenêtre de référence [30..60 j] : 50 °C
    let s2 = db::insert_snapshot(&conn, &ts_days_ago(35), Some(5.0), None, "idle").unwrap();
    db::insert_reading(&conn, s2, "CPU", "Package", 50.0).unwrap();

    // Fenêtre courante : days=30, offset=0 → seulement s1 → 80
    let curr = db::get_avg_for_window(&conn, "CPU", "Package", "idle", 30, 0).unwrap();
    assert_eq!(curr, Some(80.0));

    // Fenêtre de référence : days=30, offset=30 → seulement s2 → 50
    let prev = db::get_avg_for_window(&conn, "CPU", "Package", "idle", 30, 30).unwrap();
    assert_eq!(prev, Some(50.0));

    // Fenêtre large : days=60, offset=0 → les deux → avg 65
    let all = db::get_avg_for_window(&conn, "CPU", "Package", "idle", 60, 0).unwrap();
    assert_eq!(all, Some(65.0));
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
