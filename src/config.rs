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
}

impl Default for UpdatesAppletConfig {
    fn default() -> Self {
        Self {
            check_interval_hours: 6,
            include_flatpak: true,
            show_count: true,
        }
    }
}

impl UpdatesAppletConfig {
    pub fn load(id: &str) -> Self {
        cosmic_config::Config::new(id, Self::VERSION)
            .map(|helper| Self::get_entry(&helper).unwrap_or_else(|(_errors, config)| config))
            .unwrap_or_default()
    }

    /// The interval that follows the current one in [`CHECK_INTERVALS`].
    pub fn next_check_interval(&self) -> u32 {
        let current = CHECK_INTERVALS
            .iter()
            .position(|&h| h == self.check_interval_hours)
            .unwrap_or(0);
        CHECK_INTERVALS[(current + 1) % CHECK_INTERVALS.len()]
    }
}
