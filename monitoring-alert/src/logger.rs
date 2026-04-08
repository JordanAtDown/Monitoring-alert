use anyhow::Result;
use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialise le logger :
/// - Console (stderr) avec couleurs
/// - Fichier avec rotation quotidienne dans le répertoire de `log_path`,
///   nommé d'après `log_path` (ex. `monitoring-alert.log`)
///
/// `level` accepte : `"error"`, `"warn"`, `"info"` (défaut), `"debug"`, `"trace"`.
/// La rotation produit des fichiers du type `monitoring-alert-2026-04-06.log`.
///
/// Le guard du writer non-bloquant est leaké intentionnellement — acceptable
/// pour un service/daemon qui s'exécute jusqu'à la fin du processus.
pub fn init(log_path: &Path, level: &str) -> Result<()> {
    let log_dir = log_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stem = log_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("monitoring-alert");

    if !log_dir.as_os_str().is_empty() {
        std::fs::create_dir_all(log_dir)
            .map_err(|e| anyhow::anyhow!("Cannot create log directory: {}", e))?;
    }

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(stem)
        .filename_suffix("log")
        .build(log_dir)
        .map_err(|e| anyhow::anyhow!("Cannot create log appender: {}", e))?;
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Keep the guard alive for the lifetime of the process.
    Box::leak(Box::new(guard));

    tracing_subscriber::registry()
        .with(EnvFilter::new(level))
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false)
                .with_ansi(true),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_target(false)
                .with_ansi(false),
        )
        .try_init()
        .map_err(|e| anyhow::anyhow!("Logger already initialised: {}", e))?;

    Ok(())
}
