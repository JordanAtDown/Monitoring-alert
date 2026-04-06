use crate::db;

/// Returns an ISO-8601 UTC timestamp N days in the past.
pub fn ts_days_ago(days: i64) -> String {
    (chrono::Utc::now() - chrono::TimeDelta::days(days))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

/// Opens an in-memory store with the full schema.
pub fn make_store() -> crate::store::SqliteStore {
    crate::store::SqliteStore::new(
        db::init_db(std::path::Path::new(":memory:")).expect("in-memory db"),
    )
}
