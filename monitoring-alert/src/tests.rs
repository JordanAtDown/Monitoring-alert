#[cfg(test)]
mod collector_tests {
    use crate::collector::{effective_load, load_category};

    // ── load_category boundaries ──────────────────────────────

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

    // ── effective_load — max(cpu, gpu) ────────────────────────

    /// Gaming scenario: GPU 90 %, CPU 15 %.
    /// Without the fix this would return 15 % → "light".
    /// With the fix it returns 90 % → "heavy".
    #[test]
    fn effective_load_gpu_dominates_gaming_scenario() {
        let load = effective_load(Some(15.0), Some(90.0));
        assert_eq!(load, Some(90.0));
        assert_eq!(load_category(load.unwrap()), "heavy");
    }

    /// CPU-heavy workload: CPU 85 %, GPU 10 %.
    #[test]
    fn effective_load_cpu_dominates() {
        let load = effective_load(Some(85.0), Some(10.0));
        assert_eq!(load, Some(85.0));
        assert_eq!(load_category(load.unwrap()), "heavy");
    }

    /// CPU only (no discrete GPU).
    #[test]
    fn effective_load_cpu_only() {
        assert_eq!(effective_load(Some(50.0), None), Some(50.0));
    }

    /// GPU only (integrated GPU, CPU load unavailable).
    #[test]
    fn effective_load_gpu_only() {
        assert_eq!(effective_load(None, Some(80.0)), Some(80.0));
    }

    /// No sensor data available → None → categorised as "idle".
    #[test]
    fn effective_load_none_falls_back_to_idle() {
        assert_eq!(effective_load(None, None), None);
    }

    /// Both at same value → either is fine.
    #[test]
    fn effective_load_equal_values() {
        assert_eq!(effective_load(Some(40.0), Some(40.0)), Some(40.0));
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
    use crate::{
        db, report,
        store::{SqliteStore, TemperatureStore},
    };

    fn make_store() -> SqliteStore {
        SqliteStore::new(db::init_db(std::path::Path::new(":memory:")).expect("init store"))
    }

    #[test]
    fn report_on_empty_db_does_not_panic() {
        let store = make_store();
        report::generate_report(&store, None).expect("report on empty db");
    }

    #[test]
    fn report_with_data_does_not_panic() {
        let store = make_store();
        let snap = store
            .insert_snapshot("2024-01-01T00:00:00", Some(5.0), None, "idle")
            .expect("insert");
        store
            .insert_reading(snap, "AMDCPU", "CPU Package", 38.0)
            .expect("reading");
        report::generate_report(&store, None).expect("report with data");
    }
}

// ──────────────────────────────────────────────────────────────
// Integration tests — generate_summary() aggregation algorithm
// ──────────────────────────────────────────────────────────────
// These tests simulate real data collection by inserting rows
// with dynamic UTC timestamps, exercising the full
// get_avg_for_window + generate_summary aggregation pipeline.
#[cfg(test)]
mod integration_tests {
    use crate::{
        db, report,
        store::{SqliteStore, TemperatureStore},
    };

    /// Returns an ISO-8601 UTC timestamp N days in the past.
    fn ts_days_ago(days: i64) -> String {
        (chrono::Utc::now() - chrono::TimeDelta::days(days))
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string()
    }

    /// Opens an in-memory store with the full schema.
    fn make_store() -> SqliteStore {
        SqliteStore::new(db::init_db(std::path::Path::new(":memory:")).expect("init in-memory db"))
    }

    /// Inserts one snapshot + one reading per day across [days_start, days_end).
    /// days_start=1 means yesterday; days_end=31 covers the last 30 days.
    fn seed_window(
        store: &dyn TemperatureStore,
        hardware: &str,
        sensor: &str,
        load_cat: &str,
        days_start: i64,
        days_end: i64,
        temp: f64,
    ) {
        for day in days_start..days_end {
            let ts = ts_days_ago(day);
            let snap = store
                .insert_snapshot(&ts, Some(5.0), None, load_cat)
                .expect("insert snapshot");
            store
                .insert_reading(snap, hardware, sensor, temp)
                .expect("insert reading");
        }
    }

    // ── Titre des trois périodes ──────────────────────────────

    #[test]
    fn daily_title() {
        let store = make_store();
        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(s.title.contains("Journalier"), "titre daily: {:?}", s.title);
    }

    #[test]
    fn weekly_title() {
        let store = make_store();
        let s = report::generate_summary(&store, report::ReportPeriod::Weekly).unwrap();
        assert!(
            s.title.contains("Hebdomadaire"),
            "titre weekly: {:?}",
            s.title
        );
    }

    #[test]
    fn monthly_title() {
        let store = make_store();
        let s = report::generate_summary(&store, report::ReportPeriod::Monthly).unwrap();
        assert!(s.title.contains("Mensuel"), "titre monthly: {:?}", s.title);
    }

    // ── Scénario 1 : température stable (Δ < 5°C) ────────────

    #[test]
    fn stable_no_alert() {
        let store = make_store();
        // curr window [1..31): avg 45°C
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 45.0);
        // prev window [31..61): avg 44°C
        seed_window(&store, "CPU", "CPU Package", "idle", 31, 61, 44.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("stables"),
            "should be stable, got: {:?}",
            s.body
        );
    }

    // ── Scénario 2 : alerte simple (Δ = +6°C) ─────────────────

    #[test]
    fn single_warning() {
        let store = make_store();
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 55.0);
        seed_window(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("1 alerte"),
            "should have 1 alert, got: {:?}",
            s.body
        );
        assert!(
            s.body.contains("+6.0"),
            "should show +6.0°C delta, got: {:?}",
            s.body
        );
    }

    // ── Scénario 3 : seuil critique (Δ = +12°C) ──────────────

    #[test]
    fn critical_threshold() {
        let store = make_store();
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 65.0);
        seed_window(&store, "CPU", "CPU Package", "idle", 31, 61, 53.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("1 alerte"),
            "should have 1 alert, got: {:?}",
            s.body
        );
        assert!(
            s.body.contains("12.0"),
            "should show 12.0°C delta, got: {:?}",
            s.body
        );
    }

    // ── Scénario 4 : deux capteurs, une seule alerte ──────────

    #[test]
    fn multiple_sensors_one_alert() {
        let store = make_store();
        // CPU: Δ+6°C → alerte
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 55.0);
        seed_window(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0);
        // GPU: Δ+1°C → stable
        seed_window(&store, "GPU", "GPU Temperature", "idle", 1, 31, 71.0);
        seed_window(&store, "GPU", "GPU Temperature", "idle", 31, 61, 70.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("1 alerte"),
            "should have 1 alert, got: {:?}",
            s.body
        );
    }

    // ── Scénario 5 : deux capteurs, deux alertes ─────────────

    #[test]
    fn multiple_sensors_two_alerts() {
        let store = make_store();
        // CPU: Δ+6°C
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 55.0);
        seed_window(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0);
        // GPU: Δ+7°C (pire delta → doit apparaître en 1er dans le corps)
        seed_window(&store, "GPU", "GPU Temperature", "idle", 1, 31, 77.0);
        seed_window(&store, "GPU", "GPU Temperature", "idle", 31, 61, 70.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("2 alertes"),
            "should have 2 alerts, got: {:?}",
            s.body
        );
        // Le capteur avec le pire delta (GPU, +7°C) doit être mentionné
        assert!(
            s.body.contains("GPU Temperature"),
            "GPU Temperature should be the leading alert, got: {:?}",
            s.body
        );
    }

    // ── Scénario 6 : isolation par catégorie de charge ────────

    #[test]
    fn load_cat_isolation() {
        let store = make_store();
        // idle: Δ+3°C → pas d'alerte
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 43.0);
        seed_window(&store, "CPU", "CPU Package", "idle", 31, 61, 40.0);
        // heavy: Δ+8°C → alerte
        seed_window(&store, "CPU", "CPU Package", "heavy", 1, 31, 78.0);
        seed_window(&store, "CPU", "CPU Package", "heavy", 31, 61, 70.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("1 alerte"),
            "should have 1 alert (heavy only), got: {:?}",
            s.body
        );
    }

    // ── Scénario 7 : pas de données précédentes ───────────────

    #[test]
    fn no_previous_data() {
        let store = make_store();
        // Seulement les 30 derniers jours — aucune fenêtre précédente
        seed_window(&store, "CPU", "CPU Package", "idle", 1, 31, 65.0);

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        // Sans période précédente il n'y a pas de delta → stable
        assert!(
            s.body.contains("stables"),
            "no prev data → should be stable, got: {:?}",
            s.body
        );
    }

    // ── Scénarios GPU-heavy (effective_load = max(cpu, gpu)) ──
    //
    // Ces tests valident que les sessions gaming (GPU 90 %, CPU 15 %)
    // sont stockées en catégorie "heavy" et non "light".
    // Avant le fix, la catégorie était déterminée par le seul cpu_load,
    // ce qui rendait les températures GPU de gaming comparées à des
    // périodes à charge CPU légère — baseline incorrecte.

    /// Gaming récent plus chaud qu'avant : détection correcte du drift GPU.
    ///
    /// Sessions gaming = catégorie "heavy" grâce à effective_load.
    /// Les températures GPU actuelles (heavy) sont comparées aux températures
    /// GPU passées (heavy) : même contexte de charge → delta fiable.
    #[test]
    fn gpu_heavy_drift_detected() {
        let store = make_store();
        // Sessions gaming récentes : GPU Junction 83°C, catégorie heavy
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            1,
            31,
            83.0,
        );
        // Sessions gaming il y a 30-60j : GPU Junction 71°C, catégorie heavy
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            31,
            61,
            71.0,
        );

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        // Δ = +12°C sur sessions à charge identique → alerte critique
        assert!(
            s.body.contains("1 alerte"),
            "GPU drift should be detected, got: {:?}",
            s.body
        );
        assert!(
            s.body.contains("12.0"),
            "should show +12.0°C delta, got: {:?}",
            s.body
        );
    }

    /// Sans le fix, un scénario gaming serait stocké en "light" (cpu_load=15%).
    /// Les données "light" n'auraient pas de période précédente "light" gaming →
    /// pas de comparaison possible → drift non détecté.
    /// Ce test prouve que stocker en "heavy" permet la comparaison.
    #[test]
    fn gpu_heavy_drift_not_visible_in_wrong_category() {
        let store = make_store();
        // GPU chaud pendant des sessions "heavy" (gaming)
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            1,
            31,
            83.0,
        );
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            31,
            61,
            71.0,
        );
        // Mais on insère aussi des données "light" (navigation web) stables
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "light",
            1,
            31,
            45.0,
        );
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "light",
            31,
            61,
            44.0,
        );

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        // La catégorie "light" est stable → n'écrase pas l'alerte "heavy"
        assert!(
            s.body.contains("1 alerte"),
            "heavy drift should still be detected alongside stable light sessions: {:?}",
            s.body
        );
    }

    /// CPU et GPU tous les deux en drift simultané (machine de rendu 3D).
    #[test]
    fn cpu_and_gpu_both_drift_two_alerts() {
        let store = make_store();
        // CPU Package en drift sous charge heavy
        seed_window(&store, "CPU", "CPU Package", "heavy", 1, 31, 92.0);
        seed_window(&store, "CPU", "CPU Package", "heavy", 31, 61, 80.0);
        // GPU Junction en drift sous charge heavy
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            1,
            31,
            88.0,
        );
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            31,
            61,
            74.0,
        );

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        assert!(
            s.body.contains("2 alertes"),
            "both CPU and GPU drifts should be detected: {:?}",
            s.body
        );
    }

    /// Sessions gaming stables : GPU chaud mais pas de drift → pas d'alerte.
    #[test]
    fn gpu_heavy_stable_no_alert() {
        let store = make_store();
        // GPU chaud pendant gaming mais stable dans le temps
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            1,
            31,
            78.0,
        );
        seed_window(
            &store,
            "GPU",
            "GPU Junction Temperature",
            "heavy",
            31,
            61,
            77.0,
        );

        let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
        // Δ = +1°C → stable, même sous charge GPU lourde
        assert!(
            s.body.contains("stables"),
            "stable GPU temp should not trigger alert: {:?}",
            s.body
        );
    }
}
