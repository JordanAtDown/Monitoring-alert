use crate::collector::{effective_load, load_category};

// ── load_category — frontières de plage ───────────────────────

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
fn moderate_lower_bound() {
    assert_eq!(load_category(40.0), "moderate");
}

#[test]
fn moderate_upper_bound() {
    assert_eq!(load_category(74.9), "moderate");
}

#[test]
fn heavy_lower_bound() {
    assert_eq!(load_category(75.0), "heavy");
}

#[test]
fn heavy_upper_bound() {
    assert_eq!(load_category(100.0), "heavy");
}

#[test]
fn heavy_overflow() {
    // Valeurs hors plage (capteur LHM défectueux) → heavy par sécurité
    assert_eq!(load_category(110.0), "heavy");
}

// ── effective_load — max(cpu, gpu) ────────────────────────────

/// Scénario gaming : GPU 90 %, CPU 15 %.
/// Sans le fix : 15 % → "light". Avec le fix : 90 % → "heavy".
#[test]
fn effective_load_gpu_dominates_gaming_scenario() {
    let load = effective_load(Some(15.0), Some(90.0));
    assert_eq!(load, Some(90.0));
    assert_eq!(load_category(load.unwrap()), "heavy");
}

/// Charge CPU dominante : CPU 85 %, GPU 10 %.
#[test]
fn effective_load_cpu_dominates() {
    let load = effective_load(Some(85.0), Some(10.0));
    assert_eq!(load, Some(85.0));
    assert_eq!(load_category(load.unwrap()), "heavy");
}

/// Machine sans GPU discret.
#[test]
fn effective_load_cpu_only() {
    assert_eq!(effective_load(Some(50.0), None), Some(50.0));
}

/// GPU seul disponible (charge CPU non lue).
#[test]
fn effective_load_gpu_only() {
    assert_eq!(effective_load(None, Some(80.0)), Some(80.0));
}

/// Aucun capteur de charge → None → catégorisé "idle" en amont.
#[test]
fn effective_load_none_falls_back_to_idle() {
    assert_eq!(effective_load(None, None), None);
}

/// Valeurs égales → peu importe lequel est retourné.
#[test]
fn effective_load_equal_values() {
    assert_eq!(effective_load(Some(40.0), Some(40.0)), Some(40.0));
}

/// Valeurs à zéro : GPU 0 %, CPU 0 % → 0 % → "idle".
#[test]
fn effective_load_both_zero() {
    let load = effective_load(Some(0.0), Some(0.0));
    assert_eq!(load, Some(0.0));
    assert_eq!(load_category(load.unwrap()), "idle");
}

// ── collect_and_store against a fake LibreHardwareMonitor ─────

mod fake_lhm {
    use crate::collector::{collect_and_store, CollectorStatus};
    use crate::db;
    use crate::store::SqliteStore;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    const DATA_JSON: &str = r#"{
      "Text": "Sensor", "Value": "", "ImageURL": "", "Type": "", "Children": [
        { "Text": "PC-TEST", "Value": "", "ImageURL": "", "Type": "", "Children": [
          { "Text": "AMD Ryzen 7", "Value": "", "ImageURL": "images_icon/cpu.png", "Type": "", "Children": [
            { "Text": "Temperatures", "Value": "", "ImageURL": "", "Type": "", "Children": [
              { "Text": "CPU Package", "Value": "49,0 °C", "ImageURL": "", "Type": "Temperature", "Children": [] },
              { "Text": "Core #1", "Value": "47,5 °C", "ImageURL": "", "Type": "Temperature", "Children": [] }
            ]},
            { "Text": "Load", "Value": "", "ImageURL": "", "Type": "", "Children": [
              { "Text": "CPU Total", "Value": "45,0 %", "ImageURL": "", "Type": "Load", "Children": [] }
            ]}
          ]}
        ]}
      ]}"#;

    /// Serves `body` on a free local port until `stop` is set. Returns the port.
    pub fn spawn(body: &'static str, stop: Arc<AtomicBool>) -> u16 {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let port = server.server_addr().to_ip().unwrap().port();
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok(Some(req)) = server.recv_timeout(std::time::Duration::from_millis(100)) {
                    let _ = req.respond(tiny_http::Response::from_string(body));
                }
            }
        });
        port
    }

    #[test]
    fn collects_temperatures_and_load_from_the_http_endpoint() {
        let stop = Arc::new(AtomicBool::new(false));
        let port = spawn(DATA_JSON, stop.clone());
        let store = SqliteStore::new(db::init_db(std::path::Path::new(":memory:")).unwrap());

        let n = collect_and_store(&store, "127.0.0.1", port).unwrap();
        stop.store(true, Ordering::Relaxed);

        assert_eq!(n, 2);
        let conn = store_conn(&store);
        let (cat, cpu): (String, f64) = conn
            .query_row("SELECT load_cat, cpu_load FROM snapshots", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((cat.as_str(), cpu), ("moderate", 45.0));
        let temp: f64 = conn
            .query_row(
                "SELECT value FROM readings WHERE sensor = 'CPU Package'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(temp, 49.0);
    }

    #[test]
    fn an_unreachable_endpoint_is_an_error() {
        let store = SqliteStore::new(db::init_db(std::path::Path::new(":memory:")).unwrap());
        // Port 1 on loopback: nothing listens there.
        assert!(collect_and_store(&store, "127.0.0.1", 1).is_err());
    }

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

    fn store_conn(store: &SqliteStore) -> &rusqlite::Connection {
        store.connection()
    }
}
