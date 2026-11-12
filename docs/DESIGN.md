# Design: Road Builder as an agentic VectorCraft plug-in

Status: proposal, awaiting the owner's go-ahead. Reference story: `docs/ux-example.md` (owner's words). Decision made: the
"describe it, click it, edit it" conversation lives in a **companion panel** beside VectorCraft.

## The idea in one paragraph
A road is a line you draw plus a **cross-section**: an ordered list of elements from left to right (bike lane, apron, lane, 3 m curbed
grass margin, ...). The cross-section stays with the line through splits and reconnects. A junction is a short connecting line plus a
description, turned into a template. You describe things in words; an agent turns the words into a structured spec; the plug-in turns
the spec into geometry. "Draw" is the build step. Today's plug-in already does the last part for one road with fixed lane keys; the work
is making the spec general and putting the conversation in front of it.

## What VectorCraft allows (from its plug-in docs) and what that forces
- Plug-ins are sandboxed WebAssembly: no network, no files, no AI, and they cannot add panels, open prompts after you draw, or react to
  clicks. They can show a parameter dialog when a command runs, read selected objects, write objects, keep up to 1 MiB of JSON with the
  document (`documentData`), and export a file format. Hence: **plug-in = exact geometry; agent = judgement; panel = the human face.**
- VectorCraft has a control port (`--control`) and an MCP server: anything the agent or a panel needs (selection, run command, move
  anchor, screenshot) is available without changing VectorCraft.

## Parts
1. **Spec model** (stored in `documentData`, JSON, versioned):
   `segments[id] = {cross_section: [element...], notes}`; `junctions[id] = {template, params, arms[]}`; `library` overrides.
   A drawn path is named `road.<id>` / `junction.<id>`; the geometry is the drawing, the meaning is the spec.
2. **Element library** (`library/elements.json`, data not code): lane, bike lane, apron/shoulder, curbed margin, median, turn pocket,
   crosswalk, stop bar, sidewalk... each with default width, direction, paint style, and a source note. US-typical values are marked
   "typical, not surveyed". The agent can propose a new element; a human approves it into the library.
3. **Build Road** (filter, exists): generalised from fixed profile keys to "offset every element edge along the path, split and join at
   segment ends, apply junction templates". It must honour the line exactly as drawn (second story in `docs/ux-example.md`): Bezier
   curves stay smooth, and an angle point stays an abrupt corner (a 90 degree turn must come out as a 90 degree turn, with the
   outside edges joined cleanly and the inside edges trimmed, not rounded off or crossing).
4. **Junction templates** (data + code): 4-way signalised with turn pocket and crosswalks, T, driveway, rural arms.
5. **Agent** (Claude through the VectorCraft MCP tools): description to spec edits ("turn pocket twice as long" changes one element),
   best-match element or a proposal, "connect and match / create and merge" for a joining line.
   **Agent providers** (the agent is pluggable, not just Claude): a provider is anything that takes `{description, current spec,
   element library names, selection}` and returns **validated spec edits** (a JSON patch against the spec schema). The panel (not the
   model) applies the edits, shows what will change, and only then Draw builds it. Because the model never has to drive VectorCraft
   itself, small or on-device models can be used. Three kinds, all configured in `providers.json`: `cli` (a command that reads the
   request on stdin and writes the edits on stdout; Claude via `claude -p` is the first), `http` (any OpenAI-compatible or other
   endpoint, local or hosted), and `apple-fm` (a small Swift helper around Apple's Foundation Models). A new agentic AI is added by
   dropping in a script or an endpoint entry; no plug-in change.
6. **Companion panel** (`panel/`, plain Node plus one HTML page, no build step): follows VectorCraft's selection through the control
   port, shows the selected road's elements, a description box, a Draw button. Sends text to the agent and shows what changed.
7. **pmap v2**: the exchange file becomes spec plus geometry (v1 is kept readable; the Ross Bridge map is the first migrated example).

## Phases (the owner's order, with the panel added where the story needs it)
0. **Spikes** (answer unknowns before committing): (a) what a VectorCraft split does to a path's name, so the spec survives "click in the
   middle to break the line"; (b) `documentData` survives save, reload and export; (c) selection changes are readable fast enough for a
   panel (polling the control port); (d) how the panel reaches the agent: spawn `claude -p` with the VectorCraft MCP server (uses your
   Claude login, no API key) versus a queue the interactive session reads; (e) anchor kinds: smooth versus angle points are readable
   from the path JSON, and how today's edge offsets behave at a 90 degree corner and on tight curves (the starting point for the
   offset-with-joins rewrite).
1. **Generic** (owner's step 2): spec model, element library, cross-section builder, segments and connections; migrate Ross Bridge as the
   test; no Ross Bridge names left in the core.
2. **Panel + agent loop** (the story): describe, draw, click, edit, delete an element, redraw. Defines the provider interface and the
   spec-edit schema; Claude is the first provider.
3. **Better drawing** (owner's step 3): junction templates, curves at bends, lane-count changes. **Plus agent choice (added by the
   owner):**
   - **Apple Intelligence option:** an `apple-fm` provider built on Apple's Foundation Models framework (on-device, Swift only, so a small
     helper program the panel starts). It runs on the Mac, nothing leaves it, and needs no account or API key, which suits sensitive
     descriptions such as an incident reconstruction. Uses guided generation so the model fills in the edit schema directly.
     The framework is in this Mac's SDK (macOS 27.0.1, Swift 6.4). To confirm in a short spike: that Apple Intelligence is enabled
     here (`SystemLanguageModel.default.availability`), the real context size, and how well the model handles the element library and
     a junction description.
   - **Other agentic AI:** the `cli` and `http` providers above; documented so a user can add a local model (Ollama, MLX, llama.cpp),
     another hosted model, or their own script.
   - **Provider chain (optional):** try the on-device model first and, if its edit fails validation, escalate to a larger provider.
     Off by default.
   - **Choice in the panel:** a provider picker, a "dry run" that shows the proposed edits, and undo, so any model's output is safe.
4. **Polish and ship** (owner's step 4): CLAUDE.md, CI, tagged release with the `.wasm` files, install guide.
5. **Image to road** (owner's step 1): the agent reads a screenshot, proposes centerlines, junctions and cross-sections; you review.

## Phase 0 results (run against VectorCraft 0.24.1; all five unknowns answered)
| Spike | Result | Consequence |
|---|---|---|
| (a) Split keeps the spec? | Yes. `path.split` keeps the name on **both** pieces (`road.a`, `road.a`), and names survive save and reload. | Both pieces inherit the same cross-section automatically, exactly as in the story. Segment identity cannot be the name alone: duplicates are expected, so a new "segment id" must be added when a piece is detached and given its own description. |
| (b) Data stored with the document | Plug-in data placed in `document.unknown.pluginData.<plug-in id>` survives reopen and re-save. | The spec table can live in the document. Reading and writing it from the filter (`_context.documentData` and the `documentData` output) is documented but still to be confirmed in phase 1. |
| (c) Panel follows selection | Poll of the control port: median 17 ms, worst 77 ms; a selection change is visible within about 36 ms; the selected line's name and anchor count are readable. | A panel polling about 5 times a second is more than enough. |
| (d) Panel reaches the agent | `claude -p` runs headless on the existing login: no API key, about 2.4 s and under a cent for a trivial call, and it sees all 32 VectorCraft tools. | The panel can start Claude per request. It must give that Claude its own MCP config using `--connect 127.0.0.1:7979` so it edits the same live VectorCraft the panel shows (the default server is a separate headless copy). |
| (e) Corners | Today's method (sample every 2 m, offset along the local normal) mishandles a 90 degree corner. Outside edge: cut straight across, the sharp corner is 7 m from the nearest drawn point. Inside edge: runs 7 m past the true corner, then jumps back 8.6 m and crosses itself. Smooth and angle anchors are distinguishable in the path data (`kind: "Smooth"`, handles). | The offset must be rewritten with proper joins: mitre (or chosen join) outside, trim inside, smooth through curves. This is the first piece of phase 1. |

## Phase 1 status (first slice done, in the real VectorCraft engine)
Built and tested (33 native tests, plus scripted runs of both stories in the real engine, with renders):
- `offset.rs`: offset with proper joins (sharp outside corner, trimmed inside, smooth through curves, loops cut out). Fixes spike (e).
- `library/elements.json` + `library.rs`: 15 elements with typical US widths, aliases, per-drawing overrides.
- `crosssection.rs`: the shorthand and plain words ("2 south lanes", "3 m grassy curbed margin"), canonical printing, layout from the
  path's heading, compass words resolved against the path direction, anchors (auto, center, left, right, or an element).
- `roads.rs`: segments `road.<id>`, description stored in `documentData`, strips and painted boundaries, pieces share an id,
  command **Set Cross-section** (plain line becomes a road; `scope: selected` gives selected pieces their own id).
- Real-engine findings: a fill is the paint itself (no `paint` wrapper); `documentData` written by one run is read by the next
  (confirms spike b end to end); command ids are lower-case with underscores and a parameter cannot be called `id`.

Still to do in phase 1: migrate Ross Bridge onto cross-sections, pmap v2 export of roads (the importer cannot restore `documentData`,
so the `.vectorcraft` file stays the master and pmap v2 is the hand-off to the simulation), connecting segments end to end, and removing
the last Ross-Bridge-specific names from the core. Junction templates, part-way turn pockets and arrows are phase 3.

## Phase 2 status (companion panel and agent loop: done for roads; junction connections wait for phase 3)
- `road-builder-cli` (native): parse, canonical text, validate and edit (delete, move, width, insert) cross-sections.
- `panel/lib/providers.mjs`: agents are text in, shorthand out, then validated and, if invalid, sent back once with the exact error.
  Kinds: `claude` (headless, text only, no tools, no hooks), `cli` (any command, JSON on stdin), `http` (OpenAI-compatible).
  Tested with a real Claude on the story sentence, the "delete the bike lane going south" edit, a rural-road description and a turn pocket.
- `panel/server.mjs` + `panel/lib/panel.mjs`: follows the selection through the control port (about 17 ms a poll), reads the stored
  description from a saved copy, shows a proposal before anything changes, applies Describe Road then Build Road, restores the
  user's selection. Loopback only; Host and Origin checked.
- `panel/public/`: the page (to-scale bar, element list with remove/move/width, describe box, scope choice, Draw).
- Verified live: the first story, end to end, through the panel in the real app with a real agent (`panel/live-check.mjs`).
- Real-world snag found: macOS asks permission before one app may script another, so the panel never uses AppleScript.
- Apple Intelligence and other agents (phase 3): the provider interface is in place; `apple-fm` needs the small Swift helper.

## Verification at each phase
Native unit tests for the core; the real VectorCraft engine via `scripts/roundtrip.sh` and the MCP loop (`scripts/mcp-smoke.mjs`);
a visual check by screenshot against the story, step by step.

## Risks
- Splits and detaches may not carry names (spike a). Fallback: identify segments by endpoint position plus the spec's own geometry.
- A small on-device model is limited: a short context window, so the prompt must stay compact (library names only, not full definitions),
  and it is text only, so it cannot read screenshots (phase 5 needs a multimodal provider). Expect to escalate hard descriptions and
  edits. Whatever the provider, its output is validated against the schema before anything is changed.
- Instruction budget: more elements per road means more work per Build; keep text output and thinned points.
- The panel depends on VectorCraft's control port (documented, loopback only, no authentication; only open while in use).
- Dimensions from standards must be sourced and labelled; the output is an illustrative reconstruction, not a survey.
