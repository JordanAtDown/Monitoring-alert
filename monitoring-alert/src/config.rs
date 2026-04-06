use std::path::PathBuf;

#[cfg(windows)]
use serde::Deserialize;

// ──────────────────────────────────────────────────────────────
// Schedule configuration (cross-platform — used by tests too)
// ──────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub struct ScheduleConfig {
    pub daily_enabled: bool,
    pub daily_time: String, // "HH:MM"
    pub weekly_enabled: bool,
    pub weekly_day: String, // "MON".."SUN"
    pub weekly_time: String,
    pub monthly_enabled: bool,
    pub monthly_day: u8, // 1–28
    pub monthly_time: String,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            daily_enabled: true,
            daily_time: "08:00".to_string(),
            weekly_enabled: false,
            weekly_day: "MON".to_string(),
            weekly_time: "08:00".to_string(),
            monthly_enabled: false,
            monthly_day: 1,
            monthly_time: "08:00".to_string(),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Public config
// ──────────────────────────────────────────────────────────────

pub struct AppConfig {
    pub db_path: PathBuf,
    /// Sensor collection interval in seconds (min 60, default 300).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub collect_interval_secs: u64,
    /// How many days of data to keep (default 365, enforced minimum 180).
    pub retention_days: u32,
    /// tracing log level: "error", "warn", "info" (default), "debug", "trace".
    pub log_level: String,
    #[allow(dead_code)]
    pub schedule: ScheduleConfig,
}

// ──────────────────────────────────────────────────────────────
// Windows: TOML-backed loader
// ──────────────────────────────────────────────────────────────

#[cfg(windows)]
#[derive(Deserialize, Default)]
struct RawConfig {
    db_path: Option<String>,
    install_dir: Option<String>,
    collect_interval_secs: Option<u64>,
    retention_days: Option<u32>,
    log_level: Option<String>,
    daily_report_enabled: Option<bool>,
    daily_report_time: Option<String>,
    weekly_report_enabled: Option<bool>,
    weekly_report_day: Option<String>,
    weekly_report_time: Option<String>,
    monthly_report_enabled: Option<bool>,
    monthly_report_day: Option<u8>,
    monthly_report_time: Option<String>,
}

#[cfg(windows)]
const VALID_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
#[cfg(windows)]
const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

#[cfg(windows)]
impl AppConfig {
    pub fn load() -> Self {
        let config_path = PathBuf::from(r"C:\ProgramData\MonitoringAlert\config.toml");
        let raw: RawConfig = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        let db_path = raw
            .db_path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData\MonitoringAlert\temperatures.db"));

        let collect_interval_secs = raw.collect_interval_secs.unwrap_or(300).max(60);
        // Minimum 180 days to cover the full 90-day current + 90-day reference window.
        let retention_days = raw.retention_days.unwrap_or(365).max(180);

        let log_level = raw
            .log_level
            .map(|l| l.to_lowercase())
            .filter(|l| VALID_LOG_LEVELS.contains(&l.as_str()))
            .unwrap_or_else(|| "info".to_string());

        let weekly_day = raw
            .weekly_report_day
            .filter(|d| VALID_DAYS.contains(&d.as_str()))
            .unwrap_or_else(|| "MON".to_string());

        let monthly_day = raw.monthly_report_day.map(|d| d.clamp(1, 28)).unwrap_or(1);

        let schedule = ScheduleConfig {
            daily_enabled: raw.daily_report_enabled.unwrap_or(true),
            daily_time: raw.daily_report_time.unwrap_or_else(|| "08:00".to_string()),
            weekly_enabled: raw.weekly_report_enabled.unwrap_or(false),
            weekly_day,
            weekly_time: raw
                .weekly_report_time
                .unwrap_or_else(|| "08:00".to_string()),
            monthly_enabled: raw.monthly_report_enabled.unwrap_or(false),
            monthly_day,
            monthly_time: raw
                .monthly_report_time
                .unwrap_or_else(|| "08:00".to_string()),
        };

        // Suppress unused-field warning for install_dir (used only by scripts)
        let _ = raw.install_dir;

        AppConfig {
            db_path,
            collect_interval_secs,
            retention_days,
            log_level,
            schedule,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Non-Windows: sensible defaults
// ──────────────────────────────────────────────────────────────

#[cfg(not(windows))]
impl AppConfig {
    pub fn load() -> Self {
        AppConfig {
            db_path: std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("temperatures.db"),
            collect_interval_secs: 300,
            retention_days: 365,
            log_level: "info".to_string(),
            schedule: ScheduleConfig::default(),
        }
    }
}
