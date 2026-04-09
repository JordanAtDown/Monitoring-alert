#![cfg(windows)]

use anyhow::{Context, Result};
use std::path::PathBuf;
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
/// When `report_path` is `Some`, the toast body and an "Ouvrir le rapport"
/// button both open the file with its default application (activationType=protocol).
///
/// Requires the AUMID `MonitoringAlert.TemperatureMonitor` to be
/// registered in `HKCU\Software\Classes\AppUserModelId\` (done by
/// `install.bat`).
pub struct ToastSender {
    pub report_path: Option<PathBuf>,
}

impl crate::reporter::ReportSender for ToastSender {
    fn send(&self, title: &str, body: &str) -> Result<()> {
        let file_uri = self.report_path.as_deref().map(path_to_file_uri);
        send_toast(title, body, file_uri.as_deref())
    }
}

// ──────────────────────────────────────────────────────────────
// WinRT implementation (internal)
// ──────────────────────────────────────────────────────────────

fn send_toast(title: &str, body: &str, file_uri: Option<&str>) -> Result<()> {
    let xml = build_toast_xml(title, body, file_uri);
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

fn build_toast_xml(title: &str, body: &str, file_uri: Option<&str>) -> String {
    let visual = format!(
        r#"<visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual>"#,
        xml_escape(title),
        xml_escape(body),
    );

    match file_uri {
        Some(uri) => {
            let uri_escaped = xml_escape(uri);
            format!(
                r#"<toast activationType="protocol" launch="{uri}"><actions><action content="Ouvrir le rapport" arguments="{uri}" activationType="protocol"/></actions>{visual}</toast>"#,
                uri = uri_escaped,
                visual = visual,
            )
        }
        None => format!("<toast>{}</toast>", visual),
    }
}

/// Converts a Windows path to a `file:///` URI (forward slashes, spaces as %20).
fn path_to_file_uri(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    // Encode only spaces — common enough in Windows paths, other chars are fine for file URIs.
    let encoded = s.replace(' ', "%20");
    format!("file:///{}", encoded)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
