use crate::adapters::http::{self as server, route, AppState};
use crate::adapters::sqlite::{self as db, open_read_model};
use crate::application::app_info::AppInfo;
use crate::application::clock;
use crate::application::ports::ReadModel;
use crate::application::status::CollectorStatus;
use crate::application::updates::UpdateStatus;
use crate::tests::common::insert_daily_series;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct Fixture {
    _dir: tempfile::TempDir,
    path: std::path::PathBuf,
    state: AppState,
}

fn reader_for(path: std::path::PathBuf) -> server::ReadModelFactory {
    Box::new(move || Ok(Box::new(open_read_model(&path)?) as Box<dyn ReadModel>))
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
        open_read_model: reader_for(path.clone()),
        status: Arc::new(CollectorStatus::new(300)),
        updates: Arc::new(UpdateStatus::new("2.0.0", true)),
        autostart: Box::new(|| Some(true)),
        info: AppInfo {
            db_path: r"E:\monitoring.db\temperatures.db".into(),
            lhm_host: "127.0.0.1".into(),
            lhm_port: 8085,
            raw_retention_days: 90,
        },
        allowed_hosts: server::allowed_hosts_for("127.0.0.1"),
    };
    Fixture {
        _dir: dir,
        path,
        state,
    }
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
    let conn = db::init_db(&f.path).unwrap();
    crate::tests::common::insert_daily(
        &conn,
        &clock::day_string(0),
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

// ── /api/update, and the figures about the history ────────────

#[test]
fn update_reports_the_running_version_and_a_known_newer_one() {
    let f = fixture();
    let (code, v) = get(&f, "/api/update");
    assert_eq!(code, 200);
    assert_eq!(v["current"], "2.0.0");
    assert_eq!(v["available"], false);

    f.state
        .updates
        .record_release(crate::application::ports::ReleaseInfo {
            version: "2.1.0".into(),
            page_url: "https://github.com/x/y/releases/tag/v2.1.0".into(),
            download_url: Some(
                "https://github.com/x/y/releases/download/v2.1.0/monitoring_2.1.0.exe".into(),
            ),
            notes: None,
            published_at: None,
        });
    let (_, v) = get(&f, "/api/update");
    assert_eq!(v["available"], true);
    assert_eq!(v["latest"]["version"], "2.1.0");
    assert!(v["latest"]["download_url"]
        .as_str()
        .unwrap()
        .ends_with("monitoring_2.1.0.exe"));
}

#[test]
fn status_tells_where_the_database_is_and_since_when_the_history_goes() {
    let f = fixture();
    let (_, v) = get(&f, "/api/status");
    assert_eq!(v["db"]["path"], r"E:\monitoring.db\temperatures.db");
    // The oldest daily aggregate of the fixture: 365 days + 1 back.
    assert_eq!(v["db"]["history_since"], clock::day_string(366));
}

#[test]
fn status_tells_what_is_kept_in_words() {
    let f = fixture();
    let (_, v) = get(&f, "/api/status");
    let data = v["data"].as_array().unwrap();
    let titles: Vec<&str> = data.iter().map(|d| d["title"].as_str().unwrap()).collect();
    assert_eq!(
        titles,
        ["Résumé de chaque jour", "Mesures détaillées", "Fichier"]
    );
    assert!(data[0]["text"]
        .as_str()
        .unwrap()
        .contains("conservé sans limite"));
    assert!(data[1]["text"]
        .as_str()
        .unwrap()
        .contains("une toutes les 5 minutes"));
    assert!(data[1]["text"]
        .as_str()
        .unwrap()
        .contains("conservées 90 jours, 1 relevés"));
    let file = data[2]["text"].as_str().unwrap();
    assert!(
        file.starts_with(r" : E:\monitoring.db\temperatures.db ("),
        "{file}"
    );
}

// ── /api/health ───────────────────────────────────────────────

#[test]
fn health_lists_every_check_of_a_working_system() {
    let f = fixture();
    f.state.status.record_success(12);
    f.state.status.record_maintenance_ok();
    let (code, v) = get(&f, "/api/health");
    assert_eq!(code, 200);
    // The fixture's only snapshot dates from 2030: it is "in the future", so it counts as fresh.
    let ids: Vec<&str> = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "collection",
            "sensors",
            "database",
            "maintenance",
            "autostart",
            "updates"
        ]
    );
    assert_eq!(v["checks"][0]["level"], "ok");
    assert_eq!(v["checks"][4]["level"], "ok");
}

#[test]
fn health_turns_bad_when_the_collector_fails() {
    let f = fixture();
    f.state
        .status
        .record_failure("Cannot reach LibreHardwareMonitor");
    let (_, v) = get(&f, "/api/health");
    assert_eq!(v["overall"], "bad");
    assert_eq!(v["summary"], "Un problème empêche le suivi");
}

#[test]
fn health_without_a_known_startup_state_leaves_that_check_out() {
    let mut f = fixture();
    f.state.autostart = Box::new(|| None);
    let (_, v) = get(&f, "/api/health");
    assert!(v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["id"] != "autostart"));
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
fn every_dashboard_file_is_served_with_its_content_type() {
    let f = fixture();
    for (path, mime) in [
        ("/index.html", "text/html"),
        ("/assets/app.js", "text/javascript"),
        ("/assets/app.css", "text/css"),
        ("/favicon.svg", "image/svg+xml"),
    ] {
        let reply = route(&f.state, "GET", path, HOST);
        assert_eq!(reply.status, 200, "{path}");
        assert!(
            reply.content_type.starts_with(mime),
            "{path}: {}",
            reply.content_type
        );
    }
}

#[test]
fn a_missing_database_is_an_error_not_a_panic() {
    let mut f = fixture();
    f.state.open_read_model = reader_for(f.path.with_file_name("missing.db"));
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

// ── /api/overview ─────────────────────────────────────────────

#[test]
fn the_overview_gives_the_components_and_the_headline_ready_to_display() {
    let f = fixture();
    let (code, v) = get(&f, "/api/overview?load=idle");
    assert_eq!(code, 200);

    let titles: Vec<&str> = v["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    // The processor, the graphics card, and the one disk: a single disk is not numbered.
    assert_eq!(titles, ["Processeur", "Carte graphique", "Disque"]);

    let cpu = &v["components"][0];
    assert_eq!(cpu["subtitle"], "AMD Ryzen 7");
    assert_eq!(cpu["main"]["name"], "Cœurs du processeur");
    assert_eq!(cpu["main"]["verdict"]["id"], "warmer");
    assert_eq!(cpu["main"]["verdict"]["label"], "Plus chaud");
    assert!(cpu["main"]["verdict"]["detail"]
        .as_str()
        .unwrap()
        .starts_with('+'));
    assert!(cpu["main"]["baseline"].as_str().unwrap().ends_with(" °C"));
    assert_eq!(cpu["slot"], 0);

    // The disk has too little history: said in words, no figure invented.
    let disk = &v["components"][2];
    assert_eq!(disk["main"]["verdict"]["id"], "unknown");
    assert_eq!(disk["main"]["baseline"], serde_json::Value::Null);
    assert_eq!(disk["main"]["verdict"]["detail"], "(20 j de mesures)");

    assert_eq!(v["headline"]["state"], "ok");
    let lead = v["headline"]["lead"].as_str().unwrap();
    assert!(lead.starts_with("Depuis le "), "{lead}");
    assert!(lead.ends_with("quand le PC est au repos :"), "{lead}");
    let tiles = v["headline"]["tiles"].as_array().unwrap();
    assert_eq!(tiles[0]["verdict"], "warmer");
    assert_eq!(tiles[0]["words"], "composant plus chaud qu'au début");
    assert_eq!(tiles[1]["words"], "composant stable");
    assert_eq!(tiles[3]["words"], "composant sans assez de recul");
    assert!(v["headline"]["detail"]
        .as_str()
        .unwrap()
        .starts_with("Le changement le plus marqué : Processeur (+"));
}

#[test]
fn the_overview_offers_the_load_levels_with_their_explanation() {
    let f = fixture();
    let (_, v) = get(&f, "/api/overview?load=heavy");
    assert_eq!(v["load"]["id"], "heavy");
    assert_eq!(v["load"]["choices"].as_array().unwrap().len(), 4);
    assert!(v["load"]["note"]
        .as_str()
        .unwrap()
        .starts_with("Charge élevée : plus de 75 % d'utilisation"));
    // 100 days at heavy load for the processor only.
    assert_eq!(v["components"].as_array().unwrap().len(), 1);
}

#[test]
fn an_overview_without_enough_history_says_how_much_is_needed() {
    let f = fixture();
    let (_, v) = get(&f, "/api/overview?load=moderate");
    assert_eq!(v["headline"]["state"], "empty");
    assert_eq!(v["components"].as_array().unwrap().len(), 0);
}

#[test]
fn the_overview_rejects_an_unknown_load() {
    let f = fixture();
    assert_eq!(get(&f, "/api/overview?load=turbo").0, 400);
}

// ── /api/curves ───────────────────────────────────────────────

fn series_named<'a>(v: &'a serde_json::Value, title: &str) -> &'a serde_json::Value {
    v["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["title"] == title)
        .unwrap_or_else(|| panic!("no series {title}"))
}

#[test]
fn the_curves_give_one_line_per_component_in_temperatures() {
    let f = fixture();
    let (code, v) = get(&f, "/api/curves?load=idle");
    assert_eq!(code, 200);
    assert_eq!(v["mode"], "temp");
    let cpu = series_named(&v, "Processeur");
    assert_eq!(cpu["available"], true);
    assert_eq!(cpu["sensor_name"], "Cœurs du processeur");
    assert!(cpu["now"].as_str().unwrap().ends_with(" °C"));
    assert!(cpu["points"].as_array().unwrap().len() > 300);
    assert!(cpu["reference"]["label"]
        .as_str()
        .unwrap()
        .starts_with("Au début : "));
    // The disk has 20 days only: drawn, but with no beginning to compare to.
    let disk = series_named(&v, "Disque");
    assert_eq!(disk["available"], true);
    assert_eq!(disk["reference"], serde_json::Value::Null);
}

#[test]
fn the_curves_can_show_the_change_since_the_beginning() {
    let f = fixture();
    let (_, v) = get(&f, "/api/curves?load=idle&mode=delta");
    assert_eq!(v["mode"], "delta");
    let cpu = series_named(&v, "Processeur");
    assert!(cpu["now"].as_str().unwrap().starts_with('+'));
    assert_eq!(cpu["reference"]["value"], 0.0);
    // Starts around 0, ends where the sensor drifted to.
    let points = cpu["points"].as_array().unwrap();
    assert!(points[0]["value"].as_f64().unwrap().abs() < 1.0);
    assert!(points.last().unwrap()["value"].as_f64().unwrap() > 3.0);
    // No beginning yet: said in words, not drawn.
    let disk = series_named(&v, "Disque");
    assert_eq!(disk["available"], false);
    assert_eq!(disk["note"], "pas assez de recul");
    assert!(disk["points"].as_array().unwrap().is_empty());
}

#[test]
fn the_curves_come_with_their_months_as_a_table() {
    let f = fixture();
    let (_, v) = get(&f, "/api/curves?load=idle");
    let months = v["months"].as_array().unwrap();
    assert!(months.len() >= 12);
    assert!(months[0]["label"].as_str().unwrap().contains(' '));
    // One cell per series, in the same order; a month a sensor did not cover is empty.
    assert_eq!(
        months[0]["cells"].as_array().unwrap().len(),
        v["series"].as_array().unwrap().len()
    );
    assert!(months.last().unwrap()["cells"][0]
        .as_str()
        .unwrap()
        .ends_with(" °C"));
}

#[test]
fn the_curves_reject_an_unknown_mode_or_load() {
    let f = fixture();
    assert_eq!(get(&f, "/api/curves?mode=percent").0, 400);
    assert_eq!(get(&f, "/api/curves?load=turbo").0, 400);
}

// ── /api/sensor ───────────────────────────────────────────────

#[test]
fn a_sensor_is_explained_in_a_sentence_with_its_figures_in_bold() {
    let f = fixture();
    let (code, v) = get(
        &f,
        "/api/sensor?hardware=AMD%20Ryzen%207&sensor=Core%20(Tctl%2FTdie)&load=idle",
    );
    assert_eq!(code, 200);
    assert_eq!(v["name"], "Cœurs du processeur");
    assert_eq!(
        v["subtitle"],
        "Processeur · AMD Ryzen 7 · au repos · capteur « Core (Tctl/Tdie) »"
    );
    let story = &v["story"][0];
    assert_eq!(story["kind"], "story");
    let spans = story["spans"].as_array().unwrap();
    let text: String = spans.iter().map(|s| s["text"].as_str().unwrap()).collect();
    assert!(text.starts_with("Au début (du "), "{text}");
    assert!(text.contains("sa température habituelle était de "));
    assert!(text.contains("de plus qu'au début."));
    assert_eq!(spans.iter().filter(|s| s["strong"] == true).count(), 2);
    // It got warmer: what may explain it is said, as a separate line.
    assert_eq!(v["story"][1]["kind"], "advice");
    assert_eq!(v["legend"].as_array().unwrap().len(), 2);
    assert!(!v["rolling"].as_array().unwrap().is_empty());
    assert!(v["months"][0]["usual"].as_str().unwrap().ends_with(" °C"));
    assert_eq!(v["technical"][0]["label"], "Pente générale");
    assert!(v["technical"][0]["text"].as_str().unwrap().starts_with('+'));
}

#[test]
fn a_young_sensor_says_it_is_too_early_and_offers_no_statistics() {
    let f = fixture();
    let (_, v) = get(&f, "/api/sensor?hardware=NVMe&sensor=Temperature&load=idle");
    let text = v["story"][0]["spans"][0]["text"].as_str().unwrap();
    assert!(
        text.starts_with("Pour l'instant : 20 jours de mesures sur 20 jours de suivi"),
        "{text}"
    );
    assert_eq!(v["story"].as_array().unwrap().len(), 1);
    assert!(v["technical"].as_array().unwrap().is_empty());
    assert_eq!(v["reference"], serde_json::Value::Null);
    assert_eq!(v["legend"].as_array().unwrap().len(), 1);
}

#[test]
fn an_unknown_sensor_is_an_empty_page_not_an_error() {
    let f = fixture();
    let (code, v) = get(&f, "/api/sensor?hardware=Nope&sensor=Nothing&load=idle");
    assert_eq!(code, 200);
    assert!(v["story"][0]["spans"][0]["text"]
        .as_str()
        .unwrap()
        .starts_with("Pas encore de journée complète"));
    assert!(v["rolling"].as_array().unwrap().is_empty());
}

#[test]
fn a_sensor_needs_a_hardware_and_a_name() {
    let f = fixture();
    assert_eq!(get(&f, "/api/sensor?sensor=x").0, 400);
}
