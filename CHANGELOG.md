# Changelog

## 0.6.0 Lane changes, gores, standards, release packaging
- Lane transitions: a road gains or loses 1 to 3 lanes in one direction on the driver's left or right, over a taper that follows the MUTCD formula (or a length you give). Several in a row take a road from 1 to 4 lanes and back. Command **Add or Drop a Lane**; a card in the panel; the agent understands "the right lane ends after 300 m".
- Diverge and converge: on-ramps and off-ramps work on either side and draw the gore with white chevrons (MUTCD 3B.25). "Merge", "converge", "diverge" and "exit" are understood.
- Primary signal heads stand on the far side, at least 40 ft beyond the stop line; the head at the stop line is the supplemental one.
- The panel shows notes from the last build, lists what every proposal changes in plain words, offers a fallback agent when one cannot answer, and hides optional agents that are not installed.
- CI (tests on every push) and a release workflow; `scripts/package.sh` builds the release package.

## 0.5.0 More intersections, standards
- Turn pockets, driveway, slip lanes, median opening, on/off ramps, mid-block crossing, railroad crossing, cloverleaf, three or more roads (hub), roads that only touch.
- `library/standards.json`: 56 entries from the MUTCD 11th Edition with Revision 1, PROWAG and state manuals, each with section, quote and a verified status; `docs/STANDARDS.md` is generated from it. Seven drawing dimensions corrected to the MUTCD.

## 0.4.0 Crossing roads, layers
- Select two roads and make an intersection: lighted, two-way or four-way stop, no control, overpass or underpass, roundabout. Short stubs trimmed to a three-way or kept as a dead end. Medians end short of the crosswalk.
- One layer per road (spine and drawing) so the red line can be hidden while the road stays visible; the panel keeps your hiding and locking across rebuilds.

## 0.3.0 Junctions, the companion panel
- Junctions drawn from a connecting line (four-way or T, legs, paved box, kerb arcs, crosswalks, stop bars, signals or stop signs).
- The companion panel beside VectorCraft, agent providers (Claude, a command, an OpenAI-style endpoint, Apple Intelligence on this Mac).

## 0.2.0 Roads from a description
- A line named `road.<id>` becomes a road from a cross-section written in shorthand or described in words; a library of 16 road elements.

## 0.1.0 First slice
- Build Road filter, pmap importer and exporter for VectorCraft.
