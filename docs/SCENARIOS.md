# Intersection scenarios

What the plug-in can build where roads meet, what is planned, and the rules each one follows. Update this file in the same commit that
changes a scenario. Everything is in metres; `r` is the kerb radius, `cw` the crosswalk depth (3 m), `gap` the space between box and crosswalk (1 m).
Sizes are typical values, not surveyed ones. The FHWA MUTCD is the authoritative US source for markings; the values below are from memory and
have not been checked against it yet (see "Standards" at the end).

## Built

### Crossing: two roads cross (`core/src/crossing.rs`, stored as `crossings.<id>`)
The gesture: draw one road across another, select both, press **Draw this intersection** in the panel. Any angle, curved roads allowed (the
local direction at the crossing point is used). The paved box is the parallelogram where the two bands overlap, a quarter metre larger on every
side so it covers the roads' edge lines. Kerb returns are true arcs of radius `r` (tangent length `r / tan(theta / 2)`).

| Treatment | What is drawn |
|---|---|
| `signal` (lighted) | Stop bar across the inbound lanes of each arm, a signal head at the driver's right-hand kerb, optional crosswalks. |
| `two-way-stop` | Stop bar and stop sign on the minor road only (`minor`, else the narrower road). |
| `four-way-stop` | Stop bar and sign on every arm. |
| `uncontrolled` | Box, kerb returns, optional crosswalks. |
| `overpass` | The road named in `over` is redrawn across the other as a deck with parapets and abutments. No crosswalks or stop bars. An underpass is the same thing from the road below. |
| `roundabout` | Counter-clockwise single-lane ring (`diameter`, default 40), central island with apron, splitter islands, yield lines, optional crosswalks. |
| `cloverleaf` | Not built: reports "not built yet". |

Rules that apply to the at-grade treatments:
- **Which side traffic enters from.** Right-hand traffic. Before the crossing, a road's `fwd` lanes enter; after it, the `back` lanes enter. The stop bar spans only those lanes, and the signal head or sign stands at the driver's right.
- **Median cap.** A median, margin or verge in the middle of a road ends in a kerbed nose just past the crosswalk (0.3 m beyond it; a metre from the box when there are no crosswalks). The cap starts at the crossing centre, because on an oblique crossing the box edge is nearer than its far corner.
- **Stubs.** A road that runs under `stub_length` (default 25 m) past the crossing centre on one side is a stub. `keep` (default) draws a kerb across its end: a four-way with a dead end. `trim` cuts the built road at the nearer far corner of the box and stops the through road's paving at its outer travel lane on that side, so its far kerb, bike lane and verge carry on: a three-way. No kerb return, crosswalk, stop bar or signal is drawn for the trimmed arm. The drawn line is never cut. A road short on both ends is a short road, not a stub.
- **Not parallel.** Roads within about 10 degrees of each other do not make an intersection (a note says so).
- Notes (problems in plain words) are returned by `crossing::build`; the panel does not show them yet.

### Junction: a connecting line between two road pieces (`core/src/junction.rs`, stored as `junctions.<id>`)
A short line named `junction.<id>` joins two road pieces; the road carries through. Up to two side legs (a T or a four-way at right angles), each with its own cross-section and length; control none, signal or stop; crosswalks; kerb radius. The main road's median also caps short of the crosswalk. "Connect and match" is a junction with no legs.

## Planned (in this order)
1. **Turn pockets** (a lane with length and taper, variable offset along the road). Needs the offset code to take a width that changes along the line. Already in the first user story: "turn pocket probably twice as long".
2. **Driveway or side entrance.** A narrow curb cut: small radius, no crosswalk, no control, the kerb line and sidewalk interrupted or dipped.
3. **Median opening or U-turn crossover** on a divided road. The Ross Bridge Parkway has these (openings 14-23 m, paired crosswalks on the north side).
4. **Right-turn slip lane** with a triangular island.
5. **Skewed and offset tees** (a Y, or two side streets that do not line up).
6. **On/off ramp or merge**, with an acceleration lane, at a very sharp angle.
7. **Mid-block pedestrian crossing**, with or without a flashing beacon.
8. **Railroad crossing.**
9. **Three or more roads at one point.**
10. **Cloverleaf.**
Also from the traffic tool's own list: school-zone beacons (20 mph when flashing), several signals, and stop-controlled side streets that yield to the parkway.

## How to add one
See `CLAUDE.md` in the repo root ("Adding an intersection scenario").

## Standards
Not yet checked against the source. Values used so far (from memory, to be verified from the FHWA MUTCD and cited here): stop line 12-24 in wide, at least 4 ft before the crosswalk; crosswalk longitudinal bars 12-24 in wide, spaced 12-60 in apart, crosswalk at least 6 ft wide; normal line 4-6 in; broken line 10 ft on, 30 ft off. The code currently uses a 3 m crosswalk and 0.5 m bars at 1.2 m spacing, which are simplified, not standard. Do not present these as survey or code-compliant values.
