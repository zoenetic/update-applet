// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! Running programs and checking files on the host system.
//!
//! Inside the Flatpak sandbox the host's package tools, terminals and files
//! are not visible, so programs run through `flatpak-spawn --host` (allowed
//! by the manifest's `--talk-name=org.freedesktop.Flatpak`). Outside a
//! sandbox everything runs directly.

use std::{path::Path, process::Stdio, sync::LazyLock};

static SANDBOXED: LazyLock<bool> = LazyLock::new(|| Path::new("/.flatpak-info").exists());

/// Whether the applet is running inside a Flatpak sandbox.
pub fn sandboxed() -> bool {
    *SANDBOXED
}

/// A command that runs `program` on the host, with extra environment
/// variables. Variables must go through here: `flatpak-spawn` does not pass on
/// the caller's environment.
pub fn std_command(program: &str, env: &[(&str, &str)]) -> std::process::Command {
    if sandboxed() {
        let mut cmd = std::process::Command::new("flatpak-spawn");
        cmd.arg("--host");
        cmd.args(
            env.iter()
                .map(|(key, value)| format!("--env={key}={value}")),
        );
        cmd.arg(program);
        cmd
    } else {
        let mut cmd = std::process::Command::new(program);
        cmd.envs(env.iter().copied());
        cmd
    }
}

/// [`std_command`] for async code, without extra environment variables.
pub fn command(program: &str) -> tokio::process::Command {
    std_command(program, &[]).into()
}

/// Whether `program` is an executable in the host's `PATH`.
pub async fn in_path(program: &str) -> bool {
    if sandboxed() {
        return quiet_success(command("sh").args(["-c", r#"command -v "$1""#, "sh", program]))
            .await;
    }

    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            std::fs::metadata(dir.join(program)).is_ok_and(|m| {
                use std::os::unix::fs::PermissionsExt;
                m.is_file() && m.permissions().mode() & 0o111 != 0
            })
        })
    })
}

/// Whether `path` exists on the host.
pub async fn exists(path: &str) -> bool {
    if sandboxed() {
        quiet_success(command("test").args(["-e", path])).await
    } else {
        Path::new(path).exists()
    }
}

/// The host's `os-release` file, which Flatpak exposes under `/run/host`.
pub fn os_release() -> Option<String> {
    let path = if sandboxed() {
        "/run/host/os-release"
    } else {
        "/etc/os-release"
    };
    std::fs::read_to_string(path).ok()
}

async fn quiet_success(cmd: &mut tokio::process::Command) -> bool {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}
