Name:           cosmic-applet-updates
Version:        0.1.0
Release:        1%{?dist}
Summary:        COSMIC panel applet for system and Flatpak updates

License:        GPL-3.0-only
URL:            https://github.com/zoenetic/update-applet
Source0:        %{url}/archive/v%{version}/update-applet-%{version}.tar.gz
# Rust dependencies, made by `.copr/Makefile` (or `just vendor-tarball`)
Source1:        %{name}-%{version}-vendor.tar.xz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  just
BuildRequires:  binutils
BuildRequires:  pkgconfig(xkbcommon)

# Loaded at runtime, so not picked up automatically
Requires:       libwayland-client
Recommends:     PackageKit
Recommends:     flatpak

%description
A COSMIC panel applet that shows available updates and installs them.
System packages go through PackageKit, or rpm-ostree on Fedora Atomic.
Flatpak apps are updated too.

%prep
%autosetup -n update-applet-%{version}
# Unpacks vendor/ and .cargo/config.toml pointing cargo at it
tar -xJf %{SOURCE1}

%build
just build-release --frozen --offline

%install
just rootdir=%{buildroot} prefix=%{_prefix} install

# No desktop-file-validate: Categories=COSMIC, as used by every COSMIC
# applet, is not a registered category.

%files
%license LICENSE
%doc README.md
%{_bindir}/%{name}
%{_datadir}/applications/io.github.zoenetic.CosmicAppletUpdates.desktop
%{_datadir}/applications/io.github.zoenetic.CosmicAppletUpdates.Settings.desktop
%{_datadir}/icons/hicolor/scalable/apps/io.github.zoenetic.CosmicAppletUpdates-symbolic.svg

%changelog
* Mon Oct 19 2026 zoenetic - 0.1.0-1
- Initial package
