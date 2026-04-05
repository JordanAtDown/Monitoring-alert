use std::path::PathBuf;

#[cfg(windows)]
use serde::Deserialize;

#[cfg(windows)]
#[derive(Deserialize, Default)]
struct RawConfig {
    db_path: Option<String>,
}

pub struct AppConfig {
    pub db_path: PathBuf,
}

impl AppConfig {
    pub fn load() -> Self {
        AppConfig {
            db_path: Self::resolve_db_path(),
        }
    }

    fn resolve_db_path() -> PathBuf {
        #[cfg(windows)]
        {
            let config_path = PathBuf::from(r"C:\ProgramData\MonitoringAlert\config.toml");
            if let Ok(contents) = std::fs::read_to_string(&config_path) {
                if let Ok(raw) = toml::from_str::<RawConfig>(&contents) {
                    if let Some(p) = raw.db_path {
                        return PathBuf::from(p);
                    }
                }
            }
            PathBuf::from(r"C:\ProgramData\MonitoringAlert\temperatures.db")
        }
        #[cfg(not(windows))]
        {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("temperatures.db")
        }
    }
}
