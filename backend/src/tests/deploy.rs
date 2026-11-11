use crate::adapters::deploy::deploy_files;
use crate::adapters::pe::subsystem;
use crate::application::ports::DeployPlan;
use crate::tests::pe::fake_pe;
use std::fs;
use std::path::Path;

struct Layout {
    _dir: tempfile::TempDir,
    plan: DeployPlan,
}

/// An extracted release archive and an empty install location.
fn layout() -> Layout {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("archive");
    fs::create_dir_all(&archive).unwrap();
    fs::write(archive.join("monitoring-alert.exe"), fake_pe(0x20b, 3)).unwrap();
    fs::write(archive.join("config.toml"), "web_port = 9000\n").unwrap();
    let plan = DeployPlan {
        source_exe: archive.join("monitoring-alert.exe"),
        install_dir: dir.path().join("Program Files").join("MonitoringAlert"),
        data_dir: dir.path().join("AppData").join("MonitoringAlert"),
        db_path: None,
    };
    Layout { _dir: dir, plan }
}

fn exists(p: &Path) -> bool {
    p.is_file()
}

#[test]
fn a_fresh_install_puts_everything_in_place() {
    let l = layout();
    let done = deploy_files(&l.plan).unwrap();

    assert!(exists(&l.plan.cli_exe()));
    assert_eq!(fs::read(l.plan.cli_exe()).unwrap(), fake_pe(0x20b, 3));
    assert!(exists(&l.plan.daemon_exe()));
    assert_eq!(subsystem(&fs::read(l.plan.daemon_exe()).unwrap()), Some(2));
    assert_eq!(
        fs::read_to_string(l.plan.data_dir.join("config.toml")).unwrap(),
        "web_port = 9000\n"
    );
    assert_eq!(done.len(), 3);
}

#[test]
fn the_daemon_executable_is_named_monitoring_daemon() {
    let l = layout();
    assert!(l.plan.daemon_exe().ends_with("monitoring-daemon.exe"));
    assert!(l.plan.cli_exe().ends_with("monitoring-alert.exe"));
}

#[test]
fn paths_with_spaces_are_fine() {
    let l = layout();
    assert!(l
        .plan
        .install_dir
        .to_string_lossy()
        .contains("Program Files"));
    deploy_files(&l.plan).unwrap();
    assert!(exists(&l.plan.cli_exe()));
}

#[test]
fn an_existing_config_is_never_overwritten() {
    let l = layout();
    fs::create_dir_all(&l.plan.data_dir).unwrap();
    fs::write(l.plan.data_dir.join("config.toml"), "web_port = 1234\n").unwrap();

    deploy_files(&l.plan).unwrap();
    assert_eq!(
        fs::read_to_string(l.plan.data_dir.join("config.toml")).unwrap(),
        "web_port = 1234\n"
    );
}

#[test]
fn an_upgrade_replaces_the_executables() {
    let l = layout();
    deploy_files(&l.plan).unwrap();

    fs::write(
        &l.plan.source_exe,
        fake_pe(0x20b, 3).iter().map(|b| b ^ 1).collect::<Vec<u8>>(),
    )
    .unwrap();
    // The new executable must be a valid PE: build it properly.
    let mut v2 = fake_pe(0x20b, 3);
    v2[0x300] = 0x42;
    fs::write(&l.plan.source_exe, &v2).unwrap();

    deploy_files(&l.plan).unwrap();
    assert_eq!(fs::read(l.plan.cli_exe()).unwrap(), v2);
    assert_eq!(fs::read(l.plan.daemon_exe()).unwrap()[0x300], 0x42);
}

#[test]
fn running_from_the_install_directory_only_refreshes_the_daemon_copy() {
    // The executable is already where it belongs (what the installer's --in-place does).
    let l = layout();
    fs::create_dir_all(&l.plan.install_dir).unwrap();
    fs::write(l.plan.cli_exe(), fake_pe(0x20b, 3)).unwrap();
    let plan = DeployPlan {
        source_exe: l.plan.cli_exe(),
        ..l.plan.clone()
    };

    let done = deploy_files(&plan).unwrap();
    assert!(exists(&plan.daemon_exe()));
    assert!(done
        .iter()
        .all(|line| !line.starts_with("Exécutable installé")));
    // No config / scripts next to it: nothing else to copy, and nothing fails.
    assert!(!plan.data_dir.join("config.toml").exists());
}

#[test]
fn a_source_that_is_not_a_windows_executable_is_refused() {
    let l = layout();
    fs::write(&l.plan.source_exe, b"not an exe").unwrap();
    assert!(deploy_files(&l.plan).is_err());
}
