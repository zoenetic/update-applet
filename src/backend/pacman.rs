// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! System updates on Arch and derivatives (CachyOS, EndeavourOS, Manjaro).
//!
//! Updates are listed with `checkupdates` from pacman-contrib, which syncs a
//! private copy of the package databases and so never causes a partial
//! upgrade. Installing opens a terminal running `sudo pacman -Syu`, because
//! Arch expects the user to see its prompts, news and `.pacnew` notices.

use std::process::Stdio;

use super::{Kind, Update};
use crate::host;

/// `checkupdates` exits with this code when nothing is available.
const EXIT_NO_UPDATES: i32 = 2;

/// Terminals to try, with the arguments that make them run a command and
/// stay in the foreground until their window closes.
const TERMINALS: &[(&str, &[&str])] = &[
    ("cosmic-term", &["--no-daemon", "-e"]),
    ("xdg-terminal-exec", &[]),
];

const UPGRADE_SCRIPT: &str = "sudo pacman -Syu; echo; printf 'Press Enter to close. '; read -r _";

pub async fn detected() -> bool {
    host::in_path("pacman").await
}

/// Lists available updates. Without `refresh` the last synced databases are used.
pub async fn get_updates(refresh: bool) -> Result<Vec<Update>, String> {
    if !host::in_path("checkupdates").await {
        return Err("checkupdates not found: install pacman-contrib to see updates".into());
    }

    let mut cmd = host::command("checkupdates");
    if !refresh {
        cmd.arg("--nosync");
    }
    let output = cmd
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| e.to_string())?;

    match output.status.code() {
        Some(0) => Ok(parse_checkupdates(&String::from_utf8_lossy(&output.stdout))),
        Some(EXIT_NO_UPDATES) => Ok(Vec::new()),
        _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// Opens a terminal running `sudo pacman -Syu` and waits until it is closed.
pub async fn update_in_terminal() -> Result<(), String> {
    for (terminal, args) in TERMINALS {
        if !host::in_path(terminal).await {
            continue;
        }
        return host::command(terminal)
            .args(*args)
            .args(["sh", "-c", UPGRADE_SCRIPT])
            .stdin(Stdio::null())
            .status()
            .await
            .map(|_| ())
            .map_err(|e| e.to_string());
    }

    Err("no terminal found: run `sudo pacman -Syu` yourself".into())
}

/// Lines look like `linux 6.17.1.arch1-1 -> 6.17.2.arch1-1`.
fn parse_checkupdates(stdout: &str) -> Vec<Update> {
    let mut updates: Vec<Update> = stdout
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?;
            let old = parts.next().unwrap_or_default();
            let new = parts.nth(1).unwrap_or_default();
            Some(Update {
                id: name.to_string(),
                name: name.to_string(),
                version: new.to_string(),
                summary: format!("from {old}"),
                kind: Kind::Normal,
            })
        })
        .collect();
    updates.sort_by(|a, b| a.name.cmp(&b.name));
    updates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checkupdates_output() {
        let out = "linux 6.17.1.arch1-1 -> 6.17.2.arch1-1\nfirefox 143.0-1 -> 143.0.1-1\n\n";
        let updates = parse_checkupdates(out);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].name, "firefox");
        assert_eq!(updates[0].version, "143.0.1-1");
        assert_eq!(updates[1].summary, "from 6.17.1.arch1-1");
    }
}
