// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! Flatpak updates through the `flatpak` command line tool.

use std::process::Stdio;

use super::{Kind, Update};
use crate::host;

/// Whether the `flatpak` command is installed.
pub async fn available() -> bool {
    host::command("flatpak")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

/// Lists Flatpak apps and runtimes with updates, across system and user installs.
pub async fn get_updates() -> Result<Vec<Update>, String> {
    let output = host::command("flatpak")
        .args([
            "remote-ls",
            "--updates",
            "--columns=application,name,version,branch,origin",
        ])
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut updates = parse_remote_ls(&String::from_utf8_lossy(&output.stdout));
    updates.sort_by(|a, b| a.name.cmp(&b.name));
    updates.dedup_by(|a, b| a.id == b.id);
    Ok(updates)
}

/// Updates every installed Flatpak.
pub async fn update() -> Result<(), String> {
    let output = host::command("flatpak")
        .args(["update", "--noninteractive", "-y"])
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

fn parse_remote_ls(stdout: &str) -> Vec<Update> {
    stdout
        .lines()
        .filter_map(|line| {
            let mut columns = line.split('\t').map(str::trim);
            let app_id = columns.next().filter(|id| !id.is_empty())?;
            let name = columns.next().unwrap_or_default();
            let version = columns.next().unwrap_or_default();
            let branch = columns.next().unwrap_or_default();
            let origin = columns.next().unwrap_or_default();

            Some(Update {
                id: format!("{app_id}//{branch}"),
                name: if name.is_empty() { app_id } else { name }.to_string(),
                version: if version.is_empty() { branch } else { version }.to_string(),
                summary: format!("{app_id} ({origin})"),
                kind: Kind::Normal,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_remote_ls_output() {
        let out = "org.mozilla.firefox\tFirefox\t131.0\tstable\tflathub\n\
                   org.freedesktop.Platform.GL.default\tMesa\t\t24.08\tflathub\n\
                   \n";
        let updates = parse_remote_ls(out);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].name, "Firefox");
        assert_eq!(updates[0].version, "131.0");
        assert_eq!(updates[0].id, "org.mozilla.firefox//stable");
        // Falls back to the branch when there is no version.
        assert_eq!(updates[1].version, "24.08");
    }
}
