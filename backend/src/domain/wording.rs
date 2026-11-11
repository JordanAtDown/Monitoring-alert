//! How the dashboard talks: plain French, no statistics. The thresholds that turn a number into a
//! word ("stable", "plus chaud") live here, next to the words.

use serde::Serialize;

/// Below this change (°C) a component is called "stable": most sensors only report whole degrees.
pub const STABLE_BAND: f64 = 1.5;

/// How large a change has to be before it is called anything: at least the resolution of the
/// sensors, and more when the day-to-day scatter of this particular series makes the comparison
/// less certain than that.
pub fn stable_band(margin: Option<f64>) -> f64 {
    STABLE_BAND.max(margin.unwrap_or(0.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Warmer,
    Stable,
    Cooler,
    Unknown,
}

/// `margin` is how much of the change could be noise; `None` leaves only the resolution floor.
pub fn verdict_of(delta: Option<f64>, margin: Option<f64>) -> Verdict {
    let band = stable_band(margin);
    match delta {
        None => Verdict::Unknown,
        Some(d) if d >= band => Verdict::Warmer,
        Some(d) if d <= -band => Verdict::Cooler,
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

/// What a change is put down to, once the drift shared by the whole machine is taken out.
/// This is the difference between "it is warmer in here" and "this part is in trouble".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Cause {
    /// It moved, but no more than everything else: the room, not the machine.
    Machine,
    /// It moved clearly more than the rest of the machine.
    Alone,
    /// Nothing moved, here or elsewhere.
    Nothing,
}

impl Cause {
    /// `excess` only gives the direction: a component can stand out by cooling down too.
    pub fn label(self, excess: f64) -> &'static str {
        match self {
            Cause::Alone if excess < 0.0 => "Plus froid que le reste",
            Cause::Alone => "Plus chaud que le reste",
            Cause::Machine => "Comme toute la machine",
            Cause::Nothing => "Rien n'a bougé",
        }
    }
}

/// Puts a change down to the machine or to the component itself. `delta` is its own change,
/// `excess` how much more it changed than the machine as a whole, `margin` the noise floor.
pub fn cause_of(delta: f64, excess: f64, margin: Option<f64>) -> Cause {
    let band = stable_band(margin);
    if excess.abs() >= band {
        Cause::Alone
    } else if delta.abs() >= band {
        Cause::Machine
    } else {
        Cause::Nothing
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

/// "0,31 °C/W": a thermal resistance, two decimals since it is a few tenths of a degree per watt.
pub fn resistance(v: f64) -> String {
    format!("{:.2} °C/W", v).replace('.', ",")
}

/// "1 293 tr/min".
pub fn rpm(v: f64) -> String {
    format!("{} tr/min", int(v.round() as i64))
}

/// "+6 %", "−12 %" (a real minus sign); no sign when it rounds to nothing.
pub fn relative(fraction: f64) -> String {
    let percent = (fraction * 100.0).round();
    let sign = if percent > 0.0 {
        "+"
    } else if percent < 0.0 {
        "−"
    } else {
        ""
    };
    format!("{sign}{} %", percent.abs())
}

/// "± 0,6 °C": how much of a change could be noise.
pub fn margin(v: f64) -> String {
    format!("± {}", temp(v))
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

/// "6 654" (a narrow no-break space between thousands, as in French).
pub fn int(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('\u{202f}');
        }
        out.push(c);
    }
    if n < 0 {
        out.insert(0, '-');
    }
    out
}

/// "11,6 Mo".
pub fn bytes(n: u64) -> String {
    let n = n as f64;
    if n < 1024.0 * 1024.0 {
        format!("{} Ko", one_decimal(n / 1024.0))
    } else if n < 1024.0 * 1024.0 * 1024.0 {
        format!("{} Mo", one_decimal(n / 1024.0 / 1024.0))
    } else {
        format!("{} Go", one_decimal(n / 1024.0 / 1024.0 / 1024.0))
    }
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
