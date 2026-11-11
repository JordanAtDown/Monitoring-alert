//! The home of the dashboard, ready to display: the answer in a few words, then one entry per
//! component of the PC. Everything the page prints comes from here; it only lays it out.

use anyhow::Result;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::application::clock::day_string;
use crate::application::ports::ReadModel;
use crate::domain::components::{self, sensor_name, Component, Kind};
use crate::domain::trends::{self, SensorSummary, MIN_SPAN_DAYS};
use crate::domain::wording::{self, plural, verdict_of, Verdict, STABLE_BAND};

/// "Plus chaud +4,7 °C", "Pas assez de recul (12 j de mesures)".
#[derive(Debug, Clone, Serialize)]
pub struct VerdictView {
    pub id: Verdict,
    pub label: &'static str,
    /// The figure that goes with the word: the change, or how little history there is.
    pub detail: Option<String>,
}

fn verdict_view(s: &SensorSummary) -> VerdictView {
    let delta = if s.sufficient { s.delta } else { None };
    let id = verdict_of(delta);
    VerdictView {
        id,
        label: id.label(),
        detail: match delta {
            Some(d) => Some(wording::signed(d)),
            None => Some(format!("({} j de mesures)", s.days)),
        },
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SensorView {
    /// To ask for its curve (`/api/trend`).
    pub hardware: String,
    pub sensor: String,
    pub name: String,
    /// The technical name, when it differs from `name`.
    pub tech: Option<String>,
    pub baseline: Option<String>,
    pub current: Option<String>,
    pub verdict: VerdictView,
    pub sparkline: Vec<f64>,
}

#[derive(Debug, Serialize)]
pub struct ComponentView {
    pub key: String,
    pub kind: Kind,
    pub title: String,
    pub subtitle: Option<String>,
    /// Position among the components of the machine: the page gives a color per position.
    pub slot: usize,
    /// The sensor that stands for the component; the other fields mirror it.
    pub main: SensorView,
    /// All its sensors, `main` first.
    pub sensors: Vec<SensorView>,
}

#[derive(Debug, Serialize)]
pub struct Tile {
    pub verdict: Verdict,
    pub count: usize,
    pub words: String,
}

#[derive(Debug, Serialize)]
pub struct Headline {
    /// `empty`, `few` (not enough history) or `ok`.
    pub state: &'static str,
    pub lead: String,
    /// What the figures say, or what is missing.
    pub detail: Option<String>,
    pub tiles: Vec<Tile>,
}

#[derive(Debug, Serialize)]
pub struct LoadView {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Serialize)]
pub struct LoadChoice {
    pub id: &'static str,
    pub label: &'static str,
    /// "Au repos : moins de 15 % d'utilisation… On compare toujours des mesures prises dans les mêmes conditions."
    pub note: String,
    /// Every level, to offer the choice.
    pub choices: Vec<LoadView>,
}

#[derive(Debug, Serialize)]
pub struct Overview {
    pub load: LoadChoice,
    pub headline: Headline,
    pub components: Vec<ComponentView>,
}

pub fn load_choice(load: &str) -> LoadChoice {
    let words = wording::load_words(load).unwrap_or(&wording::LOADS[0]);
    LoadChoice {
        id: words.id,
        label: words.label,
        note: format!(
            "{} : {}. On compare toujours des mesures prises dans les mêmes conditions.",
            words.label, words.hint
        ),
        choices: wording::LOADS
            .iter()
            .map(|l| LoadView {
                id: l.id,
                label: l.label,
            })
            .collect(),
    }
}

fn sensor_view(s: &SensorSummary, kind: Kind) -> SensorView {
    let (name, tech) = sensor_name(&s.sensor, kind);
    SensorView {
        hardware: s.hardware.clone(),
        sensor: s.sensor.clone(),
        name,
        tech,
        baseline: s.baseline.filter(|_| s.sufficient).map(wording::temp),
        current: s.current.map(wording::temp),
        verdict: verdict_view(s),
        sparkline: s.sparkline.clone(),
    }
}

/// Every component of the machine, in display order, with the summary of its sensors at one load.
/// `seen` lists every sensor the machine reports, so the numbering of the disks stays stable.
pub fn components_with_data<'a>(
    seen: &[(String, String)],
    summaries: &'a [SensorSummary],
) -> Vec<(usize, Component, Vec<&'a SensorSummary>)> {
    let by_key: BTreeMap<(&str, &str), &SensorSummary> = summaries
        .iter()
        .map(|s| ((s.hardware.as_str(), s.sensor.as_str()), s))
        .collect();
    let mut all: Vec<(String, String)> = seen.to_vec();
    all.extend(
        summaries
            .iter()
            .map(|s| (s.hardware.clone(), s.sensor.clone())),
    );
    components::components(&all)
        .into_iter()
        .enumerate()
        .filter_map(|(slot, c)| {
            let c = c.restricted_to(|(h, s)| by_key.contains_key(&(h.as_str(), s.as_str())))?;
            let mut rows: Vec<&SensorSummary> = c
                .sensors
                .iter()
                .filter_map(|(h, s)| by_key.get(&(h.as_str(), s.as_str())).copied())
                .collect();
            // The sensor that stands for the component first, then the most changed (disks and
            // cards: by name, so the order never jumps).
            let by_name = matches!(c.kind, Kind::Disk | Kind::Gpu);
            rows.sort_by(|a, b| {
                let main = |s: &SensorSummary| {
                    (s.hardware.as_str(), s.sensor.as_str())
                        != (c.main.0.as_str(), c.main.1.as_str())
                };
                main(a).cmp(&main(b)).then_with(|| {
                    if by_name {
                        a.sensor.cmp(&b.sensor)
                    } else {
                        b.delta
                            .unwrap_or(f64::NEG_INFINITY)
                            .total_cmp(&a.delta.unwrap_or(f64::NEG_INFINITY))
                    }
                })
            });
            Some((slot, c, rows))
        })
        .collect()
}

fn headline(
    load: &str,
    components: &[ComponentView],
    mains: &[&SensorSummary],
    since: Option<&str>,
) -> Headline {
    let when = wording::load_words(load).map_or("", |l| l.when);
    if mains.is_empty() {
        return Headline {
            state: "empty",
            lead: "Aucune mesure résumée pour l'instant.".into(),
            detail: Some(
                "Les premiers résumés apparaissent le lendemain de la première collecte.".into(),
            ),
            tiles: vec![],
        };
    }
    let known: Vec<&&SensorSummary> = mains
        .iter()
        .filter(|m| m.sufficient && m.delta.is_some())
        .collect();
    if known.is_empty() {
        let days = mains.iter().map(|m| m.days).max().unwrap_or(0);
        return Headline {
            state: "few",
            lead: "Pas encore assez d'historique pour comparer.".into(),
            detail: Some(format!(
                "Pour dire si une température a changé, il faut au moins {MIN_SPAN_DAYS} jours de suivi dont 40 jours avec des mesures {when}. Pour l'instant : {days} jour{} de mesures.",
                if days > 1 { "s" } else { "" }
            )),
            tiles: vec![],
        };
    }

    let span = known.iter().map(|m| m.span_days).max().unwrap_or(0);
    let from = since.map_or("Depuis le début".to_string(), |d| {
        format!("Depuis le {}", wording::date(d))
    });
    let lead = format!(
        "{from} ({} de suivi), {when} :",
        wording::span_in_words(span)
    );

    let mut counts = [0usize; 4];
    for c in components {
        counts[c.main.verdict.id as usize] += 1;
    }
    let words = |v: Verdict| -> (&str, &str) {
        match v {
            Verdict::Warmer => (
                "composant plus chaud qu'au début",
                "composants plus chauds qu'au début",
            ),
            Verdict::Stable => ("composant stable", "composants stables"),
            Verdict::Cooler => (
                "composant plus froid qu'au début",
                "composants plus froids qu'au début",
            ),
            Verdict::Unknown => (
                "composant sans assez de recul",
                "composants sans assez de recul",
            ),
        }
    };
    let tiles = [
        Verdict::Warmer,
        Verdict::Stable,
        Verdict::Cooler,
        Verdict::Unknown,
    ]
    .into_iter()
    .filter(|v| *v != Verdict::Unknown || counts[*v as usize] > 0)
    .map(|v| {
        let n = counts[v as usize];
        let (one, many) = words(v);
        Tile {
            verdict: v,
            count: n,
            words: plural(n, one, many).to_string(),
        }
    })
    .collect();

    // The component whose main sensor changed the most.
    let top = components
        .iter()
        .filter(|c| c.main.verdict.id != Verdict::Unknown)
        .filter_map(|c| {
            let d = mains
                .iter()
                .find(|m| m.hardware == c.main.hardware && m.sensor == c.main.sensor)?
                .delta?;
            Some((c, d))
        })
        .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    let detail = top.map(|(c, d)| {
        let first = if d.abs() < STABLE_BAND {
            "Aucun composant n'a vraiment changé de température.".to_string()
        } else {
            format!(
                "Le changement le plus marqué : {} ({}).",
                c.title,
                wording::signed(d)
            )
        };
        let second = if d.abs() < 3.0 {
            "Les écarts restent faibles (moins de 3 °C)."
        } else {
            "Une saison chaude peut expliquer une partie de l'écart."
        };
        format!("{first} {second}")
    });
    Headline {
        state: "ok",
        lead,
        detail,
        tiles,
    }
}

/// The home page at one load level.
pub fn overview(read: &dyn ReadModel, load: &str) -> Result<Overview> {
    let groups = read.daily_stats_for_load(load, &day_string(0))?;
    let summaries: Vec<SensorSummary> = groups
        .iter()
        .map(|(h, s, stats)| trends::summarize(h, s, load, stats))
        .collect();
    let seen: Vec<(String, String)> = read
        .latest_snapshot()?
        .map(|l| {
            l.readings
                .into_iter()
                .map(|r| (r.hardware, r.sensor))
                .collect()
        })
        .unwrap_or_default();
    let since = read
        .overall_stats()?
        .history_start_day()
        .map(str::to_string);

    let components: Vec<ComponentView> = components_with_data(&seen, &summaries)
        .into_iter()
        .map(|(slot, c, rows)| {
            let sensors: Vec<SensorView> = rows.iter().map(|s| sensor_view(s, c.kind)).collect();
            // `components_with_data` puts the sensor that stands for the component first.
            let main = sensors[0].clone();
            ComponentView {
                key: c.key.clone(),
                kind: c.kind,
                title: c.title.clone(),
                subtitle: c.subtitle.clone(),
                slot,
                main,
                sensors,
            }
        })
        .collect();
    let mains: Vec<&SensorSummary> = components
        .iter()
        .filter_map(|c| {
            summaries
                .iter()
                .find(|s| s.hardware == c.main.hardware && s.sensor == c.main.sensor)
        })
        .collect();
    Ok(Overview {
        load: load_choice(load),
        headline: headline(load, &components, &mains, since.as_deref()),
        components,
    })
}
