use crate::trends::{self, DayStat};
use chrono::NaiveDate;

fn day(n: i64) -> NaiveDate {
    NaiveDate::from_ymd_opt(2024, 1, 1).unwrap() + chrono::Duration::days(n)
}

/// One stat per day for `days` consecutive days, the daily median given by `f(day_index)`.
fn series(days: i64, f: impl Fn(i64) -> f64) -> Vec<DayStat> {
    (0..days)
        .map(|i| {
            let m = f(i);
            DayStat {
                day: day(i),
                n: 288,
                min: m - 2.0,
                median: m,
                p95: m + 3.0,
                max: m + 5.0,
            }
        })
        .collect()
}

#[test]
fn flat_series_has_no_drift() {
    let t = trends::compute_trend(&series(200, |_| 40.0));
    assert!(t.sufficient);
    assert_eq!(t.baseline, Some(40.0));
    assert!(t.delta.unwrap().abs() < 1e-9);
    assert!(t.slope_per_month.unwrap().abs() < 1e-9);
}

#[test]
fn steady_drift_gives_the_slope_and_the_delta() {
    // +0.1 °C per day starting at 40 °C.
    let t = trends::compute_trend(&series(180, |i| 40.0 + 0.1 * i as f64));
    assert!(t.sufficient);

    // 0.1 °C/day ≈ 3.04 °C/month
    let slope = t.slope_per_month.unwrap();
    assert!((slope - 3.04375).abs() < 1e-6, "slope = {slope}");

    // Baseline = median of days 0..30 = 40 + 0.1 * 14.5 ; current = median of days 150..180.
    assert!((t.baseline.unwrap() - 41.45).abs() < 1e-9);
    let delta = t.delta.unwrap();
    assert!((delta - 0.1 * 150.0).abs() < 1e-9, "delta = {delta}");
}

#[test]
fn fit_start_lets_a_chart_draw_the_trend_line() {
    // y = 40 + 0.1 * day: the line starts at 40 on the first day.
    let t = trends::compute_trend(&series(180, |i| 40.0 + 0.1 * i as f64));
    assert!((t.fit_start.unwrap() - 40.0).abs() < 1e-9);
    let slope_per_day = t.slope_per_month.unwrap() / 30.4375;
    assert!((t.fit_start.unwrap() + slope_per_day * 100.0 - 50.0).abs() < 1e-9);
}

#[test]
fn cooling_gives_a_negative_drift() {
    let t = trends::compute_trend(&series(120, |i| 50.0 - 0.05 * i as f64));
    assert!(t.slope_per_month.unwrap() < 0.0);
    assert!(t.delta.unwrap() < 0.0);
}

#[test]
fn baseline_window_is_the_first_thirty_days() {
    let t = trends::compute_trend(&series(120, |i| if i < 30 { 40.0 } else { 45.0 }));
    assert_eq!(t.baseline, Some(40.0));
    assert_eq!(t.baseline_from.as_deref(), Some("2024-01-01"));
    assert_eq!(t.baseline_to.as_deref(), Some("2024-01-30"));
    assert!((t.delta.unwrap() - 5.0).abs() < 1e-9);
}

#[test]
fn not_enough_history_reports_no_drift_but_keeps_the_points() {
    let t = trends::compute_trend(&series(45, |i| 40.0 + i as f64));
    assert!(!t.sufficient);
    assert_eq!(t.span_days, 45);
    assert_eq!(t.baseline, None);
    assert_eq!(t.delta, None);
    assert_eq!(t.slope_per_month, None);
    assert_eq!(t.fit_start, None);
    assert_eq!(t.points.len(), 45);
    assert!(!t.rolling.is_empty());
}

#[test]
fn history_is_sufficient_from_sixty_days() {
    assert!(!trends::compute_trend(&series(59, |_| 40.0)).sufficient);
    assert!(trends::compute_trend(&series(60, |_| 40.0)).sufficient);
}

#[test]
fn sparse_series_over_a_long_span_needs_enough_days_with_data() {
    // 60-day span but only every 5th day (12 days): not enough to build a baseline.
    let stats: Vec<DayStat> = series(60, |_| 40.0)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 5 == 0)
        .map(|(_, s)| s)
        .collect();
    let t = trends::compute_trend(&stats);
    assert_eq!(t.span_days, 56);
    assert!(!t.sufficient);
}

#[test]
fn days_with_too_few_readings_are_ignored() {
    let mut stats = series(10, |_| 40.0);
    stats[3].n = trends::MIN_DAY_SAMPLES - 1;
    let t = trends::compute_trend(&stats);
    assert_eq!(t.days, 9);
    assert!(t.points.iter().all(|p| p.day != "2024-01-04"));
}

#[test]
fn empty_series_is_harmless() {
    let t = trends::compute_trend(&[]);
    assert_eq!((t.days, t.span_days), (0, 0));
    assert!(!t.sufficient);
    assert!(t.points.is_empty() && t.rolling.is_empty());
}

#[test]
fn rolling_median_needs_ten_days_in_its_window() {
    let t = trends::compute_trend(&series(40, |_| 40.0));
    // First defined on the 10th day of data.
    assert_eq!(t.rolling.first().unwrap().day, "2024-01-10");
    assert_eq!(t.rolling.len(), 31);
}

#[test]
fn rolling_median_uses_calendar_days_not_a_count_of_points() {
    // Days 0..9 at 40 °C, then a gap, then 10 days at 60 °C around day 100.
    let mut stats = series(10, |_| 40.0);
    stats.extend(series(110, |_| 60.0).into_iter().skip(100));
    let t = trends::compute_trend(&stats);
    let last = t.rolling.last().unwrap();
    assert_eq!(
        last.value, 60.0,
        "old days outside the 30-day window must not leak in"
    );
}

#[test]
fn rolling_median_ignores_a_single_outlier_day() {
    let t = trends::compute_trend(&series(60, |i| if i == 45 { 90.0 } else { 40.0 }));
    assert!(t.rolling.iter().all(|p| p.value == 40.0));
}

#[test]
fn summary_exposes_current_value_delta_and_a_bounded_sparkline() {
    let s = trends::summarize(
        "CPU",
        "Package",
        "idle",
        &series(400, |i| 40.0 + 0.02 * i as f64),
    );
    assert_eq!(
        (s.hardware.as_str(), s.sensor.as_str(), s.load_cat.as_str()),
        ("CPU", "Package", "idle")
    );
    assert!(s.sufficient);
    assert!(s.sparkline.len() <= 60);
    // The sparkline always ends on the current value.
    assert_eq!(*s.sparkline.last().unwrap(), s.current.unwrap());
    assert!(s.delta.unwrap() > 6.0);
}

#[test]
fn summaries_are_sorted_by_drift_then_unknowns_by_name() {
    let mk = |sensor: &str, f: fn(i64) -> f64, days: i64| {
        trends::summarize("CPU", sensor, "idle", &series(days, f))
    };
    let mut v = vec![
        mk("B young", |_| 40.0, 20),
        mk("C small", |i| 40.0 + 0.01 * i as f64, 120),
        mk("A young", |_| 40.0, 20),
        mk("D big", |i| 40.0 + 0.1 * i as f64, 120),
    ];
    trends::sort_summaries(&mut v);
    let order: Vec<&str> = v.iter().map(|s| s.sensor.as_str()).collect();
    assert_eq!(order, ["D big", "C small", "A young", "B young"]);
}
