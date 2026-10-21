// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! Image-based system updates through `rpm-ostree`, for Fedora COSMIC Atomic
//! and other ostree systems (Bazzite, Universal Blue, ...).
//!
//! The whole OS is one update. Installing it stages a new deployment that
//! takes effect on the next boot, so there is no "install now". The commands
//! go through the rpm-ostree daemon, which asks for authorization via polkit.

use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;

use super::{Kind, Update};

/// `rpm-ostree upgrade --check` exits with this code when nothing is available.
const EXIT_NO_UPDATE: i32 = 77;

/// What a check found.
#[derive(Debug, Default)]
pub struct Status {
    /// The available image, unless it is already the staged one.
    pub update: Option<Update>,
    /// A deployment is staged and will boot next.
    pub staged: bool,
}

#[derive(Deserialize)]
struct StatusJson {
    deployments: Vec<Deployment>,
}

#[derive(Deserialize)]
struct Deployment {
    #[serde(default)]
    staged: bool,
    checksum: String,
}

/// Whether this system boots an ostree deployment managed by rpm-ostree.
pub fn detected() -> bool {
    std::path::Path::new("/run/ostree-booted").exists() && super::in_path("rpm-ostree")
}

pub async fn check() -> Result<Status, String> {
    let staged_commit = staged_commit().await?;

    let output = Command::new("rpm-ostree")
        .args(["upgrade", "--check"])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;

    let update = match output.status.code() {
        Some(0) => parse_check(&String::from_utf8_lossy(&output.stdout), os_name()),
        Some(EXIT_NO_UPDATE) => None,
        _ => return Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    };

    // Once an update is staged, `--check` keeps reporting it until reboot.
    let update = update.filter(|u| staged_commit.as_deref() != Some(u.id.as_str()));

    Ok(Status {
        update,
        staged: staged_commit.is_some(),
    })
}

/// Downloads the latest image and stages it for the next boot.
pub async fn upgrade() -> Result<(), String> {
    let output = Command::new("rpm-ostree")
        .arg("upgrade")
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

async fn staged_commit() -> Result<Option<String>, String> {
    let output = Command::new("rpm-ostree")
        .args(["status", "--json"])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    parse_staged(&output.stdout)
}

fn parse_staged(json: &[u8]) -> Result<Option<String>, String> {
    let status: StatusJson = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    Ok(status
        .deployments
        .into_iter()
        .find(|d| d.staged)
        .map(|d| d.checksum))
}

/// Parses the `AvailableUpdate:` block printed by `rpm-ostree upgrade --check`.
fn parse_check(stdout: &str, os_name: String) -> Option<Update> {
    let mut lines = stdout
        .lines()
        .skip_while(|l| !l.trim().starts_with("AvailableUpdate:"));
    lines.next()?;

    let mut version = None;
    let mut commit = None;
    let mut diff = None;
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            // "44.20261019.0 (2026-10-19T00:43:04Z)"
            "Version" => version = value.split_whitespace().next().map(str::to_string),
            "Commit" => commit = Some(value.to_string()),
            "Diff" => diff = Some(value.to_string()),
            _ => (),
        }
    }

    let commit = commit?;
    Some(Update {
        summary: diff.unwrap_or_else(|| commit.chars().take(12).collect()),
        id: commit,
        name: os_name,
        version: version.unwrap_or_default(),
        kind: Kind::Normal,
    })
}

/// The OS name to show for the image, from `/etc/os-release`.
fn os_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|release| {
            release.lines().find_map(|line| {
                line.strip_prefix("NAME=")
                    .map(|name| name.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| "System image".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECK: &str = "\
1 metadata, 0 images
Note: --check and --preview may be unreliable.  See https://github.com/coreos/rpm-ostree/issues/1579
AvailableUpdate:
        Version: 44.20261019.0 (2026-10-19T00:43:04Z)
         Commit: 2ac5a5e5c5d1f0a6b3e8f9c0d7e6b5a4c3d2e1f0a9b8c7d6e5f4a3b2c1d0e9f8
   GPGSignature: Valid signature by 115DF9AEF857853EE8445D0A0727707EA15B79CC
           Diff: 20 upgraded, 1 added
";

    #[test]
    fn parses_available_update() {
        let update = parse_check(CHECK, "Fedora Linux".into()).unwrap();
        assert_eq!(update.version, "44.20261019.0");
        assert!(update.id.starts_with("2ac5a5e5c5d1"));
        assert_eq!(update.summary, "20 upgraded, 1 added");
        assert_eq!(update.name, "Fedora Linux");
    }

    #[test]
    fn no_update_block_means_no_update() {
        assert!(parse_check("1 metadata, 0 images\nNo updates available.\n", "x".into()).is_none());
    }

    #[test]
    fn finds_staged_deployment() {
        let json = br#"{"deployments":[
            {"checksum":"new","staged":true,"booted":false},
            {"checksum":"old","staged":false,"booted":true}
        ]}"#;
        assert_eq!(parse_staged(json).unwrap().as_deref(), Some("new"));

        let json = br#"{"deployments":[{"checksum":"old","booted":true}]}"#;
        assert_eq!(parse_staged(json).unwrap(), None);
    }
}
