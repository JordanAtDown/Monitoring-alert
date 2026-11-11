use crate::adapters::config::AppConfig;
use crate::adapters::deploy::deploy_files;
use crate::adapters::pe::subsystem;
use crate::application::ports::DeployPlan;
use crate::tests::pe::fake_pe;
use std::fs;
use std::path::Path;

const DEFAULT_CONFIG: &str = include_str!("../adapters/default-config.toml");

struct Layout {
    _dir: tempfile::TempDir,
    plan: DeployPlan,
}

/// An extracted release archive and an empty install location.
fn layout() -> Layout {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("archive");
    fs::create_dir_all(&archive).unwrap();
    fs::write(archive.join("monitoring.exe"), fake_pe(0x20b, 3)).unwrap();
    let plan = DeployPlan {
        source_exe: archive.join("monitoring.exe"),
        install_dir: dir.path().join("Program Files").join("Monitoring"),
        data_dir: dir.path().join("AppData").join("Monitoring"),
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
        DEFAULT_CONFIG
    );
    assert_eq!(done.len(), 3);
}

#[test]
fn the_daemon_executable_is_named_monitoring_daemon() {
    let l = layout();
    assert!(l.plan.daemon_exe().ends_with("monitoring-daemon.exe"));
    assert!(l.plan.cli_exe().ends_with("monitoring.exe"));
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
fn running_from_the_install_directory_copies_no_executable_but_still_sets_up_the_rest() {
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
    // Nothing is copied, but the daemon copy and the default configuration are created.
    assert!(done
        .iter()
        .any(|line| line.starts_with("Configuration créée")));
    assert!(plan.data_dir.join("config.toml").is_file());
}

#[test]
fn a_source_that_is_not_a_windows_executable_is_refused() {
    let l = layout();
    fs::write(&l.plan.source_exe, b"not an exe").unwrap();
    assert!(deploy_files(&l.plan).is_err());
}

#[test]
fn the_default_configuration_written_by_a_fresh_install_loads_with_the_defaults() {
    let l = layout();
    deploy_files(&l.plan).unwrap();

    let cfg = AppConfig::load_from(&l.plan.data_dir.join("config.toml"));
    assert_eq!(
        (cfg.raw_retention_days, cfg.web_port, cfg.lhm_port),
        (90, 8787, 8085)
    );
    // The database defaults to the data folder, next to the configuration.
    assert_eq!(cfg.db_path, l.plan.data_dir.join("temperatures.db"));
}
