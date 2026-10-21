#!/bin/sh
# Writes a tarball with vendor/ and the .cargo/config.toml that points cargo at
# it, for offline builds (Fedora RPM, distro build systems).
# Usage: packaging/vendor-tarball.sh OUTPUT.tar.xz
set -eu
out=$(realpath -m "$1")
cd "$(dirname "$0")/.."

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$tmp/.cargo"
# Written to a file first so a cargo failure stops the script
cargo vendor --locked "$tmp/vendor" > "$tmp/config.toml"
sed 's|^directory = .*|directory = "vendor"|' "$tmp/config.toml" > "$tmp/.cargo/config.toml"
tar -C "$tmp" -cJf "$out" vendor .cargo
echo "wrote $out"
