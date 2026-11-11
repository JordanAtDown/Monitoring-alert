//! What is kept, for how long and where, in words (the "Vos données" part of the status page).

use serde::Serialize;

use crate::domain::wording;

/// A sentence about the stored data, with its heading.
#[derive(Debug, Serialize)]
pub struct DataFact {
    pub title: &'static str,
    pub text: String,
}

pub struct DataFigures<'a> {
    /// First day with daily aggregates.
    pub history_since: Option<&'a str>,
    pub interval_secs: u64,
    pub raw_retention_days: u32,
    pub total_snapshots: i64,
    pub db_path: &'a str,
    pub db_size_bytes: u64,
}

pub fn facts(f: &DataFigures) -> Vec<DataFact> {
    let minutes = (f.interval_secs / 60).max(1);
    vec![
        DataFact {
            title: "Résumé de chaque jour",
            text: format!(
                " pour tous les capteurs{} : conservé sans limite. C'est lui qui permet de suivre l'évolution sur des mois.",
                f.history_since
                    .map_or(String::new(), |d| format!(", depuis le {}", wording::date(d)))
            ),
        },
        DataFact {
            title: "Mesures détaillées",
            text: format!(
                " ({}) : conservées {} jours, {} relevés actuellement. Avant d'être supprimées, elles sont résumées.",
                if minutes == 1 {
                    "une par minute".to_string()
                } else {
                    format!("une toutes les {minutes} minutes")
                },
                f.raw_retention_days,
                wording::int(f.total_snapshots)
            ),
        },
        DataFact {
            title: "Fichier",
            text: format!(" : {} ({})", f.db_path, wording::bytes(f.db_size_bytes)),
        },
    ]
}
