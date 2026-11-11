//! Is the cooling of the processor getting worse?
//!
//! A temperature is the room, plus the effort asked of the machine, plus the state of the cooling.
//! Only the last one can be acted upon, and a temperature alone cannot isolate it. What can:
//!
//! * the **gap between the processor and the inside of the case, per watt the processor draws**.
//!   The room warming up moves both the processor and the case (the gap stays), a heavier use
//!   draws more watts (the division absorbs it), whereas a clogged heat sink or dried-out paste
//!   keeps more heat in the processor for the same watts. It is a thermal resistance, in °C/W;
//! * the **speed of the processor's fan**: it says whether the cooling is working harder to hold
//!   the same temperature.
//!
//! Neither tells dust from dried-out paste: a fan driven by temperature speeds up for both. What
//! does is cleaning first, which is free, and looking at whether the resistance comes back down.
//!
//! The inside of the case stands in for the room. It also warms up with what the machine
//! dissipates, so it is a proxy, not a measurement; it is consistent over time, which is what a
//! comparison with the first days needs.

use chrono::NaiveDate;
use std::collections::BTreeMap;

use crate::domain::components::{components, Kind};
use crate::domain::model::{ExtraKind, ExtraSeries};
use crate::domain::trends::{DayStat, Trend, MIN_DAY_SAMPLES};

/// Below this draw (W) the resistance is dominated by the resolution of the temperature sensors:
/// one degree over a few watts is a huge swing.
pub const MIN_POWER_W: f64 = 15.0;

/// How much the resistance or the fan speed has to change, relatively, before it is called a
/// change: a tenth. The sensors report whole degrees, which already makes a few per cent of
/// jitter on a resistance of a few tenths of a degree per watt. A noisier series needs more, as
/// for temperatures (its own margin, when larger).
pub const RELATIVE_BAND: f64 = 0.10;

/// The sensors the analysis reads, among everything the machine reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parts {
    /// The temperature of the processor: (hardware, sensor).
    pub cpu: (String, String),
    /// The temperature inside the case, which stands in for the room.
    pub case: Option<(String, String)>,
    /// What the processor draws.
    pub power: Option<(String, String)>,
    /// The fan that cools the processor.
    pub fan: Option<(String, String)>,
}

/// Finds the parts: `sensors` are the temperature sensors the machine reports, `extras` the fans
/// and power draws recorded. `None` when there is no processor.
pub fn find_parts(sensors: &[(String, String)], extras: &[ExtraSeries]) -> Option<Parts> {
    let list = components(sensors);
    let cpu = list.iter().find(|c| c.kind == Kind::Cpu)?.main.clone();
    let case = list
        .iter()
        .find(|c| c.kind == Kind::Case)
        .map(|c| c.main.clone());
    // The power of the whole package of that very processor, not of a core.
    let power = extras
        .iter()
        .find(|e| e.kind == ExtraKind::Power && e.hardware == cpu.0 && e.sensor.contains("Package"))
        .map(|e| (e.hardware.clone(), e.sensor.clone()));
    // The boards name it ("CPU Fan"); another fan says nothing about the processor.
    let fan = extras
        .iter()
        .find(|e| e.kind == ExtraKind::Fan && e.sensor.to_lowercase().contains("cpu"))
        .map(|e| (e.hardware.clone(), e.sensor.clone()));
    Some(Parts {
        cpu,
        case,
        power,
        fan,
    })
}

/// The daily thermal resistance of the processor, in °C/W: `(processor − case) / power`, from the
/// daily medians of the three. A day only counts when all three were measured long enough, the
/// processor drew enough for the division to mean something, and it was warmer than the case.
pub fn resistance_series(
    processor: &[DayStat],
    case: &[DayStat],
    power: &[DayStat],
) -> Vec<DayStat> {
    let by_day = |stats: &'_ [DayStat]| -> BTreeMap<NaiveDate, DayStat> {
        stats.iter().map(|s| (s.day, s.clone())).collect()
    };
    let (case, power) = (by_day(case), by_day(power));
    processor
        .iter()
        .filter_map(|cpu| {
            let (case, power) = (case.get(&cpu.day)?, power.get(&cpu.day)?);
            let n = cpu.n.min(case.n).min(power.n);
            let gap = cpu.median - case.median;
            if n < MIN_DAY_SAMPLES || power.median < MIN_POWER_W || gap <= 0.0 {
                return None;
            }
            let resistance = gap / power.median;
            Some(DayStat {
                day: cpu.day,
                n,
                min: resistance,
                median: resistance,
                p95: resistance,
                max: resistance,
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Up,
    Steady,
    Down,
}

/// How a series changed since the first days, relatively.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    pub baseline: f64,
    pub current: f64,
    /// `(current − baseline) / baseline`.
    pub fraction: f64,
    pub change: Change,
}

/// The relative change of a trend; `None` without enough history, with a stale series or with a
/// baseline of zero (a fan that never turned: nothing to compare with).
pub fn measured(trend: &Trend) -> Option<Measured> {
    if !(trend.sufficient && trend.fresh) {
        return None;
    }
    let (baseline, delta) = (trend.baseline?, trend.delta?);
    if baseline <= 0.0 {
        return None;
    }
    let band = RELATIVE_BAND.max(trend.margin.unwrap_or(0.0) / baseline);
    let fraction = delta / baseline;
    let change = if fraction >= band {
        Change::Up
    } else if fraction <= -band {
        Change::Down
    } else {
        Change::Steady
    };
    Some(Measured {
        baseline,
        current: baseline + delta,
        fraction,
        change,
    })
}
