use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

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

// ──────────────────────────────────────────────────────────────
// ReportData — shared data model
// ──────────────────────────────────────────────────────────────

const WINDOWS: &[(u32, &str)] = &[
    (1, "24h"),
    (7, "7j"),
    (30, "30j"),
    (90, "90j"),
    (180, "180j"),
];
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

#[derive(Serialize)]
pub struct DistRow {
    pub label: String,
    pub count: i64,
    pub pct: u32,
}

#[derive(Serialize)]
pub struct WindowRow {
    pub window: String,
    pub current: String,
    pub previous: String,
    pub delta: String,
    pub status: String,
    /// Raw delta value, used by the ASCII renderer for alignment.
    #[serde(skip)]
    pub delta_f: f64,
    #[serde(skip)]
    pub current_f: f64,
    #[serde(skip)]
    pub previous_f: f64,
    /// True when there is no previous period yet.
    pub no_previous: bool,
}

#[derive(Serialize)]
pub struct CategoryTable {
    pub category: String,
    pub rows: Vec<WindowRow>,
}

#[derive(Serialize)]
pub struct SensorSection {
    pub name: String,
    pub tables: Vec<CategoryTable>,
}

#[derive(Serialize)]
pub struct HardwareSection {
    pub hardware: String,
    pub sensors: Vec<SensorSection>,
}

#[derive(Serialize)]
pub struct MaintenanceBlock {
    /// "ok" | "cleaning" | "preventive" | "urgent"
    pub level: String,
    pub peak_delta: String,
    pub peak_sensor: String,
}

#[derive(Serialize)]
pub struct ReportData {
    pub generated_at: String,
    pub first_date: String,
    pub last_ts: String,
    pub total_snapshots: i64,
    /// Days collected so far; -1 when the DB is empty.
    pub days_collected: i64,
    /// Date from which the 90-day comparison becomes available.
    pub ready_90j_date: String,
    /// Date from which the 180-day (seasonal) comparison becomes available.
    pub ready_180j_date: String,
    pub distribution: Vec<DistRow>,
    pub sections: Vec<HardwareSection>,
    /// Alert lines for the summary section.
    pub alerts: Vec<String>,
    pub maintenance: MaintenanceBlock,
}

/// Collects all data from the store into a [`ReportData`] value.
/// Both the ASCII and Markdown renderers consume this instead of
/// querying the DB independently, eliminating duplication.
pub fn build_report_data(store: &dyn TemperatureStore) -> Result<ReportData> {
    let stats = store
        .get_overall_stats()
        .context("Failed to get overall stats")?;
    let sensors = store
        .get_distinct_sensors()
        .context("Failed to get distinct sensors")?;

    let now = chrono::Local::now();
    let generated_at = now.format("%d/%m/%Y %H:%M").to_string();

    let first_date = stats
        .first_ts
        .as_deref()
        .map(|s| s[..10].to_string())
        .unwrap_or_else(|| "N/A".to_string());
    let last_ts = stats.last_ts.as_deref().unwrap_or("N/A").to_string();

    let (days_collected, ready_90j_date, ready_180j_date) = if let Some(ref ts) = stats.first_ts {
        if let Ok(first) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S") {
            let days = (now.naive_local() - first).num_days();
            let r90 = (first + chrono::TimeDelta::days(180))
                .format("%d/%m/%Y")
                .to_string();
            let r180 = (first + chrono::TimeDelta::days(360))
                .format("%d/%m/%Y")
                .to_string();
            (days, r90, r180)
        } else {
            (-1, String::new(), String::new())
        }
    } else {
        (-1, String::new(), String::new())
    };

    // ── Distribution ─────────────────────────────────────────
    let dist_raw = store
        .get_category_distribution(90)
        .context("Failed to get distribution")?;
    let total_dist: i64 = dist_raw.iter().map(|c| c.count).sum();
    let distribution = ["idle", "light", "moderate", "heavy"]
        .iter()
        .map(|&cat| {
            let count = dist_raw
                .iter()
                .find(|c| c.load_cat == cat)
                .map(|c| c.count)
                .unwrap_or(0);
            let pct = if total_dist > 0 {
                (count as f64 / total_dist as f64 * 100.0).round() as u32
            } else {
                0
            };
            DistRow {
                label: cat_label(cat).to_string(),
                count,
                pct,
            }
        })
        .collect();

    // ── Per-sensor sections ───────────────────────────────────
    let mut hw_map: HashMap<String, Vec<String>> = HashMap::new();
    for s in sensors {
        hw_map.entry(s.hardware).or_default().push(s.sensor);
    }
    let mut hw_sorted: Vec<String> = hw_map.keys().cloned().collect();
    hw_sorted.sort();

    let mut all_alerts: Vec<String> = Vec::new();
    let mut peak_30: (f64, String) = (f64::NEG_INFINITY, String::new());
    let mut peak_90: (f64, String) = (f64::NEG_INFINITY, String::new());
    let mut peak_180: (f64, String) = (f64::NEG_INFINITY, String::new());

    let mut sections: Vec<HardwareSection> = Vec::new();

    for hardware in &hw_sorted {
        let sensor_names = hw_map
            .get(hardware.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let mut sensor_sections: Vec<SensorSection> = Vec::new();

        for sensor_name in sensor_names {
            let mut tables: Vec<CategoryTable> = Vec::new();

            for &cat in DISPLAY_CATS {
                let mut rows: Vec<WindowRow> = Vec::new();

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
                            let status = delta_status(delta).to_string();

                            // Update maintenance peaks
                            let pk = match days {
                                30 => Some(&mut peak_30),
                                90 => Some(&mut peak_90),
                                180 => Some(&mut peak_180),
                                _ => None,
                            };
                            if let Some(pk) = pk {
                                if delta > pk.0 {
                                    pk.0 = delta;
                                    pk.1 = format!("{}/{}", hardware, sensor_name);
                                }
                            }

                            if days == 30 && delta >= 5.0 {
                                all_alerts.push(format!(
                                    "{} {}/{} [{}] → {}{:.1}°C sur {}",
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

                            rows.push(WindowRow {
                                window: label.to_string(),
                                current: format!("{:.1}", c),
                                previous: format!("{:.1}", p),
                                delta: format!("{}{:.1}", sign, delta),
                                status,
                                delta_f: delta,
                                current_f: c,
                                previous_f: p,
                                no_previous: false,
                            });
                        }
                        (Some(c), None) => {
                            rows.push(WindowRow {
                                window: label.to_string(),
                                current: format!("{:.1}", c),
                                previous: "—".to_string(),
                                delta: "—".to_string(),
                                status: "(pas de période précédente)".to_string(),
                                delta_f: 0.0,
                                current_f: c,
                                previous_f: 0.0,
                                no_previous: true,
                            });
                        }
                        _ => {}
                    }
                }

                if !rows.is_empty() {
                    tables.push(CategoryTable {
                        category: cat_label(cat).to_string(),
                        rows,
                    });
                }
            }

            if !tables.is_empty() {
                sensor_sections.push(SensorSection {
                    name: sensor_name.clone(),
                    tables,
                });
            }
        }

        if !sensor_sections.is_empty() {
            sections.push(HardwareSection {
                hardware: hardware.clone(),
                sensors: sensor_sections,
            });
        }
    }

    // ── Maintenance block ─────────────────────────────────────
    let maintenance = if peak_180.0 >= 10.0 {
        MaintenanceBlock {
            level: "urgent".to_string(),
            peak_delta: format!("+{:.1}°C sur 180j", peak_180.0),
            peak_sensor: peak_180.1,
        }
    } else if peak_90.0 >= 8.0 {
        MaintenanceBlock {
            level: "preventive".to_string(),
            peak_delta: format!("+{:.1}°C sur 90j", peak_90.0),
            peak_sensor: peak_90.1,
        }
    } else if peak_30.0 >= 5.0 {
        MaintenanceBlock {
            level: "cleaning".to_string(),
            peak_delta: format!("+{:.1}°C sur 30j", peak_30.0),
            peak_sensor: peak_30.1,
        }
    } else {
        MaintenanceBlock {
            level: "ok".to_string(),
            peak_delta: String::new(),
            peak_sensor: String::new(),
        }
    };

    Ok(ReportData {
        generated_at,
        first_date,
        last_ts,
        total_snapshots: stats.total_snapshots,
        days_collected,
        ready_90j_date,
        ready_180j_date,
        distribution,
        sections,
        alerts: all_alerts,
        maintenance,
    })
}

// ──────────────────────────────────────────────────────────────
// ASCII report renderer
// ──────────────────────────────────────────────────────────────

fn bar(pct: u32, width: usize) -> String {
    let filled = ((pct as f64 / 100.0) * width as f64).round() as usize;
    "█".repeat(filled.min(width))
}

/// Writes the full temperature report in ASCII format to `writer`.
pub fn generate_report_to_writer(
    store: &dyn TemperatureStore,
    writer: &mut impl Write,
) -> Result<()> {
    let data = build_report_data(store)?;
    render_text(&data, writer)
}

fn render_text(data: &ReportData, writer: &mut impl Write) -> Result<()> {
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;
    writeln!(writer, "  RAPPORT DE TEMPÉRATURE  —  {}", data.generated_at)?;
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;
    writeln!(
        writer,
        "  Données depuis    : {}   ({} snapshots)",
        data.first_date, data.total_snapshots
    )?;
    writeln!(writer, "  Dernière mesure   : {}", data.last_ts)?;
    writeln!(writer)?;

    if data.days_collected < 0 {
        writeln!(
            writer,
            "  ⚠  Base de données vide — aucune donnée collectée."
        )?;
        writeln!(writer)?;
    } else if data.days_collected < 360 {
        writeln!(
            writer,
            "  ⚠  Données insuffisantes : {} jour(s) enregistré(s) sur 360 requis.",
            data.days_collected
        )?;
        if data.days_collected < 180 {
            writeln!(
                writer,
                "     Comparaison 90j disponible le {}.",
                data.ready_90j_date
            )?;
        }
        writeln!(
            writer,
            "     Comparaison 180j (saisonnière) complète le {}.",
            data.ready_180j_date
        )?;
        writeln!(writer)?;
    }

    // Distribution
    writeln!(writer, "  Distribution des états (90 derniers jours) :")?;
    for row in &data.distribution {
        writeln!(
            writer,
            "    {:<28}  {:3}%  {}",
            row.label,
            row.pct,
            bar(row.pct, 28)
        )?;
    }
    writeln!(writer)?;

    // Per-sensor
    writeln!(
        writer,
        "────────────────────────────────────────────────────────────────"
    )?;
    writeln!(writer, "  ANALYSE PAR CAPTEUR — comparaison à charge égale")?;
    writeln!(
        writer,
        "────────────────────────────────────────────────────────────────"
    )?;
    writeln!(writer)?;

    for hw in &data.sections {
        writeln!(writer, "  ┌─ {} ─", hw.hardware)?;
        writeln!(writer, "  │")?;
        for sensor in &hw.sensors {
            writeln!(writer, "  │  {}", sensor.name)?;
            for table in &sensor.tables {
                writeln!(writer, "  │  ├─ {} ", table.category)?;
                for row in &table.rows {
                    if row.no_previous {
                        writeln!(
                            writer,
                            "  │  │  moy. {:4}   {:5.1} °C  (pas de période précédente)",
                            row.window, row.current_f
                        )?;
                    } else {
                        let sign = if row.delta_f >= 0.0 { "+" } else { "" };
                        writeln!(
                            writer,
                            "  │  │  moy. {:4}   {:5.1} °C  vs préc.  {:5.1} °C  ({}{:.1}°C)  {}",
                            row.window,
                            row.current_f,
                            row.previous_f,
                            sign,
                            row.delta_f,
                            row.status
                        )?;
                    }
                }
                writeln!(writer, "  │  │")?;
            }
        }
        writeln!(writer, "  └─")?;
        writeln!(writer)?;
    }

    // Alerts summary
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;
    writeln!(writer, "  RÉSUMÉ DES ALERTES")?;
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;
    if data.alerts.is_empty() {
        writeln!(writer, "  ✓ Aucune alerte — températures stables.")?;
    } else {
        for alert in &data.alerts {
            writeln!(writer, "  {}", alert)?;
        }
    }

    // Maintenance
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;
    writeln!(writer, "  RECOMMANDATION MAINTENANCE")?;
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;
    match data.maintenance.level.as_str() {
        "urgent" => {
            writeln!(writer, "  🔴 Inspection matérielle urgente")?;
            writeln!(
                writer,
                "     Dérive de {} détectée — {}",
                data.maintenance.peak_delta, data.maintenance.peak_sensor
            )?;
            writeln!(
                writer,
                "     → Remplacer la pâte thermique (vieillissement)"
            )?;
            writeln!(
                writer,
                "     → Inspecter les ventilateurs (roulements, encrassement)"
            )?;
            writeln!(writer, "     → Nettoyer radiateurs et filtres à poussière")?;
            writeln!(
                writer,
                "     → Envisager le remplacement si le matériel a > 5 ans"
            )?;
        }
        "preventive" => {
            writeln!(writer, "  ⚠  Maintenance préventive recommandée")?;
            writeln!(
                writer,
                "     Dérive de {} détectée — {}",
                data.maintenance.peak_delta, data.maintenance.peak_sensor
            )?;
            writeln!(writer, "     → Nettoyer les filtres et radiateurs")?;
            writeln!(writer, "     → Vérifier l'état des ventilateurs")?;
            writeln!(
                writer,
                "     → Envisager le renouvellement de la pâte thermique"
            )?;
        }
        "cleaning" => {
            writeln!(writer, "  ⚠  Nettoyage conseillé")?;
            writeln!(
                writer,
                "     Dérive de {} détectée — {}",
                data.maintenance.peak_delta, data.maintenance.peak_sensor
            )?;
            writeln!(writer, "     → Nettoyer les filtres à poussière")?;
            writeln!(
                writer,
                "     → Vérifier que les ventilateurs tournent librement"
            )?;
        }
        _ => {
            writeln!(
                writer,
                "  ✓  Aucune action requise — comportement thermique stable."
            )?;
        }
    }
    writeln!(
        writer,
        "════════════════════════════════════════════════════════════════"
    )?;

    Ok(())
}

// ──────────────────────────────────────────────────────────────
// Markdown report renderer (minijinja)
// ──────────────────────────────────────────────────────────────

const REPORT_MD_TEMPLATE: &str = include_str!("report_template.md.j2");

/// Writes the full temperature report in Markdown format to `writer`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn generate_report_md_to_writer(
    store: &dyn TemperatureStore,
    writer: &mut impl Write,
) -> Result<()> {
    let data = build_report_data(store)?;
    render_md(&data, writer)
}

fn render_md(data: &ReportData, writer: &mut impl Write) -> Result<()> {
    let mut env = minijinja::Environment::new();
    env.add_template("report.md", REPORT_MD_TEMPLATE)
        .context("Failed to load report template")?;
    let tmpl = env
        .get_template("report.md")
        .context("Failed to get report template")?;
    let rendered = tmpl
        .render(minijinja::context!(data => data))
        .context("Failed to render report template")?;
    write!(writer, "{}", rendered)?;
    Ok(())
}

/// Saves a Markdown report to `reports_dir/rapport-YYYY-MM-DD-{period_label}.md`.
/// Creates the directory if needed. Returns the path of the created file.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn save_report_md(
    store: &dyn TemperatureStore,
    reports_dir: &Path,
    period_label: &str,
) -> Result<PathBuf> {
    std::fs::create_dir_all(reports_dir)
        .with_context(|| format!("Failed to create reports dir: {}", reports_dir.display()))?;

    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let filename = format!("rapport-{}-{}.md", date, period_label);
    let path = reports_dir.join(&filename);

    let mut buf: Vec<u8> = Vec::new();
    generate_report_md_to_writer(store, &mut buf)?;
    std::fs::write(&path, &buf)
        .with_context(|| format!("Failed to write report: {}", path.display()))?;

    Ok(path)
}

pub fn generate_report(store: &dyn TemperatureStore, output: Option<&str>) -> Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    generate_report_to_writer(store, &mut buf)?;
    let text = String::from_utf8(buf).context("Report contains invalid UTF-8")?;
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
