# CLAUDE.md: Road Builder for VectorCraft

A VectorCraft plug-in plus a companion panel that turn a drawn line and a description in words into a road with lanes, kerbs, markings and
intersections. Read `README.md` for what it does and `docs/SCENARIOS.md` for the intersection scenarios (built, planned, rules). The owner is
not a developer: keep chat answers short and plain. Never push to `main`; work on a branch.

## Layout
- `core/` Rust library (compiled to the plug-in and to a native CLI): `crosssection.rs` (shorthand and layout), `offset.rs`, `roads.rs`, `junction.rs`, `crossing.rs` (spec, find, trims, approach marks, at-grade, overpass, roundabout), `scenarios.rs` (median opening, mid-block, railroad, ramp lanes, cloverleaf), `hub.rs` (three or more roads), `builder.rs` (the commands), `output.rs` (output written as text), `library/elements.json` (the 15 road elements).
- `filter/` the plug-in's manifest and commands (version, parameters). `cli/` `road-builder-cli` (parse, edit, diff, parse-junction, parse-crossing, parse-hub, library): the panel uses it to check what an agent returns.
- `panel/` zero-dependency Node server on 127.0.0.1:8787 and the page; `panel/lib/layers.mjs` sorts the drawing into layers.
- Tests: `cargo test --release` (all Rust) and `cd panel && node --test test/` (panel, against a fake VectorCraft in `panel/test/fake-vectorcraft.mjs`).

## Build, install, run
- `sh scripts/build.sh` builds the wasm plug-ins (it sets RUSTC to the rustup toolchain: Homebrew's rustc has no wasm target). `cargo +stable build --release -p road-builder-cli` rebuilds the native CLI; the panel tests need it, and a stale one fails with "usage: ..." for new commands.
- Install into a running app without restarting: `node scripts/vc.mjs engine.execute '{"command":"plugin.install","params":{"path":"<abs path to road_builder_filter.wasm>","replace":true}}'`. Start VectorCraft with `open -a VectorCraft --args --control 7979`. Never touch the user's open document without asking: test in a headless session instead (`vectorcraft-cli mcp` over stdio, as `scripts/mcp-smoke.mjs` does, installing the plug-ins into that session first).
- Restart the panel after changing `panel/` code: `node panel/server.mjs --control 7979 --port 8787`. The user refreshes the page.

## Traps we hit (do not repeat)
- A filter sees only **visible, unlocked** selected objects. A hidden spine layer made its road's drawing vanish on rebuild. The panel therefore shows and unlocks its own layers for the build and restores them (`unlockOurs`). New objects land in the layer of the first selected object, so `organise` sorts them afterwards.
- `layer.new` without `top: true` nests inside a current sublayer. Use `top: true` for top-level layers.
- A filter cannot create layers or lock its output (locked or hidden objects are not given to filters). Layers are the panel's job.
- New objects are listed first and stack **below** kept objects: that is how the spine stays on top.
- Fills are `{"type":"solid","color":...}` with no `paint` wrapper; strokes have `paint` and `width`. A command parameter cannot be called `id`; command ids are lower-case with underscores.
- Instruction budget (about 50 M plus 4,000 per input byte): write output as text and thin out points. Check long roads with `core/tests/real_map.rs`.
- Right-hand traffic: facing along the line the RIGHT lane is `fwd` and the LEFT lane is `back`, so a two-lane road is `lane back | lane fwd`. Getting this backwards put stop bars on the wrong half of the road.
- macOS `sed -i` needs `''`. Do not use AppleScript here (it triggers a permission prompt on the user's Mac).
- When splicing code with a script, never compute offsets from text you have already changed (a splice once dropped the overpass and roundabout code: restore from `git show HEAD:<file>`).
- A trim keeps one end of a line; a *gap* trim cuts the middle out (two pieces, second named `.part2` so click mapping still works).
- A road that only touches another is found by extending its ends 30 m; test with the road stopping short, and with it stopping more than 30 m short (no intersection).
- Headless renders: export with `scale: 10` and crop tight; at scale 4 a 7 m road is a smudge.

## Adding an intersection scenario
1. **Spec.** Add the field(s) to `CrossingSpec` (or a new spec) in `core/src/crossing.rs`: parse every spelling a person might use, return a plain-words error for anything else, write it in `to_json`, say it in `summary`.
2. **Geometry.** Emit shapes named `gen.crossing.<id>.<part>[.<road>.<before|after>]` through `Out` (polygons, strokes). Anything that changes how a road is drawn (a trim, a pocket) must work on the drawn copy and never on the document's line: see `crossing::trims` and `roads::build_clipped`.
3. **Tests first.** Add native tests in `core/tests/crossings.rs` with exact numbers worked out by hand (positions, widths, which side traffic enters from). Also an oblique case. Run the whole suite.
4. **Panel.** Add the choice to the card in `panel/public/index.html` and `app.js`, the key to `editCrossing` in `panel/lib/panel.mjs`, the field to the agent schema and an example in `panel/lib/prompt.mjs`, and a panel test.
5. **Look at it.** Render it in a headless session at 8x and read the picture: tests do not catch an ugly sliver (an oblique median fragment was found this way).
6. **Docs and commit.** Update `docs/SCENARIOS.md` (status, rules) and the README; commit with the trailer; push the branch; install into the running app and restart the panel.

## Rules
- Standards: values come from `library/standards.json` (`docs/STANDARDS.md` is generated: `node scripts/standards.mjs md`). Name a dimension in `core/src/standards.rs` and add it to `core/tests/standards.rs`; mark a database entry `verified` only if you read it in a federal source, `verified-secondary` for a state manual, `unverified` for anything from memory. The FHWA safety site and highways.dot.gov return 403 to scripts: do not spoof a browser to get past that, download by hand or use another source.
- Widths are "typical, not surveyed". Never describe a generated drawing as a survey or as compliant with a standard unless the value is verified from the source and cited in `docs/SCENARIOS.md`.
- Do not copy the owner's other projects' data into this repo (it is public). The Ross Bridge Parkway migration is parked until the owner says otherwise.
- Never force-push or squash, and never push to `main`.
