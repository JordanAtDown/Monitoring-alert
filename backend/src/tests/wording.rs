use crate::domain::wording::{
    bytes, cause_of, date, int, margin, month_year, plural, signed, span_in_words, stable_band,
    temp, verdict_of, Cause, Verdict,
};

#[test]
fn a_change_becomes_a_word_with_a_band_of_stability() {
    assert_eq!(verdict_of(None, None), Verdict::Unknown);
    assert_eq!(verdict_of(Some(1.49), None), Verdict::Stable);
    assert_eq!(verdict_of(Some(1.5), None), Verdict::Warmer);
    assert_eq!(verdict_of(Some(-1.49), None), Verdict::Stable);
    assert_eq!(verdict_of(Some(-1.5), None), Verdict::Cooler);
    assert_eq!(Verdict::Warmer.label(), "Plus chaud");
}

#[test]
fn a_noisy_series_has_to_change_more_before_anything_is_claimed() {
    // The resolution of the sensors is the floor; a noisier series needs more than that.
    assert_eq!(stable_band(None), 1.5);
    assert_eq!(stable_band(Some(0.4)), 1.5);
    assert_eq!(stable_band(Some(2.5)), 2.5);

    // The same change: a verdict on a quiet series, nothing on a noisy one.
    assert_eq!(verdict_of(Some(2.0), Some(0.3)), Verdict::Warmer);
    assert_eq!(verdict_of(Some(2.0), Some(2.5)), Verdict::Stable);
    assert_eq!(verdict_of(Some(-2.0), Some(2.5)), Verdict::Stable);
}

#[test]
fn the_margin_also_decides_whether_a_component_stands_out() {
    // An excess of 2 °C on a quiet series is the component; on a noisy one it is nothing.
    assert_eq!(cause_of(2.0, 2.0, Some(0.3)), Cause::Alone);
    assert_eq!(cause_of(2.0, 2.0, Some(3.0)), Cause::Nothing);
    // Moved as much as the machine did, and clearly enough to be said.
    assert_eq!(cause_of(4.0, 0.2, Some(0.3)), Cause::Machine);
}

#[test]
fn a_margin_is_written_with_its_sign_both_ways() {
    assert_eq!(margin(0.63), "± 0,6 °C");
}

#[test]
fn temperatures_are_written_the_french_way() {
    assert_eq!(temp(42.3), "42,3 °C");
    assert_eq!(temp(37.0), "37,0 °C");
    assert_eq!(temp(-3.0), "-3,0 °C");
    assert_eq!(signed(4.68), "+4,7 °C");
    assert_eq!(signed(-1.08), "−1,1 °C");
    assert_eq!(signed(0.02), "0,0 °C", "no sign when it rounds to nothing");
}

#[test]
fn dates_are_spelled_out_and_garbage_is_given_back() {
    assert_eq!(date("2025-09-17"), "17 septembre 2025");
    assert_eq!(date("2026-01-01"), "1 janvier 2026");
    assert_eq!(date("2025-09-17T10:00:00"), "17 septembre 2025");
    assert_eq!(date("not a date"), "not a date");
    assert_eq!(date("2025-13-01"), "2025-13-01");
    assert_eq!(month_year("2025-09-17"), "septembre 2025");
}

#[test]
fn a_span_is_told_in_days_months_or_years() {
    assert_eq!(span_in_words(30), "30 jours");
    assert_eq!(span_in_words(217), "environ 7 mois");
    assert_eq!(span_in_words(420), "environ 1,2 an");
    assert_eq!(span_in_words(800), "environ 2,2 ans");
}

#[test]
fn plurals_follow_the_count() {
    assert_eq!(plural(0, "jour", "jours"), "jour");
    assert_eq!(plural(1, "jour", "jours"), "jour");
    assert_eq!(plural(2, "jour", "jours"), "jours");
}

#[test]
fn integers_and_sizes_are_written_the_french_way() {
    assert_eq!(int(0), "0");
    assert_eq!(int(999), "999");
    assert_eq!(int(6654), "6\u{202f}654");
    assert_eq!(int(1234567), "1\u{202f}234\u{202f}567");
    assert_eq!(bytes(512), "0,5 Ko");
    assert_eq!(bytes(1937632), "1,8 Mo");
    assert_eq!(bytes(3 * 1024 * 1024 * 1024), "3,0 Go");
}
