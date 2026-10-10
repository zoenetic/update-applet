// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! System package updates through the PackageKit D-Bus API.
//!
//! PackageKit wraps the native package manager (dnf on Fedora, apt on Debian,
//! zypper on openSUSE, ...), so this backend is not Fedora specific.
//! Authorization for installs goes through polkit, which COSMIC answers with
//! its own password prompt.

use std::collections::HashMap;

use futures::{Stream, StreamExt};
use zbus::{
    Connection, MatchRule, Message, MessageStream, proxy,
    zvariant::{OwnedObjectPath, OwnedValue},
};

use super::{Kind, Update};

/// `PK_FILTER_ENUM_NONE` as a bitfield.
const FILTER_NONE: u64 = 1 << 1;
/// `PK_TRANSACTION_FLAG_ENUM_ONLY_TRUSTED`
const FLAG_ONLY_TRUSTED: u64 = 1 << 1;
/// `PK_TRANSACTION_FLAG_ENUM_ONLY_DOWNLOAD`
const FLAG_ONLY_DOWNLOAD: u64 = 1 << 3;
/// `PK_EXIT_ENUM_SUCCESS`
const EXIT_SUCCESS: u32 = 1;
/// `PK_EXIT_ENUM_CANCELLED`
const EXIT_CANCELLED: u32 = 3;
/// Reported by PackageKit when the percentage is unknown.
const PERCENTAGE_UNKNOWN: u32 = 101;

#[proxy(
    interface = "org.freedesktop.PackageKit",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
pub trait PackageKit {
    fn create_transaction(&self) -> zbus::Result<OwnedObjectPath>;

    #[zbus(signal)]
    fn updates_changed(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.PackageKit.Offline",
    default_service = "org.freedesktop.PackageKit",
    default_path = "/org/freedesktop/PackageKit"
)]
pub trait Offline {
    fn trigger(&self, action: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn update_prepared(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn trigger_action(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.PackageKit.Transaction",
    default_service = "org.freedesktop.PackageKit"
)]
pub trait Transaction {
    fn set_hints(&self, hints: &[&str]) -> zbus::Result<()>;

    fn refresh_cache(&self, force: bool) -> zbus::Result<()>;

    fn get_updates(&self, filter: u64) -> zbus::Result<()>;

    fn update_packages(&self, transaction_flags: u64, package_ids: &[&str]) -> zbus::Result<()>;
}

enum Op<'a> {
    RefreshCache,
    GetUpdates,
    UpdatePackages { flags: u64, ids: &'a [String] },
}

#[derive(Debug)]
enum TxEvent {
    Package {
        info: u32,
        id: String,
        summary: String,
    },
    ErrorCode(String),
    Percentage(Option<u32>),
    Finished {
        exit: u32,
    },
}

/// Creates a transaction, starts `op` on it and returns its signals in order.
///
/// A single match rule on the transaction path is used rather than one stream
/// per signal, so `Finished` can never overtake the last `Package`.
async fn start(
    conn: &Connection,
    hints: &[&str],
    op: Op<'_>,
) -> zbus::Result<impl Stream<Item = TxEvent> + use<>> {
    let path = PackageKitProxy::new(conn)
        .await?
        .create_transaction()
        .await?;

    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path(path.clone())?
        .build();
    let signals = MessageStream::for_match_rule(rule, conn, Some(256)).await?;

    let tx = TransactionProxy::builder(conn).path(path)?.build().await?;
    tx.set_hints(hints).await?;
    match op {
        Op::RefreshCache => tx.refresh_cache(false).await?,
        Op::GetUpdates => tx.get_updates(FILTER_NONE).await?,
        Op::UpdatePackages { flags, ids } => {
            let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
            tx.update_packages(flags, &ids).await?;
        }
    }

    Ok(signals.filter_map(|msg| async move { parse_signal(&msg.ok()?) }))
}

fn parse_signal(msg: &Message) -> Option<TxEvent> {
    let header = msg.header();
    let body = msg.body();
    match header.member()?.as_str() {
        "Package" => {
            let (info, id, summary): (u32, String, String) = body.deserialize().ok()?;
            Some(TxEvent::Package { info, id, summary })
        }
        "ErrorCode" => {
            let (_code, details): (u32, String) = body.deserialize().ok()?;
            Some(TxEvent::ErrorCode(details))
        }
        "Finished" => {
            let (exit, _runtime): (u32, u32) = body.deserialize().ok()?;
            Some(TxEvent::Finished { exit })
        }
        "PropertiesChanged" => {
            let (_iface, changed, _invalidated): (
                String,
                HashMap<String, OwnedValue>,
                Vec<String>,
            ) = body.deserialize().ok()?;
            let percentage = changed.get("Percentage")?.downcast_ref::<u32>().ok()?;
            Some(TxEvent::Percentage(
                (percentage < PERCENTAGE_UNKNOWN).then_some(percentage),
            ))
        }
        _ => None,
    }
}

/// Runs a transaction to completion, passing every event to `on_event`.
async fn run(
    conn: &Connection,
    hints: &[&str],
    op: Op<'_>,
    mut on_event: impl FnMut(&TxEvent),
) -> Result<(), String> {
    let mut events = std::pin::pin!(start(conn, hints, op).await.map_err(|e| e.to_string())?);
    let mut error = None;

    while let Some(event) = events.next().await {
        on_event(&event);
        match event {
            TxEvent::ErrorCode(details) => error = Some(details),
            TxEvent::Finished { exit } => {
                return match exit {
                    EXIT_SUCCESS => Ok(()),
                    EXIT_CANCELLED => Err(error.unwrap_or_else(|| "cancelled".into())),
                    _ => Err(error.unwrap_or_else(|| format!("transaction failed ({exit})"))),
                };
            }
            _ => (),
        }
    }

    Err("PackageKit closed the transaction unexpectedly".into())
}

/// Whether PackageKit is installed and reachable.
pub async fn available(conn: &Connection) -> bool {
    match PackageKitProxy::new(conn).await {
        Ok(proxy) => proxy.inner().introspect().await.is_ok(),
        Err(_) => false,
    }
}

/// Whether updates have already been downloaded and staged for the next boot.
pub async fn offline_prepared(conn: &Connection) -> bool {
    match OfflineProxy::new(conn).await {
        Ok(proxy) => proxy.update_prepared().await.unwrap_or(false),
        Err(_) => false,
    }
}

/// Lists available updates, optionally refreshing the metadata first.
pub async fn get_updates(conn: &Connection, refresh: bool) -> Result<Vec<Update>, String> {
    if refresh {
        // A failed refresh (no network, a broken repo) should not hide updates
        // already known from the cached metadata.
        if let Err(why) = run(conn, &["background=true"], Op::RefreshCache, |_| ()).await {
            tracing::warn!("failed to refresh package metadata: {why}");
        }
    }

    let mut updates = Vec::new();
    run(conn, &["background=true"], Op::GetUpdates, |event| {
        if let TxEvent::Package { info, id, summary } = event
            && let Some(kind) = kind_from_info(*info)
        {
            updates.push(update_from_id(id, summary, kind));
        }
    })
    .await?;

    updates.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(updates)
}

/// Installs the given packages, reporting progress as a percentage.
///
/// When `offline` is set the packages are only downloaded and staged, then
/// installed by PackageKit on the next reboot.
pub async fn update(
    conn: &Connection,
    ids: &[String],
    offline: bool,
    mut on_progress: impl FnMut(Option<u32>),
) -> Result<(), String> {
    let flags = if offline {
        FLAG_ONLY_TRUSTED | FLAG_ONLY_DOWNLOAD
    } else {
        FLAG_ONLY_TRUSTED
    };

    run(
        conn,
        &["interactive=true"],
        Op::UpdatePackages { flags, ids },
        |event| {
            if let TxEvent::Percentage(percentage) = event {
                on_progress(*percentage);
            }
        },
    )
    .await?;

    if offline {
        OfflineProxy::new(conn)
            .await
            .map_err(|e| e.to_string())?
            .trigger("reboot")
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Emits whenever PackageKit reports that the set of updates changed, for
/// example after another tool installed packages.
pub async fn updates_changed(conn: &Connection) -> zbus::Result<impl Stream<Item = ()> + use<>> {
    let proxy = PackageKitProxy::new(conn).await?;
    Ok(proxy.receive_updates_changed().await?.map(|_| ()))
}

/// Maps `PK_INFO_ENUM` to an update kind, skipping blocked updates.
fn kind_from_info(info: u32) -> Option<Kind> {
    Some(match info {
        3 => Kind::Low,
        4 => Kind::Enhancement,
        6 => Kind::Bugfix,
        7 => Kind::Important,
        8 => Kind::Security,
        // PK_INFO_ENUM_BLOCKED: held back by the package manager.
        9 => return None,
        _ => Kind::Normal,
    })
}

/// Package IDs have the form `name;version;arch;repo`.
fn update_from_id(id: &str, summary: &str, kind: Kind) -> Update {
    let mut parts = id.split(';');
    let name = parts.next().unwrap_or(id).to_string();
    let version = parts.next().unwrap_or_default().to_string();

    Update {
        id: id.to_string(),
        name,
        version,
        summary: summary.to_string(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_id() {
        let update = update_from_id(
            "firefox;131.0-1.fc41;x86_64;updates",
            "Mozilla Firefox Web browser",
            Kind::Security,
        );
        assert_eq!(update.name, "firefox");
        assert_eq!(update.version, "131.0-1.fc41");
        assert_eq!(update.id, "firefox;131.0-1.fc41;x86_64;updates");
    }

    #[test]
    fn skips_blocked_updates() {
        assert_eq!(kind_from_info(9), None);
        assert_eq!(kind_from_info(8), Some(Kind::Security));
        assert_eq!(kind_from_info(5), Some(Kind::Normal));
    }
}
