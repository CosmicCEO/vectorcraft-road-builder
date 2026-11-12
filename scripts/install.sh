#!/bin/sh
# Installs the built plug-ins into VectorCraft's Plug-ins folder (via vectorcraft-cli) so the app and the CLI both see them.
# Usage: sh scripts/install.sh [path-to-vectorcraft-cli]
set -eu
cd "$(dirname "$0")/.."
CLI="${1:-${VECTORCRAFT_CLI:-vectorcraft-cli}}"
for kind in importer filter exporter; do
  "$CLI" run --cmd plugin.install --params "{\"path\":\"$(pwd)/target/wasm32-unknown-unknown/release/road_builder_$kind.wasm\",\"replace\":true}" | tail -1
done
"$CLI" run --cmd plugin.list | tail -1 | tr ',' '\n' | grep -E '"(id|name|kind)":"com.cosmicceo' || true
