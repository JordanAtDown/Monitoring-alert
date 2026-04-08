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
