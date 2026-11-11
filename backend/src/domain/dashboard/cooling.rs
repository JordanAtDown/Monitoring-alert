//! The cooling card, ready to display: is the processor's cooling getting worse, in a sentence,
//! with the two figures it rests on (see [`crate::domain::cooling`]).

use serde::Serialize;

use crate::domain::cooling::{
    find_parts, measured, resistance_series, Change, Measured, Parts, MIN_POWER_W,
};
use crate::domain::dashboard::Facts;
use crate::domain::model::{ExtraKind, ExtraSeries};
use crate::domain::trends::{compute_trend, DayStat, Trend, MIN_SPAN_DAYS};
use crate::domain::wording::{self, load_words};

/// One of the two figures, with how it changed.
#[derive(Debug, Serialize)]
pub struct CoolingRow {
    pub label: &'static str,
    /// What it means, in a few words.
    pub hint: &'static str,
    pub baseline: String,
    pub current: String,
    /// "+6 %".
    pub change: String,
}

#[derive(Debug, Serialize)]
pub struct Cooling {
    /// `none` (nothing recorded to talk about), `few` (not enough history) or `ok`.
    pub state: &'static str,
    /// `worse`, `watch`, `better`, `same` or `unknown`: what the page says about the cooling.
    pub verdict: &'static str,
    pub lead: String,
    pub detail: Option<String>,
    pub rows: Vec<CoolingRow>,
    /// How the figures are obtained, for those who want to know.
    pub how: String,
    /// First day a fan speed or power draw was recorded.
    pub since: Option<String>,
}

const HOW: &str = "L'efficacité du refroidissement est l'écart entre la température du processeur et celle de l'intérieur du boîtier, divisé par la puissance que le processeur consomme (en °C par watt) : la pièce qui chauffe monte les deux températures et laisse l'écart, un usage plus lourd fait consommer plus de watts et la division le compense, alors qu'un radiateur encrassé ou une pâte thermique sèche gardent plus de chaleur dans le processeur pour les mêmes watts. L'intérieur du boîtier sert de repère pour la pièce : il chauffe aussi un peu avec ce que le PC dissipe, c'est donc une approximation, mais elle est la même d'un mois à l'autre, ce qui suffit pour comparer. La vitesse du ventilateur dit si le refroidissement travaille plus pour tenir la même température.";

fn stats_of<'a>(
    extras: &'a [ExtraSeries],
    key: &(String, String),
    kind: ExtraKind,
) -> &'a [DayStat] {
    extras
        .iter()
        .find(|e| e.kind == kind && e.hardware == key.0 && e.sensor == key.1)
        .map_or(&[][..], |e| e.stats.as_slice())
}

fn temperature_stats<'a>(facts: &'a Facts, key: &(String, String)) -> &'a [DayStat] {
    facts
        .groups
        .iter()
        .find(|(h, s, _)| *h == key.0 && *s == key.1)
        .map_or(&[][..], |(_, _, d)| d.as_slice())
}

fn none(lead: &str, detail: Option<String>) -> Cooling {
    Cooling {
        state: "none",
        verdict: "unknown",
        lead: lead.into(),
        detail,
        rows: vec![],
        how: HOW.into(),
        since: None,
    }
}

/// What is not there yet, in words, to explain why nothing can be said.
fn missing(parts: &Parts) -> Vec<&'static str> {
    let mut out = Vec::new();
    if parts.case.is_none() {
        out.push("la sonde de température de l'intérieur du boîtier");
    }
    if parts.power.is_none() {
        out.push("la puissance du processeur (capteur « Package »)");
    }
    if parts.fan.is_none() {
        out.push("la vitesse du ventilateur du processeur (capteur « CPU Fan »)");
    }
    out
}

fn row_resistance(m: &Measured) -> CoolingRow {
    CoolingRow {
        label: "Chaleur gardée par le processeur",
        hint: "écart avec l'intérieur du boîtier, par watt consommé : plus c'est bas, mieux il est refroidi",
        baseline: wording::resistance(m.baseline),
        current: wording::resistance(m.current),
        change: wording::relative(m.fraction),
    }
}

fn row_fan(m: &Measured) -> CoolingRow {
    CoolingRow {
        label: "Ventilateur du processeur",
        hint: "vitesse habituelle",
        baseline: wording::rpm(m.baseline),
        current: wording::rpm(m.current),
        change: wording::relative(m.fraction),
    }
}

/// What the fan does, as a half-sentence to follow the finding about the resistance.
fn fan_sentence(fan: Option<&Measured>, has_fan: bool) -> String {
    match fan {
        Some(f) if f.change == Change::Up => format!(
            "Le ventilateur tourne {} plus vite : le système compense, ce qui masque encore la hausse de température mais fait plus de bruit.",
            wording::relative(f.fraction).trim_start_matches('+')
        ),
        Some(_) => "Le ventilateur ne tourne pas plus vite qu'au début : il ne compense pas.".into(),
        None if has_fan => "On n'a pas encore assez de recul sur la vitesse du ventilateur.".into(),
        None => "Aucun ventilateur « CPU » n'est reconnu : sa vitesse n'est pas suivie.".into(),
    }
}

/// The cooling card at one load level.
pub fn build(facts: &Facts, extras: &[ExtraSeries], load: &str) -> Cooling {
    if extras.is_empty() {
        return none(
            "Le refroidissement n'est pas encore suivi à ce niveau de charge.",
            Some("Le programme enregistre la vitesse des ventilateurs et la puissance du processeur depuis la version 3 ; rien n'est encore arrivé pour ce niveau de charge.".into()),
        );
    }

    let mut sensors = facts.seen.clone();
    sensors.extend(facts.groups.iter().map(|(h, s, _)| (h.clone(), s.clone())));
    let Some(parts) = find_parts(&sensors, extras) else {
        return none(
            "Aucun processeur reconnu : le refroidissement n'est pas suivi.",
            None,
        );
    };

    let since = extras
        .iter()
        .filter_map(|e| e.stats.first())
        .map(|d| d.day)
        .min()
        .map(|d| d.format("%Y-%m-%d").to_string());

    // The thermal resistance, when the three it needs are there.
    let resistance_trend: Option<Trend> = match (&parts.case, &parts.power) {
        (Some(case), Some(power)) => {
            let series = resistance_series(
                temperature_stats(facts, &parts.cpu),
                temperature_stats(facts, case),
                stats_of(extras, power, ExtraKind::Power),
            );
            Some(compute_trend(&series, facts.today))
        }
        _ => None,
    };
    let fan_trend: Option<Trend> = parts
        .fan
        .as_ref()
        .map(|fan| compute_trend(stats_of(extras, fan, ExtraKind::Fan), facts.today));

    let resistance = resistance_trend.as_ref().and_then(measured);
    let fan = fan_trend.as_ref().and_then(measured);

    // Nothing can be said yet: how much there is, and what is missing.
    let few = || {
        let days = [&resistance_trend, &fan_trend]
            .iter()
            .filter_map(|t| t.as_ref().map(|t| t.days))
            .max()
            .unwrap_or(0);
        let when = load_words(load).map_or("", |l| l.when);
        let mut detail = format!(
            "Pour dire si le refroidissement a changé, il faut au moins {MIN_SPAN_DAYS} jours de suivi dont 40 jours avec des mesures {when}. Pour l'instant : {days} jour{} de mesures{}.",
            if days > 1 { "s" } else { "" },
            since
                .as_deref()
                .map_or(String::new(), |d| format!(", depuis le {}", wording::date(d)))
        );
        let absent = missing(&parts);
        if !absent.is_empty() {
            detail.push_str(&format!(" Il manque aussi : {}.", absent.join(", ")));
        }
        Cooling {
            state: "few",
            verdict: "unknown",
            lead: "Pas encore assez d'historique pour juger le refroidissement.".into(),
            detail: Some(detail),
            rows: vec![],
            how: HOW.into(),
            since: since.clone(),
        }
    };

    let has_fan = parts.fan.is_some();
    let (verdict, lead, detail) = match (resistance.as_ref(), fan.as_ref()) {
        (Some(r), f) if r.change == Change::Up => (
            "worse",
            format!(
                "Le processeur garde {} de chaleur en plus qu'au début, pour la même puissance : son refroidissement est moins efficace.",
                wording::relative(r.fraction).trim_start_matches('+')
            ),
            format!(
                "{} Dans cet ordre : nettoyer le radiateur, les ventilateurs et les filtres, ce qui ne coûte rien ; si l'efficacité ne revient pas après, c'est la pâte thermique. La vitesse du ventilateur ne départage pas les deux : un ventilateur piloté par la température accélère dans les deux cas.",
                fan_sentence(f, has_fan)
            ),
        ),
        (Some(r), _) if r.change == Change::Down => (
            "better",
            format!(
                "Le processeur garde {} de chaleur en moins qu'au début, pour la même puissance : son refroidissement est plus efficace.",
                wording::relative(r.fraction).trim_start_matches('−')
            ),
            "C'est ce qu'on voit après un nettoyage ou un changement de pâte thermique.".into(),
        ),
        (Some(_), f) => match f {
            Some(f) if f.change == Change::Up => (
                "watch",
                format!(
                    "Le refroidissement tient, mais le ventilateur du processeur tourne {} plus vite qu'au début.",
                    wording::relative(f.fraction).trim_start_matches('+')
                ),
                "C'est souvent le premier signe d'un début d'encrassement ; une pièce plus chaude peut aussi l'expliquer. À surveiller.".into(),
            ),
            _ => (
                "same",
                "Le refroidissement du processeur n'a pas changé depuis le début.".into(),
                "L'écart avec l'intérieur du boîtier, par watt consommé, est resté le même.".into(),
            ),
        },
        (None, Some(f)) => (
            "unknown",
            match f.change {
                Change::Up => format!(
                    "Le ventilateur du processeur tourne {} plus vite qu'au début.",
                    wording::relative(f.fraction).trim_start_matches('+')
                ),
                Change::Down => format!(
                    "Le ventilateur du processeur tourne {} moins vite qu'au début.",
                    wording::relative(f.fraction).trim_start_matches('−')
                ),
                Change::Steady => "Le ventilateur du processeur tourne comme au début.".into(),
            },
            format!(
                "Sans l'efficacité du refroidissement, on ne peut pas dire si c'est le refroidissement ou la pièce.{}",
                {
                    let absent = missing(&parts);
                    if absent.is_empty() {
                        String::new()
                    } else {
                        format!(" Il manque : {}.", absent.join(", "))
                    }
                }
            ),
        ),
        (None, None) => return few(),
    };

    let mut rows = Vec::new();
    rows.extend(resistance.as_ref().map(row_resistance));
    rows.extend(fan.as_ref().map(row_fan));
    Cooling {
        state: "ok",
        verdict,
        lead,
        detail: Some(detail),
        rows,
        how: format!("{HOW} Les journées où le processeur consomme moins de {MIN_POWER_W:.0} W sont écartées : à si peu de watts, un degré de résolution fausse le calcul."),
        since,
    }
}
