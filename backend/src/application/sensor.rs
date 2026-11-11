//! The page of one sensor, ready to display: what its numbers mean in a sentence, its curve, its
//! figures month by month.

use anyhow::Result;
use serde::Serialize;

use crate::application::curves::{CurvePoint, Reference};
use crate::application::overview::{context, load_choice};
use crate::application::ports::ReadModel;
use crate::domain::components::{components, sensor_name};
use crate::domain::trends::{self, MIN_SPAN_DAYS};
use crate::domain::wording::{self, verdict_of, Verdict};

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
    /// For those who want the statistics.
    pub technical: Vec<Fact>,
}

fn span(text: impl Into<String>, strong: bool) -> Span {
    Span {
        text: text.into(),
        strong,
    }
}

fn story(t: &trends::Trend) -> Vec<Line> {
    let plain = |text: String| Line {
        kind: "story",
        spans: vec![span(text, false)],
    };
    if t.points.is_empty() {
        return vec![plain(
            "Pas encore de journée complète avec assez de mesures pour ce capteur à ce niveau de charge.".into(),
        )];
    }
    let (Some(baseline), Some(delta)) = (t.baseline, t.delta) else {
        return vec![plain(format!(
            "Pour l'instant : {} jours de mesures sur {} jours de suivi. C'est trop court pour dire si la température a changé (il faut au moins {MIN_SPAN_DAYS} jours de suivi, dont 40 avec des mesures). La courbe est déjà tracée.",
            t.days, t.span_days
        ))];
    };
    let verdict = verdict_of(Some(delta));
    let gap = wording::temp(delta.abs());
    let outcome = match verdict {
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
    if verdict == Verdict::Warmer {
        lines.push(Line {
            kind: "advice",
            spans: vec![span(
                "Si d'autres composants ont aussi chauffé, la chaleur de la saison peut l'expliquer. Si seul celui-ci chauffe, pensez à la poussière, à la pâte thermique ou à un ventilateur.",
                false,
            )],
        });
    }
    lines
}

pub fn sensor_page(
    read: &dyn ReadModel,
    hardware: &str,
    sensor: &str,
    load: &str,
) -> Result<SensorPage> {
    let ctx = context(read, load)?;
    let stats = ctx
        .groups
        .iter()
        .find(|(h, s, _)| h == hardware && s == sensor)
        .map_or(&[][..], |(_, _, d)| d.as_slice());
    let trend = trends::compute_trend(stats);

    let mut all = ctx.seen.clone();
    all.extend(
        ctx.summaries
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
    Ok(SensorPage {
        name,
        subtitle: subtitle.join(" · "),
        slot: found.map(|(i, _)| i),
        story: story(&trend),
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
        months: trends::monthly(&trend.points)
            .into_iter()
            .map(|m| MonthDetail {
                label: wording::month_year(&m.first_day),
                days: m.days,
                usual: wording::temp(m.median),
                lowest: wording::temp(m.min),
                highest: wording::temp(m.max),
            })
            .collect(),
        technical,
    })
}
