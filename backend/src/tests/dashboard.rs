//! The pages of the dashboard: what the figures are put down to — the room, or one component.

use crate::domain::dashboard::{overview, sensor, Facts};
use crate::domain::trends::{self, DayStat};
use crate::domain::wording::Cause;
use chrono::NaiveDate;

const DAYS: i64 = 200;

/// A series of [`DAYS`] daily medians rising linearly, built so that the drift it reports
/// (last rolling median minus baseline) is exactly `drift`: the baseline is centred on day
/// 14.5 and the last rolling median on day `DAYS` − 15.5, i.e. `DAYS` − 30 days apart.
fn ramp(drift: f64) -> Vec<DayStat> {
    let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let slope = drift / (DAYS - 30) as f64;
    (0..DAYS)
        .map(|i| {
            let m = 40.0 + slope * i as f64;
            DayStat {
                day: start + chrono::Duration::days(i),
                n: 288,
                min: m - 2.0,
                median: m,
                p95: m + 3.0,
                max: m + 5.0,
            }
        })
        .collect()
}

/// Same as [`ramp`], but swinging `swing` degrees either side from one day to the next: a series
/// whose own noise is wider than the change being looked for.
fn noisy_ramp(drift: f64, swing: f64) -> Vec<DayStat> {
    ramp(drift)
        .into_iter()
        .enumerate()
        .map(|(i, mut d)| {
            d.median += if i % 2 == 0 { -swing } else { swing };
            d
        })
        .collect()
}

/// A machine whose sensors drift by the given amounts, at idle.
fn machine(drifts: &[(&str, &str, f64)]) -> Facts {
    let noisy: Vec<(&str, &str, f64, f64)> =
        drifts.iter().map(|(h, s, d)| (*h, *s, *d, 0.0)).collect();
    machine_with_noise(&noisy)
}

/// A machine whose sensors drift by the given amounts, each with its own day-to-day swing.
fn machine_with_noise(drifts: &[(&str, &str, f64, f64)]) -> Facts {
    let groups: Vec<(String, String, Vec<DayStat>)> = drifts
        .iter()
        .map(|(h, s, d, swing)| (h.to_string(), s.to_string(), noisy_ramp(*d, *swing)))
        .collect();
    let today = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap() + chrono::Duration::days(DAYS);
    let summaries = groups
        .iter()
        .map(|(h, s, stats)| trends::summarize(h, s, "idle", stats, today))
        .collect();
    let seen = groups
        .iter()
        .map(|(h, s, _)| (h.clone(), s.clone()))
        .collect();
    Facts {
        groups,
        summaries,
        seen,
        since: Some("2024-01-01".into()),
        today,
    }
}

/// Five components, each drifting by the amount given, in that order.
fn five(cpu: f64, gpu: f64, board: f64, disk1: f64, disk2: f64) -> Facts {
    machine(&[
        ("AMD Ryzen 7", "Core (Tctl/Tdie)", cpu),
        ("NVIDIA GPU", "GPU Core", gpu),
        ("Gigabyte B550", "Chipset", board),
        ("Samsung SSD 970", "Temperature", disk1),
        ("Crucial CT500", "Temperature", disk2),
    ])
}

fn detail(facts: &Facts) -> String {
    overview::build(facts, "idle").headline.detail.unwrap()
}

/// The line of the processor's page that says what may explain its change.
fn advice(facts: &Facts) -> Option<String> {
    sensor::build(facts, "AMD Ryzen 7", "Core (Tctl/Tdie)", "idle")
        .story
        .into_iter()
        .find(|l| l.kind == "advice")
        .map(|l| l.spans.iter().map(|s| s.text.clone()).collect())
}

#[test]
fn the_machine_drift_is_the_median_of_the_components() {
    let facts = five(6.0, 0.5, 0.5, 0.5, 0.5);
    assert_eq!(facts.machine_drift().map(|d| (d * 10.0).round()), Some(5.0));
}

#[test]
fn everything_drifting_together_is_put_down_to_the_room() {
    let text = detail(&five(3.0, 3.1, 2.9, 3.0, 3.2));
    assert!(
        text.contains("Tous les composants ont bougé ensemble"),
        "{text}"
    );
    assert!(text.contains("+3,0 °C"), "{text}");
    assert!(text.contains("température de la pièce"), "{text}");
    // Nothing is blamed on a component when nothing stands out.
    assert!(!text.contains("poussière"), "{text}");
}

#[test]
fn one_component_drifting_alone_is_put_down_to_that_component() {
    let text = detail(&five(6.0, 0.5, 0.5, 0.5, 0.5));
    assert!(
        text.starts_with("Un composant se détache : Processeur,"),
        "{text}"
    );
    // 6,0 of its own, 0,5 for the machine: 5,5 is what it did on its own.
    assert!(text.contains("5,5 °C de plus que le reste"), "{text}");
    assert!(text.contains("poussière"), "{text}");
}

#[test]
fn each_component_says_whether_it_moved_alone_or_with_the_machine() {
    let o = overview::build(&five(6.0, 3.0, 3.0, 3.0, 3.0), "idle");
    let cause = |i: usize| o.components[i].cause.clone().unwrap();
    assert_eq!(cause(0).id, Cause::Alone);
    assert_eq!(cause(0).label, "Plus chaud que le reste");
    assert_eq!(
        cause(0).detail.unwrap(),
        "3,0 °C de plus que le reste de la machine"
    );
    // The others rose as much as the machine did: the room, not them.
    assert_eq!(cause(1).id, Cause::Machine);
    assert_eq!(cause(1).label, "Comme toute la machine");
    assert!(o.components[1].excess.unwrap().abs() < 0.05);
}

#[test]
fn a_component_warming_up_alone_comes_before_one_cooling_down_further() {
    // The graphics card stands out further (−5 °C), but only the processor can be acted upon.
    let text = detail(&five(2.0, -5.0, 0.0, 0.0, 0.0));
    assert!(
        text.starts_with("Un composant se détache : Processeur,"),
        "{text}"
    );
}

#[test]
fn a_component_cooling_down_alone_is_told_as_good_news() {
    let o = overview::build(&five(0.0, -4.0, 0.0, 0.0, 0.0), "idle");
    let text = o.headline.detail.unwrap();
    assert!(
        text.starts_with("Aucun composant ne chauffe plus que les autres."),
        "{text}"
    );
    assert!(text.contains("Carte graphique"), "{text}");
    assert!(text.contains("dans le bon sens"), "{text}");
    // The card itself says which way it stands out.
    let gpu = o.components[1].cause.clone().unwrap();
    assert_eq!(gpu.id, Cause::Alone);
    assert_eq!(gpu.label, "Plus froid que le reste");
    assert_eq!(
        gpu.detail.unwrap(),
        "4,0 °C de moins que le reste de la machine"
    );
}

#[test]
fn a_still_machine_blames_nobody() {
    let o = overview::build(&five(0.2, 0.0, -0.3, 0.1, 0.0), "idle");
    assert_eq!(o.components[0].cause.clone().unwrap().id, Cause::Nothing);
    let text = o.headline.detail.unwrap();
    assert!(
        text.contains("Aucun composant n'a vraiment changé"),
        "{text}"
    );
}

#[test]
fn too_few_components_cannot_tell_the_room_from_a_component() {
    // Two components: a median over two of them would say nothing about the machine.
    let facts = machine(&[
        ("AMD Ryzen 7", "Core (Tctl/Tdie)", 6.0),
        ("NVIDIA GPU", "GPU Core", 0.5),
    ]);
    assert_eq!(facts.machine_drift(), None);
    let o = overview::build(&facts, "idle");
    assert!(o.components[0].cause.is_none());
    assert!(o.components[0].excess.is_none());
    let text = o.headline.detail.unwrap();
    assert!(
        text.starts_with("Le changement le plus marqué : Processeur (+"),
        "{text}"
    );
    assert!(text.contains("Trop peu de composants"), "{text}");
}

// ── The page of one sensor ───────────────────────────────────────────────────

#[test]
fn a_sensor_warming_up_alone_is_told_to_look_at_dust_and_paste() {
    let text = advice(&five(6.0, 0.5, 0.5, 0.5, 0.5)).unwrap();
    assert!(
        text.starts_with("C'est 5,5 °C de plus que le reste de la machine"),
        "{text}"
    );
    assert!(text.contains("poussière"), "{text}");
}

#[test]
fn a_sensor_warming_up_with_the_whole_machine_is_put_down_to_the_room() {
    let text = advice(&five(4.0, 4.0, 4.0, 4.0, 4.0)).unwrap();
    assert!(
        text.contains("Le reste de la machine a chauffé autant (+4,0 °C)"),
        "{text}"
    );
    assert!(text.contains("température de la pièce"), "{text}");
    // Nothing to act on: dust and paste are not mentioned.
    assert!(!text.contains("poussière"), "{text}");
}

#[test]
fn a_change_barely_above_the_band_is_not_turned_into_a_cause() {
    let text = advice(&five(2.0, 1.0, 1.0, 1.0, 1.0)).unwrap();
    assert!(text.contains("trop proche du bruit de mesure"), "{text}");
}

#[test]
fn without_a_machine_wide_drift_the_sensor_page_leaves_both_doors_open() {
    let facts = machine(&[
        ("AMD Ryzen 7", "Core (Tctl/Tdie)", 6.0),
        ("NVIDIA GPU", "GPU Core", 0.5),
    ]);
    let text = advice(&facts).unwrap();
    assert!(
        text.starts_with("Si d'autres composants ont aussi chauffé"),
        "{text}"
    );
}

#[test]
fn a_sensor_that_did_not_warm_up_gets_no_advice_line() {
    assert_eq!(advice(&five(0.0, 0.0, 0.0, 0.0, 0.0)), None);
}

#[test]
fn a_noisy_component_has_to_move_further_before_it_is_named() {
    // The processor drifts 2 °C more than the rest of the machine — but its own idle temperature
    // swings 4 °C from one day to the next, so 2 °C says nothing about it.
    let facts = machine_with_noise(&[
        ("AMD Ryzen 7", "Core (Tctl/Tdie)", 2.0, 4.0),
        ("NVIDIA GPU", "GPU Core", 0.0, 0.0),
        ("Gigabyte B550", "Chipset", 0.0, 0.0),
        ("Samsung SSD 970", "Temperature", 0.0, 0.0),
        ("Crucial CT500", "Temperature", 0.0, 0.0),
    ]);
    let o = overview::build(&facts, "idle");
    assert_eq!(o.components[0].cause.clone().unwrap().id, Cause::Nothing);
    let text = o.headline.detail.unwrap();
    assert!(text.contains("Aucun composant"), "{text}");

    // The very same drift on a quiet processor is named.
    let quiet = overview::build(&five(2.0, 0.0, 0.0, 0.0, 0.0), "idle");
    assert_eq!(quiet.components[0].cause.clone().unwrap().id, Cause::Alone);
    assert!(quiet
        .headline
        .detail
        .unwrap()
        .starts_with("Un composant se détache : Processeur,"),);
}
