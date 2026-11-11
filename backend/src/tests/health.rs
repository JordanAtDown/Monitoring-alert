use crate::application::health::{ago, evaluate, Check, Health, HealthInputs, Level};
use crate::application::ports::{LatestReading, LatestSnapshot};
use crate::application::status::StatusSnapshot;
use crate::application::updates::UpdateSnapshot;

const NOW: &str = "2026-11-11T15:00:00";

fn collector() -> StatusSnapshot {
    StatusSnapshot {
        started_at: "2026-11-11T08:00:00".into(),
        interval_secs: 300,
        last_attempt: Some("2026-11-11T14:58:00".into()),
        last_success: Some("2026-11-11T14:58:00".into()),
        last_error: None,
        consecutive_failures: 0,
        last_sensor_count: 3,
        last_rollup: Some("2026-11-11T14:30:00".into()),
        maintenance_error: None,
    }
}

fn updates() -> UpdateSnapshot {
    UpdateSnapshot {
        current: "2.0.0".into(),
        enabled: true,
        available: false,
        latest: Some(crate::application::ports::ReleaseInfo {
            version: "2.0.0".into(),
            page_url: "https://github.com/x/y".into(),
            download_url: None,
            notes: None,
            published_at: None,
        }),
        checked_at: Some(NOW.into()),
        error: None,
    }
}

fn reading(hardware: &str, sensor: &str) -> LatestReading {
    LatestReading {
        hardware: hardware.into(),
        sensor: sensor.into(),
        value: 40.0,
    }
}

fn latest(readings: &[(&str, &str)]) -> LatestSnapshot {
    LatestSnapshot {
        ts: "2026-11-11T14:58:00".into(),
        cpu_load: Some(3.0),
        gpu_load: None,
        load_cat: "idle".into(),
        readings: readings.iter().map(|(h, s)| reading(h, s)).collect(),
    }
}

fn sensors() -> Vec<(String, String)> {
    [("CPU", "Package"), ("CPU", "Core 1"), ("GPU", "Core")]
        .iter()
        .map(|(h, s)| (h.to_string(), s.to_string()))
        .collect()
}

struct Scenario {
    collector: StatusSnapshot,
    latest: Option<LatestSnapshot>,
    recent: Vec<(String, String)>,
    updates: UpdateSnapshot,
    autostart: Option<bool>,
}

fn healthy() -> Scenario {
    Scenario {
        collector: collector(),
        latest: Some(latest(&[
            ("CPU", "Package"),
            ("CPU", "Core 1"),
            ("GPU", "Core"),
        ])),
        recent: sensors(),
        updates: updates(),
        autostart: Some(true),
    }
}

fn run(s: &Scenario) -> Health {
    evaluate(&HealthInputs {
        now: NOW,
        collector: &s.collector,
        latest: s.latest.as_ref(),
        recent_sensors: &s.recent,
        db_size_bytes: 12_201_984,
        db_snapshots: 6654,
        updates: &s.updates,
        autostart: s.autostart,
    })
}

fn find<'a>(h: &'a Health, id: &str) -> &'a Check {
    h.checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id}"))
}

// ── The whole picture ─────────────────────────────────────────

#[test]
fn when_everything_works_the_page_says_so() {
    let h = run(&healthy());
    assert_eq!(h.overall, Level::Ok);
    assert_eq!(h.summary, "Tout fonctionne");
    assert!(
        h.checks.iter().all(|c| c.level == Level::Ok),
        "{:?}",
        h.checks
    );
    let ids: Vec<&str> = h.checks.iter().map(|c| c.id).collect();
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
}

#[test]
fn every_check_that_is_fine_has_no_advice() {
    assert!(run(&healthy()).checks.iter().all(|c| c.advice.is_none()));
}

// ── Collection ────────────────────────────────────────────────

#[test]
fn a_collector_that_cannot_reach_lhm_is_a_problem_with_a_remedy() {
    let mut s = healthy();
    s.collector.consecutive_failures = 3;
    s.collector.last_error = Some("Cannot reach LibreHardwareMonitor".into());
    s.collector.last_success = Some("2026-11-11T14:50:00".into());
    let h = run(&s);

    assert_eq!(h.overall, Level::Bad);
    assert_eq!(h.summary, "Un problème empêche le suivi");
    let c = find(&h, "collection");
    assert_eq!(c.level, Level::Bad);
    assert!(
        c.message.contains("ne sont plus relevées")
            && c.message.contains("dernière mesure il y a 10 min")
    );
    assert!(c.advice.as_deref().unwrap().contains("Remote Web Server"));
}

#[test]
fn right_after_a_failure_the_last_measurement_is_not_mentioned() {
    let mut s = healthy();
    s.collector.consecutive_failures = 1;
    let h = run(&s); // last success was 2 minutes ago
    assert!(!find(&h, "collection").message.contains("dernière mesure"));
}

#[test]
fn before_the_first_measurement_it_is_a_warning_not_a_problem() {
    let mut s = healthy();
    s.collector.last_success = None;
    let c = run(&s);
    assert_eq!(find(&c, "collection").level, Level::Warn);
    assert_eq!(c.overall, Level::Warn);
}

#[test]
fn a_collector_that_went_silent_is_noticed_even_without_errors() {
    let mut s = healthy();
    s.collector.last_success = Some("2026-11-11T13:00:00".into());
    let h = run(&s);
    let c = find(&h, "collection");
    assert_eq!(c.level, Level::Warn);
    assert!(
        c.message.contains("Aucune mesure depuis 2 h"),
        "{}",
        c.message
    );
    assert!(c.advice.as_deref().unwrap().contains("restart"));
}

#[test]
fn a_measurement_just_inside_three_intervals_is_still_fine() {
    let mut s = healthy();
    s.collector.last_success = Some("2026-11-11T14:45:30".into()); // 14 min 30 s: < 3 x 5 min
    assert_eq!(find(&run(&s), "collection").level, Level::Ok);
    s.collector.last_success = Some("2026-11-11T14:44:00".into()); // 16 min
    assert_eq!(find(&run(&s), "collection").level, Level::Warn);
}

#[test]
fn lhm_answering_without_any_temperature_is_flagged() {
    let mut s = healthy();
    s.collector.last_sensor_count = 0;
    let h = run(&s);
    let c = find(&h, "collection");
    assert_eq!(c.level, Level::Warn);
    assert!(c.message.contains("ne fournit aucune température"));
}

// ── Sensors ───────────────────────────────────────────────────

#[test]
fn a_sensor_that_stopped_reporting_is_named() {
    let mut s = healthy();
    s.latest = Some(latest(&[("CPU", "Package"), ("CPU", "Core 1")]));
    let h = run(&s);
    let c = find(&h, "sensors");
    assert_eq!(c.level, Level::Warn);
    assert_eq!(c.message, "1 capteur ne répond plus : Core (GPU).");
    assert!(c.advice.is_some());
}

#[test]
fn many_missing_sensors_are_counted_and_only_a_few_named() {
    let mut s = healthy();
    s.recent = (1..=7)
        .map(|i| ("Disk".to_string(), format!("Temp {i}")))
        .collect();
    s.latest = Some(latest(&[]));
    let h = run(&s);
    let c = find(&h, "sensors");
    assert!(c.message.starts_with(
        "7 capteurs ne répondent plus : Temp 1 (Disk), Temp 2 (Disk), Temp 3 (Disk), Temp 4 (Disk)"
    ));
    assert!(c.message.ends_with("et 3 autre(s)."), "{}", c.message);
}

#[test]
fn a_new_sensor_that_was_never_seen_before_is_not_a_problem() {
    let mut s = healthy();
    s.latest = Some(latest(&[
        ("CPU", "Package"),
        ("CPU", "Core 1"),
        ("GPU", "Core"),
        ("USB", "New"),
    ]));
    assert_eq!(find(&run(&s), "sensors").level, Level::Ok);
}

#[test]
fn sensors_are_not_judged_against_an_old_measurement() {
    // Collection stopped long ago: that is the collection check's business, not a missing-sensor alarm.
    let mut s = healthy();
    s.latest = Some(LatestSnapshot {
        ts: "2026-11-10T08:00:00".into(),
        ..latest(&[("CPU", "Package")])
    });
    assert_eq!(find(&run(&s), "sensors").level, Level::Info);
}

#[test]
fn without_any_measurement_yet_the_sensors_check_is_informative() {
    let mut s = healthy();
    s.latest = None;
    assert_eq!(find(&run(&s), "sensors").level, Level::Info);
}

// ── Database and daily summaries ──────────────────────────────

#[test]
fn the_database_check_gives_its_size_and_last_write() {
    let h = run(&healthy());
    assert_eq!(
        find(&h, "database").message,
        "Lisible, 11,6 Mo et 6654 relevés, dernière écriture il y a 2 min."
    );
}

#[test]
fn a_failed_daily_summary_is_a_warning_that_reassures_about_the_data() {
    let mut s = healthy();
    s.collector.maintenance_error = Some("database is locked".into());
    let h = run(&s);
    let c = find(&h, "maintenance");
    assert_eq!(c.level, Level::Warn);
    assert!(c.message.contains("database is locked"));
    assert!(c.advice.as_deref().unwrap().contains("ne sont pas perdues"));
    assert_eq!(h.summary, "1 point à vérifier");
}

#[test]
fn before_the_first_daily_summary_it_is_only_informative() {
    let mut s = healthy();
    s.collector.last_rollup = None;
    assert_eq!(find(&run(&s), "maintenance").level, Level::Info);
}

// ── Start-up and updates ──────────────────────────────────────

#[test]
fn a_missing_startup_entry_is_flagged_and_an_unknown_one_is_left_out() {
    let mut s = healthy();
    s.autostart = Some(false);
    let h = run(&s);
    assert_eq!(find(&h, "autostart").level, Level::Warn);
    assert!(find(&h, "autostart")
        .advice
        .as_deref()
        .unwrap()
        .contains("installateur"));

    s.autostart = None;
    assert!(run(&s).checks.iter().all(|c| c.id != "autostart"));
}

#[test]
fn an_available_update_never_turns_the_page_orange() {
    let mut s = healthy();
    s.updates.available = true;
    s.updates.latest.as_mut().unwrap().version = "2.1.0".into();
    let h = run(&s);
    assert_eq!(find(&h, "updates").level, Level::Info);
    assert!(find(&h, "updates").message.contains("2.1.0"));
    assert_eq!(h.overall, Level::Ok, "an update is not a malfunction");
}

#[test]
fn being_offline_or_having_updates_off_is_only_informative() {
    let mut s = healthy();
    s.updates.latest = None;
    s.updates.error = Some("offline".into());
    assert_eq!(find(&run(&s), "updates").level, Level::Info);
    s.updates.enabled = false;
    assert!(find(&run(&s), "updates").message.contains("désactivée"));
    assert_eq!(run(&s).overall, Level::Ok);
}

// ── Summing up ────────────────────────────────────────────────

#[test]
fn a_problem_outweighs_warnings_and_warnings_are_counted() {
    let mut s = healthy();
    s.autostart = Some(false);
    s.collector.maintenance_error = Some("x".into());
    assert_eq!(run(&s).summary, "2 points à vérifier");

    s.collector.consecutive_failures = 1;
    let h = run(&s);
    assert_eq!(h.overall, Level::Bad);
    assert_eq!(h.summary, "Un problème empêche le suivi");
}

// ── Durations in words ────────────────────────────────────────

#[test]
fn durations_are_written_in_plain_french() {
    assert_eq!(ago(10), "à l'instant");
    assert_eq!(ago(89), "à l'instant");
    assert_eq!(ago(180), "il y a 3 min");
    assert_eq!(ago(2 * 3600), "il y a 2 h");
    assert_eq!(ago(4 * 86400), "il y a 4 j");
}
