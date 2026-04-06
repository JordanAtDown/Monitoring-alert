#![cfg(windows)]

use anyhow::{Context, Result};
use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};

pub const TOAST_APP_ID: &str = "MonitoringAlert.TemperatureMonitor";

// ──────────────────────────────────────────────────────────────
// ReportSender implementation
// ──────────────────────────────────────────────────────────────

/// Windows toast notification sender.
///
/// Requires the AUMID `MonitoringAlert.TemperatureMonitor` to be
/// registered in `HKCU\Software\Classes\AppUserModelId\` (done by
/// `install.bat`).
pub struct ToastSender;

impl crate::reporter::ReportSender for ToastSender {
    fn send(&self, title: &str, body: &str) -> Result<()> {
        send_toast(title, body)
    }
}

// ──────────────────────────────────────────────────────────────
// WinRT implementation (internal)
// ──────────────────────────────────────────────────────────────

fn send_toast(title: &str, body: &str) -> Result<()> {
    let xml = build_toast_xml(title, body);
    let doc = XmlDocument::new().context("XmlDocument::new")?;
    doc.LoadXml(&HSTRING::from(xml)).context("LoadXml")?;
    let toast =
        ToastNotification::CreateToastNotification(&doc).context("CreateToastNotification")?;
    let notifier =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(TOAST_APP_ID))
            .context("CreateToastNotifierWithId")?;
    notifier.Show(&toast).context("Show")?;
    Ok(())
}

fn build_toast_xml(title: &str, body: &str) -> String {
    format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        xml_escape(title),
        xml_escape(body),
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
