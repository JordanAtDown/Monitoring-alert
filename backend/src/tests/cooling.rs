//! The cooling of the processor: the figure that stays put when the room warms up or the machine
//! is used harder, and moves when the heat sink clogs.

use crate::domain::cooling::{
    find_parts, measured, resistance_series, Change, MIN_POWER_W, RELATIVE_BAND,
};
use crate::domain::dashboard::cooling::{self, Cooling};
use crate::domain::dashboard::Facts;
use crate::domain::model::{ExtraKind, ExtraSeries};
use crate::domain::trends::{compute_trend, DayStat};
use crate::domain::wording::{relative, resistance, rpm};
use chrono::NaiveDate;

const DAYS: i64 = 200;
/// The day the values change in the scenarios: the first days are the baseline, the last the present.
const STEP: i64 = 100;

fn start() -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()
}

fn stats(days: i64, f: impl Fn(i64) -> f64) -> Vec<DayStat> {
    (0..days)
        .map(|i| {
            let m = f(i);
            DayStat {
                day: start() + chrono::Duration::days(i),
                n: 288,
                min: m,
                median: m,
                p95: m,
                max: m,
            }
        })
        .collect()
}

/// `before` for the first days, `after` from the step on.
fn step(before: f64, after: f64) -> impl Fn(i64) -> f64 {
    move |i| if i < STEP { before } else { after }
}

fn extra(hardware: &str, sensor: &str, kind: ExtraKind, stats: Vec<DayStat>) -> ExtraSeries {
    ExtraSeries {
        hardware: hardware.into(),
        sensor: sensor.into(),
        kind,
        stats,
    }
}

const CPU: (&str, &str) = ("AMD Ryzen 7 5700X", "Core (Tctl/Tdie)");
const CASE: (&str, &str) = ("Gigabyte B550I AORUS PRO AX", "System");

/// A machine read at the end of `days` days: the processor and case temperatures, what the
/// processor draws, and what its fan does.
fn machine(
    days: i64,
    cpu: impl Fn(i64) -> f64,
    case: impl Fn(i64) -> f64,
    watts: impl Fn(i64) -> f64,
    fan: impl Fn(i64) -> f64,
) -> (Facts, Vec<ExtraSeries>) {
    let groups = vec![
        (CPU.0.to_string(), CPU.1.to_string(), stats(days, cpu)),
        (CASE.0.to_string(), CASE.1.to_string(), stats(days, case)),
    ];
    let extras = vec![
        extra(CPU.0, "Package", ExtraKind::Power, stats(days, watts)),
        extra(CASE.0, "CPU Fan", ExtraKind::Fan, stats(days, fan)),
        // Another fan of the case: says nothing about the processor.
        extra(
            CASE.0,
            "System Fan #1",
            ExtraKind::Fan,
            stats(days, |_| 700.0),
        ),
    ];
    let facts = Facts {
        seen: groups
            .iter()
            .map(|(h, s, _)| (h.clone(), s.clone()))
            .collect(),
        groups,
        summaries: vec![],
        since: None,
        today: start() + chrono::Duration::days(days),
    };
    (facts, extras)
}

fn card(facts: &Facts, extras: &[ExtraSeries]) -> Cooling {
    cooling::build(facts, extras, "idle")
}

fn text(c: &Cooling) -> String {
    format!("{} {}", c.lead, c.detail.clone().unwrap_or_default())
}

// ── The thermal resistance ────────────────────────────────────────────────────

#[test]
fn a_clogged_heat_sink_keeps_more_heat_in_the_processor_for_the_same_watts() {
    // The processor goes from 6 °C to 12 °C above the case, drawing 60 W all along.
    let (facts, extras) = machine(DAYS, step(40.0, 46.0), |_| 34.0, |_| 60.0, |_| 1000.0);
    let c = card(&facts, &extras);
    assert_eq!((c.state, c.verdict), ("ok", "worse"));
    assert!(
        c.lead.contains("garde 100 % de chaleur en plus"),
        "{}",
        c.lead
    );
    assert_eq!(c.rows[0].baseline, "0,10 °C/W");
    assert_eq!(c.rows[0].current, "0,20 °C/W");
    assert_eq!(c.rows[0].change, "+100 %");
}

#[test]
fn a_warmer_room_is_not_a_worse_cooling() {
    // Everything is 4 °C warmer: the processor and the case alike. The gap, the resistance, stay.
    let (facts, extras) = machine(
        DAYS,
        step(40.0, 44.0),
        step(34.0, 38.0),
        |_| 60.0,
        |_| 1000.0,
    );
    let c = card(&facts, &extras);
    assert_eq!(c.verdict, "same", "{}", text(&c));
    assert_eq!(c.rows[0].change, "0 %");
}

#[test]
fn a_heavier_use_is_not_a_worse_cooling() {
    // The processor is 6 °C hotter because it draws twice the power: the resistance is the same.
    let (facts, extras) = machine(
        DAYS,
        step(40.0, 46.0),
        |_| 34.0,
        step(60.0, 120.0),
        |_| 1000.0,
    );
    let c = card(&facts, &extras);
    assert_eq!(c.verdict, "same", "{}", text(&c));
}

#[test]
fn a_cooling_that_improved_says_so() {
    // After a cleaning, or fresh paste.
    let (facts, extras) = machine(DAYS, step(46.0, 40.0), |_| 34.0, |_| 60.0, |_| 1000.0);
    let c = card(&facts, &extras);
    assert_eq!(c.verdict, "better");
    assert!(
        c.lead.contains("garde 50 % de chaleur en moins"),
        "{}",
        c.lead
    );
    assert!(text(&c).contains("après un nettoyage"));
}

#[test]
fn only_a_whole_power_draw_above_the_floor_counts() {
    let cpu = stats(10, |_| 50.0);
    let case = stats(10, |_| 40.0);
    // 5 W: one degree of resolution would swing the resistance by 20 %.
    assert!(resistance_series(&cpu, &case, &stats(10, |_| MIN_POWER_W - 1.0)).is_empty());
    let series = resistance_series(&cpu, &case, &stats(10, |_| 50.0));
    assert_eq!(series.len(), 10);
    assert!((series[0].median - 0.2).abs() < 1e-12);
}

#[test]
fn a_day_missing_one_of_the_three_is_left_out() {
    let cpu = stats(10, |_| 50.0);
    let case = stats(5, |_| 40.0);
    let power = stats(10, |_| 50.0);
    assert_eq!(resistance_series(&cpu, &case, &power).len(), 5);

    // A processor cooler than the case is not a measurement of the cooling.
    assert!(resistance_series(&stats(10, |_| 30.0), &case, &power).is_empty());

    // A day with too few readings is not either.
    let mut thin = stats(3, |_| 50.0);
    thin[1].n = 2;
    assert_eq!(
        resistance_series(&stats(3, |_| 50.0), &stats(3, |_| 40.0), &thin).len(),
        2
    );
}

// ── The fan ───────────────────────────────────────────────────────────────────

#[test]
fn a_fan_compensating_is_told_even_while_the_cooling_holds() {
    // The resistance has not moved, but the fan has to turn 30 % faster to hold it.
    let (facts, extras) = machine(DAYS, |_| 40.0, |_| 34.0, |_| 60.0, step(1000.0, 1300.0));
    let c = card(&facts, &extras);
    assert_eq!((c.state, c.verdict), ("ok", "watch"));
    assert!(c.lead.contains("tourne 30 % plus vite"), "{}", c.lead);
    assert!(
        text(&c).contains("pièce plus chaude"),
        "the room is the other explanation"
    );
}

#[test]
fn a_worse_cooling_with_a_fan_speeding_up_says_the_system_compensates() {
    let (facts, extras) = machine(
        DAYS,
        step(40.0, 46.0),
        |_| 34.0,
        |_| 60.0,
        step(1000.0, 1400.0),
    );
    let t = text(&card(&facts, &extras));
    assert!(t.contains("tourne 40 % plus vite"), "{t}");
    assert!(t.contains("le système compense"), "{t}");
}

#[test]
fn a_worse_cooling_with_a_steady_fan_says_it_does_not_compensate() {
    let (facts, extras) = machine(DAYS, step(40.0, 46.0), |_| 34.0, |_| 60.0, |_| 1000.0);
    assert!(text(&card(&facts, &extras)).contains("il ne compense pas"));
}

#[test]
fn the_advice_is_to_clean_first_because_the_fan_cannot_tell_dust_from_paste() {
    let (facts, extras) = machine(
        DAYS,
        step(40.0, 46.0),
        |_| 34.0,
        |_| 60.0,
        step(1000.0, 1400.0),
    );
    let t = text(&card(&facts, &extras));
    let clean = t.find("nettoyer").expect("cleaning is advised");
    let paste = t.find("pâte thermique").expect("paste is mentioned");
    assert!(clean < paste, "cleaning comes first: {t}");
    assert!(
        t.contains("ne départage pas"),
        "it says what the fan cannot tell: {t}"
    );
}

#[test]
fn the_fan_that_cools_the_processor_is_the_one_that_says_cpu() {
    let (facts, extras) = machine(DAYS, |_| 40.0, |_| 34.0, |_| 60.0, |_| 1000.0);
    let mut sensors = facts.seen.clone();
    sensors.extend(facts.groups.iter().map(|(h, s, _)| (h.clone(), s.clone())));
    let parts = find_parts(&sensors, &extras).unwrap();
    assert_eq!(parts.fan.unwrap().1, "CPU Fan");
    assert_eq!(parts.power.unwrap().1, "Package");
    assert_eq!(parts.case.unwrap().1, "System");
    assert_eq!(parts.cpu.1, "Core (Tctl/Tdie)");
}

#[test]
fn the_power_of_the_graphics_card_is_not_taken_for_the_processor_s() {
    let (facts, mut extras) = machine(DAYS, |_| 40.0, |_| 34.0, |_| 60.0, |_| 1000.0);
    extras.retain(|e| e.kind != ExtraKind::Power);
    extras.push(extra(
        "NVIDIA GeForce RTX 5060",
        "GPU Package",
        ExtraKind::Power,
        stats(DAYS, |_| 9.0),
    ));
    let parts = find_parts(&facts.seen, &extras).unwrap();
    assert_eq!(parts.power, None);
}

// ── What cannot be said ───────────────────────────────────────────────────────

#[test]
fn nothing_recorded_yet_is_said_plainly() {
    let (facts, _) = machine(DAYS, |_| 40.0, |_| 34.0, |_| 60.0, |_| 1000.0);
    let c = card(&facts, &[]);
    assert_eq!((c.state, c.verdict), ("none", "unknown"));
    assert!(c.rows.is_empty());
    assert!(c.detail.unwrap().contains("depuis la version 3"));
}

#[test]
fn a_few_days_of_data_say_how_much_is_missing() {
    let (facts, extras) = machine(20, |_| 40.0, |_| 34.0, |_| 60.0, |_| 1000.0);
    let c = card(&facts, &extras);
    assert_eq!((c.state, c.verdict), ("few", "unknown"));
    let detail = c.detail.unwrap();
    assert!(detail.contains("au moins 60 jours de suivi"), "{detail}");
    assert!(
        detail.contains("Pour l'instant : 20 jours de mesures, depuis le 1 janvier 2024"),
        "{detail}"
    );
    assert_eq!(c.since.as_deref(), Some("2024-01-01"));
}

#[test]
fn without_the_power_only_the_fan_can_speak_and_it_says_what_is_missing() {
    let (facts, mut extras) = machine(DAYS, |_| 40.0, |_| 34.0, |_| 60.0, step(1000.0, 1300.0));
    extras.retain(|e| e.kind != ExtraKind::Power);
    let c = card(&facts, &extras);
    assert_eq!((c.state, c.verdict), ("ok", "unknown"));
    assert!(c.lead.contains("tourne 30 % plus vite"));
    let detail = c.detail.unwrap();
    assert!(
        detail.contains("Sans l'efficacité du refroidissement"),
        "{detail}"
    );
    assert!(detail.contains("la puissance du processeur"), "{detail}");
    assert_eq!(c.rows.len(), 1, "only the fan figure is shown");
}

#[test]
fn a_series_that_stopped_is_not_a_present_figure() {
    let (mut facts, extras) = machine(DAYS, step(40.0, 46.0), |_| 34.0, |_| 60.0, |_| 1000.0);
    // Read two months after the last measurement.
    facts.today += chrono::Duration::days(60);
    let c = card(&facts, &extras);
    assert_eq!(c.state, "few");
}

// ── The change of a series ────────────────────────────────────────────────────

fn trend(f: impl Fn(i64) -> f64) -> crate::domain::trends::Trend {
    compute_trend(&stats(DAYS, f), start() + chrono::Duration::days(DAYS))
}

#[test]
fn a_change_is_relative_to_the_beginning() {
    let m = measured(&trend(step(1000.0, 1250.0))).unwrap();
    assert_eq!(m.change, Change::Up);
    assert!((m.fraction - 0.25).abs() < 1e-9);
    assert_eq!((m.baseline, m.current), (1000.0, 1250.0));
    assert_eq!(
        measured(&trend(step(1000.0, 800.0))).unwrap().change,
        Change::Down
    );
}

#[test]
fn a_change_below_a_tenth_is_not_one() {
    let just_under = 1000.0 * (1.0 + RELATIVE_BAND - 0.01);
    assert_eq!(
        measured(&trend(step(1000.0, just_under))).unwrap().change,
        Change::Steady
    );
    let just_over = 1000.0 * (1.0 + RELATIVE_BAND + 0.01);
    assert_eq!(
        measured(&trend(step(1000.0, just_over))).unwrap().change,
        Change::Up
    );
}

#[test]
fn a_noisy_series_needs_a_larger_change_than_a_tenth() {
    // A fan whose daily speed swings by 400 RPM: 15 % is within the noise.
    let swing = |i: i64| if i % 2 == 0 { -400.0 } else { 400.0 };
    let m = measured(&trend(|i| {
        1000.0 + swing(i) + if i >= STEP { 150.0 } else { 0.0 }
    }))
    .unwrap();
    assert_eq!(m.change, Change::Steady, "{m:?}");
}

#[test]
fn a_fan_that_never_turned_has_nothing_to_be_compared_with() {
    assert_eq!(measured(&trend(|_| 0.0)), None);
}

#[test]
fn too_little_history_measures_nothing() {
    assert_eq!(
        measured(&compute_trend(
            &stats(30, |_| 1000.0),
            start() + chrono::Duration::days(30)
        )),
        None
    );
}

// ── Words ─────────────────────────────────────────────────────────────────────

#[test]
fn the_figures_are_written_the_french_way() {
    assert_eq!(resistance(0.3123), "0,31 °C/W");
    assert_eq!(rpm(1293.4), "1\u{202f}293 tr/min");
    assert_eq!(rpm(674.0), "674 tr/min");
    assert_eq!(relative(0.06), "+6 %");
    assert_eq!(relative(-0.124), "−12 %");
    assert_eq!(relative(0.001), "0 %");
}
