name := 'cosmic-applet-updates'
appid := 'io.github.zoenetic.CosmicAppletUpdates'
version := `sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1`

rootdir := ''
prefix := '/usr'
debug := '0'
vendor := '0'

target := if debug == '1' { 'debug' } else { 'release' }
vendor_args := if vendor == '1' { '--frozen --offline' } else { '' }
debug_args := if debug == '1' { '' } else { '--release' }
cargo_args := vendor_args + ' ' + debug_args
targetdir := env('CARGO_TARGET_DIR', 'target')

sharedir := rootdir + prefix + '/share'
iconsdir := sharedir + '/icons/hicolor'
bindir := rootdir + prefix + '/bin'

default: build-release

# Compiles with debug profile
build-debug *args:
    cargo build {{ args }}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compile with a vendored tarball
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs clippy and the tests
check *args:
    cargo clippy --all-targets {{ args }} -- -D warnings
    cargo test {{ args }}

# Checks dependencies for known vulnerabilities (needs `cargo install cargo-audit`)
audit:
    cargo audit

# Prints what the applet would show, without starting it
list-updates *args:
    cargo run --example check -- {{ args }}

# Opens the settings window
settings *args:
    cargo run {{ args }} -- --settings

# Runs the applet outside the panel, for development
run *args:
    env RUST_LOG=cosmic_applet_updates=debug RUST_BACKTRACE=full cargo run {{ args }}

# Installs files into the system
install: check-installable
    install -Dm0755 {{ targetdir }}/{{ target }}/{{ name }} {{ bindir }}/{{ name }}
    install -Dm0644 data/{{ appid }}.desktop {{ sharedir }}/applications/{{ appid }}.desktop
    install -Dm0644 data/{{ appid }}.Settings.desktop {{ sharedir }}/applications/{{ appid }}.Settings.desktop
    find data/icons -type f -printf '%P\n' | xargs -I {} install -Dm0644 data/icons/{} {{ iconsdir }}/{}

# Removes installed files
uninstall:
    rm -f {{ bindir }}/{{ name }}
    rm -f {{ sharedir }}/applications/{{ appid }}.desktop
    rm -f {{ sharedir }}/applications/{{ appid }}.Settings.desktop
    find data/icons -type f -printf '%P\n' | xargs -I {} rm -f {{ iconsdir }}/{}

# Vendor Cargo dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --locked | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    tar pcf vendor.tar vendor
    rm -rf vendor

# Writes vendor/ and its cargo config into a tarball, for offline distro builds
vendor-tarball out=(name + '-' + version + '-vendor.tar.xz'):
    packaging/vendor-tarball.sh {{ out }}

# Builds a .deb into target/debian (needs `cargo install cargo-deb`)
deb: build-release
    cargo deb --no-build

# Refuses a missing binary, or one built in a Nix shell (it loads libraries
# from the Nix store, so the panel can't start it)
[private]
check-installable:
    #!/usr/bin/env sh
    set -eu
    bin='{{ targetdir }}/{{ target }}/{{ name }}'
    if [ ! -x "$bin" ]; then
        echo "error: $bin not found, run 'just' first" >&2
        exit 1
    fi
    if readelf -l "$bin" 2>/dev/null | grep -q 'interpreter: /nix/store/'; then
        echo "error: $bin was built in a Nix shell and only runs there." >&2
        echo "Rebuild it outside Nix with 'just', then install again." >&2
        exit 1
    fi

# Extracts vendored dependencies
[private]
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar
