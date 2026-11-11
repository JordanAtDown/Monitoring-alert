use crate::application::install::{install, uninstall};
use crate::application::ports::{ConfigMigration, DeployPlan, HostSystem, LegacyFindings};
use anyhow::{bail, Result};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// A host that records what it is asked to do, in order.
#[derive(Default)]
struct FakeHost {
    calls: RefCell<Vec<String>>,
    findings: LegacyFindings,
    daemon_running: bool,
    service_removal_fails: bool,
    tasks_removal_fails: bool,
    deploy_fails: bool,
    autostart_present: bool,
    config: Option<ConfigMigration>,
}

impl FakeHost {
    fn log(&self, call: &str) {
        self.calls.borrow_mut().push(call.to_string());
    }
    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    fn position(&self, call: &str) -> usize {
        self.calls()
            .iter()
            .position(|c| c == call)
            .unwrap_or_else(|| panic!("{call} was not called: {:?}", self.calls()))
    }
    fn called(&self, call: &str) -> bool {
        self.calls().iter().any(|c| c == call)
    }
}

impl HostSystem for FakeHost {
    fn legacy_findings(&self, _: &Path) -> Result<LegacyFindings> {
        self.log("legacy_findings");
        Ok(self.findings.clone())
    }
    fn remove_legacy_service(&self) -> Result<()> {
        self.log("remove_legacy_service");
        if self.service_removal_fails {
            bail!("accès refusé");
        }
        Ok(())
    }
    fn remove_legacy_tasks(&self) -> Result<()> {
        self.log("remove_legacy_tasks");
        if self.tasks_removal_fails {
            bail!("schtasks a échoué");
        }
        Ok(())
    }
    fn remove_legacy_aumid(&self) -> Result<()> {
        self.log("remove_legacy_aumid");
        Ok(())
    }
    fn remove_legacy_scripts(&self, _: &Path) -> Result<Vec<String>> {
        self.log("remove_legacy_scripts");
        Ok(self.findings.scripts.clone())
    }
    fn migrate_config(&self, _: &Path) -> Result<ConfigMigration> {
        self.log("migrate_config");
        Ok(self.config.clone().unwrap_or(ConfigMigration::Current))
    }
    fn deploy(&self, _: &DeployPlan) -> Result<Vec<String>> {
        self.log("deploy");
        if self.deploy_fails {
            bail!("Access is denied. (os error 5)");
        }
        Ok(vec!["Exécutable installé : X".into()])
    }
    fn stop_daemon(&self) -> Result<bool> {
        self.log("stop_daemon");
        Ok(self.daemon_running)
    }
    fn start_daemon(&self, _: &Path) -> Result<()> {
        self.log("start_daemon");
        Ok(())
    }
    fn autostart_enable(&self, _: &Path) -> Result<()> {
        self.log("autostart_enable");
        Ok(())
    }
    fn autostart_disable(&self) -> Result<bool> {
        self.log("autostart_disable");
        Ok(self.autostart_present)
    }
    fn autostart_enabled(&self) -> Result<bool> {
        Ok(self.autostart_present)
    }
}

fn plan() -> DeployPlan {
    DeployPlan {
        source_exe: PathBuf::from("C:/zip/monitoring-alert.exe"),
        install_dir: PathBuf::from("C:/Program Files/MonitoringAlert"),
        data_dir: PathBuf::from("C:/Users/me/AppData/Local/Programs/MonitoringAlert"),
    }
}

fn v1_findings() -> LegacyFindings {
    LegacyFindings {
        service: true,
        tasks: vec!["MonitoringAlert\\RapportJournalier".into()],
        aumid: true,
        scripts: vec!["apply-config.bat".into()],
    }
}

#[test]
fn a_fresh_machine_just_gets_the_new_system() {
    let host = FakeHost::default();
    let report = install(&host, &plan()).unwrap();

    assert_eq!(
        host.calls(),
        [
            "legacy_findings",
            "stop_daemon",
            "migrate_config",
            "deploy",
            "autostart_enable",
            "start_daemon"
        ]
    );
    assert!(report
        .lines
        .iter()
        .any(|l| l.contains("Démarrage automatique")));
    assert!(!report
        .lines
        .iter()
        .any(|l| l.contains("ancien") || l.contains("Ancien")));
}

#[test]
fn a_v1_installation_is_cleaned_up_before_the_new_one_is_deployed() {
    let host = FakeHost {
        findings: v1_findings(),
        daemon_running: false,
        ..Default::default()
    };
    let report = install(&host, &plan()).unwrap();

    // Nothing may still be collecting when files are replaced.
    assert!(host.position("stop_daemon") < host.position("remove_legacy_service"));
    assert!(host.position("remove_legacy_service") < host.position("deploy"));
    for cleanup in [
        "remove_legacy_tasks",
        "remove_legacy_aumid",
        "remove_legacy_scripts",
    ] {
        assert!(
            host.position(cleanup) < host.position("deploy"),
            "{cleanup}"
        );
    }
    assert!(host.position("deploy") < host.position("autostart_enable"));
    assert!(host.position("autostart_enable") < host.position("start_daemon"));

    let text = report.lines.join("\n");
    assert!(text.contains("service Windows « MonitoringAlert » supprimé"));
    assert!(text.contains("Tâches planifiées de rapport supprimées"));
    assert!(text.contains("notifications toast supprimé"));
    assert!(text.contains("apply-config.bat"));
}

#[test]
fn only_what_exists_is_removed() {
    let host = FakeHost {
        findings: LegacyFindings {
            service: true,
            ..Default::default()
        },
        ..Default::default()
    };
    install(&host, &plan()).unwrap();
    assert!(host.called("remove_legacy_service"));
    for untouched in [
        "remove_legacy_tasks",
        "remove_legacy_aumid",
        "remove_legacy_scripts",
    ] {
        assert!(!host.called(untouched), "{untouched} should not run");
    }
}

#[test]
fn without_the_right_to_remove_the_old_service_nothing_is_changed() {
    let host = FakeHost {
        findings: v1_findings(),
        service_removal_fails: true,
        ..Default::default()
    };
    let error = install(&host, &plan()).unwrap_err();

    assert!(format!("{error:#}").contains("administrateur"), "{error:#}");
    // Two collectors must never write to the same database: no new daemon.
    for forbidden in [
        "deploy",
        "autostart_enable",
        "start_daemon",
        "migrate_config",
    ] {
        assert!(!host.called(forbidden), "{forbidden} must not run");
    }
}

#[test]
fn failing_to_remove_secondary_leftovers_does_not_stop_the_install() {
    let host = FakeHost {
        findings: v1_findings(),
        tasks_removal_fails: true,
        ..Default::default()
    };
    let report = install(&host, &plan()).unwrap();

    assert!(host.called("start_daemon"));
    assert!(report
        .lines
        .iter()
        .any(|l| l.starts_with("⚠") && l.contains("schtasks")));
}

#[test]
fn a_running_daemon_is_stopped_before_the_files_are_replaced() {
    let host = FakeHost {
        daemon_running: true,
        ..Default::default()
    };
    let report = install(&host, &plan()).unwrap();
    assert!(host.position("stop_daemon") < host.position("deploy"));
    assert!(report.lines.iter().any(|l| l.contains("arrêté")));
}

#[test]
fn a_migrated_config_is_reported_with_its_backup() {
    let host = FakeHost {
        config: Some(ConfigMigration::Rewritten {
            backup: PathBuf::from("C:/x/config.toml.v1.bak"),
            removed_keys: vec!["reports_dir".into(), "retention_days".into()],
        }),
        ..Default::default()
    };
    let report = install(&host, &plan()).unwrap();
    let line = report
        .lines
        .iter()
        .find(|l| l.contains("config.toml migré"))
        .unwrap();
    assert!(line.contains("reports_dir, retention_days"));
    assert!(line.contains("config.toml.v1.bak"));
}

#[test]
fn a_refused_copy_tells_how_to_fix_it_and_starts_nothing() {
    // The v1.x config keeps install_dir = "C:\\Program Files\\MonitoringAlert".
    let host = FakeHost {
        deploy_fails: true,
        ..Default::default()
    };
    let error = format!("{:#}", install(&host, &plan()).unwrap_err());

    assert!(error.contains("Program Files"), "{error}");
    assert!(error.contains("administrateur"), "{error}");
    assert!(error.contains("install_dir"), "{error}");
    assert!(error.contains("os error 5"), "{error}");
    assert!(!host.called("autostart_enable") && !host.called("start_daemon"));
}

#[test]
fn installing_twice_converges() {
    // Second run: the v1 leftovers are gone, the daemon is running.
    let host = FakeHost {
        daemon_running: true,
        ..Default::default()
    };
    install(&host, &plan()).unwrap();
    install(&host, &plan()).unwrap();
    assert_eq!(host.calls().iter().filter(|c| *c == "deploy").count(), 2);
}

#[test]
fn uninstall_stops_the_daemon_and_removes_the_start_up_entry() {
    let host = FakeHost {
        daemon_running: true,
        autostart_present: true,
        ..Default::default()
    };
    let report = uninstall(&host, Path::new("C:/data")).unwrap();
    assert!(host.called("stop_daemon") && host.called("autostart_disable"));
    assert!(report
        .lines
        .iter()
        .any(|l| l.contains("Démarrage automatique supprimé")));
    // Files and data are not its business.
    assert!(!host.called("deploy"));
}

#[test]
fn uninstall_also_cleans_a_v1_installation() {
    let host = FakeHost {
        findings: v1_findings(),
        ..Default::default()
    };
    uninstall(&host, Path::new("C:/data")).unwrap();
    for cleanup in [
        "remove_legacy_service",
        "remove_legacy_tasks",
        "remove_legacy_aumid",
    ] {
        assert!(host.called(cleanup), "{cleanup}");
    }
}

#[test]
fn uninstall_reports_an_old_service_it_cannot_remove() {
    let host = FakeHost {
        findings: v1_findings(),
        service_removal_fails: true,
        ..Default::default()
    };
    assert!(uninstall(&host, Path::new("C:/data")).is_err());
}
