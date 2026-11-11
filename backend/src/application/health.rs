//! Is everything working? The dashboard monitors the monitor: every part of the chain
//! gets a check with a plain-language message and, when it is not fine, what to do.
//!
//! Pure function over facts gathered by the caller, so every rule is testable.

use chrono::NaiveDateTime;
use serde::Serialize;
use std::collections::BTreeSet;

use crate::application::ports::LatestSnapshot;
use crate::application::status::StatusSnapshot;
use crate::application::updates::UpdateSnapshot;

/// A measurement older than this many collection intervals means collection stopped.
const STALE_AFTER_INTERVALS: i64 = 3;
/// How many missing sensors are named in a message.
const MAX_NAMED_SENSORS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    /// All good.
    Ok,
    /// Worth knowing, nothing to fix (an update exists…).
    Info,
    /// Something to look at; the monitoring still works.
    Warn,
    /// The monitoring is not doing its job.
    Bad,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub title: &'static str,
    pub level: Level,
    pub message: String,
    /// What to do, when the check is not fine.
    pub advice: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Health {
    /// `ok`, `warn` or `bad` (never `info`).
    pub overall: Level,
    pub summary: String,
    pub checks: Vec<Check>,
}

pub struct HealthInputs<'a> {
    /// Local time, `YYYY-MM-DDTHH:MM:SS`.
    pub now: &'a str,
    pub collector: &'a StatusSnapshot,
    pub latest: Option<&'a LatestSnapshot>,
    /// Sensors that reported during the last few days: (hardware, sensor).
    pub recent_sensors: &'a [(String, String)],
    pub db_size_bytes: u64,
    pub db_snapshots: i64,
    pub updates: &'a UpdateSnapshot,
    /// Whether the start-up entry exists; `None` where that does not apply.
    pub autostart: Option<bool>,
}

fn parse(ts: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S").ok()
}

fn age_secs(from: &str, now: &str) -> Option<i64> {
    Some((parse(now)? - parse(from)?).num_seconds().max(0))
}

/// "à l'instant", "il y a 3 min", "il y a 2 h", "il y a 4 j".
pub fn ago(seconds: i64) -> String {
    match seconds {
        s if s < 90 => "à l'instant".into(),
        s if s < 90 * 60 => format!("il y a {} min", (s + 30) / 60),
        s if s < 36 * 3600 => format!("il y a {} h", (s + 1800) / 3600),
        s => format!("il y a {} j", (s + 43200) / 86400),
    }
}

fn mb(bytes: u64) -> String {
    format!("{:.1} Mo", bytes as f64 / 1_048_576.0).replace('.', ",")
}

fn check(
    id: &'static str,
    title: &'static str,
    level: Level,
    message: String,
    advice: Option<&str>,
) -> Check {
    Check {
        id,
        title,
        level,
        message,
        advice: advice.map(str::to_string),
    }
}

const ADVICE_LHM: &str = "Lancez LibreHardwareMonitor en administrateur et vérifiez que Options › Remote Web Server › Run est coché.";

fn collection_check(i: &HealthInputs) -> Check {
    const TITLE: &str = "Relevé des températures";
    let c = i.collector;
    let every = format!("toutes les {} min", (c.interval_secs / 60).max(1));

    if c.consecutive_failures > 0 {
        let since = c
            .last_success
            .as_deref()
            .and_then(|s| age_secs(s, i.now))
            .map(|a| format!(" (dernière mesure {})", ago(a)))
            .unwrap_or_default();
        return check(
            "collection",
            TITLE,
            Level::Bad,
            format!("Les températures ne sont plus relevées{since} : LibreHardwareMonitor ne répond pas."),
            Some(ADVICE_LHM),
        );
    }
    let Some(last) = c.last_success.as_deref() else {
        return check(
            "collection",
            TITLE,
            Level::Warn,
            "En attente de la première mesure…".into(),
            None,
        );
    };
    let age = age_secs(last, i.now).unwrap_or(0);
    if age > c.interval_secs as i64 * STALE_AFTER_INTERVALS {
        return check(
            "collection",
            TITLE,
            Level::Warn,
            format!("Aucune mesure depuis {} alors qu'elle devrait avoir lieu {every}.", ago(age).replace("il y a ", "")),
            Some("Le programme tourne mais ne relève plus rien : redémarrez-le (monitoring.exe restart)."),
        );
    }
    if c.last_sensor_count == 0 {
        return check(
            "collection",
            TITLE,
            Level::Warn,
            "LibreHardwareMonitor répond mais ne fournit aucune température.".into(),
            Some(ADVICE_LHM),
        );
    }
    check(
        "collection",
        TITLE,
        Level::Ok,
        format!("Dernière mesure {} ({every}).", ago(age)),
        None,
    )
}

fn sensors_check(i: &HealthInputs) -> Check {
    const TITLE: &str = "Capteurs";
    let Some(latest) = i.latest else {
        return check(
            "sensors",
            TITLE,
            Level::Info,
            "Aucune mesure enregistrée pour l'instant.".into(),
            None,
        );
    };
    // Only judge the sensors against a recent measurement.
    let fresh = age_secs(&latest.ts, i.now)
        .is_some_and(|a| a <= i.collector.interval_secs as i64 * STALE_AFTER_INTERVALS);
    let reporting: BTreeSet<(&str, &str)> = latest
        .readings
        .iter()
        .map(|r| (r.hardware.as_str(), r.sensor.as_str()))
        .collect();
    let missing: Vec<String> = i
        .recent_sensors
        .iter()
        .filter(|(h, s)| !reporting.contains(&(h.as_str(), s.as_str())))
        .map(|(h, s)| format!("{s} ({h})"))
        .collect();
    if !fresh {
        return check(
            "sensors",
            TITLE,
            Level::Info,
            format!("{} capteurs lors de la dernière mesure.", reporting.len()),
            None,
        );
    }
    if missing.is_empty() {
        return check(
            "sensors",
            TITLE,
            Level::Ok,
            format!("Les {} capteurs répondent.", reporting.len()),
            None,
        );
    }
    let named: Vec<&str> = missing
        .iter()
        .take(MAX_NAMED_SENSORS)
        .map(String::as_str)
        .collect();
    let more = missing.len().saturating_sub(MAX_NAMED_SENSORS);
    let plural = if missing.len() > 1 {
        "capteurs ne répondent plus"
    } else {
        "capteur ne répond plus"
    };
    check(
        "sensors",
        TITLE,
        Level::Warn,
        format!(
            "{} {plural} : {}{}.",
            missing.len(),
            named.join(", "),
            if more > 0 { format!(" et {more} autre(s)") } else { String::new() }
        ),
        Some("Un disque en veille ou un composant retiré peut l'expliquer. S'ils reviennent d'eux-mêmes, rien à faire."),
    )
}

fn database_check(i: &HealthInputs) -> Check {
    const TITLE: &str = "Base de données";
    let written = i
        .latest
        .and_then(|l| age_secs(&l.ts, i.now))
        .map(|a| format!(", dernière écriture {}", ago(a)))
        .unwrap_or_default();
    check(
        "database",
        TITLE,
        Level::Ok,
        format!(
            "Lisible, {} et {} relevés{written}.",
            mb(i.db_size_bytes),
            i.db_snapshots
        ),
        None,
    )
}

fn maintenance_check(i: &HealthInputs) -> Check {
    const TITLE: &str = "Résumés quotidiens";
    let c = i.collector;
    if let Some(error) = &c.maintenance_error {
        return check(
            "maintenance",
            TITLE,
            Level::Warn,
            format!("Le calcul des résumés quotidiens a échoué : {error}"),
            Some("Vos mesures ne sont pas perdues. Relancez le programme (monitoring.exe restart) ; si cela persiste, regardez le journal."),
        );
    }
    match c.last_rollup.as_deref().and_then(parse) {
        Some(at) => check(
            "maintenance",
            TITLE,
            Level::Ok,
            format!("À jour (dernier calcul à {}).", at.format("%H:%M")),
            None,
        ),
        None => check(
            "maintenance",
            TITLE,
            Level::Info,
            "Premier calcul en attente.".into(),
            None,
        ),
    }
}

fn autostart_check(enabled: bool) -> Check {
    const TITLE: &str = "Démarrage automatique";
    if enabled {
        check(
            "autostart",
            TITLE,
            Level::Ok,
            "Le programme redémarre tout seul à l'ouverture de session.".into(),
            None,
        )
    } else {
        check(
            "autostart",
            TITLE,
            Level::Warn,
            "Le programme ne redémarrera pas après un redémarrage du PC.".into(),
            Some("Relancez l'installateur pour réactiver le démarrage automatique."),
        )
    }
}

fn updates_check(u: &UpdateSnapshot) -> Check {
    const TITLE: &str = "Mises à jour";
    if !u.enabled {
        return check(
            "updates",
            TITLE,
            Level::Info,
            "La recherche de mises à jour est désactivée.".into(),
            None,
        );
    }
    match (&u.latest, u.available, &u.error) {
        (Some(latest), true, _) => check(
            "updates",
            TITLE,
            Level::Info,
            format!(
                "La version {} est disponible (vous avez la {}).",
                latest.version, u.current
            ),
            Some("Cliquez sur « Mise à jour disponible » en haut de la page."),
        ),
        (Some(_), false, _) => check(
            "updates",
            TITLE,
            Level::Ok,
            format!("Vous avez la dernière version ({}).", u.current),
            None,
        ),
        (None, _, Some(_)) => check(
            "updates",
            TITLE,
            Level::Info,
            "Impossible de chercher les mises à jour pour le moment (hors ligne ?).".into(),
            None,
        ),
        (None, _, None) => check(
            "updates",
            TITLE,
            Level::Info,
            "Recherche de mises à jour en cours…".into(),
            None,
        ),
    }
}

/// Runs every check and sums them up.
pub fn evaluate(i: &HealthInputs) -> Health {
    let mut checks = vec![
        collection_check(i),
        sensors_check(i),
        database_check(i),
        maintenance_check(i),
    ];
    if let Some(enabled) = i.autostart {
        checks.push(autostart_check(enabled));
    }
    checks.push(updates_check(i.updates));

    let bad = checks.iter().filter(|c| c.level == Level::Bad).count();
    let warn = checks.iter().filter(|c| c.level == Level::Warn).count();
    let (overall, summary) = match (bad, warn) {
        (0, 0) => (Level::Ok, "Tout fonctionne".to_string()),
        (0, n) => (
            Level::Warn,
            format!("{n} point{} à vérifier", if n > 1 { "s" } else { "" }),
        ),
        (b, _) => (
            Level::Bad,
            if b > 1 {
                format!("{b} problèmes empêchent le suivi")
            } else {
                "Un problème empêche le suivi".into()
            },
        ),
    };
    Health {
        overall,
        summary,
        checks,
    }
}
