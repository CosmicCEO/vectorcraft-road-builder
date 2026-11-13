import assert from "node:assert/strict";
import { after, test } from "node:test";
import { Control } from "../lib/control.mjs";
import { SET_CROSSING, SET_CROSS_SECTION, SET_HUB, SET_JUNCTION, PLUGIN_ID } from "../lib/doc.mjs";
import { Panel } from "../lib/panel.mjs";
import { fakeVectorCraft } from "./fake-vectorcraft.mjs";

const STORY = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";
const agent = (text) => ({ default: "fake", providers: { fake: { kind: "function", label: "Fake", fn: () => text } } });

// The commands the user's own steps made, without the layer tidying the panel does after a build.
const steps = (vc) => vc.doc.calls.filter((c) => c.command !== "file.saveCopy" && !c.command.startsWith("layer.")).map((c) => c.command);

const cleanups = [];
after(() => cleanups.forEach((f) => f()));

async function setup(vc, answer = STORY) {
  const control = new Control({ port: vc.port });
  const panel = new Panel({ control, providers: agent(answer) });
  const done = () => { control.close(); vc.close(); };
  cleanups.push(done); // even when a test fails half way, nothing is left open
  await panel.tick();
  return { panel, control, done };
}

test("a plain selected line is offered for description", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 5, name: "" }], selection: [5] });
  const { panel, done } = await setup(vc);
  assert.equal(panel.snapshot().target.kind, "line");
  assert.equal(panel.snapshot().road, null);
  done();
});

test("describe in words, then Draw: the right commands, in order, and the selection comes back", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 5, name: "" }, { id: 9, name: "profile.sbEdge" }], selection: [5] });
  const { panel, done } = await setup(vc);
  await panel.propose("a road with a margin in the middle", "fake");
  const s = panel.snapshot();
  assert.equal(s.pending.canonical, STORY);
  assert.equal(s.pending.items.length, 9);
  assert.equal(vc.doc.calls.length, 0, "nothing is applied until Draw");

  await panel.draw();
  assert.deepEqual(steps(vc), ["select.set", SET_CROSS_SECTION, "select.all", "plugin.run", "select.set"]);
  const names = vc.doc.layers.map((l) => l.name);
  assert.ok(names.includes("road r1") && names.includes("road r1 spine") && names.includes("road r1 drawing"), `each road gets a layer with a spine and a drawing: ${names}`);
  assert.deepEqual(vc.doc.paths.find((x) => x.id === 5).path, [vc.doc.layers.find((l) => l.name === "road r1 spine").id], "the line is on its spine layer");
  assert.equal(vc.doc.calls.find((c) => c.command === SET_CROSS_SECTION).params.params.cross_section, STORY);
  assert.deepEqual(vc.doc.calls.find((c) => c.command === "plugin.run").params, { id: PLUGIN_ID });
  assert.deepEqual(vc.doc.selection, [5], "the user's selection is restored");
  const after = panel.snapshot();
  assert.equal(after.pending, null);
  assert.equal(after.target.kind, "road");
  assert.equal(after.road.text, STORY);
  assert.equal(after.road.items.length, 9);
  done();
});

test("selecting a described road shows its elements; deleting one is previewed, then drawn for that piece only", async () => {
  const spec = { segments: { r1: { cross_section: STORY, anchor: "auto" } } };
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.r1" }, { id: 3, name: "road.r1" }], selection: [3], spec });
  const { panel, done } = await setup(vc);
  let s = panel.snapshot();
  assert.equal(s.road.segment, "r1");
  assert.equal(s.road.items.length, 9);
  assert.deepEqual(s.road.pieces, { selected: 1, total: 2 }, "the panel knows another piece shares this road");

  await panel.edit(["delete", 0]);
  s = panel.snapshot();
  assert.equal(s.pending.canonical, "facing N: apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane");
  assert.equal(vc.doc.spec.segments.r1.cross_section, STORY, "still not applied");

  await panel.draw({ scope: "selected" });
  const cmd = vc.doc.calls.find((c) => c.command === SET_CROSS_SECTION);
  assert.equal(cmd.params.params.scope, "selected");
  assert.equal(vc.doc.spec.segments.r1.cross_section, STORY, "the other piece keeps its description");
  assert.match(vc.doc.spec.segments.r2.cross_section, /^facing N: apron/);
  done();
});

test("an invalid answer from the agent never reaches VectorCraft", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 5, name: "" }], selection: [5] });
  const { panel, done } = await setup(vc, "warp-drive | lane");
  await panel.propose("something odd", "fake");
  assert.equal(panel.snapshot().pending, null);
  assert.match(panel.snapshot().message, /could not produce a valid cross-section/);
  assert.equal(vc.doc.calls.length, 0);
  done();
});

test("changing the selection drops a proposal made for something else", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 5, name: "" }, { id: 6, name: "" }], selection: [5] });
  const { panel, done } = await setup(vc);
  await panel.propose("anything", "fake");
  assert.ok(panel.snapshot().pending);
  vc.doc.selection = [6];
  await panel.tick();
  assert.equal(panel.snapshot().pending, null);
  done();
});

test("with nothing sensible selected, it says so instead of acting", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 5, name: "profile.sbEdge" }], selection: [5] });
  const { panel, done } = await setup(vc);
  await panel.propose("x", "fake");
  assert.match(panel.snapshot().message, /Select one line/);
  assert.equal(vc.doc.calls.length, 0);
  done();
});

test("no open document is reported plainly; a missing VectorCraft too", async () => {
  const vc = await fakeVectorCraft({ hasDocument: false });
  const { panel, done } = await setup(vc);
  assert.match(panel.snapshot().error, /Open or create a document/);
  done();
  const gone = new Panel({ control: new Control({ port: 1 }), providers: agent(STORY) });
  await gone.tick();
  assert.match(gone.snapshot().error, /not answering on port 1/);
  assert.equal(gone.snapshot().connected, false);
});

test("typing the shorthand directly is validated like an agent's answer", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 5, name: "" }], selection: [5] });
  const { panel, done } = await setup(vc);
  await panel.setText("lane | trolley");
  assert.match(panel.snapshot().message, /unknown element "trolley"/);
  await panel.setText("squishy margin 3 | lane");
  assert.equal(panel.snapshot().pending.canonical, "margin 3 | lane");
  assert.match(panel.snapshot().message, /Read as/);
  done();
});

const FOUR_WAY = JSON.stringify({ legs: [{ side: "west", cross_section: "lane back | lane fwd", length: 40 }, { side: "east", cross_section: "lane back | lane fwd", length: 40 }], control: "signal", crosswalks: true, radius: 6 });

test("a plain line can be a connection: describe the junction, then Draw", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 7, name: "" }], selection: [7] });
  const { panel, done } = await setup(vc, `Sure: ${FOUR_WAY}`);
  await panel.setMode("connection");
  await panel.propose("This is a 4 way intersection. West and east are two lane rural roads. There are traffic lights and cross walks.", "fake");
  const pending = panel.snapshot().pending;
  assert.equal(pending.kind, "junction");
  assert.match(pending.summary, /four-way junction, traffic signals, crosswalks/);
  assert.equal(vc.doc.calls.length, 0, "nothing is applied until Draw");

  await panel.draw();
  const set = vc.doc.calls.find((c) => c.command === SET_JUNCTION);
  assert.equal(JSON.parse(set.params.params.description).legs.length, 2);
  assert.deepEqual(steps(vc), ["select.set", SET_JUNCTION, "select.all", "plugin.run", "select.set"]);
  assert.deepEqual(vc.doc.selection, [7]);
  const after = panel.snapshot();
  assert.equal(after.target.kind, "junction");
  assert.equal(after.junction.spec.control, "signal");
  assert.match(after.junction.summary, /four-way junction/);
  done();
});

test("Connect and match carries the road across with no side roads", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 7, name: "" }], selection: [7] });
  const { panel, done } = await setup(vc);
  await panel.setMode("connection");
  await panel.match();
  assert.match(panel.snapshot().pending.summary, /road carried across the gap/);
  await panel.draw();
  assert.deepEqual(JSON.parse(vc.doc.calls.find((c) => c.command === SET_JUNCTION).params.params.description).legs, []);
  done();
});

test("a selected junction shows its description and its settings can be changed before drawing", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 7, name: "junction.j1" }], selection: [7], spec: { segments: {}, junctions: { j1: JSON.parse(FOUR_WAY) } } });
  const { panel, done } = await setup(vc);
  let s = panel.snapshot();
  assert.equal(s.target.kind, "junction");
  assert.equal(s.junction.spec.legs.length, 2);
  await panel.editJunction({ control: "stop", crosswalks: false });
  s = panel.snapshot();
  assert.equal(s.pending.junction.control, "stop");
  assert.match(s.pending.summary, /stop signs on the legs/);
  await panel.editJunction({ removeLeg: 1 });
  assert.match(panel.snapshot().pending.summary, /T-junction/, "removing a leg makes it a T-junction");
  assert.equal(vc.doc.spec.junctions.j1.control, "signal", "still not applied");
  done();
});

test("an agent's invalid junction never reaches VectorCraft", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 7, name: "" }], selection: [7] });
  const { panel, done } = await setup(vc, '{"control": "roundabout"}');
  await panel.setMode("connection");
  await panel.propose("a roundabout", "fake");
  assert.equal(panel.snapshot().pending, null);
  assert.match(panel.snapshot().message, /could not produce a valid junction/);
  assert.equal(vc.doc.calls.length, 0);
  done();
});

test("clicking a generated shape of a road selects that road in the panel, and Select the line puts the line in the selection", async () => {
  const spec = { segments: { r1: { cross_section: STORY, anchor: "auto" } } };
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.r1" }, { id: 3, name: "road.r1" }, { id: 40, name: "gen.road.r1.2.surface.4.margin" }, { id: 41, name: "gen.road.r1.2.line.3" }], selection: [40, 41], spec });
  const { panel, done } = await setup(vc);
  let s = panel.snapshot();
  assert.equal(s.target.kind, "road");
  assert.equal(s.target.segment, "r1");
  assert.deepEqual(s.target.ids, [3], "piece 2 of road.r1 is the second line with that name");
  assert.equal(s.road.items.length, 9);
  assert.deepEqual(s.road.pieces, { selected: 1, total: 2 });
  await panel.selectLine();
  assert.deepEqual(vc.doc.selection, [3], "the line itself is now selected in VectorCraft");
  assert.equal(panel.snapshot().target.kind, "road");
  done();
});

test("clicking a generated shape of a junction selects the junction", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 7, name: "junction.j1" }, { id: 50, name: "gen.junction.j1.box" }], selection: [50], spec: { segments: {}, junctions: { j1: JSON.parse(FOUR_WAY) } } });
  const { panel, done } = await setup(vc);
  const s = panel.snapshot();
  assert.equal(s.target.kind, "junction");
  assert.deepEqual(s.target.ids, [7]);
  assert.equal(s.junction.spec.legs.length, 2);
  done();
});

const TWO_LANE = { cross_section: "lane back | lane fwd", anchor: "auto" };
const roads2 = { segments: { a: TWO_LANE, b: TWO_LANE } };
const SIGNAL = '{"treatment": "signal", "crosswalks": true, "radius": 6, "minor": null, "over": null, "diameter": 40}';

test("selecting two different roads offers an intersection; a quick choice is previewed, then Draw makes it", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec: roads2 });
  const { panel, done } = await setup(vc);
  let s = panel.snapshot();
  assert.equal(s.target.kind, "crossing");
  assert.deepEqual(s.crossing.roads, ["a", "b"]);
  assert.equal(s.crossing.described, false);
  assert.equal(s.crossing.spec.treatment, "signal", "a lighted intersection with crosswalks is the starting suggestion");

  await panel.editCrossing({ treatment: "2 way stop", minor: "b" });
  s = panel.snapshot();
  assert.equal(s.pending.kind, "crossing");
  assert.equal(s.pending.crossing.treatment, "two-way-stop");
  assert.match(s.pending.summary, /two-way stop, road b stops, crosswalks/);
  assert.equal(steps(vc).length, 0, "nothing is applied until Draw");

  await panel.draw();
  assert.deepEqual(steps(vc), ["select.set", SET_CROSSING, "select.all", "plugin.run", "select.set"]);
  assert.equal(vc.doc.spec.crossings.k1.treatment, "two-way-stop");
  assert.deepEqual(vc.doc.selection, [2, 3], "the user's selection comes back");
  s = panel.snapshot();
  assert.equal(s.crossing.described, true);
  assert.equal(s.crossing.id, "k1");
  done();
});

test("an agent turns words into an intersection treatment", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec: roads2 });
  const { panel, done } = await setup(vc, 'Here: {"treatment": "traffic circle", "crosswalks": true, "diameter": 50}');
  await panel.propose("make it a roundabout about 50 metres across with crosswalks", "fake");
  const p = panel.snapshot().pending;
  assert.equal(p.kind, "crossing");
  assert.equal(p.crossing.treatment, "roundabout");
  assert.equal(p.crossing.diameter, 50);
  assert.deepEqual(p.crossing.roads, ["a", "b"], "the program supplies the road ids");
  done();
});

test("a cloverleaf is previewed but not drawn yet, and says so", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec: roads2 });
  const { panel, done } = await setup(vc);
  await panel.editCrossing({ treatment: "cloverleaf" });
  assert.match(panel.snapshot().message, /not built yet/);
  await panel.draw();
  assert.match(panel.snapshot().message, /not built yet/);
  assert.equal(vc.doc.calls.filter((c) => c.command === SET_CROSSING).length, 0);
  done();
});

test("clicking an intersection's drawing selects the two roads it joins", async () => {
  const spec = { ...roads2, crossings: { k1: { roads: ["a", "b"], treatment: "signal", crosswalks: true, radius: 6 } } };
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }, { id: 40, name: "gen.crossing.k1.box" }], selection: [40], spec });
  const { panel, done } = await setup(vc);
  const s = panel.snapshot();
  assert.equal(s.target.kind, "crossing");
  assert.deepEqual(s.target.ids.sort(), [2, 3]);
  assert.equal(s.crossing.described, true);
  assert.equal(s.crossing.spec.treatment, "signal");
  done();
});

test("a rebuild shows hidden and locked road layers for the build, then puts them back", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }], selection: [2], spec: { segments: { a: TWO_LANE } } });
  vc.doc.layers.push({ id: 50, name: "road a", visible: true, locked: false, parent: null }, { id: 51, name: "road a spine", visible: false, locked: false, parent: 50 }, { id: 52, name: "road a drawing", visible: true, locked: true, parent: 50 });
  const { panel, done } = await setup(vc);
  await panel.draw();
  const props = vc.doc.calls.filter((c) => c.command === "layer.setProps").map((c) => c.params);
  const runAt = vc.doc.calls.findIndex((c) => c.command === "plugin.run");
  const before = vc.doc.calls.slice(0, runAt).filter((c) => c.command === "layer.setProps").map((c) => c.params);
  assert.deepEqual(before, [{ id: 51, visible: true, locked: false }, { id: 52, visible: true, locked: false }], "everything is shown and unlocked before the build");
  assert.equal(props.length, 4);
  assert.equal(vc.doc.layers.find((l) => l.id === 51).visible, false, "the spine is hidden again afterwards");
  assert.equal(vc.doc.layers.find((l) => l.id === 52).locked, true, "the drawing is locked again afterwards");
  assert.equal(vc.doc.layers.filter((l) => l.name === "road a").length, 1, "the existing road layer is reused, not duplicated");
  done();
});

test("a short stub can be kept as a dead end or trimmed to a three-way", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec: roads2 });
  const { panel, done } = await setup(vc);
  assert.equal(panel.snapshot().crossing.spec.stub, "keep", "a dead end is the default");
  await panel.editCrossing({ stub: "3-way" });
  const p = panel.snapshot().pending;
  assert.equal(p.crossing.stub, "trim");
  assert.match(p.summary, /short stub is trimmed \(three-way\)/);
  await panel.draw();
  assert.equal(vc.doc.spec.crossings.k1.stub, "trim");
  assert.equal(panel.snapshot().crossing.spec.stub, "trim", "it is remembered with the intersection");
  done();
});

test("an agent can ask for a three-way by trimming the stub", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec: roads2 });
  const { panel, done } = await setup(vc, '{"treatment": "signal", "crosswalks": true, "stub": "trim"}');
  await panel.propose("lighted three-way, trim off the stub", "fake");
  assert.equal(panel.snapshot().pending.crossing.stub, "trim");
  done();
});

test("the new intersection types and the pocket and slip choices go through the panel", async () => {
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec: roads2 });
  const { panel, done } = await setup(vc);
  await panel.editCrossing({ treatment: "level crossing", gates: false });
  let p = panel.snapshot().pending;
  assert.equal(p.crossing.treatment, "railroad");
  assert.equal(p.crossing.gates, false);
  assert.match(p.summary, /railroad crossing$/);
  await panel.editCrossing({ treatment: "signal", pocket: "left", pocket_length: 60, slip: true });
  p = panel.snapshot().pending;
  assert.match(p.summary, /left turn pockets \(60 m\), right-turn slip lanes/);
  assert.equal(p.crossing.radius, null, "the kerb radius stays automatic until it is set");
  await panel.editCrossing({ treatment: "driveway" });
  assert.match(panel.snapshot().pending.summary, /driveway/);
  await panel.editCrossing({ treatment: "median-opening", opening: 24, main: "a" });
  assert.match(panel.snapshot().pending.summary, /median opening, 24 m long/);
  assert.equal(panel.snapshot().pending.crossing.main, "a");
  await panel.editCrossing({ pocket: "sideways" });
  assert.match(panel.snapshot().message, /pocket must be/);
  await panel.draw();
  assert.equal(vc.doc.spec.crossings.k1.treatment, "median-opening");
  assert.equal(vc.doc.spec.crossings.k1.opening, 24);
  done();
});

test("an agent can ask for turn pockets twice as long", async () => {
  const spec = { ...roads2, crossings: { k1: { roads: ["a", "b"], treatment: "signal", crosswalks: true, pocket: "left", pocket_length: 30 } } };
  const vc = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }], selection: [2, 3], spec });
  const { panel, done } = await setup(vc, '{"treatment": "signal", "crosswalks": true, "pocket": "left", "pocket_length": 60}');
  assert.equal(panel.snapshot().crossing.spec.pocket, "left");
  await panel.propose("turn pocket probably twice as long", "fake");
  assert.equal(panel.snapshot().pending.crossing.pocket_length, 60);
  done();
});

const roads3 = { segments: { a: TWO_LANE, b: TWO_LANE, c: TWO_LANE } };
const threeRoads = () => fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }, { id: 4, name: "road.c" }], selection: [2, 3, 4], spec: roads3 });

test("selecting three roads offers a hub; a choice is previewed, then Draw makes it", async () => {
  const vc = await threeRoads();
  const { panel, done } = await setup(vc);
  let s = panel.snapshot();
  assert.equal(s.target.kind, "hub");
  assert.deepEqual(s.hub.roads, ["a", "b", "c"]);
  assert.equal(s.hub.described, false);
  assert.equal(s.hub.spec.treatment, "signal");
  await panel.editHub({ treatment: "all way stop", stub: "trim" });
  s = panel.snapshot();
  assert.equal(s.pending.kind, "hub");
  assert.equal(s.pending.hub.treatment, "four-way-stop");
  assert.match(s.pending.summary, /3 roads meet \(a, b, c\): all-way stop, crosswalks, short stubs trimmed/);
  assert.equal(steps(vc).length, 0, "nothing is applied until Draw");
  await panel.draw();
  assert.deepEqual(steps(vc), ["select.set", SET_HUB, "select.all", "plugin.run", "select.set"]);
  assert.equal(vc.doc.spec.hubs.h1.treatment, "four-way-stop");
  assert.deepEqual(vc.doc.selection, [2, 3, 4]);
  assert.equal(panel.snapshot().hub.described, true);
  assert.equal(panel.snapshot().hub.id, "h1");
  done();
});

test("an agent turns words into a hub, and a click on its drawing selects its roads", async () => {
  const vc = await threeRoads();
  const { panel, done } = await setup(vc, '{"treatment": "traffic lights", "crosswalks": true}');
  await panel.propose("five way, lighted, crosswalks", "fake");
  const p = panel.snapshot().pending;
  assert.equal(p.kind, "hub");
  assert.equal(p.hub.treatment, "signal");
  assert.deepEqual(p.hub.roads, ["a", "b", "c"], "the program supplies the road ids");
  done();
  const spec = { ...roads3, hubs: { h1: { roads: ["a", "b", "c"], treatment: "signal", crosswalks: true } } };
  const vc2 = await fakeVectorCraft({ paths: [{ id: 2, name: "road.a" }, { id: 3, name: "road.b" }, { id: 4, name: "road.c" }, { id: 40, name: "gen.hub.h1.core" }], selection: [40], spec });
  const two = await setup(vc2);
  const s = two.panel.snapshot();
  assert.equal(s.target.kind, "hub");
  assert.deepEqual(s.target.ids.sort(), [2, 3, 4]);
  assert.equal(s.hub.described, true);
  two.done();
});
