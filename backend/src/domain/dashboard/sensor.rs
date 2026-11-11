//! The page of one sensor, ready to display: what its numbers mean in a sentence, its curve, its
//! figures month by month.

use serde::Serialize;

use crate::domain::components::{components, sensor_name};
use crate::domain::dashboard::curves::{CurvePoint, Reference};
use crate::domain::dashboard::overview::load_choice;
use crate::domain::dashboard::Facts;
use crate::domain::trends::{self, MIN_SPAN_DAYS};
use crate::domain::wording::{self, verdict_of, Verdict, STABLE_BAND};

/// A piece of a sentence; the figures are in bold.
#[derive(Debug, Serialize)]
pub struct Span {
    pub text: String,
    pub strong: bool,
}

#[derive(Debug, Serialize)]
pub struct Line {
    /// `story` (what the figures say) or `advice` (what may explain them).
    pub kind: &'static str,
    pub spans: Vec<Span>,
}

#[derive(Debug, Serialize)]
pub struct MonthDetail {
    pub label: String,
    pub days: usize,
    pub usual: String,
    pub lowest: String,
    pub highest: String,
}

/// One calendar month against the same month a year earlier, ready to put in a table.
#[derive(Debug, Serialize)]
pub struct SeasonRow {
    /// "novembre 2026".
    pub label: String,
    /// The figure of the same month a year earlier, then of this one, then the change.
    pub before: String,
    pub after: String,
    pub change: String,
}

#[derive(Debug, Serialize)]
pub struct Fact {
    pub label: &'static str,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct SensorPage {
    pub name: String,
    /// "Disque 3 · Samsung SSD 970 EVO Plus 250GB · au repos · capteur « Temperature #1 »".
    pub subtitle: String,
    pub slot: Option<usize>,
    pub story: Vec<Line>,
    /// Empty when there is nothing to draw.
    pub rolling: Vec<CurvePoint>,
    pub reference: Option<Reference>,
    pub legend: Vec<&'static str>,
    pub months: Vec<MonthDetail>,
    /// Each month against the same month a year earlier, newest first; empty before thirteen
    /// months of history.
    pub seasons: Vec<SeasonRow>,
    /// For those who want the statistics.
    pub technical: Vec<Fact>,
}

fn span(text: impl Into<String>, strong: bool) -> Span {
    Span {
        text: text.into(),
        strong,
    }
}

/// Why it got warmer. The drift of the whole machine is what tells the room from the component
/// itself: everything moved together, or this one moved on its own.
fn warmer_advice(delta: f64, machine: Option<f64>) -> String {
    let Some(machine) = machine else {
        return "Si d'autres composants ont aussi chauffé, la chaleur de la saison peut l'expliquer. Si seul celui-ci chauffe, pensez à la poussière, à la pâte thermique ou à un ventilateur.".to_string();
    };
    let excess = delta - machine;
    if excess >= STABLE_BAND {
        format!(
            "C'est {} de plus que le reste de la machine, qui a bougé de {}. Ce capteur chauffe donc pour une raison qui lui est propre : poussière, pâte thermique ou ventilateur.",
            wording::temp(excess),
            wording::signed(machine)
        )
    } else if machine >= STABLE_BAND {
        format!(
            "Le reste de la machine a chauffé autant ({}) : c'est très probablement la température de la pièce, pas ce composant.",
            wording::signed(machine)
        )
    } else {
        format!(
            "Le reste de la machine a bougé de {}. L'écart de ce capteur est trop proche du bruit de mesure pour en conclure quelque chose.",
            wording::signed(machine)
        )
    }
}

/// What the hot moments of each day say. They say more about cooling than the usual temperature
/// does: a heat sink that no longer keeps up lets the peaks climb while the median barely moves,
/// which is exactly the shape of dust in the fins or paste that has dried out.
fn peak_line(t: &trends::Trend) -> Option<Line> {
    let (peak_delta, delta) = (t.peak_delta?, t.delta?);
    // It has to have risen beyond its own noise, and clearly more than the usual temperature.
    if peak_delta < wording::stable_band(t.peak_margin) || peak_delta - delta < STABLE_BAND {
        return None;
    }
    Some(Line {
        kind: "advice",
        spans: vec![span(
            format!(
                "Ses pointes de température ont monté davantage que sa température habituelle ({} contre {}). C'est ce qu'on voit quand le refroidissement ne suit plus dans les moments de forte chaleur : la poussière et la pâte thermique se remarquent là en premier.",
                wording::signed(peak_delta),
                wording::signed(delta)
            ),
            false,
        )],
    })
}

/// The same month, a year apart. The season cancels out — both sides are the same time of year —
/// so whatever is left is the machine. It is the cleanest answer to "is it the weather?", and the
/// only one that needs no assumption at all; it just takes thirteen months of history to exist.
fn season_line(pairs: &[trends::YearOverYear]) -> Option<Line> {
    let p = pairs.first()?;
    let month = wording::month_year(&p.first_day);
    let before = wording::month_year(&p.previous_first_day);
    let text = if p.delta.abs() < STABLE_BAND {
        format!(
            "À saison égale, {month} est au même niveau que {before} ({}) : sur un an, rien n'a bougé pour ce capteur.",
            wording::temp(p.median)
        )
    } else if p.delta > 0.0 {
        format!(
            "À saison égale, {month} est {} plus chaud que {before} ({} contre {}). La saison n'explique pas cet écart : les deux mois sont la même période de l'année.",
            wording::temp(p.delta),
            wording::temp(p.median),
            wording::temp(p.previous_median)
        )
    } else {
        format!(
            "À saison égale, {month} est {} plus frais que {before} ({} contre {}).",
            wording::temp(p.delta.abs()),
            wording::temp(p.median),
            wording::temp(p.previous_median)
        )
    };
    Some(Line {
        kind: "story",
        spans: vec![span(text, false)],
    })
}

fn story(t: &trends::Trend, machine: Option<f64>, pairs: &[trends::YearOverYear]) -> Vec<Line> {
    let plain = |text: String| Line {
        kind: "story",
        spans: vec![span(text, false)],
    };
    if t.points.is_empty() {
        return vec![plain(
            "Pas encore de journée complète avec assez de mesures pour ce capteur à ce niveau de charge.".into(),
        )];
    }
    if !t.fresh {
        let last = t
            .last_day
            .as_deref()
            .map_or("un moment".to_string(), wording::date);
        return vec![plain(format!(
            "Ce capteur n'a plus été mesuré à ce niveau de charge depuis le {last}. Il n'y a donc rien à dire de ses 30 derniers jours ; sa courbe s'arrête à cette date."
        ))];
    }
    let (Some(baseline), Some(delta)) = (t.baseline, t.delta) else {
        return vec![plain(format!(
            "Pour l'instant : {} jours de mesures sur {} jours de suivi. C'est trop court pour dire si la température a changé (il faut au moins {MIN_SPAN_DAYS} jours de suivi, dont 40 avec des mesures). La courbe est déjà tracée.",
            t.days, t.span_days
        ))];
    };
    let verdict = verdict_of(Some(delta), t.margin);
    let gap = wording::temp(delta.abs());
    let outcome = match verdict {
        // Large enough to be noticed, too small to be trusted: say which it is.
        Verdict::Stable if delta.abs() >= STABLE_BAND => format!(
            "une différence de {gap}, trop petite pour être sûre : d'un jour à l'autre, ce capteur varie déjà d'au moins autant."
        ),
        Verdict::Stable => format!("une différence de {gap} : c'est stable."),
        Verdict::Warmer => format!("{gap} de plus qu'au début."),
        _ => format!("{gap} de moins qu'au début : c'est plus frais."),
    };
    let from = t
        .baseline_from
        .as_deref()
        .map_or("le début".to_string(), wording::date);
    let to = t
        .baseline_to
        .as_deref()
        .map_or(String::new(), wording::date);
    let mut lines = vec![Line {
        kind: "story",
        spans: vec![
            span(
                format!("Au début (du {from} au {to}), sa température habituelle était de "),
                false,
            ),
            span(wording::temp(baseline), true),
            span(". Ces 30 derniers jours : ", false),
            span(wording::temp(baseline + delta), true),
            span(format!(", soit {outcome}"), false),
        ],
    }];
    lines.extend(season_line(pairs));
    lines.extend(peak_line(t));
    if verdict == Verdict::Warmer {
        lines.push(Line {
            kind: "advice",
            spans: vec![span(warmer_advice(delta, machine), false)],
        });
    }
    lines
}

pub fn build(facts: &Facts, hardware: &str, sensor: &str, load: &str) -> SensorPage {
    let stats = facts
        .groups
        .iter()
        .find(|(h, s, _)| h == hardware && s == sensor)
        .map_or(&[][..], |(_, _, d)| d.as_slice());
    let trend = trends::compute_trend(stats, facts.today);

    let mut all = facts.seen.clone();
    all.extend(
        facts
            .summaries
            .iter()
            .map(|s| (s.hardware.clone(), s.sensor.clone())),
    );
    all.push((hardware.to_string(), sensor.to_string()));
    let list = components(&all);
    let found = list
        .iter()
        .enumerate()
        .find(|(_, c)| c.sensors.iter().any(|(h, s)| h == hardware && s == sensor));
    let kind = found.map(|(_, c)| c.kind);
    let (name, tech) = sensor_name(
        sensor,
        kind.unwrap_or(crate::domain::components::Kind::Other),
    );

    let mut subtitle = Vec::new();
    if let Some((_, c)) = found {
        subtitle.push(c.title.clone());
        subtitle.extend(c.subtitle.clone());
    }
    subtitle.push(load_choice(load).label.to_lowercase());
    if let Some(tech) = tech {
        subtitle.push(format!("capteur « {tech} »"));
    }

    let months = trends::monthly(&trend.points);
    let seasons = trends::year_over_year(&months);
    let sufficient = trend.baseline.is_some();
    let technical = match (sufficient, trend.slope_per_month) {
        (true, Some(slope)) => vec![
            Fact {
                label: "Pente générale",
                text: format!(
                    "{} par mois (droite des moindres carrés sur toutes les journées)",
                    wording::signed(slope)
                ),
            },
            Fact {
                label: "Pointes de la journée",
                text: match (trend.peak_baseline, trend.peak_current, trend.peak_delta) {
                    (Some(b), Some(c), Some(d)) => format!(
                        "{} au début, {} ces 30 derniers jours, soit {}. (p95 : la journée est plus chaude que cette valeur 5 % du temps.)",
                        wording::temp(b),
                        wording::temp(c),
                        wording::signed(d)
                    ),
                    _ => "indéterminées".into(),
                },
            },
            Fact {
                label: "Marge d'incertitude",
                text: match trend.margin {
                    Some(m) => format!(
                        "{} sur l'écart (95 %, d'après la dispersion des médianes quotidiennes des deux périodes). Un écart plus petit que cette marge n'est pas annoncé.",
                        wording::margin(m)
                    ),
                    None => "indéterminée".into(),
                },
            },
            Fact {
                label: "Jours avec mesures",
                text: format!("{} sur {} jours de suivi", trend.days, trend.span_days),
            },
            Fact {
                label: "Niveau du début",
                text: "médiane des 30 premiers jours avec mesures".into(),
            },
            Fact {
                label: "Température récente",
                text: "médiane des 30 derniers jours calendaires".into(),
            },
        ],
        _ => vec![],
    };

    let mut legend = Vec::new();
    if !trend.points.is_empty() {
        legend.push("ligne pleine = température habituelle sur 30 jours");
        if sufficient {
            legend.push("tirets = niveau du début");
        }
    }
    SensorPage {
        name,
        subtitle: subtitle.join(" · "),
        slot: found.map(|(i, _)| i),
        story: story(&trend, facts.machine_drift(), &seasons),
        rolling: trend
            .rolling
            .iter()
            .map(|r| CurvePoint {
                day: r.day.clone(),
                value: r.value,
            })
            .collect(),
        reference: trend.baseline.map(|b| Reference {
            value: b,
            label: format!("Au début : {}", wording::temp(b)),
        }),
        legend,
        months: months
            .iter()
            .map(|m| MonthDetail {
                label: wording::month_year(&m.first_day),
                days: m.days,
                usual: wording::temp(m.median),
                lowest: wording::temp(m.min),
                highest: wording::temp(m.max),
            })
            .collect(),
        seasons: seasons
            .iter()
            .map(|p| SeasonRow {
                label: wording::month_year(&p.first_day),
                before: wording::temp(p.previous_median),
                after: wording::temp(p.median),
                change: wording::signed(p.delta),
            })
            .collect(),
        technical,
    }
}
