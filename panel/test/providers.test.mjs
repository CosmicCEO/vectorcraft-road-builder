import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { library } from "../lib/cli.mjs";
import { extractJson, extractLine, junctionSystemPrompt, junctionUserPrompt, systemPrompt, userPrompt } from "../lib/prompt.mjs";
import { ask, loadProviders, propose, proposeJunction } from "../lib/providers.mjs";

const STORY = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";
const fake = (...replies) => {
  const seen = [];
  return { kind: "function", seen, fn: ({ user }) => (seen.push(user), replies[Math.min(seen.length, replies.length) - 1]) };
};

test("the prompt carries the grammar and every library element", async () => {
  const lib = await library();
  const p = systemPrompt(lib);
  for (const e of lib.elements) assert.ok(p.includes(e.id), `prompt lists ${e.id}`);
  assert.match(p, /LEFT TO RIGHT/);
  assert.match(p, /facing N:/);
  assert.match(userPrompt({ description: "x", current: STORY }), /Current cross-section:\nfacing N/);
  assert.match(userPrompt({ description: "add parking" , current: null }), /^Description: add parking/);
});

test("the answer is pulled out of quotes and code fences", () => {
  assert.equal(extractLine("```\nlane | lane\n```"), "lane | lane");
  assert.equal(extractLine('"facing N: lane S | lane N"'), "facing N: lane S | lane N");
  assert.equal(extractLine("Sure! Here you go:\nfacing S: lane | lane"), "facing S: lane | lane");
  assert.equal(extractLine(""), "");
});

test("a valid answer comes back validated and in canonical form", async () => {
  const r = await propose({ description: "the story road", provider: fake("```\nfacing n: bike-lane | apron | lane s x2 | margin 3 | lane n x2 | apron | bike-lane\n```") });
  assert.equal(r.ok, true);
  assert.equal(r.canonical, STORY);
  assert.equal(r.items.length, 9);
  assert.equal(r.total_width, 21.6);
  assert.equal(r.attempts.length, 1);
});

test("an invalid answer is sent back once with the exact error", async () => {
  const p = fake("lane | warp-drive", "lane back | lane fwd");
  const r = await propose({ description: "two lanes", provider: p });
  assert.equal(r.ok, true);
  assert.equal(r.canonical, "lane back | lane fwd");
  assert.equal(r.attempts.length, 2);
  assert.match(p.seen[1], /rejected: unknown element "warp-drive"/);
});

test("it gives up politely after the allowed attempts", async () => {
  const r = await propose({ description: "x", provider: fake("nonsense-thing"), maxAttempts: 2 });
  assert.equal(r.ok, false);
  assert.match(r.error, /could not produce a valid cross-section/);
  assert.equal(r.attempts.length, 2);
});

test("a provider that fails is reported, not thrown", async () => {
  const r = await propose({ description: "x", provider: { kind: "function", fn: () => { throw new Error("no network"); } } });
  assert.deepEqual([r.ok, r.error], [false, "no network"]);
});

test("an edit request shows the model the current text", async () => {
  const p = fake("facing N: apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane");
  const r = await propose({ description: "remove the west bike lane", current: STORY, provider: p });
  assert.match(p.seen[0], /Current cross-section:\nfacing N: bike-lane/);
  assert.equal(r.items.length, 8);
});

test("a command provider gets the request on stdin and answers on stdout", async () => {
  const dir = mkdtempSync(join(tmpdir(), "prov-"));
  const script = join(dir, "agent.mjs");
  writeFileSync(script, 'let t="";process.stdin.on("data",d=>t+=d).on("end",()=>{const r=JSON.parse(t);console.log(JSON.stringify({cross_section:r.description.includes("two")?"lane back | lane fwd":"lane"}))})');
  const r = await propose({ description: "two lane road", provider: { kind: "cli", command: process.execPath, args: [script] } });
  assert.equal(r.canonical, "lane back | lane fwd");
});

test("an http provider talks to an OpenAI-style endpoint", async () => {
  let body;
  const server = createServer((req, res) => {
    let t = "";
    req.on("data", (d) => (t += d)).on("end", () => {
      body = JSON.parse(t);
      res.setHeader("content-type", "application/json");
      res.end(JSON.stringify({ choices: [{ message: { content: "lane back | lane fwd" } }] }));
    });
  });
  await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
  try {
    const cfg = { kind: "http", base_url: `http://127.0.0.1:${server.address().port}/v1`, model: "tiny" };
    assert.equal(await ask(cfg, { system: "S", user: "U", request: {} }), "lane back | lane fwd");
    assert.equal(body.model, "tiny");
    assert.deepEqual(body.messages.map((m) => m.role), ["system", "user"]);
  } finally {
    server.close();
  }
});

test("a missing key or program gives a plain message", async () => {
  await assert.rejects(ask({ kind: "http", base_url: "http://127.0.0.1:1", api_key_env: "NO_SUCH_KEY_XYZ" }, { system: "", user: "" }), /set the NO_SUCH_KEY_XYZ/);
  await assert.rejects(ask({ kind: "cli", command: "definitely-not-a-program" }, { system: "", user: "", request: {} }), /not installed/);
  await assert.rejects(ask({ kind: "wibble" }, { system: "", user: "" }), /unknown provider kind/);
});

test("the user's providers file adds to the built-in ones", () => {
  const dir = mkdtempSync(join(tmpdir(), "prov-"));
  const mine = join(dir, "providers.json");
  writeFileSync(mine, JSON.stringify({ default: "local", providers: { local: { kind: "http", base_url: "http://localhost:11434/v1", model: "llama3" } } }));
  const cfg = loadProviders(mine);
  assert.equal(cfg.default, "local");
  assert.ok(cfg.providers.claude && cfg.providers.local, "built-in and user providers are both there");
});

const FOUR_WAY = '{"legs": [{"side": "west", "cross_section": "lane back | lane fwd"}, {"side": "east", "cross_section": "lane back | lane fwd"}], "control": "signal", "crosswalks": true}';

test("JSON is found inside whatever else the model said", () => {
  assert.equal(extractJson('Here:\n```json\n{"a": {"b": "}"}, "c": 1}\n```\nDone'), '{"a": {"b": "}"}, "c": 1}');
  assert.equal(extractJson("no braces here"), "");
  assert.equal(extractJson('{"unfinished": '), "");
});

test("the junction prompt carries the schema, the elements and an example", async () => {
  const p = junctionSystemPrompt(await library());
  for (const w of ["legs", "control", "crosswalks", "radius", "traffic lights", "lane back | lane fwd", "turn-pocket"]) assert.ok(p.includes(w), w);
  assert.match(junctionUserPrompt({ description: "add a crosswalk", current: { legs: [] } }), /Current junction:\n\{"legs":\[\]\}/);
});

test("a junction description is validated, and an invalid one is sent back once", async () => {
  const p = fake('{"control": "roundabout"}', `Here you go: ${FOUR_WAY}`);
  const r = await proposeJunction({ description: "four way with lights", provider: p });
  assert.equal(r.ok, true);
  assert.equal(r.junction.kind, "cross");
  assert.match(r.summary, /four-way junction, traffic signals, crosswalks/);
  assert.equal(r.attempts.length, 2);
  assert.match(p.seen[1], /rejected: control must be/);
  const bad = await proposeJunction({ description: "x", provider: fake("I cannot do that") });
  assert.equal(bad.ok, false);
  assert.match(bad.error, /could not produce a valid junction/);
});
