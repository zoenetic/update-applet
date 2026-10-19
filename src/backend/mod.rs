// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! Update sources. Each source lists and installs its own updates; the applet
//! only sees the combined result.
//!
//! System packages come from whichever [`SystemKind`] this machine uses,
//! picked by [`SystemKind::detect`]. To support another distribution, add a
//! module here, a [`SystemKind`] variant, and hook it into [`check`] and
//! [`install`].

pub mod flatpak;
pub mod packagekit;
pub mod pacman;
pub mod rpm_ostree;

use std::{hash::Hash, time::Duration};

use cosmic::iced::{Subscription, stream};
use futures::{SinkExt, StreamExt, channel::mpsc};
use zbus::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    /// Distribution packages or the OS image, see [`SystemKind`].
    System,
    Flatpak,
}

/// How this machine's own packages are updated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SystemKind {
    /// PackageKit: Fedora, Pop!_OS, Ubuntu, Debian, openSUSE and others.
    PackageKit,
    /// Image-based ostree systems such as Fedora COSMIC Atomic.
    RpmOstree,
    /// Arch and derivatives.
    Pacman,
    /// NixOS, where updates come from rebuilding the system configuration.
    NixOs,
    /// Nothing supported was found.
    #[default]
    Unsupported,
}

impl SystemKind {
    /// Environment variable that overrides detection, for testing.
    const OVERRIDE: &str = "COSMIC_APPLET_UPDATES_SYSTEM";

    /// Picks the update method from what is installed. Cheap enough to call
    /// on every check.
    pub fn detect() -> Self {
        match std::env::var(Self::OVERRIDE).as_deref() {
            Ok("packagekit") => return Self::PackageKit,
            Ok("rpm-ostree") => return Self::RpmOstree,
            Ok("pacman") => return Self::Pacman,
            Ok("nixos") => return Self::NixOs,
            _ => (),
        }

        // Atomic systems may also ship PackageKit, which cannot update them.
        if rpm_ostree::detected() {
            Self::RpmOstree
        } else if std::path::Path::new("/etc/NIXOS").exists() {
            Self::NixOs
        } else if pacman::detected() {
            Self::Pacman
        } else {
            Self::PackageKit
        }
    }

    /// Updates can be applied to the running system.
    pub fn can_install_now(self) -> bool {
        matches!(self, Self::PackageKit | Self::Pacman)
    }

    /// Updates can be staged to install on the next boot.
    pub fn can_stage(self) -> bool {
        matches!(self, Self::PackageKit | Self::RpmOstree)
    }
}

/// Whether `program` is an executable somewhere in `PATH`.
pub fn in_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            std::fs::metadata(dir.join(program)).is_ok_and(|m| {
                m.is_file()
                    && std::os::unix::fs::PermissionsExt::mode(&m.permissions()) & 0o111 != 0
            })
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Low,
    Normal,
    Enhancement,
    Bugfix,
    Important,
    Security,
}

#[derive(Debug, Clone)]
pub struct Update {
    /// Identifier the source needs to install this update.
    pub id: String,
    pub name: String,
    pub version: String,
    pub summary: String,
    pub kind: Kind,
}

/// Result of a check. A source is `None` when it is not available on this
/// system, so the applet can hide it instead of showing an empty list.
#[derive(Debug, Clone, Default)]
pub struct Updates {
    pub system_kind: SystemKind,
    pub system: Option<Vec<Update>>,
    pub flatpak: Option<Vec<Update>>,
    /// System updates are downloaded and will install on the next boot.
    pub offline_prepared: bool,
    pub errors: Vec<String>,
}

impl Updates {
    pub fn system(&self) -> &[Update] {
        self.system.as_deref().unwrap_or_default()
    }

    pub fn flatpak(&self) -> &[Update] {
        self.flatpak.as_deref().unwrap_or_default()
    }

    pub fn count(&self) -> usize {
        self.system().len() + self.flatpak().len()
    }

    pub fn has_security(&self) -> bool {
        self.system().iter().any(|u| u.kind == Kind::Security)
    }
}

/// Checks every available source for updates.
pub async fn check(refresh: bool, include_flatpak: bool) -> Updates {
    let mut updates = Updates {
        system_kind: SystemKind::detect(),
        ..Default::default()
    };

    let result = match updates.system_kind {
        SystemKind::PackageKit => check_packagekit(refresh, &mut updates).await,
        SystemKind::RpmOstree => rpm_ostree::check().await.map(|status| {
            updates.offline_prepared = status.staged;
            Some(status.update.into_iter().collect())
        }),
        SystemKind::Pacman => pacman::get_updates(refresh).await.map(Some),
        SystemKind::NixOs | SystemKind::Unsupported => Ok(None),
    };
    match result {
        Ok(list) => updates.system = list,
        Err(why) => {
            tracing::error!("failed to get system updates: {why}");
            updates.system = Some(Vec::new());
            updates.errors.push(why);
        }
    }

    if include_flatpak && flatpak::available().await {
        match flatpak::get_updates().await {
            Ok(list) => updates.flatpak = Some(list),
            Err(why) => {
                tracing::error!("failed to get flatpak updates: {why}");
                updates.flatpak = Some(Vec::new());
                updates.errors.push(why);
            }
        }
    }

    updates
}

/// Fills in PackageKit's updates, or marks the system unsupported when
/// PackageKit is not installed.
async fn check_packagekit(
    refresh: bool,
    updates: &mut Updates,
) -> Result<Option<Vec<Update>>, String> {
    let conn = Connection::system().await.map_err(|e| {
        tracing::error!("failed to connect to the system bus: {e}");
        e.to_string()
    })?;

    if !packagekit::available(&conn).await {
        tracing::warn!("PackageKit is not available, system updates are disabled");
        updates.system_kind = SystemKind::Unsupported;
        return Ok(None);
    }

    updates.offline_prepared = packagekit::offline_prepared(&conn).await;
    packagekit::get_updates(&conn, refresh).await.map(Some)
}

/// What to install. `nonce` makes every request a new subscription, so the
/// same set of updates can be installed again after a failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstallJob {
    pub nonce: u64,
    pub system_kind: SystemKind,
    pub system_ids: Vec<String>,
    pub flatpak: bool,
    /// Stage system updates for the next boot instead of installing them now.
    pub offline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    System,
    Flatpak,
}

#[derive(Debug, Clone)]
pub enum InstallEvent {
    Progress {
        stage: Stage,
        percentage: Option<u32>,
    },
    /// Every requested update was applied or staged.
    Finished,
    Failed(String),
}

/// Runs an install job, reporting progress until it finishes or fails.
pub fn install(job: InstallJob) -> Subscription<InstallEvent> {
    Subscription::run_with(job, |job| {
        let job = job.clone();
        stream::channel(
            16,
            move |mut output: mpsc::Sender<InstallEvent>| async move {
                let result = run_install(&job, &mut output).await;
                let _ = output
                    .send(match result {
                        Ok(()) => InstallEvent::Finished,
                        Err(why) => InstallEvent::Failed(why),
                    })
                    .await;
                // Keep the subscription alive until the applet drops it, otherwise
                // iced would restart the job.
                futures::future::pending::<()>().await;
            },
        )
    })
}

async fn run_install(
    job: &InstallJob,
    output: &mut mpsc::Sender<InstallEvent>,
) -> Result<(), String> {
    if !job.system_ids.is_empty() {
        let _ = output
            .send(InstallEvent::Progress {
                stage: Stage::System,
                percentage: None,
            })
            .await;

        match job.system_kind {
            SystemKind::PackageKit => {
                let conn = Connection::system().await.map_err(|e| e.to_string())?;
                let mut progress = output.clone();
                packagekit::update(&conn, &job.system_ids, job.offline, |percentage| {
                    let _ = progress.try_send(InstallEvent::Progress {
                        stage: Stage::System,
                        percentage,
                    });
                })
                .await?;
            }
            SystemKind::RpmOstree => rpm_ostree::upgrade().await?,
            SystemKind::Pacman => pacman::update_in_terminal().await?,
            SystemKind::NixOs | SystemKind::Unsupported => (),
        }
    }

    if job.flatpak {
        let _ = output
            .send(InstallEvent::Progress {
                stage: Stage::Flatpak,
                percentage: None,
            })
            .await;
        flatpak::update().await?;
    }

    Ok(())
}

/// Emits when PackageKit reports that the available updates changed.
pub fn updates_changed() -> Subscription<()> {
    Subscription::run(|| {
        stream::channel(4, |mut output: mpsc::Sender<()>| async move {
            let mut retry = 0u32;
            loop {
                if let Ok(conn) = Connection::system().await
                    && let Ok(mut changes) = packagekit::updates_changed(&conn).await
                {
                    retry = 0;
                    while changes.next().await.is_some() {
                        let _ = output.send(()).await;
                    }
                }

                // PackageKit is activated on demand and exits when idle, so
                // keep reconnecting with a capped backoff.
                retry = retry.saturating_add(1);
                tokio::time::sleep(Duration::from_secs(2u64.pow(retry.min(8)))).await;
            }
        })
    })
}

/// Emits once every `hours` hours. Changing `hours` restarts the timer.
pub fn timer(hours: u32) -> Subscription<()> {
    Subscription::run_with(hours, |hours| {
        let period = Duration::from_secs(u64::from(*hours) * 3600);
        stream::channel(1, move |mut output: mpsc::Sender<()>| async move {
            let mut interval =
                tokio::time::interval_at(tokio::time::Instant::now() + period, period);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                let _ = output.send(()).await;
            }
        })
    })
}
