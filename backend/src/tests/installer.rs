//! The Windows installer cannot run on CI's Linux, but the names it relies on can be checked.

use crate::application::ports::DAEMON_EXE_NAME;
use std::path::Path;

// ── The Inno Setup installer ──────────────────────────────────

fn installer() -> String {
    // UTF-8 with a BOM: without it Inno reads the accents as ANSI.
    let bytes =
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../installer/monitoring.iss"))
            .expect("installer/monitoring.iss");
    assert!(
        bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
        "the .iss file must start with a UTF-8 BOM"
    );
    String::from_utf8(bytes).unwrap()
}

#[test]
fn the_installer_only_copies_and_delegates_to_the_tested_installer() {
    let iss = installer();
    // Everything else is done by `monitoring.exe install --in-place`.
    assert!(iss.contains("'install --in-place'"));
    assert!(iss.contains("Parameters: \"uninstall\""));
    assert!(iss.contains("--db-path"));
}

#[test]
fn the_installer_installs_per_user_without_administrator_rights() {
    let iss = installer();
    assert!(iss.contains("PrivilegesRequired=lowest"));
    assert!(iss.contains("DefaultDirName={autopf}\\Monitoring"));
}

#[test]
fn the_installer_ships_no_config_file_the_executable_writes_the_default_one() {
    // A config.toml dropped next to the executable would be mistaken for the user's own and
    // hide the configuration of an existing installation: `install` writes it only if absent.
    let iss = installer();
    assert!(!iss.contains("config.toml\"; DestDir"));
    assert!(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/adapters/default-config.toml")
        .is_file());
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
    assert!(ci.contains("installer\\monitoring.iss"));
    assert!(release.contains("monitoring-setup"));
    assert!(release.contains("installer-out/monitoring_*.exe"));
}

#[test]
fn the_obsolete_scripts_folder_is_gone() {
    // The installer is the only way to install: the batch scripts, the zip archive and
    // the 1.x migration shim were removed with it.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    assert!(!root.join("scripts").exists());
}

#[test]
fn the_product_is_called_monitoring_and_the_installer_is_named_after_its_version() {
    let iss = installer();
    assert!(iss.contains("#define AppName \"Monitoring\""));
    assert!(iss.contains("OutputBaseFilename=monitoring_{#AppVersion}"));
    // The user sees the product name in the wizard, the shortcut and the dashboard.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for visible in [
        iss,
        std::fs::read_to_string(root.join("frontend/index.html")).unwrap(),
        std::fs::read_to_string(root.join("frontend/src/main.ts")).unwrap(),
    ] {
        assert!(
            !visible.contains("Monitoring Alert"),
            "the old product name is still visible"
        );
    }
}
