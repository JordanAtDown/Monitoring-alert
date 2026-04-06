use crate::{
    report,
    store::TemperatureStore,
    tests::common::{make_store, ts_days_ago},
};

// ── Sanity — ne panique pas ───────────────────────────────────

#[test]
fn report_on_empty_db_does_not_panic() {
    let store = make_store();
    report::generate_report(&store, None).expect("report on empty db");
}

#[test]
fn report_with_single_snapshot_does_not_panic() {
    let store = make_store();
    let snap = store
        .insert_snapshot(&ts_days_ago(1), Some(5.0), None, "idle")
        .unwrap();
    store
        .insert_reading(snap, "CPU", "CPU Package", 38.0)
        .unwrap();
    report::generate_report(&store, None).expect("report with data");
}

// ── Avertissement données insuffisantes ───────────────────────

#[test]
fn report_shows_insufficient_data_warning_when_under_180_days() {
    let store = make_store();
    // Seulement quelques jours de données
    let snap = store
        .insert_snapshot(&ts_days_ago(5), Some(5.0), None, "idle")
        .unwrap();
    store
        .insert_reading(snap, "CPU", "CPU Package", 45.0)
        .unwrap();

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("insuffisante") || output.contains("Insuffisante"),
        "should warn about insufficient data (< 180 days), got:\n{}",
        output
    );
}

#[test]
fn report_no_warning_when_enough_data() {
    let store = make_store();
    // Données depuis 181 jours
    let snap = store
        .insert_snapshot(&ts_days_ago(181), Some(5.0), None, "idle")
        .unwrap();
    store
        .insert_reading(snap, "CPU", "CPU Package", 45.0)
        .unwrap();

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        !output.contains("insuffisante") && !output.contains("Insuffisante"),
        "should NOT warn when data spans 181+ days, got:\n{}",
        output
    );
}

// ── Résumé des alertes ────────────────────────────────────────

#[test]
fn report_shows_no_alert_when_stable() {
    let store = make_store();
    // Données stables : Δ+1°C
    for day in 1..31 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 45.0)
            .unwrap();
    }
    for day in 31..61 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 44.0)
            .unwrap();
    }

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("Aucune alerte"),
        "stable data should show 'Aucune alerte', got:\n{}",
        output
    );
}

#[test]
fn report_shows_alert_in_summary_when_drifting() {
    let store = make_store();
    // Drift Δ+8°C
    for day in 1..31 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 58.0)
            .unwrap();
    }
    for day in 31..61 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 50.0)
            .unwrap();
    }

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("ATTENTION") || output.contains("CRITIQUE"),
        "drifting data should show alert in summary, got:\n{}",
        output
    );
}

#[test]
fn report_shows_critique_label_for_large_delta() {
    let store = make_store();
    // Drift Δ+12°C → CRITIQUE
    for day in 1..31 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 72.0)
            .unwrap();
    }
    for day in 31..61 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 60.0)
            .unwrap();
    }

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("CRITIQUE"),
        "Δ+12°C should show CRITIQUE label, got:\n{}",
        output
    );
}
