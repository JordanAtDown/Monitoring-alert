//! Is a newer version available? The answer is only displayed: nothing is ever
//! downloaded or run by the application itself.

use serde::Serialize;
use std::sync::Mutex;

use crate::application::clock::now_string;
use crate::application::ports::{ReleaseInfo, UpdateSource};
use crate::domain::version::is_newer;

#[derive(Default)]
struct Inner {
    latest: Option<ReleaseInfo>,
    checked_at: Option<String>,
    error: Option<String>,
}

/// What the dashboard shows about updates.
#[derive(Debug, Serialize)]
pub struct UpdateSnapshot {
    /// The running version.
    pub current: String,
    /// `false` when checking was turned off in the configuration.
    pub enabled: bool,
    pub available: bool,
    pub latest: Option<ReleaseInfo>,
    pub checked_at: Option<String>,
    pub error: Option<String>,
    /// One line for the page footer: which version runs and whether it is the latest.
    pub line: String,
    /// The text of the "update available" button, when there is one.
    pub button: Option<String>,
    /// When the latest version was published, in words ("20 novembre 2026").
    pub published: Option<String>,
}

/// Result of the update checks, written by the checker and read by the web server.
pub struct UpdateStatus {
    current: String,
    enabled: bool,
    inner: Mutex<Inner>,
}

impl UpdateStatus {
    pub fn new(current: &str, enabled: bool) -> Self {
        Self {
            current: current.to_string(),
            enabled,
            inner: Mutex::new(Inner::default()),
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn record_release(&self, release: ReleaseInfo) {
        let mut inner = self.lock();
        inner.latest = Some(release);
        inner.checked_at = Some(now_string());
        inner.error = None;
    }

    pub fn record_error(&self, error: &str) {
        let mut inner = self.lock();
        inner.checked_at = Some(now_string());
        inner.error = Some(error.to_string());
    }

    pub fn snapshot(&self) -> UpdateSnapshot {
        let inner = self.lock();
        let available = self.enabled
            && inner
                .latest
                .as_ref()
                .is_some_and(|r| is_newer(&r.version, &self.current));
        let v = &self.current;
        let line = match (&inner.latest, available) {
            _ if !self.enabled => format!("Monitoring {v} — recherche de mises à jour désactivée"),
            (Some(latest), true) => {
                format!(
                    "Monitoring {v} — la version {} est disponible",
                    latest.version
                )
            }
            (Some(_), false) => format!("Monitoring {v} — à jour"),
            (None, _) if inner.error.is_some() => {
                format!("Monitoring {v} — impossible de chercher les mises à jour pour le moment")
            }
            (None, _) => format!("Monitoring {v} — recherche de mises à jour en cours…"),
        };
        let button = inner
            .latest
            .as_ref()
            .filter(|_| available)
            .map(|r| format!("Mise à jour disponible : version {}", r.version));
        let published = inner
            .latest
            .as_ref()
            .and_then(|r| r.published_at.as_deref())
            .map(crate::domain::wording::date);
        UpdateSnapshot {
            line,
            button,
            published,
            current: self.current.clone(),
            enabled: self.enabled,
            available,
            latest: inner.latest.clone(),
            checked_at: inner.checked_at.clone(),
            error: inner.error.clone(),
        }
    }
}

/// Asks the source for the latest release and records the answer. Returns whether it worked.
pub fn check_now(source: &dyn UpdateSource, status: &UpdateStatus) -> bool {
    match source.latest_release() {
        Ok(release) => {
            tracing::info!("Latest release: {}", release.version);
            status.record_release(release);
            true
        }
        Err(e) => {
            tracing::warn!("Update check failed: {e:#}");
            status.record_error(&format!("{e:#}"));
            false
        }
    }
}
