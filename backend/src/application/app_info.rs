//! Static facts about the running instance.

/// What the process knows about itself, given once at start-up.
pub struct AppInfo {
    /// Where the database is, as shown to the user.
    pub db_path: String,
    pub lhm_host: String,
    pub lhm_port: u16,
    pub raw_retention_days: u32,
}
