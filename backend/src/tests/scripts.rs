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

// ── The Inno Setup installer ──────────────────────────────────

fn installer() -> String {
    // UTF-8 with a BOM: without it Inno reads the accents as ANSI.
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../installer/monitoring-alert.iss"),
    )
    .expect("installer/monitoring-alert.iss");
    assert!(
        bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
        "the .iss file must start with a UTF-8 BOM"
    );
    String::from_utf8(bytes).unwrap()
}

#[test]
fn the_installer_only_copies_and_delegates_to_the_tested_installer() {
    let iss = installer();
    // Everything else is done by `monitoring-alert.exe install --in-place`.
    assert!(iss.contains("'install --in-place'"));
    assert!(iss.contains("Parameters: \"uninstall\""));
    assert!(iss.contains("--db-path"));
}

#[test]
fn the_installer_installs_per_user_without_administrator_rights() {
    let iss = installer();
    assert!(iss.contains("PrivilegesRequired=lowest"));
    assert!(iss.contains("DefaultDirName={autopf}\\MonitoringAlert"));
}

#[test]
fn the_installer_never_overwrites_or_removes_the_configuration() {
    let iss = installer();
    let config_line = iss
        .lines()
        .find(|l| l.contains("scripts\\config.toml"))
        .expect("config.toml entry");
    assert!(config_line.contains("onlyifdoesntexist"));
    assert!(config_line.contains("uninsneveruninstall"));
}

#[test]
fn the_installer_reads_the_keys_that_db_locate_prints() {
    use crate::adapters::locate::locate;
    let iss = installer();
    let out = locate(Path::new("a.db"), Path::new("b.db")).porcelain();
    for key in ["path", "default", "summary"] {
        assert!(
            out.contains(&format!("{key}=")),
            "db locate no longer prints {key}="
        );
        assert!(
            iss.contains(&format!("'{key}'")),
            "the installer does not read {key}"
        );
    }
    assert!(iss.contains("db locate --porcelain"));
}

#[test]
fn the_installer_removes_the_files_that_install_creates() {
    let iss = installer();
    assert!(iss.contains(&format!("{{app}}\\{}", DAEMON_EXE_NAME)));
    // The v1.x service can only be removed as administrator.
    assert!(iss.contains("query MonitoringAlert"));
    assert!(iss.contains("runas"));
}

#[test]
fn the_installer_is_built_by_ci_and_published_with_the_release() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let ci = std::fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let release = std::fs::read_to_string(root.join(".github/workflows/release.yml")).unwrap();
    assert!(ci.contains("installer\\monitoring-alert.iss"));
    assert!(release.contains("monitoring-alert-setup"));
    assert!(release.contains("installer-out/monitoring-alert-setup-v*.exe"));
}
