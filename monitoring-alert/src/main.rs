use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use store::TemperatureStore as _;

#[cfg(windows)]
use clap::Args;
#[cfg(windows)]
use report::ReportPeriod;

mod collector;
mod config;
mod db;
mod logger;
#[cfg(windows)]
mod notification;
mod report;
mod reporter;
mod sensors;
mod service;
mod store;
#[cfg(test)]
mod tests;

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
    /// Génère le rapport texte
    Report {
        /// Fichier de sortie (stdout si absent)
        #[arg(short, long)]
        output: Option<String>,
    },
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
    #[cfg(windows)]
    /// Envoie un rapport toast (appelé par les tâches planifiées)
    Notify(NotifyArgs),
}

#[cfg(windows)]
#[derive(Args)]
struct NotifyArgs {
    /// Rapport journalier
    #[arg(long, conflicts_with_all = ["weekly", "monthly"])]
    daily: bool,
    /// Rapport hebdomadaire
    #[arg(long, conflicts_with_all = ["daily", "monthly"])]
    weekly: bool,
    /// Rapport mensuel
    #[arg(long, conflicts_with_all = ["daily", "weekly"])]
    monthly: bool,
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

    // Init logger — log file next to the database.
    let log_path = db_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("monitoring-alert.log");
    let _ = logger::init(&log_path, &cfg.log_level);

    match cli.command {
        Commands::Collect => {
            let store = store::SqliteStore::new(db::init_db(&db_path)?);
            collector::collect_and_store(&store)?;
        }
        Commands::Watch { interval } => {
            let stop = Arc::new(AtomicBool::new(false));
            collector::watch(&db_path, interval, cfg.retention_days, stop)?;
        }
        Commands::Report { output } => {
            let store = store::SqliteStore::new(db::init_db(&db_path)?);
            report::generate_report(&store, output.as_deref())?;
        }
        Commands::Service { action } => match action {
            ServiceAction::Install => service::install()?,
            ServiceAction::Uninstall => service::uninstall()?,
            ServiceAction::Start => service::start()?,
            ServiceAction::Stop => service::stop()?,
        },
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
        #[cfg(windows)]
        Commands::Notify(args) => {
            let period = if args.daily {
                ReportPeriod::Daily
            } else if args.weekly {
                ReportPeriod::Weekly
            } else if args.monthly {
                ReportPeriod::Monthly
            } else {
                anyhow::bail!("Spécifiez --daily, --weekly ou --monthly");
            };
            let store = store::SqliteStore::new(db::init_db(&db_path)?);
            let summary = report::generate_summary(&store, period)?;
            let sender: Box<dyn reporter::ReportSender> = Box::new(notification::ToastSender);
            sender.send(&summary.title, &summary.body)?;
        }
    }
    Ok(())
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
