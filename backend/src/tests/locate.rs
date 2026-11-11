use crate::adapters::locate::locate;
use crate::adapters::sqlite as db;
use std::path::Path;

fn database_with_snapshots(path: &Path, n: usize) {
    let conn = db::init_db(path).unwrap();
    for i in 0..n {
        db::insert_snapshot(
            &conn,
            &format!("2025-09-{:02}T10:00:00", i + 1),
            None,
            None,
            "idle",
        )
        .unwrap();
    }
}

#[test]
fn an_existing_database_is_described() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("monitoring.db").join("temperatures.db");
    database_with_snapshots(&path, 3);

    let found = locate(&path, &dir.path().join("default.db"));
    assert!(found.exists);
    assert_eq!(found.snapshots, Some(3));
    assert_eq!(found.first_day.as_deref(), Some("2025-09-01"));
    assert_eq!(found.summary(), "3 relevés bruts, depuis le 2025-09-01");
}

#[test]
fn a_missing_database_says_a_new_one_will_be_created() {
    let dir = tempfile::tempdir().unwrap();
    let found = locate(&dir.path().join("none.db"), &dir.path().join("default.db"));
    assert!(!found.exists);
    assert!(found.summary().contains("nouvelle"));
    // Looking never creates anything.
    assert!(!dir.path().join("none.db").exists());
}

#[test]
fn a_file_that_is_not_a_database_is_reported_as_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    std::fs::write(
        &path,
        b"this is not sqlite, just some text that is long enough",
    )
    .unwrap();
    let found = locate(&path, &dir.path().join("default.db"));
    assert!(found.exists);
    assert_eq!(found.snapshots, None);
    assert!(found.summary().contains("illisible"));
}

#[test]
fn locating_does_not_migrate_a_v1_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE snapshots (id INTEGER PRIMARY KEY, ts TEXT NOT NULL, cpu_load REAL,
                                     gpu_load REAL, load_cat TEXT NOT NULL);
             INSERT INTO snapshots (ts, load_cat) VALUES ('2024-01-01T10:00:00', 'idle');",
        )
        .unwrap();
    }
    let found = locate(&path, &dir.path().join("default.db"));
    assert_eq!(found.snapshots, Some(1));
    // No backup, no daily_stats table: the installer only looks.
    assert!(!dir.path().join("temperatures.db.v1.bak").exists());
    let conn = rusqlite::Connection::open(&path).unwrap();
    let tables: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name = 'daily_stats'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(tables, 0);
}

#[test]
fn the_porcelain_output_has_one_key_per_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("temperatures.db");
    database_with_snapshots(&path, 1);
    let found = locate(
        &path,
        Path::new(r"C:\Users\me\AppData\Local\Programs\MonitoringAlert\temperatures.db"),
    );

    let out = found.porcelain();
    let keys: Vec<&str> = out.lines().map(|l| l.split('=').next().unwrap()).collect();
    assert_eq!(keys, ["path", "default", "exists", "summary"]);
    assert!(out.contains("exists=true"));
    assert!(
        out.contains(r"default=C:\Users\me\AppData\Local\Programs\MonitoringAlert\temperatures.db")
    );
}
