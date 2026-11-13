// Drives the owner's first story through the panel, against a live VectorCraft (started with --control) and a real agent.
//   open -a VectorCraft --args --control 7979 ; node panel/server.mjs &   then:   node panel/live-check.mjs [out-folder]
// It opens examples/story1.svg, describes the line in words, draws, breaks the line, detaches the north piece, deletes the
// southbound bike lane on that piece only, draws again, and exports a PNG. It prints what it checked.
import assert from "node:assert/strict";
import { mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { Control } from "./lib/control.mjs";

const panelUrl = process.env.PANEL_URL ?? "http://127.0.0.1:8787";
const out = resolve(process.argv[2] ?? "out");
mkdirSync(out, { recursive: true });
const vc = new Control({ port: Number(process.env.VC_PORT ?? 7979) });
const state = async () => (await fetch(`${panelUrl}/state`)).json();
const post = async (action, body = {}) => (await fetch(`${panelUrl}/api/${action}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) })).json();
const until = async (what, fn, ms = 20_000) => {
  for (const t = Date.now(); Date.now() - t < ms; await new Promise((r) => setTimeout(r, 150))) {
    const s = await state();
    if (fn(s)) return s;
  }
  throw new Error(`timed out waiting for: ${what}`);
};
const say = (m) => console.log(`- ${m}`);

await vc.call("app.open", { path: resolve(import.meta.dirname, "../examples/story1.svg") });
await vc.run("select.all");
let s = await until("a plain line is selected", (x) => x.target?.kind === "line");
say("a plain line is selected; the panel offers to describe it");

s = await post("propose", { description: "The line represents a road oriented north south. The road consists of 1 bike lane, 1 apron, 2 south lanes, a 3 m grassy curbed margin, 2 north lanes, apron, and a bike lane." });
assert.ok(s.pending, `the agent proposed something: ${s.message}`);
assert.equal(s.pending.canonical, "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane");
say(`the agent proposed: ${s.pending.canonical}  (${s.pending.total_width} m wide)`);

s = await post("draw");
s = await until("the line is now a road", (x) => x.target?.kind === "road" && x.road?.items.length === 9);
say(`drawn: road ${s.road.segment}, ${s.road.items.length} elements`);

const road = (await vc.call("document.inspect", { kind: "Path", limit: 1000 })).nodes.find((n) => n.name === `road.${s.road.segment}`);
await vc.run("path.split", { id: road.id, segment: 0, t: 0.5 });
const paths = (await vc.call("document.inspect", { kind: "Path", limit: 1000 })).nodes.filter((n) => n.name === `road.${s.road.segment}`);
const north = paths.reduce((a, b) => (a.bounds.y < b.bounds.y ? a : b));
await vc.run("object.move", { ids: [north.id], dx: 0, dy: -30 });
await vc.run("select.set", { ids: [north.id] });
s = await until("the north piece is selected", (x) => x.target?.kind === "road" && x.road.pieces.total === 2 && x.road.pieces.selected === 1);
say("broke the line and detached the north piece; the panel sees 1 of 2 pieces selected");

s = await post("propose", { description: "delete the bike lane going south" });
assert.ok(s.pending, s.message);
assert.equal(s.pending.items.length, 8);
say(`the agent proposed: ${s.pending.canonical}`);
s = await post("draw", { scope: "selected" });
s = await until("the north piece has its own road", (x) => x.target?.kind === "road" && x.road.items.length === 8);
const northId = s.road.segment;
assert.notEqual(northId, road.name.slice(5));
say(`drawn for the north piece only: road ${northId}, ${s.road.items.length} elements`);

await vc.run("select.set", { ids: [road.id] });
s = await until("the south piece shows its own description", (x) => x.target?.kind === "road" && x.road.segment === road.name.slice(5) && x.road.items.length === 9);
say(`the south piece still has all ${s.road.items.length} elements`);

await vc.call("app.export", { path: `${out}/live-story1.png`, format: "png", ppi: 288, background: "white" });
say(`exported ${out}/live-story1.png`);
vc.close();
console.log("\nALL CHECKS PASSED");
