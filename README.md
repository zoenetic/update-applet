# cosmic-applet-updates

A COSMIC panel applet that shows available updates and installs them.

- System updates on every distro that ships COSMIC (see [Distro support](#distro-support)).
- Flatpak apps and runtimes through the `flatpak` command.
- Panel icon changes when updates are waiting, and turns urgent for security updates.
  The update count can be shown as a badge on it. By default it hides itself when
  there is nothing to install (turn off *Hide when up to date* to keep it visible).
- The popup lists updates per source and can:
  - install everything now,
  - download system updates and install them on the next restart (Fedora's
    recommended "offline update"), then restart via the COSMIC dialog,
  - check now, or open COSMIC Store.
- Checks automatically every 1, 6 (default), 12 or 24 hours, or never.
- Settings live in a separate **Updates Settings** window, so they are reachable
  even while the applet is hidden: open it from the app launcher, the applet's
  **Settings…** item, or `cosmic-applet-updates --settings`.

Installs ask for your password through polkit, like COSMIC Store does.

## Distro support

The applet picks the right method for the system it runs on:

| Distro | How system updates work | Needs |
|---|---|---|
| Fedora COSMIC Spin, Pop!_OS, Ubuntu, Debian, openSUSE | PackageKit; install now or on next restart | `PackageKit` |
| Fedora COSMIC Atomic, Bazzite, Universal Blue | rpm-ostree; the new image is staged and installs on next restart | `rpm-ostree` (built in) |
| Arch, CachyOS, EndeavourOS, Manjaro | listed with `checkupdates`; installing opens a terminal running `sudo pacman -Syu` | `pacman-contrib` |
| NixOS | not handled: updates come from rebuilding your configuration | – |

Flatpak updates work on all of them when `flatpak` is installed.

To try another method on your own machine (for development), set
`COSMIC_APPLET_UPDATES_SYSTEM` to `packagekit`, `rpm-ostree`, `pacman` or `nixos`.

## Requirements

- COSMIC desktop
- One of the update methods above
- `flatpak` (optional)

Build dependencies on Fedora:

```sh
sudo dnf install cargo just libxkbcommon-devel wayland-devel
```

## Build and install

```sh
just
sudo just install
```

Then add **Updates** to a panel in *Settings → Desktop → Panel → Configure panel applets*.

To remove: `sudo just uninstall`.

### Distro packages

Packaging files live in `packaging/`; each needs a tagged release (`v0.1.0`, …).

- **Pop!_OS, Ubuntu, Debian:** each release has a `.deb`:
  `sudo apt install ./cosmic-applet-updates_*_amd64.deb`. Build one yourself with
  `cargo install cargo-deb` and `just deb`.
- **Fedora (including COSMIC Atomic):** `packaging/fedora/cosmic-applet-updates.spec`.
  For [COPR](https://copr.fedorainfracloud.org/), add a package with source type
  *SCM*, this repo's URL, build method *make srpm*, and spec
  `packaging/fedora/cosmic-applet-updates.spec`; COPR then runs `.copr/Makefile`,
  which vendors the Rust crates so the RPM builds offline. Users then run
  `sudo dnf copr enable <you>/cosmic-applet-updates && sudo dnf install cosmic-applet-updates`
  (on Atomic, add the COPR repo file and use `rpm-ostree install`).
- **Flatpak:** each release has a `.flatpak` bundle built by CI:
  `flatpak install --user cosmic-applet-updates-*.flatpak` (needs the Flathub remote for
  its runtime). Build one yourself with `just flatpak`; setup is in
  [`flatpak/README.md`](flatpak/README.md). It is not on Flathub or the COSMIC Flatpak
  repository yet.
- **Arch and derivatives:** `packaging/arch/PKGBUILD` (`makepkg -si` from that folder).
  To publish it on the AUR, run `updpkgsums` and `makepkg --printsrcinfo > .SRCINFO`
  after tagging.

### From a release

Each [release](https://github.com/zoenetic/update-applet/releases) has a prebuilt
x86_64 tarball. Unpack it, then from inside the folder:

```sh
sudo just install
```

## Development

```sh
just check           # clippy + tests
just audit           # check dependencies for known vulnerabilities
just list-updates    # print what the applet sees (add --refresh to refresh metadata)
just run             # run outside the panel
just settings        # open the settings window
```

Code layout:

- `src/lib.rs` – the applet (panel button, popup, state)
- `src/backend/` – update sources: `packagekit.rs`, `rpm_ostree.rs`, `pacman.rs`, `flatpak.rs`
- `src/settings.rs` – the settings window
- `src/config.rs` – stored settings
- `i18n/` – translations (Fluent)

### Adding another distro

Distros that ship PackageKit already work. For one that does not, add a
module in `src/backend/`, a `SystemKind` variant with its detection in
`SystemKind::detect`, and hook it into `backend::check` and `backend::install`.

## License

GPL-3.0-only
