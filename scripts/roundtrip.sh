#!/bin/sh
# Round trip through the real VectorCraft engine: pmap -> importer -> Build Road -> exporter -> pmap, then compare with the input.
# Also writes the built document as .vectorcraft, .svg and .png so you can open and look at it.
# Usage: sh scripts/roundtrip.sh <in.pmap> <out-folder> [path-to-vectorcraft-cli]
set -eu
mkdir -p "$2"
IN="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
OUT="$(cd "$2" && pwd)"
CLI="${3:-${VECTORCRAFT_CLI:-vectorcraft-cli}}"
cd "$(dirname "$0")/.."
"$CLI" run --in "$IN" --commands examples/build.jsonl \
  --export "$OUT/built.vectorcraft" --export "$OUT/built.svg" --export "$OUT/built.pmap" | cut -c1-200
node ../../scripts/pmap-check.mjs "$IN" "$OUT/built.pmap"
