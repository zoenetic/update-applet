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

### From a release

Each [release](https://github.com/zoenetic/update-applet/releases) has a prebuilt
x86_64 tarball. Unpack it, then from inside the folder:

```sh
sudo just install
```

## Development

```sh
just check           # clippy + tests
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
