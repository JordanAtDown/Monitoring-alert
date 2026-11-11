//! GitHub Releases adapter: asks for the latest published release of the project.
//!
//! One anonymous request, with a generic user agent: nothing about the machine or the
//! user is sent.

use anyhow::{anyhow, Context, Result};
use std::time::Duration;

use crate::application::ports::{ReleaseInfo, UpdateSource};
use crate::domain::version::without_prefix;

const NOTES_MAX_CHARS: usize = 1500;

pub struct GithubReleases {
    /// `owner/name`.
    repo: String,
    user_agent: String,
}

impl GithubReleases {
    pub fn new(repo: &str, app_version: &str) -> Self {
        Self {
            repo: repo.to_string(),
            user_agent: format!("monitoring/{app_version}"),
        }
    }
}

/// Only links to GitHub itself are ever offered to the user.
fn github_url(url: &str) -> Option<String> {
    url.starts_with("https://github.com/")
        .then(|| url.to_string())
}

/// Reads the JSON answer of `GET /repos/{repo}/releases/latest`.
pub fn parse_release(json: &serde_json::Value) -> Result<ReleaseInfo> {
    let tag = json["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow!("no tag_name in the release"))?;
    let page_url = json["html_url"]
        .as_str()
        .and_then(github_url)
        .ok_or_else(|| anyhow!("no GitHub page for the release"))?;
    // The installer is the only file published: monitoring_<version>.exe
    let download_url = json["assets"].as_array().and_then(|assets| {
        assets.iter().find_map(|a| {
            let name = a["name"].as_str()?;
            (name.starts_with("monitoring_") && name.ends_with(".exe"))
                .then(|| a["browser_download_url"].as_str().and_then(github_url))
                .flatten()
        })
    });
    let notes = json["body"]
        .as_str()
        .map(|b| b.trim().chars().take(NOTES_MAX_CHARS).collect::<String>())
        .filter(|b| !b.is_empty());
    Ok(ReleaseInfo {
        version: without_prefix(tag).to_string(),
        page_url,
        download_url,
        notes,
        published_at: json["published_at"].as_str().map(str::to_string),
    })
}

impl UpdateSource for GithubReleases {
    fn latest_release(&self) -> Result<ReleaseInfo> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", self.repo);
        let json: serde_json::Value = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .user_agent(&self.user_agent)
            .build()
            .get(&url)
            .set("Accept", "application/vnd.github+json")
            .call()
            .context("Cannot reach GitHub to look for updates")?
            .into_json()
            .context("Unexpected answer from GitHub")?;
        parse_release(&json)
    }
}
