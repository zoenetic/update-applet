// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> cosmic::iced::Result {
    tracing_subscriber::fmt::init();
    let _ = tracing_log::LogTracer::init();

    tracing::info!("Starting updates applet with version {VERSION}");

    cosmic_applet_updates::run()
}
