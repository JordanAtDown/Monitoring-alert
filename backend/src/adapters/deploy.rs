//! Puts the files of an installation in place.

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;

use crate::adapters::pe;
use crate::application::ports::DeployPlan;

/// Helper scripts shipped next to the executable, copied to the data directory.
const HELPER_SCRIPTS: &[&str] = &["uninstall.bat", "update.bat"];

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

/// Installs the executables, the default config (never over an existing one) and
/// the helper scripts. Returns one line per action.
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

    // Files shipped next to the executable of the archive.
    let source_dir = plan.source_exe.parent().unwrap_or(Path::new("."));
    let config = plan.data_dir.join("config.toml");
    let shipped_config = source_dir.join("config.toml");
    if !config.exists() && shipped_config.is_file() {
        std::fs::copy(&shipped_config, &config)
            .with_context(|| format!("Copie de {}", config.display()))?;
        done.push(format!("Configuration créée : {}", config.display()));
    }
    for script in HELPER_SCRIPTS {
        let from = source_dir.join(script);
        let to = plan.data_dir.join(script);
        if from.is_file() && !same_file(&from, &to) {
            std::fs::copy(&from, &to).with_context(|| format!("Copie de {}", to.display()))?;
            done.push(format!("Script installé : {}", to.display()));
        }
    }
    Ok(done)
}
