use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use monitoring_alert::adapters::locate::locate;
use monitoring_alert::adapters::sqlite::{self as db, SqliteStore};
use monitoring_alert::adapters::{config, http, lhm, logging};
use monitoring_alert::application::collect::collect_and_store;
use monitoring_alert::application::install;
use monitoring_alert::application::ports::{DeployPlan, HostSystem, OverallStats, ReadModel};
use monitoring_alert::daemon;
use monitoring_alert::domain::load::LoadTracker;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "monitoring",
    version,
    about = "Suivi long terme de la dérive des températures — collecte + tableau de bord local"
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
    /// Installe ou met à jour (copie les fichiers, nettoie une installation v1.x, démarrage automatique)
    Install {
        /// L'exécutable est déjà à sa place (installateur .exe) : ne rien copier, créer seulement
        /// la copie sans fenêtre à côté de lui
        #[arg(long)]
        in_place: bool,
        /// Base de données à utiliser (inscrite dans config.toml) ; sinon celle de la config actuelle
        #[arg(long)]
        db_path: Option<PathBuf>,
    },
    /// Arrête le processus d'arrière-plan et retire le démarrage automatique
    Uninstall,
    /// Démarre le processus d'arrière-plan
    Start,
    /// Arrête le processus d'arrière-plan
    Stop,
    /// Redémarre le processus d'arrière-plan (à faire après avoir modifié config.toml)
    Restart,
    /// Lance la collecte et le tableau de bord web au premier plan (c'est ce que fait le démarrage automatique)
    Serve,
    /// Collecte un snapshot unique (mode debug)
    Collect,
    /// Vérifie la configuration, la base de données, LHM et le tableau de bord
    Check,
    /// Maintenance de la base de données
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
}

#[derive(Subcommand)]
enum DbAction {
    /// Affiche l'emplacement et le contenu de la base de données actuelle
    Locate {
        /// Sortie `clé=valeur`, pour un script d'installation
        #[arg(long)]
        porcelain: bool,
    },
    /// Affiche les statistiques de la base de données
    Stats,
    /// Compacte la base de données et libère l'espace disque (à faire après une purge)
    Vacuum,
}

// ── Host system ──────────────────────────────────────────────────────────────

#[cfg(windows)]
fn host() -> Result<Box<dyn HostSystem>> {
    Ok(Box::new(monitoring_alert::adapters::windows::WindowsHost))
}

#[cfg(not(windows))]
fn host() -> Result<Box<dyn HostSystem>> {
    bail!("Cette commande n'est disponible que sous Windows")
}

fn data_dir() -> PathBuf {
    config::AppConfig::default_config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// The files of this installation: where this executable runs from, and where it goes.
///
/// Run from anywhere else (a download folder), it installs itself into the data directory.
/// With `in_place` (the .exe installer already put it where it belongs) it stays where it is.
fn deploy_plan(in_place: bool, db_path: Option<PathBuf>) -> Result<DeployPlan> {
    let data_dir = data_dir();
    let source_exe = std::env::current_exe().context("Chemin de l'exécutable introuvable")?;
    let install_dir = match source_exe.parent() {
        Some(dir) if in_place => dir.to_path_buf(),
        _ => data_dir.clone(),
    };
    Ok(DeployPlan {
        source_exe,
        install_dir,
        data_dir,
        db_path,
    })
}

fn print_report(report: &install::Report) {
    for line in &report.lines {
        println!("  • {line}");
    }
}

fn dashboard_url(cfg: &config::AppConfig) -> String {
    let host = if cfg.web_host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        &cfg.web_host
    };
    format!("http://{host}:{}/", cfg.web_port)
}

fn start_daemon(cfg: &config::AppConfig) -> Result<()> {
    if http::ping(&cfg.web_host, cfg.web_port) {
        println!("Déjà démarré : {}", dashboard_url(cfg));
        return Ok(());
    }
    // `start` runs from the installed executable: the daemon copy sits next to it.
    let plan = deploy_plan(true, None)?;
    let daemon_exe = plan.daemon_exe();
    if !daemon_exe.is_file() {
        bail!(
            "{} introuvable : lancez d'abord `monitoring install`",
            daemon_exe.display()
        );
    }
    host()?.start_daemon(&daemon_exe)?;
    println!("Démarré : {}", dashboard_url(cfg));
    Ok(())
}

// ── Commands ─────────────────────────────────────────────────────────────────

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::AppConfig::load();
    let db_path = cli.db.unwrap_or_else(|| cfg.db_path.clone());

    // Init logger — directory from config (defaults to same dir as db_path).
    let log_path = cfg.log_dir.join("monitoring.log");
    let _ = logging::init(&log_path, &cfg.log_level);

    match cli.command {
        Commands::Install { in_place, db_path } => {
            let plan = deploy_plan(in_place, db_path)?;
            println!("Installation de Monitoring…");
            let report = install::install(host()?.as_ref(), &plan)?;
            print_report(&report);
            println!();
            println!("Terminé. Tableau de bord : {}", dashboard_url(&cfg));
            println!("Base de données conservée : {}", cfg.db_path.display());
        }
        Commands::Uninstall => {
            let report = install::uninstall(host()?.as_ref(), &data_dir())?;
            print_report(&report);
            println!("Désinstallé. Les fichiers et la base de données n'ont pas été supprimés.");
        }
        Commands::Start => start_daemon(&cfg)?,
        Commands::Stop => {
            if host()?.stop_daemon()? {
                println!("Arrêté.");
            } else {
                println!("Le processus d'arrière-plan ne tournait pas.");
            }
        }
        Commands::Restart => {
            host()?.stop_daemon()?;
            start_daemon(&cfg)?;
        }
        Commands::Serve => {
            let result = daemon::run(&cfg, &db_path, Arc::new(AtomicBool::new(false)));
            if let Err(e) = &result {
                // The background copy has no console: the log file is the only trace.
                tracing::error!("{e:#}");
            }
            result?;
        }
        Commands::Collect => {
            let store = SqliteStore::new(db::init_db(&db_path)?);
            let source = lhm::LhmSource::new(&cfg.lhm_host, cfg.lhm_port);
            collect_and_store(&store, &source, &mut LoadTracker::new())?;
        }
        Commands::Check => run_check(&db_path, &cfg),
        Commands::Db { action } => match action {
            DbAction::Locate { porcelain } => {
                let found = locate(&db_path, &data_dir().join("temperatures.db"));
                if porcelain {
                    print!("{}", found.porcelain());
                } else {
                    println!("Base de données : {}", found.path.display());
                    println!("Contenu         : {}", found.summary());
                    println!("Emplacement par défaut : {}", found.default_path.display());
                }
            }
            DbAction::Stats => {
                let store = db::open_read_model(&db_path)?;
                let stats = ReadModel::overall_stats(&store)?;
                let size_bytes = ReadModel::storage_bytes(&store);
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

fn run_check(db_path: &Path, cfg: &config::AppConfig) {
    const SEP: &str = "════════════════════════════════════════════════════════════════";
    println!("Vérifications");
    println!("{}", SEP);
    println!();

    let mut issues: u32 = 0;

    // ── 1. Configuration ──────────────────────────────────────
    println!("  ✓  Configuration chargée");
    println!("     DB         : {}", db_path.display());
    println!(
        "     Intervalle : {} s  —  Mesures brutes conservées : {} j  —  Log : {}",
        cfg.collect_interval_secs, cfg.raw_retention_days, cfg.log_level
    );
    println!();

    // ── 2. Base de données (lecture seule : `check` ne modifie rien) ──
    match db::open_read_model(db_path) {
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
        Ok(store) => {
            let stats = ReadModel::overall_stats(&store).unwrap_or(OverallStats {
                total_snapshots: 0,
                first_ts: None,
                last_ts: None,
                history_since: None,
            });
            let size_mb = ReadModel::storage_bytes(&store) as f64 / 1_048_576.0;
            println!(
                "  ✓  Base de données accessible  ({:.1} MB — {} snapshot(s) bruts)",
                size_mb, stats.total_snapshots
            );

            // The history starts at the first daily summary: raw readings are purged after a while.
            let first_day = stats.history_start_day();
            let days = first_day
                .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                .map(|first| (chrono::Local::now().date_naive() - first).num_days());
            match (days, first_day) {
                (Some(d), Some(first)) => {
                    println!("     Historique depuis le {first} ({d} j)");
                    let min = monitoring_alert::domain::trends::MIN_SPAN_DAYS;
                    if d >= min {
                        println!("     Suivi de dérive : disponible (≥ {min} j de recul)");
                    } else {
                        let ready =
                            chrono::Local::now().date_naive() + chrono::Duration::days(min - d);
                        println!(
                            "     Suivi de dérive : dans {} j (dispo le {ready}) — il faut {min} j de recul",
                            min - d
                        );
                    }
                }
                _ => println!("     Aucune mesure encore enregistrée"),
            }
        }
    }
    println!();

    // ── 3. LibreHardwareMonitor / HTTP ───────────────────────
    match lhm::read_sensors(&cfg.lhm_host, cfg.lhm_port) {
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

    // ── 4. Tableau de bord / processus d'arrière-plan ────────
    if http::ping(&cfg.web_host, cfg.web_port) {
        println!(
            "  ✓  Processus d'arrière-plan actif — {}",
            dashboard_url(cfg)
        );
    } else {
        issues += 1;
        println!(
            "  ✗  Le tableau de bord ne répond pas ({}:{})",
            cfg.web_host, cfg.web_port
        );
        println!(
            "     → monitoring.exe start   (ou relancez l'installateur si ce n'est pas installé)"
        );
    }
    println!();

    // ── 5. Démarrage automatique + reliquats de la v1.x (Windows) ──
    if let Ok(host) = host() {
        match host.autostart_enabled() {
            Ok(true) => println!("  ✓  Démarrage automatique à l'ouverture de session"),
            _ => {
                issues += 1;
                println!("  ✗  Démarrage automatique non configuré");
                println!("     → Relancez l'installateur");
            }
        }
        if let Ok(left) = host.legacy_findings(&data_dir()) {
            if !left.is_empty() {
                issues += 1;
                println!("  ⚠  Reliquats de la version 1.x détectés");
                if left.service {
                    println!("     • service Windows « MonitoringAlert »");
                }
                for task in &left.tasks {
                    println!("     • tâche planifiée {task}");
                }
                if left.aumid {
                    println!("     • enregistrement des notifications toast");
                }
                for script in &left.scripts {
                    println!("     • script {script}");
                }
                println!("     → Relancez l'installateur (en administrateur) : ils sont supprimés automatiquement");
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
    run_cli()
}
