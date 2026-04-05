#[cfg(test)]
mod collector_tests {
    use crate::collector::load_category;

    #[test]
    fn idle_lower_bound() {
        assert_eq!(load_category(0.0), "idle");
    }

    #[test]
    fn idle_upper_bound() {
        assert_eq!(load_category(14.9), "idle");
    }

    #[test]
    fn light_lower_bound() {
        assert_eq!(load_category(15.0), "light");
    }

    #[test]
    fn light_upper_bound() {
        assert_eq!(load_category(39.9), "light");
    }

    #[test]
    fn moderate_range() {
        assert_eq!(load_category(40.0), "moderate");
        assert_eq!(load_category(74.9), "moderate");
    }

    #[test]
    fn heavy_range() {
        assert_eq!(load_category(75.0), "heavy");
        assert_eq!(load_category(100.0), "heavy");
    }
}

#[cfg(test)]
mod db_tests {
    use crate::db;

    fn in_memory_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE snapshots (
                 id        INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts        TEXT NOT NULL,
                 cpu_load  REAL,
                 gpu_load  REAL,
                 load_cat  TEXT NOT NULL
             );
             CREATE TABLE readings (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                 hardware    TEXT NOT NULL,
                 sensor      TEXT NOT NULL,
                 value       REAL NOT NULL
             );",
        )
        .expect("schema");
        conn
    }

    #[test]
    fn insert_and_query_snapshot() {
        let conn = in_memory_db();
        let id = db::insert_snapshot(&conn, "2024-01-01T12:00:00", Some(10.0), None, "idle")
            .expect("insert snapshot");
        assert!(id > 0);

        let stats = db::get_overall_stats(&conn).expect("stats");
        assert_eq!(stats.total_snapshots, 1);
        assert_eq!(stats.first_ts.as_deref(), Some("2024-01-01T12:00:00"));
    }

    #[test]
    fn insert_reading_and_query_sensors() {
        let conn = in_memory_db();
        let snap_id = db::insert_snapshot(&conn, "2024-01-01T12:00:00", Some(5.0), None, "idle")
            .expect("insert snapshot");
        db::insert_reading(&conn, snap_id, "AMDCPU", "CPU Package", 42.5).expect("insert reading");

        let sensors = db::get_distinct_sensors(&conn).expect("sensors");
        assert_eq!(sensors.len(), 1);
        assert_eq!(sensors[0].hardware, "AMDCPU");
        assert_eq!(sensors[0].sensor, "CPU Package");
    }

    #[test]
    fn stats_on_empty_db() {
        let conn = in_memory_db();
        let stats = db::get_overall_stats(&conn).expect("stats");
        assert_eq!(stats.total_snapshots, 0);
        assert!(stats.first_ts.is_none());
    }

    #[test]
    fn category_distribution_empty() {
        let conn = in_memory_db();
        let dist = db::get_category_distribution(&conn, 90).expect("distribution");
        assert!(dist.is_empty());
    }

    #[test]
    fn init_db_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        let conn = db::init_db(&path).expect("init db");
        // Verify schema exists
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM snapshots", [], |r| r.get(0))
            .expect("query");
        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod report_tests {
    use crate::{db, report};

    fn in_memory_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch(
            "CREATE TABLE snapshots (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 ts TEXT NOT NULL, cpu_load REAL, gpu_load REAL, load_cat TEXT NOT NULL
             );
             CREATE TABLE readings (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 snapshot_id INTEGER NOT NULL REFERENCES snapshots(id),
                 hardware TEXT NOT NULL, sensor TEXT NOT NULL, value REAL NOT NULL
             );",
        )
        .expect("schema");
        conn
    }

    #[test]
    fn report_on_empty_db_does_not_panic() {
        let conn = in_memory_conn();
        report::generate_report(&conn, None).expect("report on empty db");
    }

    #[test]
    fn report_with_data_does_not_panic() {
        let conn = in_memory_conn();
        let snap = db::insert_snapshot(&conn, "2024-01-01T00:00:00", Some(5.0), None, "idle")
            .expect("insert");
        db::insert_reading(&conn, snap, "AMDCPU", "CPU Package", 38.0).expect("reading");
        report::generate_report(&conn, None).expect("report with data");
    }
}
