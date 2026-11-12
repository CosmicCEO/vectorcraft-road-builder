import assert from "node:assert/strict";
import { after, test } from "node:test";
import { Control } from "../lib/control.mjs";
import { SET_CROSS_SECTION, PLUGIN_ID } from "../lib/doc.mjs";
import { Panel } from "../lib/panel.mjs";
import { fakeVectorCraft } from "./fake-vectorcraft.mjs";

const STORY = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";
const agent = (text) => ({ default: "fake", providers: { fake: { kind: "function", label: "Fake", fn: () => text } } });

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
  const sequence = vc.doc.calls.filter((c) => c.command !== "file.saveCopy").map((c) => c.command);
  assert.deepEqual(sequence, ["select.set", SET_CROSS_SECTION, "select.all", "plugin.run", "select.set"]);
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
