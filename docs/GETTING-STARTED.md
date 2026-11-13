# Getting started with Road Builder

Road Builder turns a line you draw in VectorCraft into a road, and two or more lines into an intersection. You describe what you want in plain words or with a few choices, press **Draw**, and the road is built from the line. The line itself is never changed.

## What you need
- **VectorCraft** 0.24.1 or newer (the Mac app) and its command-line tool `vectorcraft-cli`.
- **Node.js** 20 or newer (`node --version`). The panel has no other dependencies.
- For the agent that understands plain words: Claude Code (`claude`) signed in, or another agent set up in `panel/providers.json`. The choices on the cards work without any agent.

## Install (once)
1. Unzip the release. In a terminal, from inside the folder: `sh install.sh`. This puts the three plug-ins (Build Road, the pmap importer and exporter) into VectorCraft.
2. Quit VectorCraft if it is running, then start everything with `sh start.sh`. It opens VectorCraft with its control port, starts the panel and opens the panel in your browser (http://127.0.0.1:8787). Keep the panel beside VectorCraft.

## Your first road
1. In VectorCraft draw a line (the Pen tool) and select it. The panel says "A plain line is selected".
2. In the box, describe the road: "two lanes with a bike lane on each side", or the shorthand `bike-lane | lane back | lane fwd | bike-lane`. Press **Propose**, read the picture and the **What changes** list, then press **Draw this**.
3. The road appears under your line. The red line is the one you edit: move its points to reshape the road, then press **Redraw**. Hide the red line's layer to see the road alone; Road Builder keeps your hiding and locking.

## An intersection
1. Draw a second road across the first and select **both** lines. The panel offers **Make an intersection**.
2. Pick what it is (lighted, stop signs, driveway, roundabout, overpass, ramp, railroad, median opening, mid-block crossing, cloverleaf) and the extras (crosswalks, turn pockets, slip lanes). A road that only touches the other, or three or more roads meeting at a point, work too. Press **Draw this intersection**.
3. A short stub of road past the crossing can be **kept as a dead end** (a four-way) or **trimmed off** (a three-way).

## Lane changes
Select one line of a described road and press **Add or drop a lane on this line**. Choose add or drop, the side (left is toward the centre of the road, right is toward its edge), how many lanes and where it starts, or say it in words: "the right lane ends after 300 m". Several lane changes in a row take a road from one lane to four and back.

## Good to know
- **Notes from the last build** appear in the panel when something was guessed or skipped (for example "no median wide enough for a left-turn pocket").
- Sizes are typical values, not surveyed ones. Marking, crosswalk, stop-line, sign and railroad dimensions follow the MUTCD; see `docs/STANDARDS.md` for sources and `docs/SCENARIOS.md` for every treatment and its rules.
- Every drawing is "an illustrative reconstruction", not a survey.
- If the panel says it cannot reach VectorCraft, quit VectorCraft and run `sh start.sh` again: the control port only exists when it was started that way.

## If something goes wrong
- **The road did not change:** select the line (not the red spine's neighbours) and press Draw; check **Notes**.
- **"road-builder-cli not found":** run `cargo build --release -p road-builder-cli` (needs Rust) and put the result in `bin/`.
- **An agent answers with something invalid:** the panel asks it once to correct itself, then tries the agent's fallback if it has one.
