name := 'cosmic-applet-updates'
appid := 'io.github.zoenetic.CosmicAppletUpdates'

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
install:
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

# Extracts vendored dependencies
[private]
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar
