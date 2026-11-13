// A recorded replay of the first story through the panel: real VectorCraft, real agent. At each step it captures the panel and the
// VectorCraft window side by side, then cuts the steps into a short video.
//   open -a VectorCraft --args --control 7979 ; node panel/server.mjs &  ;  open "http://127.0.0.1:8787/?zoom=0.6"   (in Safari)
//   node panel/demo/demo.mjs [out-folder]
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { Control } from "../lib/control.mjs";

const panelUrl = process.env.PANEL_URL ?? "http://127.0.0.1:8787";
const out = resolve(process.argv[2] ?? "out/demo");
mkdirSync(out, { recursive: true });
const vc = new Control({ port: Number(process.env.VC_PORT ?? 7979) });
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

// The panel must be open in Safari at http://127.0.0.1:8787/?zoom=0.6 (the zoom makes the whole panel fit the window).
const browser = JSON.parse(execFileSync("swift", [resolve(import.meta.dirname, "windows.swift"), "Safari"]).toString()).find((w) => /Road Builder/.test(w.name));
if (!browser) throw new Error("Open http://127.0.0.1:8787/?zoom=0.6 in Safari first");
const steps = [];
async function shot(title, subtitle) {
  await wait(1200); // let the page redraw
  const n = String(steps.length + 1).padStart(2, "0");
  const panel = `${out}/panel-${n}.png`;
  const win = `${out}/vc-${n}.png`;
  // The panel is in the browser window at the left of the screen; crop to its page area (screen is 5120 px wide, window x 135..3343).
  // The browser window is captured by its window id, so it works even when another window covers it.
  execFileSync("screencapture", ["-x", "-o", "-l", String(browser.id), `${out}/screen-${n}.png`]);
  // Crop away the toolbar and tab bar (about 170 px at 2x) and keep the panel's column, centred in the window.
  execFileSync("python3", ["-c", `from PIL import Image; im = Image.open("${out}/screen-${n}.png"); w, h = im.size; im.crop((w // 2 - 360, 170, w // 2 + 360, h)).save("${panel}")`]);
  await vc.call("ui.screenshot", { path: win });
  execFileSync("python3", [resolve(import.meta.dirname, "compose.py"), panel, win, `${out}/step-${n}.png`, title, subtitle]);
  steps.push(`${out}/step-${n}.png`);
  console.log(`${n}. ${title}`);
}

await vc.call("app.open", { path: resolve(import.meta.dirname, "../../examples/story1.svg") });
await vc.run("view.fitArtboard");
await vc.run("select.all");
await until("a plain line is selected", (x) => x.target?.kind === "line");
await shot("1. Select a line", "The panel sees a plain line and offers to describe it.");

const words = "The line represents a road oriented north south. The road consists of 1 bike lane, 1 apron, 2 south lanes, a 3 m grassy curbed margin, 2 north lanes, apron, and a bike lane.";
await post("propose", { description: words });
let s = await until("a proposal", (x) => x.pending);
await shot("2. Describe it in words", "The agent proposes the road. Nothing is drawn until you press Draw.");

await post("draw");
s = await until("the road is drawn", (x) => x.target?.kind === "road" && x.road?.items.length === 9);
await vc.run("view.fitArtboard");
await shot("3. Draw", `The road is built from the description: road ${s.road.segment}, ${s.road.items.length} elements, ${s.road.total_width} m wide.`);

const road = (await vc.call("document.inspect", { kind: "Path", limit: 1000 })).nodes.find((n) => n.name === `road.${s.road.segment}`);
await vc.run("path.split", { id: road.id, segment: 0, t: 0.5 });
const pieces = (await vc.call("document.inspect", { kind: "Path", limit: 1000 })).nodes.filter((n) => n.name === `road.${s.road.segment}`);
const north = pieces.reduce((a, b) => (a.bounds.y < b.bounds.y ? a : b));
await vc.run("object.move", { ids: [north.id], dx: 0, dy: -30 });
await vc.run("select.set", { ids: [north.id] });
await until("north piece selected", (x) => x.target?.kind === "road" && x.road.pieces.total === 2 && x.road.pieces.selected === 1);
await vc.run("view.fitArtboard");
await shot("4. Break the line, detach the north piece", "Both pieces keep the description. The panel sees 1 of 2 pieces selected.");

await post("propose", { description: "delete the bike lane going south" });
await until("an edit proposal", (x) => x.pending && x.pending.items.length === 8);
await shot("5. Ask for a change in words", "“delete the bike lane going south”: the agent proposes the edit for this piece.");

await post("draw", { scope: "selected" });
s = await until("north piece has its own road", (x) => x.target?.kind === "road" && x.road.items.length === 8);
await vc.run("view.fitArtboard");
await shot("6. Draw, for the selected piece only", `The north piece is now road ${s.road.segment} without the west bike lane. The median stays lined up.`);

await vc.run("select.set", { ids: [road.id] });
s = await until("south piece shown", (x) => x.target?.kind === "road" && x.road.items.length === 9);
await shot("7. Click the south piece", `It still has all ${s.road.items.length} elements, bike lane included.`);

// A short video: each step held for a few seconds.
const list = steps.map((f) => `file '${f}'\nduration 5`).join("\n") + `\nfile '${steps.at(-1)}'\n`;
writeFileSync(`${out}/list.txt`, list);
execFileSync("ffmpeg", ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", `${out}/list.txt`, "-vf", "scale=1920:-2,format=yuv420p", "-c:v", "libx264", "-crf", "24", `${out}/phase2-demo.mp4`]);
console.log(`\nvideo: ${out}/phase2-demo.mp4`);
vc.close();
