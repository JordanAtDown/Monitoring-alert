//! Long-term drift analysis, computed from the daily aggregates.
//!
//! A sensor's temperature only means something when compared at an equivalent
//! load, so every series is (sensor, load category). From the daily medians we
//! derive:
//!
//! * a **baseline**: the median over the first [`BASELINE_DAYS`] days of data
//!   (the "as new" reference),
//! * a **rolling median** over the last [`ROLLING_WINDOW_DAYS`] calendar days,
//!   which smooths the day-to-day noise,
//! * the **drift** — rolling median minus baseline — and the **slope** of a
//!   least-squares line through all the daily medians, in °C per month.
//!
//! Nothing here raises an alert: the numbers are meant to be looked at.
//! There is no ambient-temperature sensor, so a hot summer can look like a
//! drift; the dashboard shows the months to make that easy to spot.

use chrono::NaiveDate;
use serde::Serialize;

/// Load categories, from the lightest to the heaviest.
pub const LOAD_CATEGORIES: [&str; 4] = ["idle", "light", "moderate", "heavy"];

pub fn is_load_category(load: &str) -> bool {
    LOAD_CATEGORIES.contains(&load)
}

/// Minimum readings (≈ 30 min at 5 min intervals) for a day to be taken into account.
pub const MIN_DAY_SAMPLES: i64 = 6;
/// Number of days of data the baseline is computed on.
pub const BASELINE_DAYS: usize = 30;
/// Width of the rolling-median window, in calendar days.
pub const ROLLING_WINDOW_DAYS: i64 = 30;
/// Minimum days inside the window for the rolling median to be defined.
pub const ROLLING_MIN_POINTS: usize = 10;
/// Minimum time span (first to last day) before a drift is reported.
pub const MIN_SPAN_DAYS: i64 = 60;
/// Minimum series with a known drift before a machine-wide drift is computed: the median of
/// one or two of them would say nothing about the machine as a whole.
pub const MIN_COMMON_SERIES: usize = 3;
/// Days without a measurement after which the latest rolling median stops standing for "the last
/// 30 days": past half the window, most of what it averages is not recent any more. It happens
/// whenever a load level is no longer reached — the day gaming stops, `heavy` goes quiet.
pub const STALE_AFTER_DAYS: i64 = 15;
/// Days per month used to express the slope.
const DAYS_PER_MONTH: f64 = 30.4375;
/// Scales a median absolute deviation into an estimate of the standard deviation.
const MAD_TO_SIGMA: f64 = 1.4826;
/// Standard error of a median, as a multiple of `σ / √n`, for a large sample.
const MEDIAN_SE_FACTOR: f64 = 1.2533;
/// Two-sided 95 % normal quantile.
const Z95: f64 = 1.96;
/// Maximum number of points of the sparkline given for each sensor.
const SPARKLINE_POINTS: usize = 60;

/// The aggregates of one day for one (sensor, load category).
#[derive(Debug, Clone, PartialEq)]
pub struct DayStat {
    pub day: NaiveDate,
    pub n: i64,
    pub min: f64,
    pub median: f64,
    pub p95: f64,
    pub max: f64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Point {
    pub day: String,
    pub median: f64,
    pub min: f64,
    pub p95: f64,
    pub max: f64,
    pub n: i64,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct RollingPoint {
    pub day: String,
    pub value: f64,
}

/// Everything needed to draw the drift chart of one sensor at one load.
#[derive(Debug, Serialize)]
pub struct Trend {
    /// Daily points (days with too few readings are left out).
    pub points: Vec<Point>,
    /// Rolling median of the daily medians.
    pub rolling: Vec<RollingPoint>,
    /// Number of days with data.
    pub days: usize,
    /// Days between the first and the last day with data (inclusive).
    pub span_days: i64,
    /// `false` while there is not enough history to talk about a drift.
    pub sufficient: bool,
    /// Median of the first days — only when `sufficient`.
    pub baseline: Option<f64>,
    /// First and last day the baseline was computed on.
    pub baseline_from: Option<String>,
    pub baseline_to: Option<String>,
    /// Latest rolling median minus baseline.
    pub delta: Option<f64>,
    /// Least-squares slope of the daily medians, °C per month.
    pub slope_per_month: Option<f64>,
    /// Value of that least-squares line on the first day with data, so a chart can
    /// draw it: `fit_start + slope_per_month / 30.4375 * days_since_first_day`.
    pub fit_start: Option<f64>,
    /// How much of `delta` could be noise: the 95 % margin on the comparison. A change smaller
    /// than this says nothing.
    pub margin: Option<f64>,
    /// The same comparison made on the **hot moments** of each day (the daily p95) instead of the
    /// usual temperature. A cooler that no longer keeps up shows there first: the median barely
    /// moves while the peaks climb.
    pub peak_baseline: Option<f64>,
    pub peak_current: Option<f64>,
    pub peak_delta: Option<f64>,
    pub peak_margin: Option<f64>,
    /// Last day with a measurement.
    pub last_day: Option<String>,
    /// Days since that day; `0` when it is today's.
    pub quiet_days: i64,
    /// `false` once the series has been quiet for more than [`STALE_AFTER_DAYS`]: its latest
    /// figure is still true, but it is not about the last 30 days any more.
    pub fresh: bool,
}

/// One row of the all-sensors overview.
#[derive(Debug, Serialize)]
pub struct SensorSummary {
    pub hardware: String,
    pub sensor: String,
    pub load_cat: String,
    pub days: usize,
    pub span_days: i64,
    pub sufficient: bool,
    pub baseline: Option<f64>,
    /// Latest rolling median.
    pub current: Option<f64>,
    pub delta: Option<f64>,
    pub slope_per_month: Option<f64>,
    /// Rolling median, down-sampled for a small inline chart.
    pub sparkline: Vec<f64>,
    /// Last day with a measurement.
    pub last_day: Option<String>,
    pub quiet_days: i64,
    /// `false` once the series went quiet: see [`Trend::fresh`].
    pub fresh: bool,
    /// See [`Trend::margin`].
    pub margin: Option<f64>,
}

impl SensorSummary {
    /// Its drift, only when there is enough history to stand behind it **and** it is still
    /// being measured: a figure from two months ago must not drive a verdict today, nor count
    /// towards the drift of the machine.
    pub fn drift(&self) -> Option<f64> {
        self.delta.filter(|_| self.sufficient && self.fresh)
    }
}

fn median_of(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.total_cmp(b));
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// Least-squares line through the points: `(slope, value at x = 0)`;
/// `None` when it is undefined.
fn least_squares_fit(xy: &[(f64, f64)]) -> Option<(f64, f64)> {
    if xy.len() < 2 {
        return None;
    }
    let n = xy.len() as f64;
    let mean_x = xy.iter().map(|p| p.0).sum::<f64>() / n;
    let mean_y = xy.iter().map(|p| p.1).sum::<f64>() / n;
    let var_x: f64 = xy.iter().map(|p| (p.0 - mean_x).powi(2)).sum();
    if var_x == 0.0 {
        return None;
    }
    let cov: f64 = xy.iter().map(|p| (p.0 - mean_x) * (p.1 - mean_y)).sum();
    let slope = cov / var_x;
    Some((slope, mean_y - slope * mean_x))
}

/// Robust spread of a sample: its median absolute deviation, scaled to estimate a standard
/// deviation. Robust because one freak day must not inflate the uncertainty of a whole month.
fn robust_sigma(values: &[f64]) -> f64 {
    let mut sample = values.to_vec();
    let centre = median_of(&mut sample);
    let mut deviations: Vec<f64> = values.iter().map(|v| (v - centre).abs()).collect();
    MAD_TO_SIGMA * median_of(&mut deviations)
}

/// Standard error of the median of a sample.
fn median_standard_error(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return f64::INFINITY;
    }
    MEDIAN_SE_FACTOR * robust_sigma(values) / (values.len() as f64).sqrt()
}

/// The 95 % margin on `median(recent) − median(baseline)`: how large a difference the day-to-day
/// scatter alone could produce.
///
/// The daily medians are treated as independent, which they are not quite — a warm spell lasts
/// several days — so this is a floor on the real uncertainty rather than the whole of it. It is
/// used to refuse a verdict, never to grant one, so under-estimating it stays on the safe side
/// of the resolution floor ([`crate::domain::wording::STABLE_BAND`]).
fn drift_margin(baseline: &[f64], recent: &[f64]) -> Option<f64> {
    let (a, b) = (
        median_standard_error(baseline),
        median_standard_error(recent),
    );
    let margin = Z95 * (a * a + b * b).sqrt();
    margin.is_finite().then_some(margin)
}

/// One daily figure over the last [`ROLLING_WINDOW_DAYS`] calendar days with data: the sample the
/// latest rolling value is computed from.
fn recent_window(points: &[DayStat], value: impl Fn(&DayStat) -> f64) -> Vec<f64> {
    let Some(last) = points.last() else {
        return Vec::new();
    };
    points
        .iter()
        .filter(|p| (last.day - p.day).num_days() < ROLLING_WINDOW_DAYS)
        .map(value)
        .collect()
}

fn rolling_median(points: &[DayStat]) -> Vec<(NaiveDate, f64)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (end, point) in points.iter().enumerate() {
        while (point.day - points[start].day).num_days() >= ROLLING_WINDOW_DAYS {
            start += 1;
        }
        let window = &points[start..=end];
        if window.len() >= ROLLING_MIN_POINTS {
            let mut medians: Vec<f64> = window.iter().map(|p| p.median).collect();
            out.push((point.day, median_of(&mut medians)));
        }
    }
    out
}

fn fmt_day(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

/// Computes the trend of one series. `stats` must be sorted by ascending day; `today` is the
/// day the figures are read on, which decides whether the series is still being measured.
pub fn compute_trend(stats: &[DayStat], today: NaiveDate) -> Trend {
    let points: Vec<DayStat> = stats
        .iter()
        .filter(|s| s.n >= MIN_DAY_SAMPLES)
        .cloned()
        .collect();

    let span_days = match (points.first(), points.last()) {
        (Some(first), Some(last)) => (last.day - first.day).num_days() + 1,
        _ => 0,
    };
    let rolling = rolling_median(&points);
    let sufficient =
        span_days >= MIN_SPAN_DAYS && points.len() >= BASELINE_DAYS + ROLLING_MIN_POINTS;

    let mut baseline = None;
    let mut baseline_from = None;
    let mut baseline_to = None;
    let mut delta = None;
    let mut margin = None;
    let mut peak_baseline = None;
    let mut peak_current = None;
    let mut peak_delta = None;
    let mut peak_margin = None;
    let mut slope_per_month = None;
    let mut fit_start = None;
    if sufficient {
        let reference = &points[..BASELINE_DAYS];
        let mut medians: Vec<f64> = reference.iter().map(|p| p.median).collect();
        let base = median_of(&mut medians);
        let recent = recent_window(&points, |p| p.median);
        margin = drift_margin(&medians, &recent);
        baseline = Some(base);

        // The same two windows, read on the high of each day.
        let mut peaks: Vec<f64> = reference.iter().map(|p| p.p95).collect();
        let recent_peaks = recent_window(&points, |p| p.p95);
        if !recent_peaks.is_empty() {
            let peak_base = median_of(&mut peaks);
            let mut recent_sorted = recent_peaks.clone();
            let peak_now = median_of(&mut recent_sorted);
            peak_baseline = Some(peak_base);
            peak_current = Some(peak_now);
            peak_delta = Some(peak_now - peak_base);
            peak_margin = drift_margin(&peaks, &recent_peaks);
        }
        baseline_from = Some(fmt_day(reference[0].day));
        baseline_to = Some(fmt_day(reference[BASELINE_DAYS - 1].day));
        delta = rolling.last().map(|(_, v)| v - base);

        let first_day = points[0].day;
        let xy: Vec<(f64, f64)> = points
            .iter()
            .map(|p| ((p.day - first_day).num_days() as f64, p.median))
            .collect();
        if let Some((slope, intercept)) = least_squares_fit(&xy) {
            slope_per_month = Some(slope * DAYS_PER_MONTH);
            fit_start = Some(intercept);
        }
    }

    let last_day = points.last().map(|p| p.day);
    let quiet_days = last_day.map_or(0, |d| (today - d).num_days());
    Trend {
        days: points.len(),
        span_days,
        margin,
        peak_baseline,
        peak_current,
        peak_delta,
        peak_margin,
        last_day: last_day.map(fmt_day),
        quiet_days,
        fresh: last_day.is_some() && quiet_days <= STALE_AFTER_DAYS,
        sufficient,
        baseline,
        baseline_from,
        baseline_to,
        delta,
        slope_per_month,
        fit_start,
        rolling: rolling
            .into_iter()
            .map(|(day, value)| RollingPoint {
                day: fmt_day(day),
                value,
            })
            .collect(),
        points: points
            .iter()
            .map(|p| Point {
                day: fmt_day(p.day),
                median: p.median,
                min: p.min,
                p95: p.p95,
                max: p.max,
                n: p.n,
            })
            .collect(),
    }
}

/// Picks at most [`SPARKLINE_POINTS`] evenly spread values, always keeping the last one.
fn downsample(values: &[f64]) -> Vec<f64> {
    if values.len() <= SPARKLINE_POINTS {
        return values.to_vec();
    }
    let stride = values.len().div_ceil(SPARKLINE_POINTS);
    let mut out: Vec<f64> = values.iter().rev().step_by(stride).copied().collect();
    out.reverse();
    out
}

/// Builds the overview row of one series.
pub fn summarize(
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    stats: &[DayStat],
    today: NaiveDate,
) -> SensorSummary {
    let trend = compute_trend(stats, today);
    let rolling: Vec<f64> = trend.rolling.iter().map(|p| p.value).collect();
    SensorSummary {
        hardware: hardware.to_string(),
        sensor: sensor.to_string(),
        load_cat: load_cat.to_string(),
        days: trend.days,
        span_days: trend.span_days,
        sufficient: trend.sufficient,
        baseline: trend.baseline,
        current: rolling.last().copied(),
        delta: trend.delta,
        slope_per_month: trend.slope_per_month,
        sparkline: downsample(&rolling),
        last_day: trend.last_day,
        quiet_days: trend.quiet_days,
        fresh: trend.fresh,
        margin: trend.margin,
    }
}

/// The drift the whole machine shares, as the median of the drifts it is given (one per
/// component). The room warming up moves every sensor at once, so this common movement is
/// what has to be taken out before a single component can be blamed.
///
/// `None` while fewer than [`MIN_COMMON_SERIES`] drifts are known.
pub fn common_drift(deltas: &[f64]) -> Option<f64> {
    if deltas.len() < MIN_COMMON_SERIES {
        return None;
    }
    let mut values = deltas.to_vec();
    Some(median_of(&mut values))
}

/// Orders an overview: series with a known drift first (largest first), then the others by name.
pub fn sort_summaries(summaries: &mut [SensorSummary]) {
    summaries.sort_by(|a, b| match (a.delta, b.delta) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => (&a.hardware, &a.sensor).cmp(&(&b.hardware, &b.sensor)),
    });
}

/// One calendar month of a sensor's daily points.
#[derive(Debug, PartialEq)]
pub struct Month {
    /// `YYYY-MM`.
    pub key: String,
    /// A day of that month (`YYYY-MM-DD`), to name it.
    pub first_day: String,
    /// Days with measurements.
    pub days: usize,
    /// Median of the daily medians.
    pub median: f64,
    pub min: f64,
    pub max: f64,
}

/// The daily points grouped by calendar month, oldest first: the table view of a chart.
pub fn monthly(points: &[Point]) -> Vec<Month> {
    let mut months: Vec<(String, Vec<&Point>)> = Vec::new();
    for p in points {
        let key = p.day.get(..7).unwrap_or(&p.day).to_string();
        match months.last_mut() {
            Some((k, list)) if *k == key => list.push(p),
            _ => months.push((key, vec![p])),
        }
    }
    months
        .into_iter()
        .map(|(key, list)| {
            let mut medians: Vec<f64> = list.iter().map(|p| p.median).collect();
            Month {
                key,
                first_day: list[0].day.clone(),
                days: list.len(),
                median: median_of(&mut medians),
                min: list.iter().map(|p| p.min).fold(f64::INFINITY, f64::min),
                max: list.iter().map(|p| p.max).fold(f64::NEG_INFINITY, f64::max),
            }
        })
        .collect()
}
