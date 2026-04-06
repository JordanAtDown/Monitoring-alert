use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Write;

use crate::store::TemperatureStore;

// ──────────────────────────────────────────────────────────────
// Compact summary for toast notifications
// ──────────────────────────────────────────────────────────────

#[cfg_attr(not(windows), allow(dead_code))]
pub enum ReportPeriod {
    Daily,
    Weekly,
    Monthly,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub struct SummaryReport {
    pub title: String,
    pub body: String,
}

/// Generate a one-line summary suitable for a Windows toast notification.
///
/// Checks the 30-day window across all sensors (idle + heavy load categories).
/// Returns an alert body if any sensor delta ≥ 5 °C, otherwise "✓ stable".
#[cfg_attr(not(windows), allow(dead_code))]
pub fn generate_summary(
    store: &dyn TemperatureStore,
    period: ReportPeriod,
) -> Result<SummaryReport> {
    let sensors = store
        .get_distinct_sensors()
        .context("Failed to get distinct sensors")?;

    // Worst delta per sensor name (deduplicates idle/heavy for the same sensor)
    let mut worst: HashMap<String, f64> = HashMap::new();

    for s in &sensors {
        for &cat in &["idle", "heavy"] {
            let curr = store
                .get_avg_for_window(&s.hardware, &s.sensor, cat, 30, 0)
                .context("Failed to query current window avg")?;
            let prev = store
                .get_avg_for_window(&s.hardware, &s.sensor, cat, 30, 30)
                .context("Failed to query previous window avg")?;
            if let (Some(c), Some(p)) = (curr, prev) {
                let delta = c - p;
                if delta >= 5.0 {
                    let entry = worst.entry(s.sensor.clone()).or_insert(delta);
                    if delta > *entry {
                        *entry = delta;
                    }
                }
            }
        }
    }

    let title = match period {
        ReportPeriod::Daily => "MonitoringAlert — Rapport Journalier",
        ReportPeriod::Weekly => "MonitoringAlert — Rapport Hebdomadaire",
        ReportPeriod::Monthly => "MonitoringAlert — Rapport Mensuel",
    }
    .to_string();

    let body = if worst.is_empty() {
        "✓ Toutes les températures stables".to_string()
    } else {
        let mut alerts: Vec<(String, f64)> = worst.into_iter().collect();
        // Sort worst delta first
        alerts.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let n = alerts.len();
        let (ref name, delta) = alerts[0];
        let sign = if delta >= 0.0 { "+" } else { "" };
        if n == 1 {
            format!("⚠ 1 alerte — {}: {}{:.1}°C sur 30j", name, sign, delta)
        } else {
            format!("⚠ {} alertes — {}: {}{:.1}°C sur 30j", n, name, sign, delta)
        }
    };

    Ok(SummaryReport { title, body })
}

const WINDOWS: &[(u32, &str)] = &[(1, "24h"), (7, "7j"), (30, "30j"), (90, "90j")];
const DISPLAY_CATS: &[&str] = &["idle", "heavy"];

fn delta_status(delta: f64) -> &'static str {
    if delta >= 10.0 {
        "🔴 CRITIQUE ← nettoyer !"
    } else if delta >= 5.0 {
        "⚠ ATTENTION"
    } else if delta >= 2.0 {
        "↑ légère hausse"
    } else if delta <= -2.0 {
        "↓ amélioration"
    } else {
        "✓ stable"
    }
}

fn cat_label(cat: &str) -> &'static str {
    match cat {
        "idle" => "Idle (0–14 %)",
        "light" => "Léger (15–39 %)",
        "moderate" => "Modéré (40–74 %)",
        "heavy" => "Chargé (75–100 %)",
        _ => "Inconnu",
    }
}

fn bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    "█".repeat(filled)
}

pub fn generate_report(store: &dyn TemperatureStore, output: Option<&str>) -> Result<()> {
    let stats = store
        .get_overall_stats()
        .context("Failed to get overall stats")?;
    let sensors = store
        .get_distinct_sensors()
        .context("Failed to get distinct sensors")?;

    let now = chrono::Local::now();
    let now_str = now.format("%d/%m/%Y %H:%M").to_string();

    let mut out: Vec<u8> = Vec::new();

    // Header
    writeln!(
        out,
        "════════════════════════════════════════════════════════════════"
    )?;
    writeln!(out, "  RAPPORT DE TEMPÉRATURE  —  {}", now_str)?;
    writeln!(
        out,
        "════════════════════════════════════════════════════════════════"
    )?;

    let first_date = stats.first_ts.as_deref().map(|s| &s[..10]).unwrap_or("N/A");
    let last_ts = stats.last_ts.as_deref().unwrap_or("N/A");
    writeln!(
        out,
        "  Données depuis    : {}   ({} snapshots)",
        first_date, stats.total_snapshots
    )?;
    writeln!(out, "  Dernière mesure   : {}", last_ts)?;
    writeln!(out)?;

    // Category distribution (last 90 days)
    let dist = store
        .get_category_distribution(90)
        .context("Failed to get distribution")?;
    let total_dist: i64 = dist.iter().map(|c| c.count).sum();
    writeln!(out, "  Distribution des états (90 derniers jours) :")?;
    let order = ["idle", "light", "moderate", "heavy"];
    let dist_map: HashMap<String, i64> = dist.into_iter().map(|c| (c.load_cat, c.count)).collect();
    for cat in order {
        let cnt = dist_map.get(cat).copied().unwrap_or(0);
        let pct = if total_dist > 0 {
            cnt as f64 / total_dist as f64 * 100.0
        } else {
            0.0
        };
        let label = match cat {
            "idle" => "Idle      (0–14 % CPU)  ",
            "light" => "Léger     (15–39 % CPU) ",
            "moderate" => "Modéré    (40–74 % CPU) ",
            "heavy" => "Chargé    (75–100 % CPU)",
            _ => continue,
        };
        writeln!(out, "    {}  {:3.0}%  {}", label, pct, bar(pct, 28))?;
    }
    writeln!(out)?;

    // Per-sensor analysis
    writeln!(
        out,
        "────────────────────────────────────────────────────────────────"
    )?;
    writeln!(out, "  ANALYSE PAR CAPTEUR — comparaison à charge égale")?;
    writeln!(
        out,
        "────────────────────────────────────────────────────────────────"
    )?;
    writeln!(out)?;

    // Group sensors by hardware
    let mut hw_map: HashMap<String, Vec<String>> = HashMap::new();
    for s in &sensors {
        hw_map
            .entry(s.hardware.clone())
            .or_default()
            .push(s.sensor.clone());
    }
    let mut hw_sorted: Vec<String> = hw_map.keys().cloned().collect();
    hw_sorted.sort();

    let mut alerts: Vec<String> = Vec::new();

    for hardware in &hw_sorted {
        writeln!(out, "  ┌─ {} ─", hardware)?;
        writeln!(out, "  │")?;
        let sensor_list = hw_map.get(hardware).cloned().unwrap_or_default();
        for sensor_name in &sensor_list {
            writeln!(out, "  │  {}", sensor_name)?;
            for &cat in DISPLAY_CATS {
                let has_data = WINDOWS.iter().any(|&(days, _)| {
                    store
                        .get_avg_for_window(hardware, sensor_name, cat, days, 0)
                        .ok()
                        .flatten()
                        .is_some()
                });
                if !has_data {
                    continue;
                }
                writeln!(out, "  │  ├─ {} ", cat_label(cat))?;
                for &(days, label) in WINDOWS {
                    let curr = store
                        .get_avg_for_window(hardware, sensor_name, cat, days, 0)
                        .context("Failed to query current window avg")?;
                    let prev = store
                        .get_avg_for_window(hardware, sensor_name, cat, days, days)
                        .context("Failed to query previous window avg")?;
                    match (curr, prev) {
                        (Some(c), Some(p)) => {
                            let delta = c - p;
                            let sign = if delta >= 0.0 { "+" } else { "" };
                            let status = delta_status(delta);
                            writeln!(
                                out,
                                "  │  │  moy. {:4}   {:5.1} °C  vs préc.  {:5.1} °C  ({}{:.1}°C)  {}",
                                label, c, p, sign, delta, status
                            )?;
                            if days == 30 && delta >= 5.0 {
                                alerts.push(format!(
                                    "  {} {}/{} [{}] → {}{:.1}°C sur {}",
                                    if delta >= 10.0 {
                                        "🔴 CRITIQUE"
                                    } else {
                                        "⚠ ATTENTION"
                                    },
                                    hardware,
                                    sensor_name,
                                    cat,
                                    sign,
                                    delta,
                                    label
                                ));
                            }
                        }
                        (Some(c), None) => {
                            writeln!(
                                out,
                                "  │  │  moy. {:4}   {:5.1} °C  (pas de période précédente)",
                                label, c
                            )?;
                        }
                        _ => {}
                    }
                }
                writeln!(out, "  │  │")?;
            }
        }
        writeln!(out, "  └─")?;
        writeln!(out)?;
    }

    // Summary
    writeln!(
        out,
        "════════════════════════════════════════════════════════════════"
    )?;
    writeln!(out, "  RÉSUMÉ DES ALERTES")?;
    writeln!(
        out,
        "════════════════════════════════════════════════════════════════"
    )?;
    if alerts.is_empty() {
        writeln!(out, "  ✓ Aucune alerte — températures stables.")?;
    } else {
        for alert in &alerts {
            writeln!(out, "{}", alert)?;
        }
        writeln!(out)?;
        writeln!(out, "  💡 Causes possibles :")?;
        writeln!(out, "     → Poussière dans les filtres / radiateurs")?;
        writeln!(out, "     → Pâte thermique à renouveler (> 2–3 ans)")?;
        writeln!(out, "     → Ventilateur défaillant ou encrassé")?;
    }
    writeln!(
        out,
        "════════════════════════════════════════════════════════════════"
    )?;

    let text = String::from_utf8(out).context("Report contains invalid UTF-8")?;

    match output {
        Some(path) => {
            std::fs::write(path, &text)
                .with_context(|| format!("Failed to write report to: {}", path))?;
            println!("Report written to: {}", path);
        }
        None => {
            print!("{}", text);
        }
    }
    Ok(())
}
