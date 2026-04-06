use anyhow::Result;
use simplelog::{
    ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};
use std::fs::OpenOptions;
use std::path::Path;

/// Initialise le logger : sortie console (Info) + fichier en mode append (Info).
///
/// Crée le répertoire parent si nécessaire. Peut être appelé une seule fois ;
/// les appels suivants retournent une erreur que l'on peut ignorer silencieusement.
pub fn init(log_path: &Path) -> Result<()> {
    if let Some(parent) = log_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Cannot create log directory: {}", e))?;
        }
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| anyhow::anyhow!("Cannot open log file {}: {}", log_path.display(), e))?;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Info, Config::default(), file),
    ])
    .map_err(|e| anyhow::anyhow!("Logger already initialised: {}", e))?;

    Ok(())
}
