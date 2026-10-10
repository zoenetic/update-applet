// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// Choices offered for the automatic check interval, in hours. 0 disables it.
pub const CHECK_INTERVALS: [u32; 5] = [1, 6, 12, 24, 0];

#[derive(Debug, Clone, CosmicConfigEntry, PartialEq, Eq, Deserialize, Serialize)]
#[version = 1]
pub struct UpdatesAppletConfig {
    /// Hours between automatic checks; 0 means only check on demand.
    pub check_interval_hours: u32,
    pub include_flatpak: bool,
    /// Show the number of updates next to the panel icon.
    pub show_count: bool,
    /// Take no space on the panel while there is nothing to install.
    pub hide_when_up_to_date: bool,
}

impl Default for UpdatesAppletConfig {
    fn default() -> Self {
        Self {
            check_interval_hours: 6,
            include_flatpak: true,
            show_count: true,
            hide_when_up_to_date: true,
        }
    }
}

impl UpdatesAppletConfig {
    pub fn load(id: &str) -> Self {
        cosmic_config::Config::new(id, Self::VERSION)
            .map(|helper| Self::get_entry(&helper).unwrap_or_else(|(_errors, config)| config))
            .unwrap_or_default()
    }
}

/// Opens the config for writing and applies `set`, logging any failure.
pub fn save(
    config: &mut UpdatesAppletConfig,
    id: &str,
    set: impl FnOnce(
        &mut UpdatesAppletConfig,
        &cosmic_config::Config,
    ) -> Result<bool, cosmic_config::Error>,
) {
    match cosmic_config::Config::new(id, UpdatesAppletConfig::VERSION) {
        Ok(helper) => {
            if let Err(why) = set(config, &helper) {
                tracing::error!(?why, "failed to save config");
            }
        }
        Err(why) => tracing::error!(?why, "failed to open config"),
    }
}
