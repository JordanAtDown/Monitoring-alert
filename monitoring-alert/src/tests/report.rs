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
fn report_shows_insufficient_data_warning_when_under_360_days() {
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
        "should warn about insufficient data (< 360 days), got:\n{}",
        output
    );
}

/// At 181 days we're past the 90j window but still below 360 — warning remains.
#[test]
fn report_still_warns_when_between_180_and_360_days() {
    let store = make_store();
    let snap = store
        .insert_snapshot(&ts_days_ago(200), Some(5.0), None, "idle")
        .unwrap();
    store
        .insert_reading(snap, "CPU", "CPU Package", 45.0)
        .unwrap();

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("insuffisante") || output.contains("Insuffisante"),
        "should still warn at 200 days (< 360 required), got:\n{}",
        output
    );
    // At 200 days the 90j window is available — "Comparaison 90j" line should NOT appear.
    assert!(
        !output.contains("Comparaison 90j"),
        "90j note should be absent when days > 180, got:\n{}",
        output
    );
}

#[test]
fn report_no_warning_when_enough_data() {
    let store = make_store();
    // Données depuis 361 jours — au-delà du minimum de 360
    let snap = store
        .insert_snapshot(&ts_days_ago(361), Some(5.0), None, "idle")
        .unwrap();
    store
        .insert_reading(snap, "CPU", "CPU Package", 45.0)
        .unwrap();

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        !output.contains("insuffisante") && !output.contains("Insuffisante"),
        "should NOT warn when data spans 361+ days, got:\n{}",
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

// ── Recommandation maintenance ────────────────────────────────

/// 360 jours de données, dérive de +12°C sur 180j → inspection urgente.
#[test]
fn report_maintenance_inspection_urgent_on_180j_drift() {
    let store = make_store();
    // Courant 180j : 75°C — référence 180j précédente : 63°C → Δ+12°C
    for day in 1..181 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 75.0)
            .unwrap();
    }
    for day in 181..361 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 63.0)
            .unwrap();
    }

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("Inspection matérielle urgente"),
        "Δ+12°C on 180j should trigger inspection urgente, got:\n{}",
        output
    );
}

/// Dérive de +9°C sur 90j seulement (180j stable) → maintenance préventive.
#[test]
fn report_maintenance_preventive_on_90j_drift() {
    let store = make_store();
    // Courant 90j : 70°C — précédent 90j : 61°C → Δ+9°C
    for day in 1..91 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 70.0)
            .unwrap();
    }
    // Jours 91-361 : température stable à 61°C (référence 90j et fenêtre 180j homogène)
    for day in 91..361 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 61.0)
            .unwrap();
    }

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("Maintenance préventive"),
        "Δ+9°C on 90j should trigger maintenance préventive, got:\n{}",
        output
    );
    assert!(
        !output.contains("Inspection matérielle urgente"),
        "should not escalate to urgente when 180j is stable, got:\n{}",
        output
    );
}

/// Dérive de +6°C sur 30j seulement (90j et 180j stables) → nettoyage conseillé.
#[test]
fn report_maintenance_cleaning_on_30j_drift_only() {
    let store = make_store();
    // Courant 30j : 56°C — précédent 30j : 50°C → Δ+6°C
    for day in 1..31 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 56.0)
            .unwrap();
    }
    // Jours 31-361 : stable à 50°C
    for day in 31..361 {
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
        output.contains("Nettoyage conseillé"),
        "Δ+6°C on 30j only should trigger nettoyage conseillé, got:\n{}",
        output
    );
}

/// Toutes les températures stables → aucune action requise.
#[test]
fn report_maintenance_no_action_when_all_stable() {
    let store = make_store();
    for day in 1..361 {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, "idle")
            .unwrap();
        store
            .insert_reading(snap, "CPU", "CPU Package", 45.0)
            .unwrap();
    }

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).expect("report");
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("Aucune action requise"),
        "stable data should show 'Aucune action requise', got:\n{}",
        output
    );
}
