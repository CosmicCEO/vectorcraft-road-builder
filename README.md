# Road Builder for VectorCraft

Draw a road as **one centerline path**; the plug-in builds everything else. Three WebAssembly plug-ins for
[VectorCraft](https://github.com/storytold/vectorcraft) (v0.24+), written in Rust, sharing one core:

| Plug-in | Kind | What it does |
|---|---|---|
| **Road Builder road (.pmap)** `…roadbuilder.import` | importer | File > Open a `.pmap` road file: the road becomes named paths you can edit. |
| **Build Road** `…roadbuilder.build` | filter | Regenerates lane lines, pavement edges and junction pieces (side street, zebra crossing, stop bar) from the paths you edit. Also has the command **Add Junction**. |
| **Road Builder road (.pmap)** `…roadbuilder.export` | exporter | File > Export As: writes the edited road back to `.pmap`. |

The `.pmap` file is a compact, diff-friendly JSON description of the road (see [Format](#the-pmap-format)). A change in VectorCraft
shows up in git as a one-line diff.

## Use it

1. **Install** (once): `sh scripts/build.sh` then `sh scripts/install.sh /path/to/vectorcraft-cli`. In the app the plug-ins appear under
   Object > Plug-ins and in File > Open / Export As.
2. **Open** a `.pmap` (File > Open).
3. **Edit** the paths whose names are listed below: move a centerline point, drag a junction marker, change a lane profile point.
4. **Build**: Edit > Select All, then Object > Plug-ins > Build Road. Old generated paths (`gen.*`) are replaced.
5. **Export** with File > Export As > Road Builder road (.pmap).

Add a junction without dragging: Select All, then Object > Plug-ins > Road Builder > Add Junction (route metre, kind, side, street name).

## Roads from a description (new in 0.2)

Draw any line (curves and angle points as you like), select it, and run **Object > Plug-ins > Road Builder > Describe Road**. Write the
road as a short list, left to right, in plain words or the shorthand:

```
facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane
```

The line becomes `road.r1`, the description is stored in the document, and **Build Road** draws every element as a coloured strip with
the right paint between them (dashed white between lanes going the same way, double yellow between opposite ways, curb lines at a
margin, white edge lines), following the line exactly: Bezier curves stay smooth, an angle point stays a sharp corner (outside mitred,
inside trimmed).

- **Elements** come from `library/elements.json` (lane, turn-pocket, bike-lane, apron, shoulder, margin, median, sidewalk, parking-lane,
  and more), with typical US widths marked "typical, not surveyed". Any width can be overridden in the description (`lane 3.3`, `margin 10ft`).
- **Break a line** (Split Path) and both pieces keep the description. To change one piece only, select it and run Describe Road with
  scope `selected`: it gets an id of its own.
- **The line follows** the middle of the median or margin when there is one (so removing a side element does not move the road), else the
  middle of the road; change it with the `anchor` setting (`center`, `left`, `right`, or an element name).
- Not built yet: junctions between described roads, turn pockets that start and end part-way along a road, lane arrows, and the
  companion panel. See `docs/DESIGN.md`.

## Junctions (new in 0.3)

Draw a short line between two road pieces (the gap), select it, and in the panel choose **A connection between two roads**. Either press
**Connect and match** (the road is simply carried across the gap) or describe a junction in words:

> This is a 4 way intersection. The west leg is a two lane rural road, the east is a two lane rural road. There are traffic lights and crosswalks.

The road runs straight through. The legs, the paved box where they cross (the median is opened up), true-arc kerb corners, crosswalks, stop
bars and signal heads or stop signs are drawn from the description. A junction is stored as small data (up to two legs, each with its own
cross-section and length; control none, signal or stop; crosswalks; kerb radius) and the panel lets you adjust the control, the crosswalks
and the legs before pressing Draw. In VectorCraft's menu the command is **Describe Junction**. Not built yet: turn pockets, curved legs,
roundabouts and junctions that are not at right angles.

## Roads that cross (new in 0.4)

Draw one road across another, select both, and the panel offers **Make an intersection**. Pick what happens where they cross:
lighted intersection, two-way stop (the narrower road stops unless you name one), four-way stop, no control, overpass/underpass (one
road bridges the other, with parapets and abutments), roundabout (counter-clockwise ring, splitter islands, yield lines). Crosswalks are
optional. Any angle works (the paved box is a parallelogram; kerb corners are true arcs), and each road can curve: the local direction at
the crossing is used. Cloverleaf is listed but not built yet.
If one road runs only a short way past the crossing (under 25 m from the centre, adjustable as `stub_length`) you choose what to do with
that **stub**: *keep it as a dead end* (a four-way with a kerbed end) or *trim it off* (a three-way: the built road stops at the other road's
travel lane, which keeps its far kerb, bike lane and verge). Your drawn line is never cut, only what is built from it. Stored as `crossings.<id>` (`treatment`, `crosswalks`, `radius`, `minor`,
`over`, `diameter`); menu command **Make Intersection**; `road-builder-cli parse-crossing`.

### More intersections (new in 0.5)
The same panel card now offers: a **driveway** or side entrance, **left and right turn pockets** (with length and taper), **right-turn slip
lanes** with an island, a **median opening** or U-turn crossover, an **on-ramp** or **off-ramp** with an acceleration or deceleration lane,
a **mid-block pedestrian crossing** (beacons optional), a **railroad crossing** (rails, ties, crossbucks, gates; use the new `railway`
element for the track), and a **cloverleaf** (the overpass and four loop ramps). A side road that only touches the other road, without
crossing, is accepted: its drawing is carried on to the pavement. Select **three or more roads** that meet at a point and the card offers the
same lighted, stop-sign or uncontrolled choices for all of them (a "hub"). Details and rules: `docs/SCENARIOS.md`.

## One layer per road (new in 0.4)

After each Draw the panel sorts the drawing into layers: `road r1` holds `road r1 spine` (the red line you edit, on top) and `road r1 drawing`
(everything built from it); junctions get the same two parts, an intersection gets one layer. Hide or lock a spine layer and the road stays
visible. The panel shows and unlocks these layers just for the rebuild (a hidden or locked object is not seen by a filter, so its road would
vanish) and puts your hiding and locking back afterwards. Rebuilding from VectorCraft's own menu does not sort layers; press Draw in the panel.

## The companion panel (new in 0.3)

A small page beside VectorCraft where you describe roads in words and click them to see and change their elements:

```
sh scripts/start-panel.sh          # starts VectorCraft with its control port, the panel, and opens your browser
```

1. **Draw a line** in VectorCraft and select it. The panel says "a plain line is selected".
2. **Describe it** in the box ("one bike lane, an apron, two south lanes, a 3 m grassy curbed margin, two north lanes, apron, bike lane")
   and press **Propose**. An agent turns the words into the shorthand; you see the proposed elements to scale before anything changes.
3. **Draw**. The road is built. Click a road later and its elements are listed: remove one (✕), move it (◀ ▶), change a width, or
   describe a change in words ("delete the bike lane going south"). If you broke the line into pieces, choose *all pieces* or *only the
   selected piece*.

The agent is a plug-in point (`panel/providers.json`): Claude (default, uses your Claude login, no key), any command that reads a JSON
request on stdin, or any OpenAI-compatible endpoint (local or hosted). Whatever the agent answers is checked by a native helper
(`road-builder-cli`) before the panel will show or apply it, so no agent can change your drawing directly. The panel listens on
127.0.0.1 only and ignores requests that did not come from its own page.

Tests: `node --test panel/test/` (24, against a fake VectorCraft) and, with VectorCraft and the panel running,
`node panel/live-check.mjs` replays the first story through the real app and a real agent.

## What you edit with the older centerline model (naming contract)

| Path name | Meaning |
|---|---|
| `centerline` | The road's centre path. Route metre `s` counts along it from its first point. Offsets are positive to the right of the direction of travel. |
| `profile.zero` | Baseline of the profile band (below the road). Its first point is route metre 0, offset 0. |
| `profile.<key>` | A lane or edge line drawn in (s, offset) space: x = route metre, height above the baseline = offset. Keys: `sbOuter sbLane sbInner nbInner nbLane nbOuter sbTurn nbTurn sbBuf nbBuf sbEdge nbEdge`. Drag a point to change a lane width at that station. |
| `flag.<name>.<n>` | A horizontal bar whose x-extent is a stretch where the flag is on: `gap` (median opening), `mouth` (driveway mouth), `closed`, `rtOnly`, `median`, `buffer`, `bridge`. |
| `<L><n>.<kind>.<side>[.<street>]` | A junction marker on the road, e.g. `C1.signal.right.Main-St`. Kinds: `tee cross street drive signal school`. Side (as seen driving along the centerline): `right left both`. |
| `arrow.<kind>.<N\|S>.<s>` | A pavement arrow; first two points are tail and tip. `<s>` is its approximate route metre. |
| `gen.*` | Generated by Build Road. Never edit these; they are replaced on every build. |

Everything is in metres (1 document point = 1 metre; y points down, so north is up).

## The pmap format

```
{"format": "vectorcraft-road-builder/pmap", "version": 1, "length": 5266.3,
 "centerline": [[x, y], ...],              metres from the first point, x east, y north
 "profile":  {"sbEdge": [[s, offset], ...], ...},
 "flags":    {"gap": [[s0, s1], ...], ...},
 "junctions": [{"id": "C1", "s": 112, "kind": "signal", "side": "right", "label": "Main-St"}],
 "arrows":   [{"s": 138, "off": -11.8, "kind": "thru", "south": true}]}
```

Numbers are rounded to a millimetre and written one item per line, so a diff shows exactly what changed. `import` then `export`
returns the same bytes.

## Develop

```
sh scripts/build.sh                                  # needs rustup + the wasm32-unknown-unknown target (see the script header)
cargo test                                           # core logic natively, incl. import -> export round trip
sh scripts/roundtrip.sh examples/sample.pmap out/ /path/to/vectorcraft-cli     # through the real VectorCraft engine
node scripts/pmap-diff.mjs a.pmap b.pmap             # what changed, in plain words
node scripts/vc.mjs <method> '<json>'                # drive a running VectorCraft (open -a VectorCraft --args --control 7979)
node scripts/mcp-smoke.mjs <cli> x.pmap --demo out/y.pmap   # the same loop over the MCP protocol, as an agent does it
```

Layout: `core/` (geometry, `pmap` import/export, `Build Road`, ABI glue; unit-tested natively), `filter/ exporter/ importer/` (thin
plug-in crates), `scripts/`, `examples/`.

### Limits worth knowing

- VectorCraft runs plug-ins in an interpreter under an instruction budget (about 50 M plus 4,000 per input byte for a filter). Build
  Road therefore writes its output as text and thins out redundant points; a 5.3 km road builds in well under the budget. Check
  new features against the full-length road (`core/tests/real_map.rs`).
- Plug-in command ids are lower-case with underscores; a parameter cannot be called `id`.
- Build Road deletes the `gen.*` paths it is given and keeps everything else, but it only sees what is selected: always Select All first.
- The Build styling (colours, dash lengths) is a plain placeholder; the website draws its own ballpoint style from the `.pmap` data.

License: MIT.
