use anyhow::Result;
use clap::{Parser, Subcommand};
#[cfg(windows)]
use monitoring_alert::sensors;
use monitoring_alert::store::TemperatureStore as _;
use monitoring_alert::{collector, config, db, logger, service, store};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(windows)]
use windows_service::{define_windows_service, service_dispatcher};

#[cfg(windows)]
define_windows_service!(ffi_service_main, handle_service_main);

#[cfg(windows)]
fn handle_service_main(args: Vec<std::ffi::OsString>) {
    if let Err(e) = service::windows::run_service_main(args) {
        tracing::error!("Service error: {:#}", e);
    }
}

#[derive(Parser)]
#[command(
    name = "monitoring-alert",
    version,
    about = "Moniteur de température long terme — service Windows"
)]
struct Cli {
    /// Chemin vers la base de données (priorité sur config.toml)
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Collecte un snapshot unique (mode debug)
    Collect,
    /// Boucle de collecte à intervalles réguliers
    Watch {
        /// Intervalle entre les collectes (secondes)
        #[arg(long, default_value = "300")]
        interval: u64,
    },
    /// Vérifie la configuration, la base de données, le service et LHM
    Check,
    /// Gestion du service Windows
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Maintenance de la base de données
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Installe le service Windows
    Install,
    /// Désinstalle le service Windows
    Uninstall,
    /// Démarre le service
    Start,
    /// Arrête le service
    Stop,
    /// Affiche l'état du service (installé, actif, PID)
    Status,
}

#[derive(Subcommand)]
enum DbAction {
    /// Affiche les statistiques de la base de données
    Stats,
    /// Compacte la base de données et libère l'espace disque (à faire après une purge)
    Vacuum,
}

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::AppConfig::load();
    let db_path = cli.db.unwrap_or_else(|| cfg.db_path.clone());

    // Init logger — directory from config (defaults to same dir as db_path).
    let log_path = cfg.log_dir.join("monitoring-alert.log");
    let _ = logger::init(&log_path, &cfg.log_level);

    match cli.command {
        Commands::Collect => {
            let store = store::SqliteStore::new(db::init_db(&db_path)?);
            collector::collect_and_store(&store, &cfg.lhm_host, cfg.lhm_port)?;
        }
        Commands::Watch { interval } => {
            let stop = Arc::new(AtomicBool::new(false));
            collector::watch(
                &db_path,
                interval,
                cfg.retention_days,
                &cfg.lhm_host,
                cfg.lhm_port,
                stop,
            )?;
        }
        Commands::Service { action } => match action {
            ServiceAction::Install => {
                let config_path = config::AppConfig::default_config_path();
                service::install(&config_path)?;
            }
            ServiceAction::Uninstall => service::uninstall()?,
            ServiceAction::Start => service::start()?,
            ServiceAction::Stop => service::stop()?,
            ServiceAction::Status => service::status()?,
        },
        Commands::Check => {
            run_check(&db_path, &cfg);
        }
        Commands::Db { action } => match action {
            DbAction::Stats => {
                let store = store::SqliteStore::new(db::init_db(&db_path)?);
                let stats = store.get_overall_stats()?;
                let size_bytes = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                println!("Base de données : {}", db_path.display());
                println!(
                    "Taille sur disque : {:.1} MB",
                    size_bytes as f64 / 1_048_576.0
                );
                println!("Snapshots : {}", stats.total_snapshots);
                println!(
                    "Première mesure : {}",
                    stats.first_ts.as_deref().unwrap_or("—")
                );
                println!(
                    "Dernière mesure : {}",
                    stats.last_ts.as_deref().unwrap_or("—")
                );
            }
            DbAction::Vacuum => {
                let size_before = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                let conn = db::init_db(&db_path)?;
                db::vacuum(&conn)?;
                let size_after = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                let freed = size_before.saturating_sub(size_after);
                println!(
                    "Vacuum terminé — avant : {:.1} MB, après : {:.1} MB, libéré : {:.1} MB",
                    size_before as f64 / 1_048_576.0,
                    size_after as f64 / 1_048_576.0,
                    freed as f64 / 1_048_576.0,
                );
            }
        },
    }
    Ok(())
}

fn run_check(db_path: &std::path::Path, cfg: &config::AppConfig) {
    const SEP: &str = "════════════════════════════════════════════════════════════════";
    println!("Vérifications pré-démarrage");
    println!("{}", SEP);
    println!();

    let mut issues: u32 = 0;

    // ── 1. Configuration ──────────────────────────────────────
    println!("  ✓  Configuration chargée");
    println!("     DB         : {}", db_path.display());
    println!(
        "     Intervalle : {} s  —  Rétention : {} j  —  Log : {}",
        cfg.collect_interval_secs, cfg.retention_days, cfg.log_level
    );
    println!();

    // ── 2. Base de données ────────────────────────────────────
    match db::init_db(db_path) {
        Err(e) => {
            issues += 1;
            println!("  ✗  Base de données inaccessible");
            println!("     Erreur : {}", e);
            if let Some(parent) = db_path.parent() {
                println!(
                    "     → Vérifiez que le répertoire {} existe",
                    parent.display()
                );
            }
        }
        Ok(conn) => {
            let store = store::SqliteStore::new(conn);
            let stats = store.get_overall_stats().unwrap_or(db::OverallStats {
                total_snapshots: 0,
                first_ts: None,
                last_ts: None,
            });
            let size_mb = std::fs::metadata(db_path)
                .map(|m| m.len() as f64 / 1_048_576.0)
                .unwrap_or(0.0);
            println!(
                "  ✓  Base de données accessible  ({:.1} MB — {} snapshot(s))",
                size_mb, stats.total_snapshots
            );

            // Compute days collected from first_ts to today.
            let days = stats
                .first_ts
                .as_deref()
                .and_then(|s| s.get(..10))
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .map(|first| (chrono::Local::now().date_naive() - first).num_days());

            if let Some(d) = days {
                let first_label = stats
                    .first_ts
                    .as_deref()
                    .and_then(|s| s.get(..10))
                    .unwrap_or("?");
                println!(
                    "     Données collectées : {} j  (depuis le {})",
                    d, first_label
                );
            } else {
                println!("     Données collectées : aucune mesure encore enregistrée");
            }

            // Report readiness: each window needs 2× its duration to compare current vs previous.
            println!("     Maturité des comparaisons :");
            const WINDOWS: &[(i64, &str, &str)] = &[
                (2, "24h", ""),
                (14, " 7j", ""),
                (60, "30j", ""),
                (180, "90j", ""),
                (360, "180j", ""),
            ];
            let today = chrono::Local::now().date_naive();
            for &(needed, label, note) in WINDOWS {
                let collected = days.unwrap_or(0);
                if collected >= needed {
                    println!("       {}  ✓ comparaison disponible{}", label, note);
                } else {
                    let remaining = needed - collected;
                    let ready_date = today + chrono::Duration::days(remaining);
                    println!(
                        "       {}  … dans {} j  (dispo le {}{})",
                        label, remaining, ready_date, note
                    );
                }
            }
        }
    }
    println!();

    // ── 3. Service Windows ────────────────────────────────────
    #[cfg(windows)]
    {
        let (installed, running) = service::check_state();
        if !installed {
            issues += 1;
            println!("  ✗  Service non installé");
            println!("     → Lancez install.bat en tant qu'administrateur");
        } else if running {
            println!("  ✓  Service installé et actif");
        } else {
            issues += 1;
            println!("  ⚠  Service installé mais arrêté");
            println!("     → monitoring-alert.exe service start");
        }
        println!();
    }

    // ── 4. LibreHardwareMonitor / HTTP ───────────────────────
    #[cfg(windows)]
    {
        match sensors::read_sensors(&cfg.lhm_host, cfg.lhm_port) {
            Ok(data) if !data.temperatures.is_empty() => {
                println!(
                    "  ✓  LibreHardwareMonitor HTTP accessible  ({} sonde(s) de température)",
                    data.temperatures.len()
                );
            }
            Ok(_) => {
                issues += 1;
                println!("  ⚠  LibreHardwareMonitor accessible mais aucune sonde trouvée");
                println!("     → Vérifiez que LHM surveille bien votre matériel");
            }
            Err(e) => {
                issues += 1;
                println!(
                    "  ✗  LibreHardwareMonitor HTTP inaccessible ({}:{})",
                    cfg.lhm_host, cfg.lhm_port
                );
                println!("     Erreur : {}", e);
                println!("     → Lancez LHM en tant qu'administrateur");
                println!("     → Activez Options › Remote Web Server › Run dans LHM");
            }
        }
        println!();
    }

    // ── Bilan ─────────────────────────────────────────────────
    println!("{}", SEP);
    if issues == 0 {
        println!("  Tout est opérationnel. ✓");
    } else {
        println!(
            "  {} problème(s) détecté(s) — voir les ✗ ci-dessus.",
            issues
        );
    }
}

fn main() -> Result<()> {
    #[cfg(windows)]
    {
        if service_dispatcher::start(service::SERVICE_NAME, ffi_service_main).is_ok() {
            return Ok(());
        }
    }

    run_cli()
}
