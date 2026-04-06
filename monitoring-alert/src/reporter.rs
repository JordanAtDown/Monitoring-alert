use anyhow::Result;

// ──────────────────────────────────────────────────────────────
// Report delivery abstraction
// ──────────────────────────────────────────────────────────────

/// Abstraction over report delivery channels.
///
/// Implement this trait to add new output formats without touching
/// report-generation logic: Windows toast, stdout, file, email, etc.
#[cfg_attr(not(windows), allow(dead_code))]
pub trait ReportSender {
    fn send(&self, title: &str, body: &str) -> Result<()>;
}
