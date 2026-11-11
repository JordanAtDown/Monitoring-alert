//! Installing, upgrading and removing the application, including the automatic
//! migration away from the v1.x layout (Windows service, scheduled report tasks,
//! toast registration, report settings in `config.toml`).
//!
//! `install` is idempotent: run it on a fresh machine, on a v1.x machine, or again
//! after an update, and it converges to the same state.

use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::application::ports::{ConfigMigration, DeployPlan, HostSystem};

/// A human-readable log of what an operation did, one line per action.
#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<String>,
}

impl Report {
    fn add(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }
}

/// Installs or upgrades: cleans up a v1.x installation, migrates the config,
/// deploys the files, registers the start-up entry and starts the daemon.
pub fn install(host: &dyn HostSystem, plan: &DeployPlan) -> Result<Report> {
    let mut report = Report::default();
    let config_path = plan.data_dir.join("config.toml");

    let findings = host.legacy_findings(&plan.data_dir)?;

    // 1. Nothing may be collecting while files are replaced, and two collectors
    //    must never write to the same database.
    if host.stop_daemon()? {
        report.add("Ancien processus d'arrière-plan arrêté");
    }
    if findings.service {
        host.remove_legacy_service().context(
            "Impossible de supprimer l'ancien service Windows « MonitoringAlert ». \
             Relancez l'installation en tant qu'administrateur (clic droit › Exécuter en tant qu'administrateur).",
        )?;
        report.add("Ancien service Windows « MonitoringAlert » supprimé");
    }

    // 2. The rest of the v1.x leftovers: best effort, they are harmless if they stay.
    if !findings.tasks.is_empty() {
        match host.remove_legacy_tasks() {
            Ok(()) => report.add(format!(
                "Tâches planifiées de rapport supprimées ({})",
                findings.tasks.join(", ")
            )),
            Err(e) => report.add(format!("⚠ Tâches planifiées non supprimées : {e:#}")),
        }
    }
    if findings.aumid {
        match host.remove_legacy_aumid() {
            Ok(()) => report.add("Enregistrement des notifications toast supprimé"),
            Err(e) => report.add(format!("⚠ Enregistrement toast non supprimé : {e:#}")),
        }
    }
    if !findings.scripts.is_empty() {
        match host.remove_legacy_scripts(&plan.data_dir) {
            Ok(removed) => {
                for script in removed {
                    report.add(format!("Ancien script supprimé : {script}"));
                }
            }
            Err(e) => report.add(format!("⚠ Anciens scripts non supprimés : {e:#}")),
        }
    }

    // 3. Configuration.
    match host
        .migrate_config(&config_path)
        .context("Migration de config.toml")?
    {
        ConfigMigration::Current => {}
        ConfigMigration::Rewritten {
            backup,
            removed_keys,
        } => report.add(format!(
            "config.toml migré (clés supprimées : {}) — ancienne version conservée dans {}",
            removed_keys.join(", "),
            backup.display()
        )),
    }

    // 4. Files, start-up entry, start.
    report
        .lines
        .extend(host.deploy(plan).context("Copie des fichiers")?);
    let daemon = plan.daemon_exe();
    host.autostart_enable(&daemon)
        .context("Enregistrement du démarrage automatique")?;
    report.add("Démarrage automatique à l'ouverture de session activé");
    host.start_daemon(&daemon)
        .context("Démarrage du processus d'arrière-plan")?;
    report.add("Processus d'arrière-plan démarré");
    Ok(report)
}

/// Stops the daemon and removes the start-up entry (and any v1.x leftover).
/// The files stay: the uninstall script deletes them once this process has exited.
pub fn uninstall(host: &dyn HostSystem, data_dir: &Path) -> Result<Report> {
    let mut report = Report::default();
    if host.stop_daemon()? {
        report.add("Processus d'arrière-plan arrêté");
    }
    if host.autostart_disable()? {
        report.add("Démarrage automatique supprimé");
    }

    let findings = host.legacy_findings(data_dir)?;
    if findings.service {
        match host.remove_legacy_service() {
            Ok(()) => report.add("Ancien service Windows supprimé"),
            Err(e) => bail!("Impossible de supprimer l'ancien service Windows (administrateur requis ?) : {e:#}"),
        }
    }
    if !findings.tasks.is_empty() && host.remove_legacy_tasks().is_ok() {
        report.add("Tâches planifiées de rapport supprimées");
    }
    if findings.aumid && host.remove_legacy_aumid().is_ok() {
        report.add("Enregistrement des notifications toast supprimé");
    }
    Ok(report)
}
