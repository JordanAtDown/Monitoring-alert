//! The wall clock, in local time (the timestamps stored in the database are local).

use chrono::{Duration, Local, NaiveDate, NaiveDateTime};

/// Now, as a local date and time.
pub fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

/// Now, as `YYYY-MM-DDTHH:MM:SS`.
pub fn now_string() -> String {
    now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Today, as a local date.
pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// The local date `days_back` days ago (0 = today), as `YYYY-MM-DD`.
pub fn day_string(days_back: u32) -> String {
    (Local::now().date_naive() - Duration::days(i64::from(days_back)))
        .format("%Y-%m-%d")
        .to_string()
}
