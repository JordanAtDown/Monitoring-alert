//! Puts the files of an installation in place.

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;

use crate::adapters::pe;
use crate::application::ports::DeployPlan;

/// The configuration written by a fresh installation (every key is a commented default).
const DEFAULT_CONFIG: &str = include_str!("default-config.toml");

/// A file just released by a process that was stopped a moment ago can stay locked
/// for a short while on Windows.
const RETRIES: u32 = 8;

fn with_retries<T>(what: &str, mut op: impl FnMut() -> std::io::Result<T>) -> Result<T> {
    let mut attempt = 1;
    loop {
        match op() {
            Ok(v) => return Ok(v),
            Err(e) if attempt < RETRIES => {
                tracing::debug!("{what}: {e} (attempt {attempt}/{RETRIES})");
                std::thread::sleep(Duration::from_millis(750));
                attempt += 1;
            }
            Err(e) => return Err(e).with_context(|| what.to_string()),
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Installs the executables and, if there is none, the default configuration.
/// Returns one line per action.
pub fn deploy_files(plan: &DeployPlan) -> Result<Vec<String>> {
    let mut done = Vec::new();
    std::fs::create_dir_all(&plan.install_dir)
        .with_context(|| format!("Création de {}", plan.install_dir.display()))?;
    std::fs::create_dir_all(&plan.data_dir)
        .with_context(|| format!("Création de {}", plan.data_dir.display()))?;

    // The command-line executable.
    let cli = plan.cli_exe();
    if !same_file(&plan.source_exe, &cli) {
        with_retries(&format!("Copie de {}", cli.display()), || {
            std::fs::copy(&plan.source_exe, &cli)
        })?;
        done.push(format!("Exécutable installé : {}", cli.display()));
    }

    // The background copy: same program, no console window.
    let gui = pe::windows_gui_copy(
        &std::fs::read(&cli).with_context(|| format!("Lecture de {}", cli.display()))?,
    )?;
    let daemon = plan.daemon_exe();
    with_retries(&format!("Écriture de {}", daemon.display()), || {
        std::fs::write(&daemon, &gui)
    })?;
    done.push(format!("Exécutable d'arrière-plan : {}", daemon.display()));

    // The default configuration, only if there is none: an existing one is never touched.
    let config = plan.data_dir.join("config.toml");
    if !config.exists() {
        std::fs::write(&config, DEFAULT_CONFIG)
            .with_context(|| format!("Écriture de {}", config.display()))?;
        done.push(format!("Configuration créée : {}", config.display()));
    }
    Ok(done)
}
