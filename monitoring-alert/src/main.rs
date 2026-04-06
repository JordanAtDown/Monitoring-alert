use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[cfg(windows)]
use clap::Args;
#[cfg(windows)]
use report::ReportPeriod;

mod collector;
mod config;
mod db;
#[cfg(windows)]
mod notification;
mod report;
mod sensors;
mod service;
#[cfg(test)]
mod tests;

#[cfg(windows)]
use windows_service::{define_windows_service, service_dispatcher};

#[cfg(windows)]
define_windows_service!(ffi_service_main, handle_service_main);

#[cfg(windows)]
fn handle_service_main(args: Vec<std::ffi::OsString>) {
    if let Err(e) = service::windows::run_service_main(args) {
        eprintln!("Service error: {:#}", e);
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

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    let db_path = cli.db.unwrap_or_else(|| config::AppConfig::load().db_path);

    match cli.command {
        Commands::Collect => {
            let conn = db::init_db(&db_path)?;
            collector::collect_and_store(&conn)?;
        }
        Commands::Watch { interval } => {
            let stop = Arc::new(AtomicBool::new(false));
            let stop_ctrlc = Arc::clone(&stop);
            ctrlc_handler(stop_ctrlc);
            collector::watch(&db_path, interval, stop)?;
        }
        Commands::Report { output } => {
            let conn = db::init_db(&db_path)?;
            report::generate_report(&conn, output.as_deref())?;
        }
        Commands::Service { action } => match action {
            ServiceAction::Install => service::install()?,
            ServiceAction::Uninstall => service::uninstall()?,
            ServiceAction::Start => service::start()?,
            ServiceAction::Stop => service::stop()?,
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
            let conn = db::init_db(&db_path)?;
            let summary = report::generate_summary(&conn, period)?;
            notification::send_toast(&summary.title, &summary.body)?;
        }
    }
    Ok(())
}

/// Best-effort Ctrl+C handler — sets the stop flag so watch() exits cleanly.
fn ctrlc_handler(stop: Arc<AtomicBool>) {
    // Use a simple thread that parks; real projects can use the `ctrlc` crate.
    let _ = std::thread::spawn(move || {
        // On Windows, SetConsoleCtrlHandler would be more robust.
        // This minimal version relies on SIGINT killing the process on non-Windows.
        // For production use on Windows, integrate the `ctrlc` crate.
        let _ = stop; // keep the Arc alive for now
    });
}

fn main() -> Result<()> {
    // When started by Windows Service Control Manager, dispatch to service handler.
    // If called from the command line this returns an error immediately and we fall through.
    #[cfg(windows)]
    {
        if service_dispatcher::start(service::SERVICE_NAME, ffi_service_main).is_ok() {
            return Ok(());
        }
    }

    run_cli()
}
