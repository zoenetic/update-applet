// Copyright 2026 the cosmic-applet-updates authors
// SPDX-License-Identifier: GPL-3.0-only

//! Prints the updates the applet would show, without starting the applet.
//!
//! `cargo run --example check -- [--refresh] [--no-flatpak]`

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let refresh = args.iter().any(|a| a == "--refresh");
    let flatpak = !args.iter().any(|a| a == "--no-flatpak");

    let updates = cosmic_applet_updates::backend::check(refresh, flatpak).await;

    println!("System type: {:?}", updates.system_kind);
    for (title, list) in [("System", &updates.system), ("Flatpak", &updates.flatpak)] {
        match list {
            Some(list) => {
                println!("{title}: {} update(s)", list.len());
                for u in list {
                    println!("  {:<40} {:<30} {:?}", u.name, u.version, u.kind);
                }
            }
            None => println!("{title}: unavailable"),
        }
    }
    println!("Staged for next boot: {}", updates.offline_prepared);
    for e in &updates.errors {
        println!("Error: {e}");
    }
}
