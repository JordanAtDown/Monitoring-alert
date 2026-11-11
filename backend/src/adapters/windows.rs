//! Windows: start-up entry (registry), background process, cleanup of a v1.x
//! installation (service, scheduled tasks, toast registration).
//!
//! Commands are run without looking at their text output — which is localised —
//! only at their exit code.

use anyhow::{bail, Context, Result};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

use crate::adapters::{config, deploy};
use crate::application::ports::{
    ConfigMigration, DeployPlan, HostSystem, LegacyFindings, DAEMON_EXE_NAME,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

const LEGACY_SERVICE: &str = "MonitoringAlert";
const LEGACY_TASKS: &[&str] = &[
    "\\MonitoringAlert\\RapportJournalier",
    "\\MonitoringAlert\\RapportHebdomadaire",
    "\\MonitoringAlert\\RapportMensuel",
    "MonitoringAlert",
];
const LEGACY_AUMID_KEY: &str =
    r"Software\Classes\AppUserModelId\MonitoringAlert.TemperatureMonitor";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "MonitoringAlert";

const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SERVICE_MARKED_FOR_DELETE: i32 = 1072;

/// Runs a program without a window and returns its exit code.
fn run(program: &str, args: &[&str]) -> Result<i32> {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .with_context(|| format!("Impossible de lancer {program}"))?;
    Ok(status.code().unwrap_or(-1))
}

pub struct WindowsHost;

impl HostSystem for WindowsHost {
    fn legacy_findings(&self, data_dir: &Path) -> Result<LegacyFindings> {
        let service = run("sc.exe", &["query", LEGACY_SERVICE])? == 0;
        let mut tasks = Vec::new();
        for task in LEGACY_TASKS {
            if run("schtasks.exe", &["/query", "/tn", task])? == 0 {
                tasks.push(task.trim_start_matches('\\').to_string());
            }
        }
        let aumid = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(LEGACY_AUMID_KEY, KEY_READ)
            .is_ok();
        // Scripts of the v1.x installation: neither is shipped any more.
        let scripts = ["apply-config.bat", "Register-Tasks.ps1"]
            .iter()
            .filter(|name| data_dir.join(name).is_file())
            .map(|name| name.to_string())
            .collect();
        Ok(LegacyFindings {
            service,
            tasks,
            aumid,
            scripts,
        })
    }

    fn remove_legacy_service(&self) -> Result<()> {
        // `sc stop` fails harmlessly if the service is already stopped.
        let _ = run("sc.exe", &["stop", LEGACY_SERVICE])?;
        // Give the service process time to exit and release its executable.
        std::thread::sleep(Duration::from_secs(5));
        match run("sc.exe", &["delete", LEGACY_SERVICE])? {
            0 | ERROR_SERVICE_MARKED_FOR_DELETE => Ok(()),
            ERROR_ACCESS_DENIED => bail!("accès refusé (il faut les droits administrateur)"),
            code => bail!("`sc delete` a échoué (code {code})"),
        }
    }

    fn remove_legacy_tasks(&self) -> Result<()> {
        for task in LEGACY_TASKS {
            // An absent task is not an error.
            let _ = run("schtasks.exe", &["/delete", "/tn", task, "/f"])?;
        }
        Ok(())
    }

    fn remove_legacy_aumid(&self) -> Result<()> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        match hkcu.delete_subkey_all(LEGACY_AUMID_KEY) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).context("Suppression de la clé AppUserModelId"),
        }
    }

    fn remove_legacy_scripts(&self, data_dir: &Path) -> Result<Vec<String>> {
        let mut removed = Vec::new();
        for finding in self.legacy_findings(data_dir)?.scripts {
            std::fs::remove_file(data_dir.join(&finding))
                .with_context(|| format!("Suppression de {finding}"))?;
            removed.push(finding);
        }
        Ok(removed)
    }

    fn migrate_config(&self, config_path: &Path) -> Result<ConfigMigration> {
        config::migrate_config_file(config_path)
    }

    fn set_db_path(&self, config_path: &Path, db_path: &Path) -> Result<()> {
        config::set_db_path(config_path, db_path)
    }

    fn deploy(&self, plan: &DeployPlan) -> Result<Vec<String>> {
        deploy::deploy_files(plan)
    }

    fn stop_daemon(&self) -> Result<bool> {
        let stopped = run("taskkill.exe", &["/F", "/IM", DAEMON_EXE_NAME])? == 0;
        if stopped {
            // Let the process release its executable and its port.
            std::thread::sleep(Duration::from_secs(1));
        }
        Ok(stopped)
    }

    fn start_daemon(&self, daemon_exe: &Path) -> Result<()> {
        Command::new(daemon_exe)
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn()
            .with_context(|| format!("Impossible de lancer {}", daemon_exe.display()))?;
        Ok(())
    }

    fn autostart_enable(&self, daemon_exe: &Path) -> Result<()> {
        let (run_key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(RUN_KEY)
            .context("Ouverture de la clé Run")?;
        // The path is quoted: install directories often contain spaces.
        run_key
            .set_value(RUN_VALUE, &format!("\"{}\" serve", daemon_exe.display()))
            .context("Écriture de la clé Run")
    }

    fn autostart_disable(&self) -> Result<bool> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE | KEY_READ) else {
            return Ok(false);
        };
        match run_key.delete_value(RUN_VALUE) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e).context("Suppression de la clé Run"),
        }
    }

    fn autostart_enabled(&self) -> Result<bool> {
        Ok(RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(RUN_VALUE))
            .is_ok())
    }
}
