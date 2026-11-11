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
}

#[test]
fn an_invalid_file_gives_the_defaults() {
    assert_eq!(load("this is [not toml").raw_retention_days, 90);
}

#[test]
fn values_are_read_from_the_file() {
    let cfg = load(
        r#"
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
    let template = include_str!("../adapters/default-config.toml");
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

// ── Migration of a v1.x config file ───────────────────────────

mod migration {
    use crate::adapters::config::{
        migrate_config_file, obsolete_keys_in, render_config, AppConfig, RawConfig,
    };
    use crate::application::ports::ConfigMigration;
    use std::fs;

    const V1_CONFIG: &str = r#"# MonitoringAlert — configuration
install_dir = "C:\\Program Files\\MonitoringAlert"
db_path = "C:\\Users\\Jordan\\AppData\\Local\\Programs\\MonitoringAlert\\temperatures.db"
reports_dir = "C:\\rapports"
collect_interval_secs = 120
retention_days = 365
lhm_host = "192.168.1.5"
lhm_port = 9000
log_level = "debug"
daily_report_enabled = true
daily_report_time    = "08:00"
weekly_report_enabled = false
weekly_report_day     = "MON"
weekly_report_time    = "08:00"
monthly_report_enabled = false
monthly_report_day     = 1
monthly_report_time    = "08:00"
"#;

    #[test]
    fn obsolete_keys_are_detected() {
        let keys = obsolete_keys_in(V1_CONFIG);
        assert_eq!(keys.len(), 11);
        assert!(keys.contains(&"retention_days".to_string()));
        assert!(obsolete_keys_in("lhm_port = 8085").is_empty());
        assert!(obsolete_keys_in("not [valid toml").is_empty());
    }

    #[test]
    fn a_v1_file_is_rewritten_keeping_its_values_and_backed_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, V1_CONFIG).unwrap();

        let result = migrate_config_file(&path).unwrap();
        let ConfigMigration::Rewritten {
            backup,
            removed_keys,
        } = result
        else {
            panic!("expected a rewrite");
        };
        assert_eq!(removed_keys.len(), 11);
        assert_eq!(fs::read_to_string(&backup).unwrap(), V1_CONFIG);
        assert!(backup.to_string_lossy().ends_with("config.toml.v1.bak"));

        let migrated = fs::read_to_string(&path).unwrap();
        assert!(obsolete_keys_in(&migrated).is_empty());
        // `retention_days` is gone, carried over as `raw_retention_days`: the user chose to keep a
        // year of readings, the migration must not shrink it to the new default (90 days).
        assert!(!migrated.lines().any(|l| l.starts_with("retention_days")));
        assert!(
            migrated.lines().any(|l| l == "raw_retention_days = 365"),
            "{migrated}"
        );
        for key in [
            "reports_dir",
            "daily_report",
            "weekly_report",
            "monthly_report",
        ] {
            assert!(!migrated.contains(key), "{key} survived the migration");
        }

        // The values the user had set are carried over, and the loader agrees.
        let cfg = AppConfig::load_from(&path);
        assert_eq!(
            cfg.db_path,
            std::path::Path::new(
                r"C:\Users\Jordan\AppData\Local\Programs\MonitoringAlert\temperatures.db"
            )
        );
        assert_eq!(cfg.collect_interval_secs, 120);
        assert_eq!((cfg.lhm_host.as_str(), cfg.lhm_port), ("192.168.1.5", 9000));
        assert_eq!(cfg.log_level, "debug");
        // The retention the user had chosen is kept; keys that did not exist get their defaults.
        assert_eq!(cfg.raw_retention_days, 365);
        assert_eq!(cfg.web_port, 8787);
    }

    #[test]
    fn migrating_twice_does_nothing_the_second_time() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, V1_CONFIG).unwrap();
        migrate_config_file(&path).unwrap();
        let after_first = fs::read_to_string(&path).unwrap();

        assert_eq!(
            migrate_config_file(&path).unwrap(),
            ConfigMigration::Current
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), after_first);
    }

    #[test]
    fn an_earlier_backup_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(dir.path().join("config.toml.v1.bak"), "precious").unwrap();
        fs::write(&path, V1_CONFIG).unwrap();

        let ConfigMigration::Rewritten { backup, .. } = migrate_config_file(&path).unwrap() else {
            panic!("expected a rewrite");
        };
        assert!(backup.to_string_lossy().ends_with("config.toml.v1.bak2"));
        assert_eq!(
            fs::read_to_string(dir.path().join("config.toml.v1.bak")).unwrap(),
            "precious"
        );
    }

    #[test]
    fn missing_current_or_broken_files_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.toml");
        assert_eq!(
            migrate_config_file(&missing).unwrap(),
            ConfigMigration::Current
        );

        let current = dir.path().join("current.toml");
        fs::write(&current, "web_port = 9000\n").unwrap();
        assert_eq!(
            migrate_config_file(&current).unwrap(),
            ConfigMigration::Current
        );
        assert_eq!(fs::read_to_string(&current).unwrap(), "web_port = 9000\n");

        let broken = dir.path().join("broken.toml");
        fs::write(&broken, "reports_dir = [unterminated").unwrap();
        assert_eq!(
            migrate_config_file(&broken).unwrap(),
            ConfigMigration::Current
        );
    }

    #[test]
    fn special_characters_in_values_survive_the_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "reports_dir = \"x\"\ndb_path = \"C:\\\\Données \\\"été\\\"\\\\t.db\"\n",
        )
        .unwrap();
        migrate_config_file(&path).unwrap();
        assert_eq!(
            AppConfig::load_from(&path).db_path,
            std::path::Path::new("C:\\Données \"été\"\\t.db")
        );
    }

    #[test]
    fn the_rendered_template_with_no_values_is_a_valid_default_config() {
        let text = render_config(&RawConfig::default());
        assert!(text.parse::<toml::Table>().is_ok());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, &text).unwrap();
        let cfg = AppConfig::load_from(&path);
        assert_eq!(
            (cfg.raw_retention_days, cfg.web_port, cfg.lhm_port),
            (90, 8787, 8085)
        );
    }

    /// A real v1.x config: database on another drive, install under Program Files,
    /// a monthly report enabled, a custom log folder.
    const REAL_V1_CONFIG: &str = r#"# =============================================================
#  MonitoringAlert — configuration
# =============================================================

install_dir = "C:\\Program Files\\MonitoringAlert"
db_path = "E:\\monitoring.db\\temperatures.db"
log_dir = "C:\\Users\\Test\\AppData\\Local\\Programs\\MonitoringAlert\\log"
reports_dir = "C:\\Users\\Test\\AppData\\Local\\Programs\\MonitoringAlert\\rapports"
collect_interval_secs = 300
retention_days = 365
lhm_host = "127.0.0.1"
lhm_port = 8085
log_level = "info"
daily_report_enabled = false
daily_report_time    = "08:00"    # HH:MM (24h)
weekly_report_enabled = false
weekly_report_day     = "MON"     # MON TUE WED THU FRI SAT SUN
weekly_report_time    = "08:00"
monthly_report_enabled = true
monthly_report_day     = 1        # 1-28
monthly_report_time    = "19:00"
"#;

    #[test]
    fn a_real_v1_config_keeps_the_database_on_its_other_drive() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, REAL_V1_CONFIG).unwrap();

        let ConfigMigration::Rewritten { removed_keys, .. } = migrate_config_file(&path).unwrap()
        else {
            panic!("expected a rewrite");
        };
        assert_eq!(removed_keys.len(), 11);

        let cfg = AppConfig::load_from(&path);
        // The existing history is found again, wherever it lives.
        assert_eq!(
            cfg.db_path,
            std::path::Path::new(r"E:\monitoring.db\temperatures.db")
        );
        assert_eq!(
            cfg.log_dir,
            std::path::Path::new(r"C:\Users\Test\AppData\Local\Programs\MonitoringAlert\log")
        );
        assert_eq!((cfg.collect_interval_secs, cfg.lhm_port), (300, 8085));
        // The year of readings chosen in v1.x (retention_days = 365) is kept, not cut to 90 days.
        assert_eq!(cfg.raw_retention_days, 365);
        // The migrated file no longer mentions the reports at all, nor install_dir: the
        // installer decides where the executables go, so Program Files is not carried over.
        let migrated = fs::read_to_string(&path).unwrap();
        assert!(!migrated.contains("report"), "{migrated}");
        assert!(
            !migrated.lines().any(|l| l.starts_with("install_dir")),
            "{migrated}"
        );
    }
}

// ── Choosing the database from the installer ──────────────────

mod set_db_path {
    use crate::adapters::config::{render_config, set_db_path, AppConfig, RawConfig};
    use std::fs;
    use std::path::Path;

    fn write(text: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, text).unwrap();
        (dir, path)
    }

    fn active_db_lines(text: &str) -> usize {
        text.lines()
            .filter(|l| l.trim_start().starts_with("db_path"))
            .count()
    }

    #[test]
    fn an_existing_setting_is_replaced_and_everything_else_is_kept() {
        let (_dir, path) = write(
            "# my notes\nlhm_port = 9000\ndb_path = \"C:\\\\old\\\\t.db\"\n\n# keep me\nlog_level = \"debug\"\n",
        );
        set_db_path(&path, Path::new(r"E:\monitoring.db\temperatures.db")).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my notes") && text.contains("# keep me"));
        assert_eq!(active_db_lines(&text), 1);
        let cfg = AppConfig::load_from(&path);
        assert_eq!(cfg.db_path, Path::new(r"E:\monitoring.db\temperatures.db"));
        assert_eq!((cfg.lhm_port, cfg.log_level.as_str()), (9000, "debug"));
    }

    #[test]
    fn a_commented_default_gets_the_real_setting_right_under_it() {
        let (_dir, path) = write(&render_config(&RawConfig::default()));
        set_db_path(&path, Path::new("/data/t.db")).unwrap();

        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let at = lines.iter().position(|l| l.starts_with("db_path")).unwrap();
        assert!(lines[at - 1].starts_with("# db_path"), "{text}");
        assert_eq!(AppConfig::load_from(&path).db_path, Path::new("/data/t.db"));
    }

    #[test]
    fn a_config_without_any_mention_gets_it_appended() {
        let (_dir, path) = write("lhm_port = 9000\n");
        set_db_path(&path, Path::new("/data/t.db")).unwrap();
        let cfg = AppConfig::load_from(&path);
        assert_eq!(
            (cfg.db_path.as_path(), cfg.lhm_port),
            (Path::new("/data/t.db"), 9000)
        );
    }

    #[test]
    fn a_missing_file_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("AppData").join("config.toml");
        set_db_path(&path, Path::new("/data/t.db")).unwrap();
        assert_eq!(AppConfig::load_from(&path).db_path, Path::new("/data/t.db"));
    }

    #[test]
    fn setting_the_same_value_twice_changes_nothing() {
        let (_dir, path) = write("lhm_port = 9000\n");
        set_db_path(&path, Path::new("/data/t.db")).unwrap();
        let once = fs::read_to_string(&path).unwrap();
        set_db_path(&path, Path::new("/data/t.db")).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), once);
        assert_eq!(active_db_lines(&once), 1);
    }

    #[test]
    fn backslashes_quotes_and_accents_survive() {
        let (_dir, path) = write("");
        let tricky = Path::new("E:\\Données \"été\"\\t.db");
        set_db_path(&path, tricky).unwrap();
        assert_eq!(AppConfig::load_from(&path).db_path, tricky);
    }

    #[test]
    fn a_key_that_merely_starts_with_db_path_is_not_touched() {
        let (_dir, path) = write("db_path_backup = \"x\"\n");
        set_db_path(&path, Path::new("/data/t.db")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("db_path_backup = \"x\""));
        assert_eq!(AppConfig::load_from(&path).db_path, Path::new("/data/t.db"));
    }
}

// ── Where the configuration lives ─────────────────────────────

mod data_folder {
    use crate::adapters::config::config_path_in;
    use std::fs;

    #[test]
    fn a_fresh_machine_uses_the_new_name() {
        let programs = tempfile::tempdir().unwrap();
        assert_eq!(
            config_path_in(programs.path()),
            programs.path().join("Monitoring").join("config.toml")
        );
    }

    #[test]
    fn an_installation_that_predates_the_rename_keeps_its_folder() {
        let programs = tempfile::tempdir().unwrap();
        let legacy = programs.path().join("MonitoringAlert");
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("config.toml"), "web_port = 9000\n").unwrap();

        // Using the new folder would silently ignore the user's configuration.
        assert_eq!(config_path_in(programs.path()), legacy.join("config.toml"));
    }

    #[test]
    fn once_the_new_folder_has_a_config_it_wins() {
        let programs = tempfile::tempdir().unwrap();
        for name in ["MonitoringAlert", "Monitoring"] {
            fs::create_dir_all(programs.path().join(name)).unwrap();
            fs::write(programs.path().join(name).join("config.toml"), "").unwrap();
        }
        assert_eq!(
            config_path_in(programs.path()),
            programs.path().join("Monitoring").join("config.toml")
        );
    }

    #[test]
    fn a_leftover_folder_without_a_config_does_not_count() {
        let programs = tempfile::tempdir().unwrap();
        // v1.x uninstalled keeping only logs and the database, no config.
        fs::create_dir_all(programs.path().join("MonitoringAlert")).unwrap();
        // An empty new folder (e.g. created by the installer) does not hide the old config either.
        fs::create_dir_all(programs.path().join("Monitoring")).unwrap();
        assert_eq!(
            config_path_in(programs.path()),
            programs.path().join("Monitoring").join("config.toml")
        );

        let legacy = programs.path().join("MonitoringAlert");
        fs::write(legacy.join("config.toml"), "").unwrap();
        assert_eq!(config_path_in(programs.path()), legacy.join("config.toml"));
    }
}
