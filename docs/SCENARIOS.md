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
| `cloverleaf` | The overpass plus four loop ramps: see "Added in 0.5". |

Rules that apply to the at-grade treatments:
- **Which side traffic enters from.** Right-hand traffic. Before the crossing, a road's `fwd` lanes enter; after it, the `back` lanes enter. The stop bar spans only those lanes, and the signal head or sign stands at the driver's right.
- **Median cap.** A median, margin or verge in the middle of a road ends in a kerbed nose just past the crosswalk (0.3 m beyond it; a metre from the box when there are no crosswalks). The cap starts at the crossing centre, because on an oblique crossing the box edge is nearer than its far corner.
- **Stubs.** A road that runs under `stub_length` (default 25 m) past the crossing centre on one side is a stub. `keep` (default) draws a kerb across its end: a four-way with a dead end. `trim` cuts the built road at the nearer far corner of the box and stops the through road's paving at its outer travel lane on that side, so its far kerb, bike lane and verge carry on: a three-way. No kerb return, crosswalk, stop bar or signal is drawn for the trimmed arm. The drawn line is never cut. A road short on both ends is a short road, not a stub.
- **Not parallel.** Roads within about 10 degrees of each other do not make an intersection (a note says so).
- Notes (problems in plain words) are returned by `crossing::build` and `hub::build`; the panel does not show them yet.

### Junction: a connecting line between two road pieces (`core/src/junction.rs`, stored as `junctions.<id>`)
A short line named `junction.<id>` joins two road pieces; the road carries through. Up to two side legs (a T or a four-way at right angles), each with its own cross-section and length; control none, signal or stop; crosswalks; kerb radius. The main road's median also caps short of the crosswalk. "Connect and match" is a junction with no legs.

### Added in 0.5 (all in `core/src/crossing.rs`, `core/src/scenarios.rs` and `core/src/hub.rs`)

**Roads that only touch.** A side road drawn up to the kerb, a driveway or a ramp does not have to cross: if one road's end, carried straight on for 30 m, meets the other, that is the intersection (`reach_*` is false for the road that stops short). That road is always trimmed on its missing side and its drawing is carried on to the far travel lane of the other road, under the box, so it meets the pavement. Offset tees are simply two crossings on one road, one per side road; a skewed tee is an oblique crossing.

| Scenario | How it is drawn |
|---|---|
| **Turn pockets** (`pocket` none/left/right/both, `pocket_length` 30, `pocket_taper` 15) | A pocket is a 3.3 m pavement strip beside the inbound lanes from the box edge to `pocket_length`, then a taper back to the lane edge, with a dashed lane line, a solid outer line and an arrow. A **left** pocket is cut into a median, margin or centre strip and needs 3.3 m of it (otherwise a note says so and none is drawn); a **right** pocket widens the road outward. Drawn before the crosswalk so the crosswalk lies over it. |
| **Driveway** (`driveway`) | An uncontrolled three-way with a 1.5 m kerb return (instead of 6 m) and no stop bars. Paving covers the through road's near kerb, bike lane and apron across the driveway's width but not its far kerb. |
| **Slip lanes** (`slip`, `slip_radius` 15) | At each kerb corner the return is rounded with the big radius and a triangular island 4 m inside it, kerbed. |
| **Median opening** (`median-opening`, `opening` 18) | The divided road (`main`, else the road with an interior median) has its median opened `opening` metres, half each way from the crossing, with a kerbed nose at each end. The other line only says where: it is cut out across the divided road (a *gap* trim leaves up to two pieces, the second named `.part2`). A road with no median reports a note. |
| **On-ramp / off-ramp** (`on-ramp`, `off-ramp`, `accel_length` 120, `accel_taper` 40) | An uncontrolled joining with no kerb return and a 3.6 m speed-change lane beside the through road's outer travel lane: downstream of the join for an on-ramp, upstream for an off-ramp ("downstream" is along the line on the right-hand side, since right-hand lanes run with it). |
| **Mid-block crossing** (`midblock-crossing`, `beacon`) | Crosswalk bars over the lanes only (a median stays a refuge), a stop line 4.5 m before it for each direction, flashing beacons 0.9 m beyond each kerb. The marker line is cut out across the road. |
| **Railroad** (`railroad`, `gates`) | The line with the new `railway` element (4 m ballast) crosses the road: rubber crossing panel, ties every 0.6 m, two rails 1.435 m apart running 6 m past the panel, a stop line 4.6 m (15 ft) from the nearest rail on each side, a crossbuck and lamp at the driver's right, and a gate arm across the inbound lane if `gates`. Needs the roads to be more than about 10 degrees apart. |
| **Cloverleaf** (`cloverleaf`, `over`) | The overpass plus four 270-degree one-lane loop ramps, one per quadrant, each tangent to the outer edge of both roads (centreline radius 40 m, 5 m wide), with a grass infield and a 70 m speed-change strip along each road. Needs roads within about 20 degrees of a right angle. The diagonal connectors of a full interchange are **not** drawn (a note says so). |
| **Three or more roads** (`hubs.<id>`, `hub.rs`) | Each road contributes an arm per side where it runs on at least 3 m (a road that stops there contributes one). Arms are sorted by bearing; the corner between neighbours is where one's left kerb line meets the next one's right kerb line; each corner gets a kerb return; every arm gets the usual crosswalk, stop bar and signal or sign. Where two neighbouring arms point opposite ways (a road side with no leg) the core stops at the outer travel lane, keeping the far kerb. Roads that stop short are carried on to the core; stubs follow `stub`/`stub_length`. Treatments: signal, two-way stop (narrowest road or `minor`), all-way stop, uncontrolled. 3 to 8 roads. |

Geometry checks: every scenario has native tests with hand-worked positions (`core/tests/crossings.rs`, `core/tests/hubs.rs`) and was rendered headless at 10x and looked at.

## Still planned
- Yield signs and priority control on the minor road of a roundabout approach; pedestrian refuge islands at mid-block crossings on roads without a median.
- Full cloverleaf connectors, collector-distributor roads, diamond and diverging-diamond interchanges.
- Curved ramps that follow the drawn line (ramp strips are straight along the through road).
- Signal phasing and school-zone beacons (20 mph when flashing), from the traffic tool's own list.

## How to add one
See `CLAUDE.md` in the repo root ("Adding an intersection scenario").

## Standards
Not yet checked against the source. Values used so far (from memory, to be verified from the FHWA MUTCD and cited here): stop line 12-24 in wide, at least 4 ft before the crosswalk; crosswalk longitudinal bars 12-24 in wide, spaced 12-60 in apart, crosswalk at least 6 ft wide; normal line 4-6 in; broken line 10 ft on, 30 ft off. The code currently uses a 3 m crosswalk and 0.5 m bars at 1.2 m spacing, which are simplified, not standard. Do not present these as survey or code-compliant values.
