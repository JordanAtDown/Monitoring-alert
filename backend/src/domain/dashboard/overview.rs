//! The home of the dashboard, ready to display: the answer in a few words, then one entry per
//! component of the PC. Everything the page prints comes from here; it only lays it out.

use serde::Serialize;
use std::collections::BTreeMap;

use crate::domain::components::{self, sensor_name, Component, Kind};
use crate::domain::dashboard::Facts;
use crate::domain::trends::{SensorSummary, MIN_SPAN_DAYS};
use crate::domain::wording::{self, cause_of, plural, verdict_of, Cause, Verdict, STABLE_BAND};

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

/// Whether a component moved on its own or with the whole machine.
#[derive(Debug, Clone, Serialize)]
pub struct CauseView {
    pub id: Cause,
    pub label: &'static str,
    /// "4,1 °C de plus que le reste de la machine".
    pub detail: Option<String>,
}

fn cause_view(delta: f64, excess: f64) -> CauseView {
    let id = cause_of(delta, excess);
    CauseView {
        id,
        label: id.label(excess),
        detail: match id {
            Cause::Alone => Some(format!(
                "{} de {} que le reste de la machine",
                wording::temp(excess.abs()),
                if excess >= 0.0 { "plus" } else { "moins" }
            )),
            Cause::Machine => Some(wording::signed(delta)),
            Cause::Nothing => None,
        },
    }
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
    /// How much more it drifted than the machine as a whole; `None` without a machine-wide
    /// drift to compare it with.
    pub excess: Option<f64>,
    /// What that excess means, in words.
    pub cause: Option<CauseView>,
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
    machine: Option<f64>,
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

    Headline {
        state: "ok",
        lead,
        detail: diagnosis(machine, components, mains),
        tiles,
    }
}

/// What the figures say, in one or two sentences. Comparing each component with the drift of the
/// whole machine is what separates "the room is warmer" from "this part is in trouble" — the one
/// question the figures can actually answer, and the one worth acting on.
fn diagnosis(
    machine: Option<f64>,
    components: &[ComponentView],
    mains: &[&SensorSummary],
) -> Option<String> {
    let drift_of = |c: &ComponentView| {
        mains
            .iter()
            .find(|m| m.hardware == c.main.hardware && m.sensor == c.main.sensor)?
            .drift()
    };
    let Some(machine) = machine else {
        // Too few components with enough history to tell the room from a component.
        let (c, d) = components
            .iter()
            .filter_map(|c| Some((c, drift_of(c)?)))
            .max_by(|a, b| a.1.abs().total_cmp(&b.1.abs()))?;
        return Some(if d.abs() < STABLE_BAND {
            "Aucun composant n'a vraiment changé de température.".to_string()
        } else {
            format!(
                "Le changement le plus marqué : {} ({}). Trop peu de composants ont assez de recul pour dire si cela vient de la pièce ou du composant.",
                c.title,
                wording::signed(d)
            )
        });
    };
    // The one that warmed up the most on its own comes first: it is the only thing here that
    // can be acted upon. A component that cools down relative to the others is only reported
    // when nothing warmed up, so that good news never hides the finding that matters.
    let excesses = || {
        components
            .iter()
            .filter_map(|c| Some((c, drift_of(c)? - machine)))
    };
    let warmest = excesses().max_by(|a, b| a.1.total_cmp(&b.1));
    let coolest = excesses().min_by(|a, b| a.1.total_cmp(&b.1));
    Some(match (warmest, coolest) {
        (Some((c, excess)), _) if excess >= STABLE_BAND => format!(
            "Un composant se détache : {}, {} de plus que le reste de la machine (qui a bougé de {}). Un composant qui chauffe seul, c'est de la poussière, de la pâte thermique ou un ventilateur : c'est là qu'il faut regarder.",
            c.title,
            wording::temp(excess),
            wording::signed(machine)
        ),
        (_, Some((c, excess))) if excess <= -STABLE_BAND => format!(
            "Aucun composant ne chauffe plus que les autres. Un seul se détache, {}, et c'est dans le bon sens : {} de moins que le reste de la machine (qui a bougé de {}).",
            c.title,
            wording::temp(excess.abs()),
            wording::signed(machine)
        ),
        _ if machine.abs() >= STABLE_BAND => format!(
            "Tous les composants ont bougé ensemble ({}) et aucun ne se détache : c'est très probablement la température de la pièce, pas votre PC.",
            wording::signed(machine)
        ),
        _ => "Aucun composant n'a vraiment changé de température, ni seul ni avec les autres.".to_string(),
    })
}

/// The home page at one load level.
pub fn build(facts: &Facts, load: &str) -> Overview {
    let summaries = &facts.summaries;
    let since = facts.since.clone();
    let machine = facts.machine_drift();

    let components: Vec<ComponentView> = components_with_data(&facts.seen, summaries)
        .into_iter()
        .map(|(slot, c, rows)| {
            let sensors: Vec<SensorView> = rows.iter().map(|s| sensor_view(s, c.kind)).collect();
            // `components_with_data` puts the sensor that stands for the component first.
            let main = sensors[0].clone();
            let excess = match (rows[0].drift(), machine) {
                (Some(d), Some(m)) => Some(d - m),
                _ => None,
            };
            let cause = match (rows[0].drift(), excess) {
                (Some(d), Some(e)) => Some(cause_view(d, e)),
                _ => None,
            };
            ComponentView {
                key: c.key.clone(),
                kind: c.kind,
                title: c.title.clone(),
                subtitle: c.subtitle.clone(),
                slot,
                main,
                sensors,
                excess,
                cause,
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
    Overview {
        load: load_choice(load),
        headline: headline(load, &components, &mains, since.as_deref(), machine),
        components,
    }
}
