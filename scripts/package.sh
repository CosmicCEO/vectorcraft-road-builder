#!/bin/sh
# Packs a release into dist/: the three plug-ins (.wasm), the panel, the library, the docs, an install script and a start script, checksums, and a zip.
# Usage: sh scripts/package.sh <version>        e.g.  sh scripts/package.sh 0.6.0
# On a Mac it also builds the helper (road-builder-cli) and includes it; elsewhere the helper is left out (the start script says how to build it).
set -eu
cd "$(dirname "$0")/.."
VERSION="${1:?usage: package.sh <version>}"
NAME="road-builder-$VERSION"
OUT="dist/$NAME"
[ -d /opt/homebrew/opt/rustup/bin ] && PATH="/opt/homebrew/opt/rustup/bin:$PATH"
if command -v rustup >/dev/null 2>&1; then RUSTC="$(rustup which rustc)"; export RUSTC; CARGO="$(dirname "$RUSTC")/cargo"; else CARGO=cargo; fi

rm -rf dist
mkdir -p "${OUT:?}/plugins"
"$CARGO" build --release --target wasm32-unknown-unknown --workspace
for kind in importer filter exporter; do cp "target/wasm32-unknown-unknown/release/road_builder_$kind.wasm" "${OUT:?}/plugins/"; done
if [ "$(uname)" = Darwin ]; then
  mkdir -p "${OUT:?}/bin"
  "$CARGO" build --release -p road-builder-cli
  cp target/release/road-builder-cli "${OUT:?}/bin/"
fi
cp -R panel library docs examples LICENSE README.md CHANGELOG.md "${OUT:?}/"
rm -rf "${OUT:?}/panel/test" "${OUT:?}/panel/demo"

cat > "${OUT:?}/install.sh" <<'INSTALL'
#!/bin/sh
# Installs the three Road Builder plug-ins into VectorCraft. Needs vectorcraft-cli (from VectorCraft's downloads); set VECTORCRAFT_CLI if it is not on your PATH.
set -eu
cd "$(dirname "$0")"
CLI="${VECTORCRAFT_CLI:-vectorcraft-cli}"
for kind in importer filter exporter; do
  "$CLI" run --cmd plugin.install --params "{\"path\":\"$(pwd)/plugins/road_builder_$kind.wasm\",\"replace\":true}" | tail -1 | cut -c1-80
done
echo "Installed. Start the panel with:  sh start.sh"
INSTALL
cat > "${OUT:?}/start.sh" <<'START'
#!/bin/sh
# Starts VectorCraft with its control port, the panel, and your browser. Usage: sh start.sh [agent]   (agent: a name in panel/providers.json)
set -eu
cd "$(dirname "$0")"
PORT=7979
if [ "$(uname)" = Darwin ] && ! pgrep -x VectorCraft >/dev/null; then
  open -a VectorCraft --args --control "$PORT"
  echo "Started VectorCraft with its control port ($PORT, this computer only)."
  sleep 5
else
  echo "If the panel cannot connect, quit VectorCraft and run this again (the control port needs a fresh start)."
fi
if [ ! -x bin/road-builder-cli ]; then
  echo "bin/road-builder-cli is missing. Build it with:  cargo build --release -p road-builder-cli  (needs Rust), then copy target/release/road-builder-cli to bin/"
  exit 1
fi
export ROAD_BUILDER_CLI="$(pwd)/bin/road-builder-cli"
( sleep 2; command -v open >/dev/null && open "http://127.0.0.1:8787" ) &
exec node panel/server.mjs --control "$PORT" ${1:+--agent "$1"}
START
chmod +x "${OUT:?}/install.sh" "${OUT:?}/start.sh"

# Release notes: this version's section of the changelog.
awk -v v="$VERSION" '$0 ~ "^## "v" " {f=1; next} f && /^## / {exit} f' CHANGELOG.md > dist/RELEASE-NOTES.md
[ -s dist/RELEASE-NOTES.md ] || echo "Road Builder $VERSION" > dist/RELEASE-NOTES.md

( cd dist && zip -qr "$NAME.zip" "$NAME" )
( cd dist && if command -v sha256sum >/dev/null 2>&1; then sha256sum "$NAME.zip" "$NAME"/plugins/*.wasm; else shasum -a 256 "$NAME.zip" "$NAME"/plugins/*.wasm; fi > SHA256SUMS )
echo "Packed dist/$NAME.zip"; cat dist/SHA256SUMS
