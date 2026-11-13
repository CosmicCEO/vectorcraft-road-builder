// Runs the native road-builder-cli (parse, edit, library) and returns its one line of JSON.
import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const here = import.meta.dirname;

/** Where the CLI binary is: ROAD_BUILDER_CLI, else the release build next to the panel. */
export function cliPath() {
  const p = process.env.ROAD_BUILDER_CLI ?? resolve(here, "../../target/release/road-builder-cli");
  if (!existsSync(p)) throw new Error(`road-builder-cli not found at ${p}. Build it: cargo build --release -p road-builder-cli (or set ROAD_BUILDER_CLI)`);
  return p;
}

export function runCli(args) {
  return new Promise((done, fail) => {
    execFile(cliPath(), args, { timeout: 10_000 }, (err, stdout) => {
      if (err && !stdout) return fail(err);
      try {
        done(JSON.parse(stdout));
      } catch {
        fail(new Error(`road-builder-cli printed something unexpected: ${stdout.slice(0, 200)}`));
      }
    });
  });
}

export const parseCrossSection = (text) => runCli(["parse", text]);
export const library = () => runCli(["library"]);
/** op: ["delete", i] | ["move", from, to] | ["width", i, metres] | ["insert", i, "text"] */
export const editCrossSection = (text, op) => runCli(["edit", text, ...op.map(String)]);
/** Checks a junction description (JSON text). {ok, junction, summary} or {ok: false, error}. */
export const parseJunction = (jsonText) => runCli(["parse-junction", jsonText]);
/** What a proposed cross-section changes compared with the current one, as sentences. */
export const diffCrossSection = (current, proposed) => runCli(["diff", current, proposed]);
/** Checks an intersection description (JSON text) for two crossing roads. {ok, crossing, summary} or {ok: false, error}. */
export const parseCrossing = (jsonText) => runCli(["parse-crossing", jsonText]);
/** Checks a hub description (JSON text) for three or more meeting roads. {ok, hub, summary} or {ok: false, error}. */
export const parseHub = (jsonText) => runCli(["parse-hub", jsonText]);
