use serde::Deserialize;
use std::path::{Path, PathBuf};

// ──────────────────────────────────────────────────────────────
// Public config
// ──────────────────────────────────────────────────────────────

pub struct AppConfig {
    pub db_path: PathBuf,
    /// Directory where log files are written (default: same dir as db_path).
    pub log_dir: PathBuf,
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

// ──────────────────────────────────────────────────────────────
// Migration of a v1.x config file
// ──────────────────────────────────────────────────────────────

/// Keys of the v1.x configuration that no longer do anything.
pub const OBSOLETE_KEYS: &[&str] = &[
    // The installer decides where the executables go (they run from where they are installed).
    "install_dir",
    "reports_dir",
    "daily_report_enabled",
    "daily_report_time",
    "weekly_report_enabled",
    "weekly_report_day",
    "weekly_report_time",
    "monthly_report_enabled",
    "monthly_report_day",
    "monthly_report_time",
    "retention_days",
];

/// The obsolete keys present in a config file's text.
pub fn obsolete_keys_in(text: &str) -> Vec<String> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    OBSOLETE_KEYS
        .iter()
        .filter(|k| table.contains_key(**k))
        .map(|k| k.to_string())
        .collect()
}

fn string_line(key: &str, value: &Option<String>, example: &str, comment: &str) -> String {
    let mut out = format!("\n{comment}\n");
    match value {
        Some(v) => out.push_str(&format!("{key} = {}\n", toml::Value::String(v.clone()))),
        None => out.push_str(&format!(
            "# {key} = {}\n",
            toml::Value::String(example.into())
        )),
    }
    out
}

fn number_line<T: Copy + Into<i64>>(
    key: &str,
    value: Option<T>,
    default: i64,
    comment: &str,
) -> String {
    match value {
        Some(v) => format!("\n{comment}\n{key} = {}\n", v.into()),
        None => format!("\n{comment}\n# {key} = {default}\n"),
    }
}

/// Writes a config file in the current format, carrying over the values of `raw`.
pub(crate) fn render_config(raw: &RawConfig) -> String {
    let mut out = String::from(
        "# =============================================================\n\
         #  MonitoringAlert — configuration\n\
         #\n\
         #  Les lignes commençant par # montrent la valeur par défaut.\n\
         #  Après modification : monitoring-alert.exe restart\n\
         # =============================================================\n",
    );
    out += &string_line(
        "db_path",
        &raw.db_path,
        "C:\\Users\\<vous>\\AppData\\Local\\Programs\\MonitoringAlert\\temperatures.db",
        "# Base de données de températures (défaut : temperatures.db à côté de ce fichier)",
    );
    out += &string_line(
        "log_dir",
        &raw.log_dir,
        "C:\\Users\\<vous>\\AppData\\Local\\Programs\\MonitoringAlert",
        "# Dossier des logs (défaut : le dossier de la base de données)",
    );
    out += &number_line(
        "collect_interval_secs",
        raw.collect_interval_secs.map(|v| v as i64),
        300,
        "# Intervalle de collecte en secondes (minimum 60)",
    );
    out += &number_line(
        "raw_retention_days",
        raw.raw_retention_days.map(i64::from),
        90,
        "# Jours de mesures brutes conservées (minimum 7). Les agrégats quotidiens sont conservés sans limite.",
    );
    out += &string_line(
        "lhm_host",
        &raw.lhm_host,
        "127.0.0.1",
        "# Adresse du serveur HTTP de LibreHardwareMonitor (Options › Remote Web Server)",
    );
    out += &number_line(
        "lhm_port",
        raw.lhm_port.map(i64::from),
        8085,
        "# Port du serveur HTTP de LibreHardwareMonitor",
    );
    out += &string_line(
        "web_host",
        &raw.web_host,
        "127.0.0.1",
        "# Adresse d'écoute du tableau de bord. \"0.0.0.0\" l'ouvre au réseau local, SANS authentification.",
    );
    out += &number_line(
        "web_port",
        raw.web_port.map(i64::from),
        i64::from(DEFAULT_WEB_PORT),
        "# Port du tableau de bord : http://127.0.0.1:8787/",
    );
    out += &string_line(
        "log_level",
        &raw.log_level,
        "info",
        "# Niveau de log : error | warn | info | debug | trace",
    );
    out
}

/// Where the previous version of a rewritten file is kept: `config.toml.v1.bak`,
/// never overwriting an earlier backup (`.v1.bak2`, `.v1.bak3`…).
fn backup_path(path: &Path) -> PathBuf {
    let mut candidate = PathBuf::from(format!("{}.v1.bak", path.display()));
    let mut n = 2;
    while candidate.exists() {
        candidate = PathBuf::from(format!("{}.v1.bak{n}", path.display()));
        n += 1;
    }
    candidate
}

/// Rewrites a v1.x `config.toml` without its obsolete keys, keeping the old file
/// next to it. A missing, current or unreadable file is left untouched.
pub fn migrate_config_file(
    path: &Path,
) -> anyhow::Result<crate::application::ports::ConfigMigration> {
    use crate::application::ports::ConfigMigration;
    use anyhow::Context;

    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(ConfigMigration::Current);
    };
    let removed_keys = obsolete_keys_in(&text);
    if removed_keys.is_empty() {
        return Ok(ConfigMigration::Current);
    }
    let raw: RawConfig = toml::from_str(&text).unwrap_or_default();

    let backup = backup_path(path);
    std::fs::copy(path, &backup)
        .with_context(|| format!("Sauvegarde de {} vers {}", path.display(), backup.display()))?;

    let tmp = PathBuf::from(format!("{}.new", path.display()));
    std::fs::write(&tmp, render_config(&raw))
        .with_context(|| format!("Écriture de {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("Remplacement de {}", path.display()))?;
    Ok(ConfigMigration::Rewritten {
        backup,
        removed_keys,
    })
}
