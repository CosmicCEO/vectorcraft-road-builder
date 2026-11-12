// The owner's intersection story through the panel, against a live VectorCraft and a real agent, recorded step by step.
//   open -a VectorCraft --args --control 7979 ; node panel/server.mjs &  ;  open "http://127.0.0.1:8787/?zoom=0.6"   (in Safari)
//   node panel/demo/junction-demo.mjs [out-folder]
import assert from "node:assert/strict";
import { resolve } from "node:path";
import { Control } from "../lib/control.mjs";
import { createRecorder } from "./recorder.mjs";

const panelUrl = process.env.PANEL_URL ?? "http://127.0.0.1:8787";
const out = resolve(process.argv[2] ?? "out/junction");
const vc = new Control({ port: Number(process.env.VC_PORT ?? 7979) });
const rec = createRecorder({ vc, out });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const state = async () => (await fetch(`${panelUrl}/state`)).json();
const post = async (action, body = {}) => (await fetch(`${panelUrl}/api/${action}`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body) })).json();
const until = async (what, fn, ms = 20_000) => {
  for (const t = Date.now(); Date.now() - t < ms; await wait(150)) {
    const s = await state();
    if (fn(s)) return s;
  }
  throw new Error(`timed out waiting for: ${what}`);
};
const lines = async () => (await vc.call("document.inspect", { kind: "Path", limit: 1000 })).nodes;

const ROAD = "The line represents a road oriented north south. The road consists of 1 bike lane, 1 apron, 2 south lanes, a 3 m grassy curbed margin, 2 north lanes, apron, and a bike lane.";
const CROSSING = "This is a 4 way intersection, the main route south adds a turn pocket (which you interpret as creating a third lane) and continues on with two. The west intersection is a two lane rural road, the east is a two lane rural road. There are traffic lights. There are cross walks at each side.";

await vc.call("app.open", { path: resolve(import.meta.dirname, "../../examples/story-connect.svg") });
await vc.run("view.fitArtboard");
const byPosition = (await lines()).sort((a, b) => b.bounds.y - a.bounds.y); // south piece lowest on the page, then the connector, then north
const [southId, connectorId, northId] = byPosition.map((n) => n.id);

// 1-3. The south road: select it, describe it, Draw.
await vc.run("select.set", { ids: [southId] });
await until("the south line is selected", (x) => x.target?.kind === "line");
await rec.shot("1. A road to the south", "Select the line and describe it in words.");
await post("propose", { description: ROAD });
await until("a road proposal", (x) => x.pending?.items?.length === 9);
await post("draw");
await until("the south road", (x) => x.target?.kind === "road");
await vc.run("view.fitArtboard");
await rec.shot("2. Draw: the road to the south", "The first road is built from the description.");

// 4. The north road, described the same way.
await vc.run("select.set", { ids: [northId] });
await until("the north line is selected", (x) => x.target?.kind === "line");
await post("propose", { description: ROAD });
await until("a road proposal", (x) => x.pending?.items?.length === 9);
await post("draw");
await until("the north road", (x) => x.target?.kind === "road");
await vc.run("view.fitArtboard");
await rec.shot("3. The road to the north", "Same description. A gap is left between the two roads.");

// 5-7. The connecting line: a connection, described as a four-way intersection.
await vc.run("select.set", { ids: [connectorId] });
await until("the connecting line is selected", (x) => x.target?.kind === "line");
await post("mode", { mode: "connection" });
await until("connection mode", (x) => x.mode === "connection");
await rec.shot("4. Select the connecting line", "It can be a road, or a connection between two roads: here, a connection.");
let s = await post("propose", { description: CROSSING });
assert.ok(s.pending?.kind === "junction", s.message);
assert.equal(s.pending.junction.legs.length, 2);
assert.equal(s.pending.junction.control, "signal");
assert.equal(s.pending.junction.crosswalks, true);
await rec.shot("5. Describe the intersection", `The agent reads it: ${s.pending.summary}`);
await post("draw");
s = await until("junction drawn", (x) => x.target?.kind === "junction" && x.junction?.described);
await vc.run("view.setZoom", { zoom: 330, center: [300, 430] });
await rec.shot("6. Draw: the intersection", "The road runs through. West and east legs, kerb corners, crosswalks, stop bars and signals.");

// 8. Change the control: stop signs on the side roads instead of lights.
await post("junction", { patch: { control: "stop" } });
s = await until("a stop-sign proposal", (x) => x.pending?.junction?.control === "stop");
await rec.shot("7. Change it: stop signs instead of lights", "Adjust the junction in the panel; nothing changes until you press Draw.");
await post("draw");
await until("redrawn", (x) => x.junction?.spec?.control === "stop");
await rec.shot("8. Draw again", "The side roads now have stop signs, and the main road is not stopped.");

rec.video("junction-demo.mp4");
vc.close();
