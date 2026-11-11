use serde::Deserialize;
use std::path::{Path, PathBuf};

// ──────────────────────────────────────────────────────────────
// Public config
// ──────────────────────────────────────────────────────────────

pub struct AppConfig {
    pub db_path: PathBuf,
    /// Directory where log files are written (default: same dir as db_path).
    pub log_dir: PathBuf,
    /// Install directory of the executables (only used by the install scripts and commands).
    pub install_dir: Option<PathBuf>,
    /// Sensor collection interval in seconds (min 60, default 300).
    pub collect_interval_secs: u64,
    /// How many days of raw readings to keep (default 90, minimum 7).
    /// The daily aggregates used for the long-term trends are never purged.
    pub raw_retention_days: u32,
    /// tracing log level: "error", "warn", "info" (default), "debug", "trace".
    pub log_level: String,
    /// LibreHardwareMonitor HTTP host (default "127.0.0.1").
    pub lhm_host: String,
    /// LibreHardwareMonitor HTTP port (default 8085).
    pub lhm_port: u16,
    /// Address the dashboard listens on (default "127.0.0.1": this machine only).
    pub web_host: String,
    /// Port the dashboard listens on (default 8787).
    pub web_port: u16,
}

pub const DEFAULT_WEB_PORT: u16 = 8787;

const VALID_LOG_LEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];

/// The keys of `config.toml`. Unknown keys — including the ones of the v1 reports
/// (`reports_dir`, `*_report_*`, `retention_days`) — are silently ignored.
#[derive(Deserialize, Default)]
pub(crate) struct RawConfig {
    pub db_path: Option<String>,
    pub log_dir: Option<String>,
    pub install_dir: Option<String>,
    pub collect_interval_secs: Option<u64>,
    pub raw_retention_days: Option<u32>,
    pub log_level: Option<String>,
    pub lhm_host: Option<String>,
    pub lhm_port: Option<u16>,
    pub web_host: Option<String>,
    pub web_port: Option<u16>,
}

impl AppConfig {
    /// Default config file location: `%LOCALAPPDATA%\Programs\MonitoringAlert\config.toml`
    /// on Windows, `./config.toml` elsewhere (development).
    #[cfg(windows)]
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

    #[cfg(not(windows))]
    pub fn default_config_path() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("config.toml")
    }

    /// Loads the config from the default path; a missing or invalid file gives the defaults.
    pub fn load() -> Self {
        Self::load_from(&Self::default_config_path())
    }

    /// Loads the config from an explicit path; a missing or invalid file gives the defaults.
    pub fn load_from(config_path: &Path) -> Self {
        let raw: RawConfig = std::fs::read_to_string(config_path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default();
        Self::from_raw(raw, config_path)
    }

    pub(crate) fn from_raw(raw: RawConfig, config_path: &Path) -> Self {
        let app_dir = config_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));

        let db_path = raw
            .db_path
            .map(PathBuf::from)
            .unwrap_or_else(|| app_dir.join("temperatures.db"));

        let log_dir = raw
            .log_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| db_path.parent().unwrap_or(Path::new(".")).to_path_buf());

        let log_level = raw
            .log_level
            .map(|l| l.to_lowercase())
            .filter(|l| VALID_LOG_LEVELS.contains(&l.as_str()))
            .unwrap_or_else(|| "info".to_string());

        AppConfig {
            db_path,
            log_dir,
            install_dir: raw.install_dir.map(PathBuf::from),
            collect_interval_secs: raw.collect_interval_secs.unwrap_or(300).max(60),
            raw_retention_days: raw.raw_retention_days.unwrap_or(90).max(7),
            log_level,
            lhm_host: raw.lhm_host.unwrap_or_else(|| "127.0.0.1".to_string()),
            lhm_port: raw.lhm_port.unwrap_or(8085),
            web_host: raw.web_host.unwrap_or_else(|| "127.0.0.1".to_string()),
            web_port: raw.web_port.unwrap_or(DEFAULT_WEB_PORT),
        }
    }
}
