use std::path::PathBuf;

#[cfg(windows)]
use std::path::Path;

#[cfg(windows)]
use serde::Deserialize;

// ──────────────────────────────────────────────────────────────
// Public config
// ──────────────────────────────────────────────────────────────

pub struct AppConfig {
    pub db_path: PathBuf,
    /// Directory where log files are written (default: same dir as db_path).
    pub log_dir: PathBuf,
    /// Sensor collection interval in seconds (min 60, default 300).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub collect_interval_secs: u64,
    /// How many days of raw readings to keep (default 90, minimum 7).
    /// The daily aggregates used for the long-term trends are never purged.
    pub raw_retention_days: u32,
    /// tracing log level: "error", "warn", "info" (default), "debug", "trace".
    pub log_level: String,
    /// LibreHardwareMonitor HTTP host (default "127.0.0.1").
    #[cfg_attr(not(windows), allow(dead_code))]
    pub lhm_host: String,
    /// LibreHardwareMonitor HTTP port (default 8085).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub lhm_port: u16,
}

// ──────────────────────────────────────────────────────────────
// Windows: TOML-backed loader
// ──────────────────────────────────────────────────────────────

#[cfg(windows)]
#[derive(Deserialize, Default)]
struct RawConfig {
    db_path: Option<String>,
    log_dir: Option<String>,
    install_dir: Option<String>,
    collect_interval_secs: Option<u64>,
    raw_retention_days: Option<u32>,
    log_level: Option<String>,
    lhm_host: Option<String>,
    lhm_port: Option<u16>,
}

#[cfg(windows)]
const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

#[cfg(windows)]
impl AppConfig {
    /// Resolves the default config file path using %LOCALAPPDATA%.
    /// Called at CLI time (user session) — do NOT call from the Windows service
    /// (SYSTEM account has no %LOCALAPPDATA%).
    pub fn default_config_path() -> PathBuf {
        std::env::var("LOCALAPPDATA")
            .map(|p| {
                PathBuf::from(p)
                    .join("Programs")
                    .join("MonitoringAlert")
                    .join("config.toml")
            })
            .unwrap_or_else(|_| {
                PathBuf::from(
                    r"C:\Users\Default\AppData\Local\Programs\MonitoringAlert\config.toml",
                )
            })
    }

    /// Loads config from the default path (CLI context only).
    pub fn load() -> Self {
        Self::load_from(&Self::default_config_path())
    }

    /// Loads config from an explicit path — safe to call from any context,
    /// including the Windows service running as SYSTEM.
    pub fn load_from(config_path: &Path) -> Self {
        let app_dir = config_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let raw: RawConfig = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();

        let db_path = raw
            .db_path
            .map(PathBuf::from)
            .unwrap_or_else(|| app_dir.join("temperatures.db"));

        let log_dir = raw
            .log_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| db_path.parent().unwrap_or(Path::new(".")).to_path_buf());

        let collect_interval_secs = raw.collect_interval_secs.unwrap_or(300).max(60);
        let raw_retention_days = raw.raw_retention_days.unwrap_or(90).max(7);

        let log_level = raw
            .log_level
            .map(|l| l.to_lowercase())
            .filter(|l| VALID_LOG_LEVELS.contains(&l.as_str()))
            .unwrap_or_else(|| "info".to_string());

        // Suppress unused-field warning for install_dir (used only by scripts)
        let _ = raw.install_dir;

        let lhm_host = raw.lhm_host.unwrap_or_else(|| "127.0.0.1".to_string());
        let lhm_port = raw.lhm_port.unwrap_or(8085);

        AppConfig {
            db_path,
            log_dir,
            collect_interval_secs,
            raw_retention_days,
            log_level,
            lhm_host,
            lhm_port,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Non-Windows: sensible defaults
// ──────────────────────────────────────────────────────────────

#[cfg(not(windows))]
impl AppConfig {
    pub fn default_config_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("config.toml")
    }

    pub fn load() -> Self {
        let db_path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("temperatures.db");
        let log_dir = db_path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        AppConfig {
            db_path,
            log_dir,
            collect_interval_secs: 300,
            raw_retention_days: 90,
            log_level: "info".to_string(),
            lhm_host: "127.0.0.1".to_string(),
            lhm_port: 8085,
        }
    }
}
