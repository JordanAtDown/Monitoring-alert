use crate::domain::trends::{self, DayStat};
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

/// The day these tests read a series on: the day after its last, so that it still counts as
/// being measured (freshness is tested on its own, below).
fn read_day(stats: &[DayStat]) -> NaiveDate {
    stats
        .last()
        .map_or_else(|| day(0), |s| s.day + chrono::Duration::days(1))
}

fn trend_of(stats: &[DayStat]) -> trends::Trend {
    trends::compute_trend(stats, read_day(stats))
}

/// The overview row of a series, read the day after its last day.
fn summary_of(hardware: &str, sensor: &str, stats: &[DayStat]) -> trends::SensorSummary {
    trends::summarize(hardware, sensor, "idle", stats, read_day(stats))
}

#[test]
fn flat_series_has_no_drift() {
    let t = trend_of(&series(200, |_| 40.0));
    assert!(t.sufficient);
    assert_eq!(t.baseline, Some(40.0));
    assert!(t.delta.unwrap().abs() < 1e-9);
    assert!(t.slope_per_month.unwrap().abs() < 1e-9);
}

#[test]
fn steady_drift_gives_the_slope_and_the_delta() {
    // +0.1 °C per day starting at 40 °C.
    let t = trend_of(&series(180, |i| 40.0 + 0.1 * i as f64));
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
    let t = trend_of(&series(180, |i| 40.0 + 0.1 * i as f64));
    assert!((t.fit_start.unwrap() - 40.0).abs() < 1e-9);
    let slope_per_day = t.slope_per_month.unwrap() / 30.4375;
    assert!((t.fit_start.unwrap() + slope_per_day * 100.0 - 50.0).abs() < 1e-9);
}

#[test]
fn cooling_gives_a_negative_drift() {
    let t = trend_of(&series(120, |i| 50.0 - 0.05 * i as f64));
    assert!(t.slope_per_month.unwrap() < 0.0);
    assert!(t.delta.unwrap() < 0.0);
}

#[test]
fn baseline_window_is_the_first_thirty_days() {
    let t = trend_of(&series(120, |i| if i < 30 { 40.0 } else { 45.0 }));
    assert_eq!(t.baseline, Some(40.0));
    assert_eq!(t.baseline_from.as_deref(), Some("2024-01-01"));
    assert_eq!(t.baseline_to.as_deref(), Some("2024-01-30"));
    assert!((t.delta.unwrap() - 5.0).abs() < 1e-9);
}

#[test]
fn not_enough_history_reports_no_drift_but_keeps_the_points() {
    let t = trend_of(&series(45, |i| 40.0 + i as f64));
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
    assert!(!trend_of(&series(59, |_| 40.0)).sufficient);
    assert!(trend_of(&series(60, |_| 40.0)).sufficient);
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
    let t = trend_of(&stats);
    assert_eq!(t.span_days, 56);
    assert!(!t.sufficient);
}

#[test]
fn days_with_too_few_readings_are_ignored() {
    let mut stats = series(10, |_| 40.0);
    stats[3].n = trends::MIN_DAY_SAMPLES - 1;
    let t = trend_of(&stats);
    assert_eq!(t.days, 9);
    assert!(t.points.iter().all(|p| p.day != "2024-01-04"));
}

#[test]
fn empty_series_is_harmless() {
    let t = trend_of(&[]);
    assert_eq!((t.days, t.span_days), (0, 0));
    assert!(!t.sufficient);
    assert!(t.points.is_empty() && t.rolling.is_empty());
}

#[test]
fn rolling_median_needs_ten_days_in_its_window() {
    let t = trend_of(&series(40, |_| 40.0));
    // First defined on the 10th day of data.
    assert_eq!(t.rolling.first().unwrap().day, "2024-01-10");
    assert_eq!(t.rolling.len(), 31);
}

#[test]
fn rolling_median_uses_calendar_days_not_a_count_of_points() {
    // Days 0..9 at 40 °C, then a gap, then 10 days at 60 °C around day 100.
    let mut stats = series(10, |_| 40.0);
    stats.extend(series(110, |_| 60.0).into_iter().skip(100));
    let t = trend_of(&stats);
    let last = t.rolling.last().unwrap();
    assert_eq!(
        last.value, 60.0,
        "old days outside the 30-day window must not leak in"
    );
}

#[test]
fn rolling_median_ignores_a_single_outlier_day() {
    let t = trend_of(&series(60, |i| if i == 45 { 90.0 } else { 40.0 }));
    assert!(t.rolling.iter().all(|p| p.value == 40.0));
}

#[test]
fn summary_exposes_current_value_delta_and_a_bounded_sparkline() {
    let s = summary_of("CPU", "Package", &series(400, |i| 40.0 + 0.02 * i as f64));
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
fn the_common_drift_is_the_median_of_the_drifts_given() {
    assert_eq!(trends::common_drift(&[1.0, 5.0, 3.0]), Some(3.0));
    // A single component drifting far does not drag the machine with it.
    assert_eq!(trends::common_drift(&[0.5, 0.5, 0.5, 0.5, 9.0]), Some(0.5));
}

#[test]
fn the_common_drift_needs_at_least_three_components() {
    assert_eq!(trends::common_drift(&[]), None);
    assert_eq!(trends::common_drift(&[2.0]), None);
    assert_eq!(trends::common_drift(&[2.0, 4.0]), None);
    assert!(trends::common_drift(&[2.0, 4.0, 6.0]).is_some());
}

#[test]
fn a_drift_is_only_given_with_enough_history_behind_it() {
    let young = summary_of("hw", "s", &series(20, |_| 40.0));
    assert!(young.delta.is_none() && young.drift().is_none());
    let old = summary_of("hw", "s", &series(200, |i| 40.0 + 0.05 * i as f64));
    assert_eq!(old.drift(), old.delta);
}

#[test]
fn summaries_are_sorted_by_drift_then_unknowns_by_name() {
    let mk =
        |sensor: &str, f: fn(i64) -> f64, days: i64| summary_of("CPU", sensor, &series(days, f));
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

#[test]
fn the_daily_points_are_grouped_by_calendar_month() {
    let t = trend_of(&series(100, |i| 40.0 + i as f64 * 0.1));
    let months = trends::monthly(&t.points);
    // 2024-01-01 + 100 days: January, February, March, April (to the 10th).
    let keys: Vec<&str> = months.iter().map(|m| m.key.as_str()).collect();
    assert_eq!(keys, ["2024-01", "2024-02", "2024-03", "2024-04"]);
    assert_eq!(months[0].days, 31);
    assert_eq!(months[0].first_day, "2024-01-01");
    assert!(months[0].min < months[0].median && months[0].median < months[0].max);
    assert!(months[1].median > months[0].median);
}

#[test]
fn no_points_means_no_months() {
    assert!(trends::monthly(&[]).is_empty());
}

#[test]
fn a_series_that_went_quiet_keeps_its_figures_but_is_no_longer_fresh() {
    let stats = series(200, |i| 40.0 + 0.05 * i as f64);
    let last = stats.last().unwrap().day;

    // Read the day after the last measurement: still being measured.
    let fresh = trends::compute_trend(&stats, last + chrono::Duration::days(1));
    assert!(fresh.fresh);
    assert_eq!(fresh.quiet_days, 1);
    assert_eq!(fresh.last_day.as_deref(), Some("2024-07-18"));

    // Just inside the limit, then just past it.
    assert!(trends::compute_trend(&stats, last + chrono::Duration::days(15)).fresh);
    let stale = trends::compute_trend(&stats, last + chrono::Duration::days(16));
    assert!(!stale.fresh);
    assert_eq!(stale.quiet_days, 16);
    // The drift itself is still a fact about the past: only its freshness changed.
    assert_eq!(stale.delta, fresh.delta);
    assert!(stale.sufficient);
}

#[test]
fn a_stale_series_neither_carries_a_verdict_nor_votes_for_the_machine() {
    let stats = series(200, |i| 40.0 + 0.05 * i as f64);
    let last = stats.last().unwrap().day;
    let quiet = trends::summarize("hw", "s", "idle", &stats, last + chrono::Duration::days(60));
    assert!(quiet.sufficient, "it does have the history");
    assert!(quiet.delta.is_some());
    // …but nothing recent to stand on, so it drives no verdict and no machine-wide drift.
    assert!(quiet.drift().is_none());
    assert_eq!(quiet.quiet_days, 60);
}

#[test]
fn an_empty_series_is_never_fresh() {
    let t = trends::compute_trend(&[], day(0));
    assert!(!t.fresh);
    assert_eq!(t.last_day, None);
    assert_eq!(t.quiet_days, 0);
}

#[test]
fn a_noisy_series_gets_a_wider_margin_than_a_steady_one() {
    let steady = trend_of(&series(200, |_| 40.0));
    // The same average, but jumping by 3 °C from one day to the next.
    let noisy = trend_of(&series(200, |i| 40.0 + if i % 2 == 0 { -3.0 } else { 3.0 }));
    assert_eq!(
        steady.margin,
        Some(0.0),
        "a series that never moves leaves no doubt"
    );
    let margin = noisy.margin.unwrap();
    assert!(margin > 2.0, "margin = {margin}");
    // Both drifted by nothing, and both say so.
    assert!(steady.delta.unwrap().abs() < 1e-9);
    assert!(noisy.delta.unwrap().abs() < 1e-9);
}

#[test]
fn the_margin_makes_a_small_change_on_a_noisy_series_unsayable() {
    // A 2 °C step, buried in a series that swings by 4 °C from day to day.
    let stats = series(200, |i| {
        let step = if i >= 100 { 2.0 } else { 0.0 };
        40.0 + step + if i % 2 == 0 { -4.0 } else { 4.0 }
    });
    let t = trend_of(&stats);
    assert!((t.delta.unwrap() - 2.0).abs() < 1e-9);
    assert!(t.margin.unwrap() > 2.0, "margin = {:?}", t.margin);
    // The same 2 °C on a steady series would be called warmer; here it is not.
    assert_eq!(
        crate::domain::wording::verdict_of(t.delta, t.margin),
        crate::domain::wording::Verdict::Stable
    );
    assert_eq!(
        crate::domain::wording::verdict_of(t.delta, Some(0.2)),
        crate::domain::wording::Verdict::Warmer
    );
}
