use crate::domain::wording::{
    bytes, date, int, month_year, plural, signed, span_in_words, temp, verdict_of, Verdict,
};

#[test]
fn a_change_becomes_a_word_with_a_band_of_stability() {
    assert_eq!(verdict_of(None), Verdict::Unknown);
    assert_eq!(verdict_of(Some(1.49)), Verdict::Stable);
    assert_eq!(verdict_of(Some(1.5)), Verdict::Warmer);
    assert_eq!(verdict_of(Some(-1.49)), Verdict::Stable);
    assert_eq!(verdict_of(Some(-1.5)), Verdict::Cooler);
    assert_eq!(Verdict::Warmer.label(), "Plus chaud");
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
