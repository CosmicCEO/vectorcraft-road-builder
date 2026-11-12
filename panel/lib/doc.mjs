// Reading what the panel needs from the live document: the selection, road names, and the descriptions the plug-in keeps.
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";

export const PLUGIN_ID = "com.cosmicceo.roadbuilder.build";
export const SET_CROSS_SECTION = `plugin.${PLUGIN_ID}.set_cross_section`;

/** The segment id of a road line (`road.a` is `a`, plain `road` is `main`), or null. Mirrors core/src/roads.rs. */
export function segmentId(name = "") {
  if (name === "road") return "main";
  if (!name.startsWith("road.")) return null;
  return name.slice(5).split(".")[0] || "main";
}

const OWN = /^(gen\.|profile\.|flag\.|arrow\.)|^centerline$|^[A-Z]\d+\./;

/** What a selected object is to the panel: a road piece, a plain line that could become a road, or something else. */
export function classify(node) {
  const isPath = node.kind === "Path";
  const segment = isPath ? segmentId(node.name) : null;
  if (segment) return { role: "road", segment };
  if (isPath && !OWN.test(node.name ?? "")) return { role: "line", segment: null };
  return { role: "other", segment: null };
}

export async function inspectState(control) {
  const r = await control.call("document.inspect", { depth: 0 });
  return { revision: r.revision, title: r.fileInfoTitle ?? r.title ?? "", selection: r.selection ?? [] };
}

export async function nodes(control, ids) {
  if (!ids.length) return [];
  const r = await control.call("document.inspect", { ids });
  return r.nodes ?? [];
}

/** All paths in the document (id and name), for counting the pieces of a road. */
export async function allPaths(control) {
  const r = await control.call("document.inspect", { kind: "Path", limit: 100000 });
  return (r.nodes ?? []).map((n) => ({ id: n.id, name: n.name ?? "" }));
}

/** The JSON part of a saved .vectorcraft file (a binary preview may follow it, after a NUL and VCBLOBS). */
export function parseNativeFile(bytes) {
  const buf = bytes[0] === 0x1f && bytes[1] === 0x8b ? gunzipSync(bytes) : bytes;
  const text = buf.toString("latin1");
  const marker = text.indexOf("\u0000VCBLOBS");
  const end = (marker >= 0 ? text.lastIndexOf("}", marker) : text.lastIndexOf("}")) + 1;
  return JSON.parse(text.slice(0, end));
}

/** The descriptions the plug-in keeps with the document: {segments: {id: {cross_section, anchor}}, ...}, read from a saved copy. */
export async function readSpec(control) {
  const dir = mkdtempSync(join(tmpdir(), "rb-panel-"));
  const path = join(dir, "copy.vectorcraft");
  try {
    await control.run("file.saveCopy", { path });
    // The app may write the copy from a background thread: wait until it is there and has stopped growing.
    let last = -1;
    for (let i = 0; i < 60; i++) {
      const size = existsSync(path) ? statSync(path).size : 0;
      if (size > 0 && size === last) break;
      last = size;
      await new Promise((r) => setTimeout(r, 50));
    }
    const doc = parseNativeFile(readFileSync(path)).document ?? {};
    return doc.unknown?.pluginData?.[PLUGIN_ID] ?? { segments: {} };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
