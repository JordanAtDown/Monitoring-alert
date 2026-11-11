use crate::collector::CollectorStatus;
use crate::db;
use crate::server::{self, route, AppState};
use crate::tests::common::insert_daily_series;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct Fixture {
    _dir: tempfile::TempDir,
    state: AppState,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let conn = db::init_db(&path).unwrap();

    // One year of a CPU slowly warming up at idle, a flat GPU, a young sensor.
    insert_daily_series(&conn, "AMD Ryzen 7", "Core (Tctl/Tdie)", "idle", 365, |i| {
        40.0 + 0.02 * i as f64
    });
    insert_daily_series(&conn, "NVIDIA GPU", "GPU Core", "idle", 365, |_| 35.0);
    insert_daily_series(&conn, "NVMe", "Temperature", "idle", 20, |_| 30.0);
    insert_daily_series(
        &conn,
        "AMD Ryzen 7",
        "Core (Tctl/Tdie)",
        "heavy",
        100,
        |_| 80.0,
    );
    let snap =
        db::insert_snapshot(&conn, "2030-01-01T10:00:00", Some(3.0), Some(1.0), "idle").unwrap();
    db::insert_reading(&conn, snap, "AMD Ryzen 7", "Core (Tctl/Tdie)", 41.5).unwrap();
    drop(conn);

    let state = AppState {
        db_path: path,
        status: Arc::new(CollectorStatus::new(300)),
        lhm_host: "127.0.0.1".into(),
        lhm_port: 8085,
        raw_retention_days: 90,
        allowed_hosts: server::allowed_hosts_for("127.0.0.1"),
    };
    Fixture { _dir: dir, state }
}

const HOST: Option<&str> = Some("127.0.0.1:8787");

fn get(f: &Fixture, url: &str) -> (u16, serde_json::Value) {
    let reply = route(&f.state, "GET", url, HOST);
    let json = serde_json::from_slice(&reply.body).unwrap_or(serde_json::Value::Null);
    (reply.status, json)
}

// ── /api/status ───────────────────────────────────────────────

#[test]
fn status_reports_collector_database_and_latest_readings() {
    let f = fixture();
    f.state.status.record_success(12);

    let (code, v) = get(&f, "/api/status");
    assert_eq!(code, 200);
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["collector"]["last_sensor_count"], 12);
    assert_eq!(v["collector"]["consecutive_failures"], 0);
    assert_eq!(v["collector"]["interval_secs"], 300);
    assert_eq!(v["lhm"]["port"], 8085);
    assert_eq!(v["db"]["total_snapshots"], 1);
    assert_eq!(v["db"]["raw_retention_days"], 90);
    assert!(v["db"]["size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(v["latest"]["load_cat"], "idle");
    assert_eq!(v["latest"]["readings"][0]["value"], 41.5);
}

#[test]
fn status_shows_a_failing_collector() {
    let f = fixture();
    f.state
        .status
        .record_failure("Cannot reach LibreHardwareMonitor");
    f.state
        .status
        .record_failure("Cannot reach LibreHardwareMonitor");

    let (_, v) = get(&f, "/api/status");
    assert_eq!(v["collector"]["consecutive_failures"], 2);
    assert_eq!(
        v["collector"]["last_error"],
        "Cannot reach LibreHardwareMonitor"
    );
    assert!(v["collector"]["last_success"].is_null());
}

#[test]
fn status_recovers_after_a_success() {
    let f = fixture();
    f.state.status.record_failure("down");
    f.state.status.record_success(3);
    let (_, v) = get(&f, "/api/status");
    assert_eq!(v["collector"]["consecutive_failures"], 0);
    assert!(v["collector"]["last_error"].is_null());
}

// ── /api/series, /api/trend, /api/summary ─────────────────────

#[test]
fn series_lists_each_sensor_per_load_category() {
    let f = fixture();
    let (code, v) = get(&f, "/api/series");
    assert_eq!(code, 200);
    assert_eq!(v.as_array().unwrap().len(), 4);
}

#[test]
fn trend_of_a_drifting_sensor() {
    let f = fixture();
    let (code, v) = get(
        &f,
        "/api/trend?hardware=AMD%20Ryzen%207&sensor=Core%20%28Tctl%2FTdie%29&load=idle",
    );
    assert_eq!(code, 200);
    assert_eq!(v["sufficient"], true);
    assert_eq!(v["days"], 365);
    // 0.02 °C/day ≈ 0.61 °C/month
    let slope = v["slope_per_month"].as_f64().unwrap();
    assert!((slope - 0.6088).abs() < 0.01, "slope = {slope}");
    assert!(v["delta"].as_f64().unwrap() > 6.0);
    assert!(v["points"].as_array().unwrap().len() == 365);
}

#[test]
fn trend_load_defaults_to_idle_and_accepts_plus_as_space() {
    let f = fixture();
    let (code, v) = get(&f, "/api/trend?hardware=NVIDIA+GPU&sensor=GPU+Core");
    assert_eq!(code, 200);
    assert_eq!(v["baseline"], 35.0);
}

#[test]
fn trend_of_a_young_sensor_reports_insufficient_history() {
    let f = fixture();
    let (_, v) = get(&f, "/api/trend?hardware=NVMe&sensor=Temperature&load=idle");
    assert_eq!(v["sufficient"], false);
    assert!(v["baseline"].is_null());
    assert_eq!(v["points"].as_array().unwrap().len(), 20);
}

#[test]
fn trend_of_an_unknown_sensor_is_empty_not_an_error() {
    let f = fixture();
    let (code, v) = get(&f, "/api/trend?hardware=Nope&sensor=Nada&load=idle");
    assert_eq!(code, 200);
    assert_eq!(v["days"], 0);
}

#[test]
fn trend_requires_hardware_and_sensor() {
    let f = fixture();
    assert_eq!(get(&f, "/api/trend?sensor=x").0, 400);
    assert_eq!(get(&f, "/api/trend?hardware=x").0, 400);
    assert_eq!(get(&f, "/api/trend").0, 400);
}

#[test]
fn unknown_load_category_is_rejected() {
    let f = fixture();
    assert_eq!(
        get(&f, "/api/trend?hardware=a&sensor=b&load=extreme").0,
        400
    );
    assert_eq!(get(&f, "/api/summary?load=extreme").0, 400);
}

#[test]
fn summary_lists_the_sensors_of_a_load_with_the_largest_drift_first() {
    let f = fixture();
    let (code, v) = get(&f, "/api/summary?load=idle");
    assert_eq!(code, 200);
    let names: Vec<&str> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["sensor"].as_str().unwrap())
        .collect();
    // Drifting CPU first, flat GPU, then the sensor without enough history.
    assert_eq!(names, ["Core (Tctl/Tdie)", "GPU Core", "Temperature"]);
    assert_eq!(v[0]["sufficient"], true);
    assert!(v[0]["sparkline"].as_array().unwrap().len() <= 60);
    assert_eq!(v[2]["sufficient"], false);
}

#[test]
fn summary_defaults_to_idle_and_filters_by_load() {
    let f = fixture();
    assert_eq!(get(&f, "/api/summary").1.as_array().unwrap().len(), 3);
    let (_, heavy) = get(&f, "/api/summary?load=heavy");
    assert_eq!(heavy.as_array().unwrap().len(), 1);
}

#[test]
fn the_current_day_is_left_out_of_the_trends() {
    let f = fixture();
    let conn = db::init_db(&f.state.db_path).unwrap();
    crate::tests::common::insert_daily(
        &conn,
        &db::day_string(0),
        "NVIDIA GPU",
        "GPU Core",
        "idle",
        99.0,
    );
    drop(conn);
    let (_, v) = get(&f, "/api/trend?hardware=NVIDIA%20GPU&sensor=GPU%20Core");
    assert_eq!(v["days"], 365);
    assert!(v["points"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["median"] == 35.0));
}

// ── Safety nets ───────────────────────────────────────────────

#[test]
fn only_get_and_head_are_accepted() {
    let f = fixture();
    for method in ["POST", "PUT", "DELETE", "PATCH"] {
        assert_eq!(route(&f.state, method, "/api/status", HOST).status, 405);
    }
    assert_eq!(route(&f.state, "HEAD", "/api/status", HOST).status, 200);
}

#[test]
fn requests_with_a_foreign_host_header_are_refused() {
    let f = fixture();
    for host in [
        Some("evil.example"),
        Some("evil.example:8787"),
        Some("127.0.0.1.evil.example"),
        None,
    ] {
        let reply = route(&f.state, "GET", "/api/status", host);
        assert_eq!(reply.status, 403, "host {host:?}");
    }
}

#[test]
fn loopback_host_names_are_accepted() {
    let f = fixture();
    for host in [
        "127.0.0.1",
        "127.0.0.1:8787",
        "localhost:8787",
        "LOCALHOST",
        "[::1]:8787",
        "[::1]",
    ] {
        let reply = route(&f.state, "GET", "/api/series", Some(host));
        assert_eq!(reply.status, 200, "host {host}");
    }
}

#[test]
fn any_host_is_accepted_when_listening_beyond_loopback() {
    let mut f = fixture();
    f.state.allowed_hosts = server::allowed_hosts_for("0.0.0.0");
    assert!(f.state.allowed_hosts.is_none());
    assert_eq!(
        route(&f.state, "GET", "/api/series", Some("pc-jordan:8787")).status,
        200
    );
}

#[test]
fn unknown_api_endpoint_and_unknown_file_are_404() {
    let f = fixture();
    assert_eq!(get(&f, "/api/nope").0, 404);
    assert_eq!(route(&f.state, "GET", "/nope.txt", HOST).status, 404);
}

#[test]
fn the_dashboard_page_is_served_at_the_root() {
    let f = fixture();
    let reply = route(&f.state, "GET", "/", HOST);
    assert_eq!(reply.status, 200);
    assert!(reply.content_type.starts_with("text/html"));
}

#[test]
fn a_missing_database_is_an_error_not_a_panic() {
    let mut f = fixture();
    f.state.db_path = f.state.db_path.with_file_name("missing.db");
    assert_eq!(route(&f.state, "GET", "/api/status", HOST).status, 500);
}

#[test]
fn query_strings_are_percent_decoded() {
    assert_eq!(
        server::percent_decode("Core%20%28Tctl%2FTdie%29"),
        "Core (Tctl/Tdie)"
    );
    assert_eq!(server::percent_decode("a+b"), "a b");
    assert_eq!(server::percent_decode("100%"), "100%");
    assert_eq!(server::percent_decode("%zz"), "%zz");
    assert_eq!(server::percent_decode("%C3%A9"), "é");
}

// ── Over a real socket ────────────────────────────────────────

#[test]
fn serves_requests_over_http_until_stopped() {
    let f = fixture();
    let listener = server::bind("127.0.0.1", 0).unwrap();
    let port = listener.server_addr().to_ip().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));

    let state = Arc::new(f.state);
    let handle = {
        let (listener, stop) = (listener.clone(), stop.clone());
        std::thread::spawn(move || server::run(listener, state, stop))
    };

    let base = format!("http://127.0.0.1:{port}");
    let status = ureq::get(&format!("{base}/api/status")).call().unwrap();
    assert_eq!(status.status(), 200);
    assert!(status
        .header("Content-Type")
        .unwrap()
        .starts_with("application/json"));
    assert_eq!(status.header("Cache-Control"), Some("no-store"));
    assert_eq!(status.header("X-Content-Type-Options"), Some("nosniff"));
    let v: serde_json::Value = status.into_json().unwrap();
    assert_eq!(v["db"]["total_snapshots"], 1);

    let page = ureq::get(&format!("{base}/")).call().unwrap();
    assert_eq!(page.status(), 200);

    let forbidden = ureq::get(&format!("{base}/api/status"))
        .set("Host", "evil.example")
        .call();
    assert!(matches!(forbidden, Err(ureq::Error::Status(403, _))));

    stop.store(true, Ordering::Relaxed);
    handle.join().unwrap();
}

#[test]
fn a_second_instance_cannot_bind_the_same_port() {
    let first = server::bind("127.0.0.1", 0).unwrap();
    let port = first.server_addr().to_ip().unwrap().port();
    assert!(server::bind("127.0.0.1", port).is_err());
}
