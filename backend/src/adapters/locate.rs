//! Where the current database is, and what is in it: lets an installer show and keep
//! the history of the installation it replaces.

use std::path::{Path, PathBuf};

use crate::adapters::sqlite::open_read_model;
use crate::application::ports::ReadModel;

pub struct DbLocation {
    /// The database the configuration points to.
    pub path: PathBuf,
    /// Where a fresh installation puts it.
    pub default_path: PathBuf,
    pub exists: bool,
    /// Raw snapshots in it, when it could be read.
    pub snapshots: Option<i64>,
    /// First day of the raw snapshots (`YYYY-MM-DD`).
    pub first_day: Option<String>,
}

/// Looks at the database at `path`, read-only: nothing is created or migrated.
pub fn locate(path: &Path, default_path: &Path) -> DbLocation {
    let exists = path.is_file();
    let stats = exists
        .then(|| open_read_model(path).and_then(|store| ReadModel::overall_stats(&store)))
        .and_then(Result::ok);
    DbLocation {
        path: path.to_path_buf(),
        default_path: default_path.to_path_buf(),
        exists,
        snapshots: stats.as_ref().map(|s| s.total_snapshots),
        first_day: stats
            .and_then(|s| s.first_ts)
            .and_then(|ts| ts.get(..10).map(str::to_string)),
    }
}

impl DbLocation {
    /// One sentence for a human (French, like the installer).
    pub fn summary(&self) -> String {
        match (self.exists, self.snapshots, &self.first_day) {
            (true, Some(n), Some(day)) => format!("{n} relevés bruts, depuis le {day}"),
            (true, Some(n), None) => format!("{n} relevés bruts"),
            (true, None, _) => "fichier présent mais illisible comme base de températures".into(),
            (false, _, _) => "aucune base à cet emplacement : une nouvelle sera créée".into(),
        }
    }

    /// `key=value` lines, easy to read from an installer script.
    pub fn porcelain(&self) -> String {
        format!(
            "path={}\ndefault={}\nexists={}\nsummary={}\n",
            self.path.display(),
            self.default_path.display(),
            self.exists,
            self.summary()
        )
    }
}
