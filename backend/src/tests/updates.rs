use crate::adapters::github::parse_release;
use crate::application::ports::{ReleaseInfo, UpdateSource};
use crate::application::updates::{check_now, UpdateStatus};
use crate::domain::version::{is_newer, without_prefix};
use anyhow::{bail, Result};
use serde_json::json;

// ── Version numbers ───────────────────────────────────────────

#[test]
fn a_higher_version_is_newer() {
    assert!(is_newer("2.0.1", "2.0.0"));
    assert!(is_newer("2.1.0", "2.0.9"));
    assert!(is_newer("3.0.0", "2.99.99"));
    assert!(is_newer("v2.1.0", "2.0.0"));
}

#[test]
fn the_same_or_an_older_version_is_not_newer() {
    assert!(!is_newer("2.0.0", "2.0.0"));
    assert!(!is_newer("1.9.9", "2.0.0"));
    assert!(!is_newer("2.0.0", "2.0.1"));
}

#[test]
fn versions_are_compared_as_numbers_not_as_text() {
    assert!(is_newer("2.10.0", "2.9.0"));
    assert!(is_newer("10.0.0", "9.0.0"));
}

#[test]
fn something_that_is_not_a_version_is_never_newer() {
    for bad in ["", "latest", "2.0", "2.0.0.1", "a.b.c", "2.x.0"] {
        assert!(!is_newer(bad, "1.0.0"), "{bad}");
        assert!(!is_newer("9.9.9", bad), "{bad}");
    }
}

#[test]
fn a_pre_release_suffix_is_ignored() {
    assert!(is_newer("2.1.0-rc1", "2.0.0"));
    assert!(!is_newer("2.0.0-rc1", "2.0.0"));
    assert_eq!(without_prefix("v2.1.0"), "2.1.0");
}

// ── Reading GitHub's answer ───────────────────────────────────

fn github_release() -> serde_json::Value {
    json!({
        "tag_name": "v2.1.0",
        "name": "Monitoring v2.1.0",
        "html_url": "https://github.com/JordanAtDown/Monitoring-alert/releases/tag/v2.1.0",
        "published_at": "2026-11-20T10:00:00Z",
        "body": "Version mineure : un tableau de bord plus lisible.\n\n- Bouton de mise à jour",
        "assets": [
            {"name": "notes.txt",
             "browser_download_url": "https://github.com/x/y/releases/download/v2.1.0/notes.txt"},
            {"name": "monitoring_2.1.0.exe",
             "browser_download_url": "https://github.com/JordanAtDown/Monitoring-alert/releases/download/v2.1.0/monitoring_2.1.0.exe"}
        ]
    })
}

#[test]
fn the_installer_and_the_page_are_read_from_a_release() {
    let r = parse_release(&github_release()).unwrap();
    assert_eq!(r.version, "2.1.0");
    assert_eq!(
        r.page_url,
        "https://github.com/JordanAtDown/Monitoring-alert/releases/tag/v2.1.0"
    );
    assert!(r
        .download_url
        .as_deref()
        .unwrap()
        .ends_with("/monitoring_2.1.0.exe"));
    assert!(r.notes.as_deref().unwrap().starts_with("Version mineure"));
    assert_eq!(r.published_at.as_deref(), Some("2026-11-20T10:00:00Z"));
}

#[test]
fn a_release_without_an_installer_still_gives_its_page() {
    let mut j = github_release();
    j["assets"] = json!([]);
    let r = parse_release(&j).unwrap();
    assert_eq!(r.download_url, None);
}

#[test]
fn only_links_to_github_are_ever_offered() {
    let mut j = github_release();
    j["assets"][1]["browser_download_url"] = json!("https://evil.example/monitoring_2.1.0.exe");
    assert_eq!(parse_release(&j).unwrap().download_url, None);

    let mut j = github_release();
    j["html_url"] = json!("http://github.com/x/y");
    assert!(parse_release(&j).is_err(), "plain http is refused too");
}

#[test]
fn a_malformed_answer_is_an_error() {
    assert!(parse_release(&json!({})).is_err());
    assert!(parse_release(&json!({"tag_name": "v1.0.0"})).is_err());
}

#[test]
fn very_long_notes_are_shortened() {
    let mut j = github_release();
    j["body"] = json!("x".repeat(10_000));
    assert!(parse_release(&j).unwrap().notes.unwrap().chars().count() <= 1500);
}

// ── The status the dashboard reads ────────────────────────────

struct FakeSource(Result<ReleaseInfo, String>);

impl UpdateSource for FakeSource {
    fn latest_release(&self) -> Result<ReleaseInfo> {
        match &self.0 {
            Ok(r) => Ok(r.clone()),
            Err(e) => bail!("{e}"),
        }
    }
}

fn release(version: &str) -> ReleaseInfo {
    ReleaseInfo {
        version: version.into(),
        page_url: format!("https://github.com/x/y/releases/tag/v{version}"),
        download_url: None,
        notes: None,
        published_at: None,
    }
}

#[test]
fn before_any_check_nothing_is_available() {
    let status = UpdateStatus::new("2.0.0", true);
    let s = status.snapshot();
    assert!(!s.available && s.latest.is_none() && s.checked_at.is_none());
    assert_eq!(s.current, "2.0.0");
}

#[test]
fn a_newer_release_is_reported_as_available() {
    let status = UpdateStatus::new("2.0.0", true);
    assert!(check_now(&FakeSource(Ok(release("2.1.0"))), &status));
    let s = status.snapshot();
    assert!(s.available);
    assert_eq!(s.latest.unwrap().version, "2.1.0");
    assert!(s.checked_at.is_some() && s.error.is_none());
}

#[test]
fn being_up_to_date_is_not_an_update() {
    let status = UpdateStatus::new("2.1.0", true);
    check_now(&FakeSource(Ok(release("2.1.0"))), &status);
    assert!(!status.snapshot().available);
}

#[test]
fn a_failed_check_keeps_the_last_known_release_and_says_why() {
    let status = UpdateStatus::new("2.0.0", true);
    check_now(&FakeSource(Ok(release("2.1.0"))), &status);

    assert!(!check_now(
        &FakeSource(Err("GitHub unreachable".into())),
        &status
    ));
    let s = status.snapshot();
    assert!(s.available, "the earlier answer is still valid");
    assert!(s.error.unwrap().contains("GitHub unreachable"));
}

#[test]
fn a_successful_check_clears_the_error() {
    let status = UpdateStatus::new("2.0.0", true);
    check_now(&FakeSource(Err("offline".into())), &status);
    check_now(&FakeSource(Ok(release("2.0.0"))), &status);
    assert!(status.snapshot().error.is_none());
}

#[test]
fn when_checking_is_turned_off_nothing_is_ever_available() {
    let status = UpdateStatus::new("2.0.0", false);
    check_now(&FakeSource(Ok(release("9.9.9"))), &status);
    let s = status.snapshot();
    assert!(!s.enabled && !s.available);
}
