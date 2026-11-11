use crate::domain::load::{
    effective_load, heats_up, load_category, LoadTracker, COOLDOWN, COOLDOWN_SECS,
};
use chrono::{Duration, NaiveDate, NaiveDateTime};

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

// ── LoadTracker — le repos réel ───────────────────────────────

fn at(minutes: i64) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 11)
        .unwrap()
        .and_hms_opt(20, 0, 0)
        .unwrap()
        + Duration::minutes(minutes)
}

#[test]
fn only_moderate_and_heavy_loads_leave_the_machine_hot() {
    assert!(!heats_up("idle"));
    assert!(!heats_up("light"));
    assert!(heats_up("moderate"));
    assert!(heats_up("heavy"));
}

#[test]
fn a_machine_that_never_worked_is_resting() {
    let mut t = LoadTracker::new();
    assert_eq!(t.classify(at(0), Some(3.0)), "idle");
    assert_eq!(t.classify(at(5), None), "idle");
}

#[test]
fn right_after_an_effort_the_machine_is_cooling_down_not_resting() {
    let mut t = LoadTracker::new();
    assert_eq!(t.classify(at(0), Some(90.0)), "heavy");
    // The load fell back to 3 %, but the heat sink is still evacuating the effort.
    assert_eq!(t.classify(at(5), Some(3.0)), COOLDOWN);
    assert_eq!(t.classify(at(10), Some(2.0)), COOLDOWN);
}

#[test]
fn the_machine_is_resting_again_once_the_cooldown_is_over() {
    let mut t = LoadTracker::new();
    t.classify(at(0), Some(90.0));
    // The limit itself is still cooling down, one second past it is not.
    let limit = COOLDOWN_SECS / 60;
    assert_eq!(t.classify(at(limit), Some(3.0)), COOLDOWN);
    assert_eq!(
        t.classify(at(limit) + Duration::seconds(1), Some(3.0)),
        "idle"
    );
}

#[test]
fn a_new_effort_starts_the_cooldown_over() {
    let mut t = LoadTracker::new();
    t.classify(at(0), Some(90.0));
    assert_eq!(t.classify(at(14), Some(60.0)), "moderate");
    // 14 + 10 minutes after the first effort, but only 10 after the last.
    assert_eq!(t.classify(at(24), Some(3.0)), COOLDOWN);
    assert_eq!(t.classify(at(40), Some(3.0)), "idle");
}

#[test]
fn a_light_load_neither_starts_nor_is_hidden_by_a_cooldown() {
    let mut t = LoadTracker::new();
    // Light work does not leave the machine hot: idle right after is idle.
    assert_eq!(t.classify(at(0), Some(25.0)), "light");
    assert_eq!(t.classify(at(5), Some(3.0)), "idle");
    // And a light reading during a cooldown stays what its load says.
    t.classify(at(10), Some(90.0));
    assert_eq!(t.classify(at(15), Some(25.0)), "light");
}

#[test]
fn a_long_gap_such_as_a_night_in_sleep_forgets_the_effort() {
    let mut t = LoadTracker::new();
    t.classify(at(0), Some(90.0));
    // The PC went to sleep; the next reading is hours later.
    assert_eq!(t.classify(at(600), Some(3.0)), "idle");
}
