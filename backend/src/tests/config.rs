use crate::adapters::config::{AppConfig, DEFAULT_WEB_PORT};
use std::path::Path;

fn load(toml: &str) -> AppConfig {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, toml).unwrap();
    AppConfig::load_from(&path)
}

#[test]
fn a_missing_file_gives_the_defaults() {
    let cfg = AppConfig::load_from(Path::new("/nonexistent/dir/config.toml"));
    assert_eq!(cfg.db_path, Path::new("/nonexistent/dir/temperatures.db"));
    assert_eq!(cfg.log_dir, Path::new("/nonexistent/dir"));
    assert_eq!(cfg.collect_interval_secs, 300);
    assert_eq!(cfg.raw_retention_days, 90);
    assert_eq!(cfg.log_level, "info");
    assert_eq!((cfg.lhm_host.as_str(), cfg.lhm_port), ("127.0.0.1", 8085));
    assert_eq!(
        (cfg.web_host.as_str(), cfg.web_port),
        ("127.0.0.1", DEFAULT_WEB_PORT)
    );
    assert!(cfg.install_dir.is_none());
}

#[test]
fn an_invalid_file_gives_the_defaults() {
    assert_eq!(load("this is [not toml").raw_retention_days, 90);
}

#[test]
fn values_are_read_from_the_file() {
    let cfg = load(
        r#"
        install_dir = "D:\\Apps\\MonitoringAlert"
        db_path = "/data/t.db"
        log_dir = "/data/logs"
        collect_interval_secs = 120
        raw_retention_days = 30
        log_level = "DEBUG"
        lhm_host = "192.168.1.5"
        lhm_port = 9000
        web_host = "0.0.0.0"
        web_port = 9999
        "#,
    );
    assert_eq!(cfg.db_path, Path::new("/data/t.db"));
    assert_eq!(cfg.log_dir, Path::new("/data/logs"));
    assert_eq!(
        cfg.install_dir.as_deref(),
        Some(Path::new("D:\\Apps\\MonitoringAlert"))
    );
    assert_eq!(cfg.collect_interval_secs, 120);
    assert_eq!(cfg.raw_retention_days, 30);
    assert_eq!(cfg.log_level, "debug");
    assert_eq!((cfg.lhm_host.as_str(), cfg.lhm_port), ("192.168.1.5", 9000));
    assert_eq!((cfg.web_host.as_str(), cfg.web_port), ("0.0.0.0", 9999));
}

#[test]
fn log_dir_defaults_to_the_database_directory() {
    assert_eq!(
        load(r#"db_path = "/data/t.db""#).log_dir,
        Path::new("/data")
    );
}

#[test]
fn limits_are_enforced() {
    let cfg = load("collect_interval_secs = 5\nraw_retention_days = 1\nlog_level = \"shout\"");
    assert_eq!(cfg.collect_interval_secs, 60);
    assert_eq!(cfg.raw_retention_days, 7);
    assert_eq!(cfg.log_level, "info");
}

#[test]
fn a_v1_config_still_loads_and_its_obsolete_keys_are_ignored() {
    let cfg = load(
        r#"
        install_dir = "C:\\Program Files\\MonitoringAlert"
        db_path = "C:\\Users\\Jordan\\temperatures.db"
        reports_dir = "C:\\rapports"
        collect_interval_secs = 300
        retention_days = 365
        lhm_port = 8085
        daily_report_enabled = true
        daily_report_time = "08:00"
        weekly_report_enabled = false
        weekly_report_day = "MON"
        monthly_report_day = 1
        "#,
    );
    assert_eq!(cfg.db_path, Path::new("C:\\Users\\Jordan\\temperatures.db"));
    // The v1 retention_days is not carried over: raw data is now kept for 90 days.
    assert_eq!(cfg.raw_retention_days, 90);
}

#[test]
fn the_shipped_config_template_parses_and_is_free_of_obsolete_keys() {
    let template = include_str!("../../../scripts/config.toml");
    for obsolete in [
        "reports_dir",
        "report_enabled",
        "report_time",
        "report_day",
        "retention_days =",
    ] {
        assert!(
            !template.lines().any(|l| !l.trim_start().starts_with('#')
                && l.contains(obsolete)
                && !l.contains("raw_retention_days")),
            "scripts/config.toml still sets an obsolete key: {obsolete}"
        );
    }
    let parsed: Result<toml::Table, _> = toml::from_str(template);
    assert!(parsed.is_ok());
    let cfg = {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, template).unwrap();
        AppConfig::load_from(&path)
    };
    assert_eq!(cfg.raw_retention_days, 90);
    assert_eq!(cfg.web_port, DEFAULT_WEB_PORT);
}
