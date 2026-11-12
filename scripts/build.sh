#!/bin/sh
# Builds the three Road Builder plug-ins (importer, filter, exporter) as WebAssembly.
# Needs rustup with the wasm target:  brew install rustup && rustup toolchain install stable --target wasm32-unknown-unknown
# If Homebrew's own `rust` is also installed it shadows rustup's compiler, so this script points cargo at rustup's rustc.
set -eu
cd "$(dirname "$0")/.."
[ -d /opt/homebrew/opt/rustup/bin ] && PATH="/opt/homebrew/opt/rustup/bin:$PATH"
RUSTC="$(rustup which rustc)"
export RUSTC
"$(dirname "$RUSTC")/cargo" build --release --target wasm32-unknown-unknown --workspace
ls -l target/wasm32-unknown-unknown/release/*.wasm
