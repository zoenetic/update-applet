# cosmic-applet-updates

A COSMIC panel applet that shows available updates and installs them.

- System packages through [PackageKit](https://www.freedesktop.org/software/PackageKit/)
  (dnf on Fedora; PackageKit also covers apt, zypper and others).
- Flatpak apps and runtimes through the `flatpak` command.
- Panel icon changes when updates are waiting, and turns urgent for security updates.
  The update count can be shown next to it.
- The popup lists updates per source and can:
  - install everything now,
  - download system updates and install them on the next restart (Fedora's
    recommended "offline update"), then restart via the COSMIC dialog,
  - check now, or open COSMIC Store.
- Checks automatically every 1, 6 (default), 12 or 24 hours, or never.
  Settings are in the popup and stored with cosmic-config.

Installs ask for your password through polkit, like COSMIC Store does.

## Requirements

- COSMIC desktop
- `PackageKit` (installed by default on Fedora Workstation and the COSMIC spin)
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

## Development

```sh
just check           # clippy + tests
just list-updates    # print what the applet sees (add --refresh to refresh metadata)
just run             # run outside the panel
```

Code layout:

- `src/lib.rs` – the applet (panel button, popup, state)
- `src/backend/` – update sources; `packagekit.rs` and `flatpak.rs`
- `src/config.rs` – settings
- `i18n/` – translations (Fluent)

### Adding another distro

Distros that ship PackageKit already work. For one that does not (for
example Fedora Atomic, which uses rpm-ostree), add a module in `src/backend/`,
a `Source` variant, and hook it into `backend::check` and `backend::install`.

## License

GPL-3.0-only
