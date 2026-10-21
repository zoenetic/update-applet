# Flatpak

`io.github.zoenetic.CosmicAppletUpdates.json` builds the applet as a Flatpak in
the style of the [COSMIC Flatpak repository](https://github.com/pop-os/cosmic-flatpak),
where COSMIC applets are published.

## Build locally

One-time setup:

```sh
sudo dnf install flatpak-builder python3-aiohttp python3-tomlkit   # or your distro's equivalent
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak install --user flathub org.freedesktop.Platform//25.08 org.freedesktop.Sdk//25.08 \
    org.freedesktop.Sdk.Extension.rust-stable//25.08 com.system76.Cosmic.BaseApp//stable
```

Then from the repository root:

```sh
just flatpak
flatpak install --user target/flatpak/io.github.zoenetic.CosmicAppletUpdates.flatpak
```

`just flatpak` first runs `just flatpak-sources`, which writes `cargo-sources.json`
(every crate from `Cargo.lock`, so the sandboxed build runs offline). It is generated,
not committed; publishing to a Flatpak repository will need it committed next to the
manifest, with the `dir` source replaced by this repository's git URL and a commit.

## Sandbox

The applet talks to PackageKit over the system bus, and runs everything else on the
host with `flatpak-spawn --host` (`src/host.rs`): `flatpak`, `rpm-ostree`,
`checkupdates`, the terminal for pacman, `cosmic-osd` and `cosmic-store`. That needs
`--talk-name=org.freedesktop.Flatpak`, which lets the app run commands on the host,
so the Flatpak gives no real isolation; it is a packaging format here, not a sandbox.
