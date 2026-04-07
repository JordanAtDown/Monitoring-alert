/// Tests d'intégration de la pipeline generate_summary :
///   insert_snapshot → insert_reading → get_avg_for_window → generate_summary
///
/// Chaque test simule une vraie collection en insérant des lignes horodatées
/// dynamiquement (N jours en arrière depuis maintenant).
use crate::{
    report,
    store::TemperatureStore,
    tests::common::{make_store, ts_days_ago},
};

/// Insère un snapshot + une lecture par jour sur l'intervalle [days_start, days_end).
fn seed(
    store: &dyn TemperatureStore,
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    days_start: i64,
    days_end: i64,
    temp: f64,
) {
    for day in days_start..days_end {
        let snap = store
            .insert_snapshot(&ts_days_ago(day), Some(5.0), None, load_cat)
            .unwrap();
        store.insert_reading(snap, hardware, sensor, temp).unwrap();
    }
}

// ── Titres des trois périodes ─────────────────────────────────

#[test]
fn daily_title() {
    let store = make_store();
    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.title.contains("Journalier"), "{:?}", s.title);
}

#[test]
fn weekly_title() {
    let store = make_store();
    let s = report::generate_summary(&store, report::ReportPeriod::Weekly).unwrap();
    assert!(s.title.contains("Hebdomadaire"), "{:?}", s.title);
}

#[test]
fn monthly_title() {
    let store = make_store();
    let s = report::generate_summary(&store, report::ReportPeriod::Monthly).unwrap();
    assert!(s.title.contains("Mensuel"), "{:?}", s.title);
}

// ── Scénario : stable (Δ < 5°C) ──────────────────────────────

#[test]
fn stable_no_alert() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 45.0);
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 44.0); // Δ+1°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("stables"), "{:?}", s.body);
}

// ── Scénario : seuil exact ────────────────────────────────────

/// Δ = +5.0°C exactement → doit déclencher une alerte.
#[test]
fn delta_exactly_at_warning_threshold_triggers_alert() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 54.0);
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0); // Δ+5.0°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(
        s.body.contains("1 alerte"),
        "Δ+5.0°C should trigger an alert, got: {:?}",
        s.body
    );
}

/// Δ = +4.9°C → sous le seuil → stable.
#[test]
fn delta_just_below_threshold_is_stable() {
    let store = make_store();
    // avg courante = 53.9, avg précédente = 49.0 → Δ = +4.9
    seed(&store, "CPU", "CPU Package", "idle", 1, 10, 54.0); // 9 jours à 54
    seed(&store, "CPU", "CPU Package", "idle", 10, 31, 53.857); // 21 jours à ~53.857 → avg ≈ 53.9
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0);

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    // Tolérance : on vérifie juste qu'il n'y a pas "1 alerte"
    assert!(
        !s.body.contains("1 alerte"),
        "Δ < 5.0°C should not trigger alert, got: {:?}",
        s.body
    );
}

// ── Scénario : seuil critique (Δ ≥ 10°C) ────────────────────

#[test]
fn critical_threshold() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 65.0);
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 53.0); // Δ+12°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("1 alerte"), "{:?}", s.body);
    assert!(s.body.contains("12.0"), "{:?}", s.body);
}

// ── Scénario : amélioration (delta négatif) ───────────────────

/// Après un nettoyage, les températures baissent : Δ = -8°C.
/// Ce n'est pas une alerte — les températures s'améliorent.
#[test]
fn negative_delta_improvement_no_alert() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 45.0); // courant : 45°C
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 53.0); // précédent : 53°C → Δ-8°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(
        s.body.contains("stables"),
        "improvement (Δ-8°C) should not trigger alert, got: {:?}",
        s.body
    );
}

// ── Scénarios multi-capteurs ──────────────────────────────────

#[test]
fn multiple_sensors_one_alert() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 55.0);
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0); // Δ+6°C → alerte
    seed(&store, "GPU", "GPU Temperature", "idle", 1, 31, 71.0);
    seed(&store, "GPU", "GPU Temperature", "idle", 31, 61, 70.0); // Δ+1°C → stable

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("1 alerte"), "{:?}", s.body);
}

#[test]
fn multiple_sensors_two_alerts() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 55.0);
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 49.0); // Δ+6°C
    seed(&store, "GPU", "GPU Temperature", "idle", 1, 31, 77.0);
    seed(&store, "GPU", "GPU Temperature", "idle", 31, 61, 70.0); // Δ+7°C (pire)

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("2 alertes"), "{:?}", s.body);
    assert!(
        s.body.contains("GPU Temperature"),
        "GPU Temperature should lead (worst delta), got: {:?}",
        s.body
    );
}

/// Même nom de capteur sur deux matériels différents.
/// Le résumé toast déduplique par nom de capteur (seul le pire delta est affiché)
/// pour garder la notification concise.
#[test]
fn same_sensor_name_different_hardware_deduped_in_summary() {
    let store = make_store();
    // CPU "Temperature" Δ+6°C
    seed(&store, "CPU", "Temperature", "idle", 1, 31, 55.0);
    seed(&store, "CPU", "Temperature", "idle", 31, 61, 49.0);
    // GPU "Temperature" Δ+6°C (même nom de capteur)
    seed(&store, "GPU", "Temperature", "idle", 1, 31, 75.0);
    seed(&store, "GPU", "Temperature", "idle", 31, 61, 69.0);

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    // Un seul entrée dans worst{} car même sensor name → "1 alerte"
    assert!(
        s.body.contains("1 alerte"),
        "same sensor name on different hardware → deduped to 1 alert, got: {:?}",
        s.body
    );
}

// ── Isolation par catégorie de charge ────────────────────────

#[test]
fn load_cat_isolation() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 43.0);
    seed(&store, "CPU", "CPU Package", "idle", 31, 61, 40.0); // Δ+3°C → stable
    seed(&store, "CPU", "CPU Package", "heavy", 1, 31, 78.0);
    seed(&store, "CPU", "CPU Package", "heavy", 31, 61, 70.0); // Δ+8°C → alerte

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(
        s.body.contains("1 alerte"),
        "only heavy category should alert, got: {:?}",
        s.body
    );
}

// ── Pas de données précédentes ────────────────────────────────

#[test]
fn no_previous_data_no_alert() {
    let store = make_store();
    // Seulement la fenêtre courante — aucune référence
    seed(&store, "CPU", "CPU Package", "idle", 1, 31, 65.0);

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(
        s.body.contains("stables"),
        "no prev data → should be stable, got: {:?}",
        s.body
    );
}

// ── Scénarios GPU (effective_load = max(cpu, gpu)) ────────────

/// Sessions gaming récentes plus chaudes qu'avant → détection correcte du drift GPU.
#[test]
fn gpu_heavy_drift_detected() {
    let store = make_store();
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        1,
        31,
        83.0,
    );
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        31,
        61,
        71.0,
    ); // Δ+12°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("1 alerte"), "{:?}", s.body);
    assert!(s.body.contains("12.0"), "{:?}", s.body);
}

/// Le drift heavy n'est pas masqué par des sessions light stables coexistantes.
#[test]
fn gpu_heavy_drift_not_masked_by_stable_light_sessions() {
    let store = make_store();
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        1,
        31,
        83.0,
    );
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        31,
        61,
        71.0,
    ); // Δ+12°C → alerte
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "light",
        1,
        31,
        45.0,
    );
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "light",
        31,
        61,
        44.0,
    ); // Δ+1°C → stable

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(
        s.body.contains("1 alerte"),
        "heavy drift should still be detected: {:?}",
        s.body
    );
}

/// CPU et GPU tous les deux en drift (machine de rendu 3D ou workstation).
#[test]
fn cpu_and_gpu_both_drift_two_alerts() {
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "heavy", 1, 31, 92.0);
    seed(&store, "CPU", "CPU Package", "heavy", 31, 61, 80.0); // Δ+12°C
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        1,
        31,
        88.0,
    );
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        31,
        61,
        74.0,
    ); // Δ+14°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("2 alertes"), "{:?}", s.body);
}

// ── Scénarios saisonniers (fenêtre 180j) ─────────────────────

/// Été plus chaud que l'hiver précédent : Δ+11°C sur 180j → alerte saisonnière.
/// Utilise generate_summary (fenêtre 30j) : le drift 180j n'y apparaît pas,
/// mais le rapport complet doit afficher "Inspection matérielle urgente".
#[test]
fn seasonal_180j_drift_shows_in_full_report() {
    use crate::report;
    let store = make_store();
    // Courant 180j : 75°C — référence 180j : 64°C → Δ+11°C
    seed(&store, "CPU", "CPU Package", "idle", 1, 181, 75.0);
    seed(&store, "CPU", "CPU Package", "idle", 181, 361, 64.0);

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("Inspection matérielle urgente"),
        "Δ+11°C on 180j should show inspection urgente in full report, got:\n{}",
        output
    );
}

/// Températures saisonnières stables (Δ < 2°C sur 180j) → aucune action.
#[test]
fn seasonal_180j_stable_no_maintenance() {
    use crate::report;
    let store = make_store();
    seed(&store, "CPU", "CPU Package", "idle", 1, 181, 46.0);
    seed(&store, "CPU", "CPU Package", "idle", 181, 361, 45.0); // Δ+1°C

    let mut buf = Vec::new();
    report::generate_report_to_writer(&store, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();

    assert!(
        output.contains("Aucune action requise"),
        "Δ+1°C on 180j should show no maintenance needed, got:\n{}",
        output
    );
}

/// Sessions gaming stables : GPU chaud mais pas de drift → pas d'alerte.
#[test]
fn gpu_heavy_stable_no_alert() {
    let store = make_store();
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        1,
        31,
        78.0,
    );
    seed(
        &store,
        "GPU",
        "GPU Junction Temperature",
        "heavy",
        31,
        61,
        77.0,
    ); // Δ+1°C

    let s = report::generate_summary(&store, report::ReportPeriod::Daily).unwrap();
    assert!(s.body.contains("stables"), "{:?}", s.body);
}
