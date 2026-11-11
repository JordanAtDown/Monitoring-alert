//! Load levels: temperatures are only comparable at an equivalent load.

use chrono::NaiveDateTime;

/// Classifies a load percentage as `idle`, `light`, `moderate` or `heavy`.
pub fn load_category(load: f64) -> &'static str {
    match load as u32 {
        0..=14 => "idle",
        15..=39 => "light",
        40..=74 => "moderate",
        75..=100 => "heavy",
        _ => "heavy",
    }
}

/// Returns the effective load to use for categorisation.
///
/// Takes the **maximum** of CPU and GPU load so that GPU-intensive workloads
/// (e.g. gaming: GPU 90 %, CPU 15 %) are classified as `heavy` rather than
/// `light`. This ensures GPU temperatures recorded during gaming sessions are
/// compared against other gaming sessions, not against idle-CPU periods.
pub fn effective_load(cpu: Option<f64>, gpu: Option<f64>) -> Option<f64> {
    [cpu, gpu].into_iter().flatten().reduce(f64::max)
}

/// The category of a reading taken while the machine is idle but still hot from an effort. It is
/// not offered on the dashboard: it exists to keep those readings **out** of the idle series.
pub const COOLDOWN: &str = "cooldown";

/// How long after an effort the machine is considered to still be cooling down. A heat sink and
/// the air in a case take minutes to settle, not seconds: five minutes after a game, the load
/// reads 3 % while the processor is still 8 °C above its resting temperature. This is an
/// estimate of the settling time, not a measured constant.
pub const COOLDOWN_SECS: i64 = 15 * 60;

/// Whether a load category leaves the machine hot enough to need time to cool down. A light load
/// (15–40 %) barely moves the temperature away from idle, so it does not.
pub fn heats_up(category: &str) -> bool {
    matches!(category, "moderate" | "heavy")
}

/// Remembers the last time the machine was under real load, to tell a **resting** machine from
/// one that merely stopped working a moment ago. Temperatures are only comparable at an
/// equivalent state, and the load read at the instant of the measurement says nothing about the
/// heat that is still being evacuated.
///
/// Lives in memory: after a restart the first readings are classified on their own load only,
/// which is right in practice since the program starts with the session.
#[derive(Debug, Default)]
pub struct LoadTracker {
    last_hot: Option<NaiveDateTime>,
}

impl LoadTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// The category of a reading taken `now` under `load`.
    pub fn classify(&mut self, now: NaiveDateTime, load: Option<f64>) -> &'static str {
        let category = load.map_or("idle", load_category);
        if heats_up(category) {
            self.last_hot = Some(now);
            return category;
        }
        let still_hot = self
            .last_hot
            .is_some_and(|hot| (now - hot).num_seconds() <= COOLDOWN_SECS);
        if category == "idle" && still_hot {
            COOLDOWN
        } else {
            category
        }
    }
}
