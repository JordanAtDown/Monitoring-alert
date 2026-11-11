//! The "Courbes" page, ready to display: one line per component, in temperatures or as the change
//! since the beginning, plus the same figures month by month.

use serde::Serialize;
use std::collections::BTreeMap;

use crate::domain::components::sensor_name;
use crate::domain::dashboard::overview::components_with_data;
use crate::domain::dashboard::Facts;
use crate::domain::trends::{self, Month};
use crate::domain::wording;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The temperatures themselves.
    Temp,
    /// How many degrees more or less than at the beginning (0 = as at the beginning).
    Delta,
}

impl Mode {
    pub fn parse(text: &str) -> Option<Mode> {
        match text {
            "temp" => Some(Mode::Temp),
            "delta" => Some(Mode::Delta),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CurvePoint {
    pub day: String,
    pub value: f64,
}

/// A dashed horizontal line marking the beginning.
#[derive(Debug, Serialize)]
pub struct Reference {
    pub value: f64,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct CurveSeries {
    pub key: String,
    pub title: String,
    /// The sensor the line is drawn from ("Cœurs du processeur").
    pub sensor_name: String,
    /// Position among the components: the page gives a color per position.
    pub slot: usize,
    /// Its figure now, written for the legend.
    pub now: Option<String>,
    /// `false` when this mode cannot draw it (no measure, no beginning yet); `note` says why.
    pub available: bool,
    pub note: Option<String>,
    pub points: Vec<CurvePoint>,
    pub reference: Option<Reference>,
}

#[derive(Debug, Serialize)]
pub struct MonthRow {
    pub label: String,
    /// One cell per series, in the order of `series`.
    pub cells: Vec<Option<String>>,
}

#[derive(Debug, Serialize)]
pub struct Curves {
    pub mode: &'static str,
    /// One line to explain what the lines show.
    pub hint: String,
    pub series: Vec<CurveSeries>,
    pub months: Vec<MonthRow>,
}

fn month_table(per_series: &[Vec<Month>], delta: &[Option<f64>]) -> Vec<MonthRow> {
    let mut labels: BTreeMap<&str, &str> = BTreeMap::new();
    for months in per_series {
        for m in months {
            labels.insert(&m.key, &m.first_day);
        }
    }
    labels
        .into_iter()
        .map(|(key, first_day)| MonthRow {
            label: wording::month_year(first_day),
            cells: per_series
                .iter()
                .zip(delta)
                .map(|(months, baseline)| {
                    let m = months.iter().find(|m| m.key == key)?;
                    Some(match baseline {
                        Some(b) => wording::signed(m.median - b),
                        None => wording::temp(m.median),
                    })
                })
                .collect(),
        })
        .collect()
}

pub fn build(facts: &Facts, mode: Mode) -> Curves {
    let stats: BTreeMap<(&str, &str), _> = facts
        .groups
        .iter()
        .map(|(h, s, d)| ((h.as_str(), s.as_str()), d))
        .collect();

    let mut series = Vec::new();
    let mut months = Vec::new();
    let mut baselines = Vec::new();
    for (slot, c, rows) in components_with_data(&facts.seen, &facts.summaries) {
        let summary = rows[0];
        let trend = trends::compute_trend(
            stats
                .get(&(c.main.0.as_str(), c.main.1.as_str()))
                .map_or(&[][..], |d| d.as_slice()),
            facts.today,
        );
        let has_points = !trend.points.is_empty();
        let delta_mode = mode == Mode::Delta;
        let available = has_points && (!delta_mode || trend.baseline.is_some());
        let note = if !has_points {
            Some("pas de mesure".to_string())
        } else if !available {
            Some("pas assez de recul".to_string())
        } else if !trend.fresh {
            Some("plus de mesures récentes".to_string())
        } else {
            None
        };
        let base = if delta_mode { trend.baseline } else { None };
        let points = if available {
            trend
                .rolling
                .iter()
                .map(|r| CurvePoint {
                    day: r.day.clone(),
                    value: r.value - base.unwrap_or(0.0),
                })
                .collect()
        } else {
            Vec::new()
        };
        let reference = trend.baseline.map(|b| {
            if delta_mode {
                Reference {
                    value: 0.0,
                    label: "Au début".into(),
                }
            } else {
                Reference {
                    value: b,
                    label: format!("Au début : {}", wording::temp(b)),
                }
            }
        });
        let now = if delta_mode {
            summary.drift().map(wording::signed)
        } else {
            summary.current.filter(|_| summary.fresh).map(wording::temp)
        };
        series.push(CurveSeries {
            key: c.key.clone(),
            title: c.title.clone(),
            sensor_name: sensor_name(&c.main.1, c.kind).0,
            slot,
            now,
            available,
            note,
            points,
            reference,
        });
        months.push(trends::monthly(&trend.points));
        baselines.push(base);
    }

    Curves {
        mode: if mode == Mode::Delta { "delta" } else { "temp" },
        hint: match mode {
            Mode::Temp => "Température habituelle de chaque composant.".into(),
            Mode::Delta => "Combien de degrés de plus ou de moins qu'au début : 0 = comme au début. Pratique pour comparer des composants de températures différentes.".into(),
        },
        months: month_table(&months, &baselines),
        series,
    }
}
