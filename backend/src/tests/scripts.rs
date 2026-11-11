//! The Windows scripts cannot run on CI's Linux, but the names they rely on can be checked.

use crate::application::ports::{DAEMON_EXE_NAME, MIGRATION_SHIM_MARKER};
use std::path::Path;

fn script(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn the_migration_shim_carries_the_marker_the_installer_looks_for() {
    // Without it the installer would delete the shim of the very update that runs it.
    assert!(script("Register-Tasks.ps1").contains(MIGRATION_SHIM_MARKER));
}

#[test]
fn the_migration_shim_runs_the_installer_with_the_parameters_v1_update_bat_passes() {
    let shim = script("Register-Tasks.ps1");
    assert!(shim.contains("[string]$ExePath"));
    assert!(shim.contains("[string]$Username"));
    assert!(shim.contains("& $ExePath install"));
    // The v1.x script was mandatory-parameter based: the shim must never prompt.
    assert!(!shim.contains("Mandatory"));
}

#[test]
fn install_and_update_run_the_install_command() {
    assert!(script("install.bat").contains("monitoring-alert.exe\" install"));
    assert!(script("update.bat").contains("monitoring-alert.exe\" install"));
}

#[test]
fn update_relaunches_itself_because_install_replaces_it() {
    let update = script("update.bat");
    assert!(update.contains("ma-update.bat"));
    assert!(update.contains("\"%~1\"==\"run\""));
}

#[test]
fn uninstall_removes_everything_a_1x_and_a_2x_install_register() {
    let u = script("uninstall.bat");
    for needed in [
        DAEMON_EXE_NAME,
        "sc delete MonitoringAlert",
        "\\MonitoringAlert\\RapportJournalier",
        "\\MonitoringAlert\\RapportHebdomadaire",
        "\\MonitoringAlert\\RapportMensuel",
        "CurrentVersion\\Run",
        "AppUserModelId\\MonitoringAlert.TemperatureMonitor",
    ] {
        assert!(u.contains(needed), "uninstall.bat does not handle {needed}");
    }
    // The data is kept unless the user says otherwise.
    assert!(u.contains("[o/N]"));
}

#[test]
fn the_old_apply_config_script_is_gone() {
    assert!(!Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/apply-config.bat")
        .exists());
}
