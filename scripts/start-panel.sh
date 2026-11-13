#!/bin/sh
# Starts everything for drawing roads with the companion panel: VectorCraft with its control port, then the panel, then your browser.
# Usage: sh scripts/start-panel.sh [agent]        (agent: a name from panel/providers.json, default claude)
set -eu
cd "$(dirname "$0")/.."
PORT=7979
if ! pgrep -x VectorCraft >/dev/null; then
  open -a VectorCraft --args --control "$PORT"
  echo "Started VectorCraft with its control port ($PORT, loopback only). Close it when you are done."
  sleep 5
else
  echo "VectorCraft is already running; if the panel cannot connect, quit it and run this again (the control port needs a fresh start)."
fi
[ -x target/release/road-builder-cli ] || { echo "Building the helper first..."; sh scripts/build.sh >/dev/null; "$(dirname "$(rustup which rustc)")/cargo" build --release -p road-builder-cli; }
( sleep 2; open "http://127.0.0.1:8787" ) &
exec node panel/server.mjs --control "$PORT" ${1:+--agent "$1"}
