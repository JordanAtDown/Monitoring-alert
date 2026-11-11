use crate::application::clock;

/// Inserts one pre-computed daily aggregate (n = 288, i.e. a full day at 5 min).
pub fn insert_daily(
    conn: &rusqlite::Connection,
    day: &str,
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    median: f64,
) {
    conn.execute(
        "INSERT INTO daily_stats (day, hardware, sensor, load_cat, n, min, median, p95, max, avg)
         VALUES (?1, ?2, ?3, ?4, 288, ?5, ?6, ?7, ?8, ?6)",
        rusqlite::params![
            day,
            hardware,
            sensor,
            load_cat,
            median - 2.0,
            median,
            median + 3.0,
            median + 5.0
        ],
    )
    .expect("insert daily_stats");
}

/// `days` consecutive days ending the day before yesterday, oldest first, medians from `f(index)`.
pub fn insert_daily_series(
    conn: &rusqlite::Connection,
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    days: u32,
    f: impl Fn(u32) -> f64,
) {
    insert_daily_series_ending(conn, hardware, sensor, load_cat, days, 2, f);
}

/// Same, but the series stops `ends_days_ago` days ago: how a load level that is not reached
/// any more looks in the database.
pub fn insert_daily_series_ending(
    conn: &rusqlite::Connection,
    hardware: &str,
    sensor: &str,
    load_cat: &str,
    days: u32,
    ends_days_ago: u32,
    f: impl Fn(u32) -> f64,
) {
    // One transaction: on a file database every autocommit insert would be fsynced.
    let tx = conn.unchecked_transaction().expect("begin");
    for i in 0..days {
        let day = clock::day_string(days - 1 - i + ends_days_ago);
        insert_daily(&tx, &day, hardware, sensor, load_cat, f(i));
    }
    tx.commit().expect("commit");
}

/// `days` consecutive days ending the day before yesterday of a fan speed or a power draw, as the
/// daily aggregates the dashboard reads.
pub fn insert_extra_daily_series(
    conn: &rusqlite::Connection,
    hardware: &str,
    sensor: &str,
    kind: &str,
    load_cat: &str,
    days: u32,
    f: impl Fn(u32) -> f64,
) {
    let tx = conn.unchecked_transaction().expect("begin");
    for i in 0..days {
        let day = clock::day_string(days - i + 1);
        let value = f(i);
        tx.execute(
            "INSERT INTO extra_daily (day, hardware, sensor, kind, load_cat, n, min, median, p95, max, avg)
             VALUES (?1, ?2, ?3, ?4, ?5, 288, ?6, ?6, ?6, ?6, ?6)",
            rusqlite::params![day, hardware, sensor, kind, load_cat, value],
        )
        .expect("insert extra_daily");
    }
    tx.commit().expect("commit");
}
