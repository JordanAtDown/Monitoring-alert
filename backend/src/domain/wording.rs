//! How the dashboard talks: plain French, no statistics. The thresholds that turn a number into a
//! word ("stable", "plus chaud") live here, next to the words.

use serde::Serialize;

/// Below this change (°C) a component is called "stable": most sensors only report whole degrees.
pub const STABLE_BAND: f64 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Warmer,
    Stable,
    Cooler,
    Unknown,
}

pub fn verdict_of(delta: Option<f64>) -> Verdict {
    match delta {
        None => Verdict::Unknown,
        Some(d) if d >= STABLE_BAND => Verdict::Warmer,
        Some(d) if d <= -STABLE_BAND => Verdict::Cooler,
        Some(_) => Verdict::Stable,
    }
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Warmer => "Plus chaud",
            Verdict::Stable => "Stable",
            Verdict::Cooler => "Plus froid",
            Verdict::Unknown => "Pas assez de recul",
        }
    }
}

/// A load level, in words.
pub struct LoadWords {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    /// To complete a sentence: "quand le PC est au repos".
    pub when: &'static str,
}

pub const LOADS: [LoadWords; 4] = [
    LoadWords {
        id: "idle",
        label: "Au repos",
        hint: "moins de 15 % d'utilisation (bureau, navigation, veille)",
        when: "quand le PC est au repos",
    },
    LoadWords {
        id: "light",
        label: "Charge légère",
        hint: "15 à 40 % d'utilisation (vidéo, tâches courantes)",
        when: "quand le PC est peu sollicité",
    },
    LoadWords {
        id: "moderate",
        label: "Charge moyenne",
        hint: "40 à 75 % d'utilisation",
        when: "quand le PC est moyennement sollicité",
    },
    LoadWords {
        id: "heavy",
        label: "Charge élevée",
        hint: "plus de 75 % d'utilisation (jeux, calculs lourds)",
        when: "quand le PC est très sollicité",
    },
];

pub fn load_words(id: &str) -> Option<&'static LoadWords> {
    LOADS.iter().find(|l| l.id == id)
}

// ── Numbers and dates ────────────────────────────────────────────────────────

fn one_decimal(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

/// "42,3 °C".
pub fn temp(v: f64) -> String {
    format!("{} °C", one_decimal(v))
}

/// "+4,7 °C", "−1,1 °C" (a real minus sign); no sign when it rounds to nothing.
pub fn signed(v: f64) -> String {
    let sign = if v >= 0.05 {
        "+"
    } else if v <= -0.05 {
        "−"
    } else {
        ""
    };
    format!("{sign}{} °C", one_decimal(v.abs()))
}

const MONTHS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// "2025-09-17" → "17 septembre 2025"; anything else is given back unchanged.
pub fn date(day: &str) -> String {
    let mut parts = day.get(..10).unwrap_or(day).split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return day.to_string();
    };
    match (m.parse::<usize>(), d.parse::<u32>()) {
        (Ok(m), Ok(d)) if (1..=12).contains(&m) => format!("{d} {} {y}", MONTHS[m - 1]),
        _ => day.to_string(),
    }
}

/// "janvier 2025" for a "YYYY-MM-DD" day.
pub fn month_year(day: &str) -> String {
    let d = date(day);
    d.split_once(' ')
        .map_or(d.clone(), |(_, rest)| rest.to_string())
}

/// How long ago the history starts, in words ("environ 7 mois").
pub fn span_in_words(days: i64) -> String {
    if days < 45 {
        format!("{days} jours")
    } else if days < 365 {
        format!("environ {} mois", (days as f64 / 30.0).round() as i64)
    } else {
        let years = days as f64 / 365.0;
        format!(
            "environ {} an{}",
            one_decimal(years),
            if years >= 2.0 { "s" } else { "" }
        )
    }
}

pub fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n > 1 {
        many
    } else {
        one
    }
}
