# Road design standards used by Road Builder

Generated from `library/standards.json` by `node scripts/standards.mjs md`. Edit the JSON, not this file.

Every entry names its source and section. status: verified = read in the primary text; verified-secondary = read in a state design manual that applies or reproduces a federal guide (the primary was not available); unverified = from general knowledge or not found, do not present as a standard. Values are the standard's own units (us) with a metric conversion (si, metres or metres per second). The code field records how the plug-in's own constant compares (conforms true, false or null when not comparable). Widths in the plug-in's library are typical values, not surveyed ones.

**The NTSB** investigates accidents and recommends changes; it does not publish design dimensions. The federal source for markings, signs and signals is the FHWA MUTCD; for pedestrian facilities the US Access Board; for lane, kerb and turn-lane geometry the AASHTO Green Book (copyrighted, not reproduced here) and state design manuals.

**56 entries:** 42 verified in the primary text, 12 verified in a state manual, 2 unverified.

## Sources

| Key | Source | Kind | Retrieved |
|---|---|---|---|
| `mutcd` | [Manual on Uniform Traffic Control Devices for Streets and Highways, 11th Edition (December 2023) with Revision 1 incorporated (December 2025)](https://mutcd.fhwa.dot.gov/kno_11th_Editionr1.htm), US DOT Federal Highway Administration | federal-standard | 2026-11-12 |
| `prowag` | [Accessibility Guidelines for Pedestrian Facilities in the Public Right-of-Way (PROWAG), Chapter R3 Technical Requirements](https://www.access-board.gov/prowag/technical.html), US Access Board (36 CFR Part 1190) | federal-standard | 2026-11-12 |
| `fdot212` | [FDOT Design Manual, Chapter 212 Intersections (topic 625-000-002, January 1, 2018)](https://www.fdot.gov/docs/default-source/roadway/FDM/current/2018FDM212Intersections.pdf), Florida Department of Transportation | state-manual | 2026-11-12 |
| `scdot_rbt` | [SCDOT Roundabout Design Criteria (Figure 9.7-A to D)](https://www.scdot.org/business/pdf/I85/RoadwayDesign/Roundabout_Criteria.pdf), South Carolina Department of Transportation | state-manual | 2026-11-12 |
| `wisdot_1126` | [WisDOT Facilities Development Manual 11-26 Roundabouts](https://wisconsindot.gov/rdwy/fdm/fd-11-26.pdf), Wisconsin Department of Transportation | state-manual | 2026-11-12 |

- `mutcd`: Official current edition. Quotes are verbatim from the PDF text; page numbers are the MUTCD's own. File SHA-256 `f508594285e5fccc…`. US Government work, public domain.
- `prowag`: Web edition of the technical requirements. File SHA-256 `f0f32edf4a56c297…`. US Government work, public domain.
- `fdot212`: A state manual (not federal). Used for the numbers the MUTCD leaves to design guides. The 2018 edition is old; check the current FDM before relying on a value. File SHA-256 `a3eef8609c10d70e…`. State agency publication, freely published.
- `scdot_rbt`: States that it follows NCHRP Report 672 (Roundabouts: An Informational Guide, 2nd ed.); the primary text could not be read (see sources_blocked). File SHA-256 `3e031509dfb1c7f6…`. State agency publication, freely published.
- `wisdot_1126`: Cites NCHRP Report 1043 (2023) for its Table 8.1. File SHA-256 `969e5c1c13da3bff…`. State agency publication, freely published.

## Markings

### Normal longitudinal line width
`line.normal-width` · verified · Federal Highway Administration MUTCD 3A.04 para 02A · standard

- width: 4 in (0.102 m) to 6 in (0.152 m)

> Normal line—4 to 6 inches wide.

Plug-in: lane/edge/centre line stroke widths (roads.rs boundary(): 0.10 to 0.15 m): **conforms**. The thinnest marking line is now MARKING_LINE_MIN = 0.11 m (4.3 in): the buffer line was 0.10 m (3.9 in) and is fixed. Lane and centre lines are 0.12 m (4.7 in), bike and edge lines 0.15 m (5.9 in).

### Wide line
`line.wide-width` · verified · Federal Highway Administration MUTCD 3A.04 para 02B · standard

- width: at least twice the width of a normal line

> Wide line—at least twice the width of a normal line.

### Double line space
`line.double-spacing` · verified · Federal Highway Administration MUTCD 3A.04 para 03 · guidance

- space between lines: not more than 2 x the width of a single line

> the discernible space separating the parallel lines of a double line should not exceed two times the line width of a single line.

Plug-in: double yellow: 0.3 m between line centres, 0.12 m lines: **conforms**. Space between the lines is 0.18 m, under the 0.24 m limit.

### Broken line segment and gap
`line.broken-pattern` · verified · Federal Highway Administration MUTCD 3A.04 para 06 · guidance

- segment: 10 ft (3.048 m)
- gap: 30 ft (9.144 m)

> Broken lines should consist of 10-foot line segments and 30-foot gaps, or dimensions in a similar ratio of line segments to gaps as appropriate for traffic speeds and the need for delineation.

Plug-in: lane line dash 3.0 m on, 9.0 m off (roads.rs boundary()): **conforms**. 9.8 ft / 29.5 ft: the 1:3 ratio of the standard.

### Dotted lane line
`line.dotted-lane` · verified · Federal Highway Administration MUTCD 3A.04 para 07 · guidance

- segment: 3 ft (0.914 m)
- gap: 9 ft (2.743 m)

> A dotted line used as a lane line (see Section 3B.07) should consist of 3-foot line segments and 9-foot gaps.

Plug-in: dotted lane line beside pockets and speed-change lanes: 0.914 m on, 2.743 m off (standards.rs): **conforms**. Fixed: DOTTED_LANE_ON/OFF are 0.914 m / 2.743 m (3 ft / 9 ft).

### Dotted line extension through an intersection, taper or ramp area
`line.dotted-extension` · verified · Federal Highway Administration MUTCD 3A.04 para 07 · guidance

- segment: 2 ft (0.61 m)
- gap: 2 ft (0.61 m) to 6 ft (1.829 m)

> A dotted line for line extensions within an intersection, taper area, or interchange ramp area (see Section 3B.11) should consist of 2-foot line segments and 2-foot to 6-foot gaps.

### Two-way left-turn lane markings
`line.tcwl` · verified · Federal Highway Administration MUTCD 3B.05 para 01 · standard

- pattern: normal width broken yellow line toward the lane and normal width solid yellow line toward the adjacent traffic lane, each side

> the lane line pavement markings on each side of the two-way left-turn lane shall consist of a normal width broken yellow line and a normal width solid yellow line

### Minimum passing sight distance for no-passing zones (Table 3B-1)
`line.nopass-sight` · verified · Federal Highway Administration MUTCD Table 3B-1 · table

- speed to passing sight distance: 25 mph 450 ft, 30 mph 500 ft, 35 mph 550 ft, 40 mph 600 ft, 45 mph 700 ft, 50 mph 800 ft, 55 mph 900 ft, 60 mph 1000 ft, 65 mph 1100 ft, 70 mph 1200 ft

> Table 3B-1. Minimum Passing Sight Distances for No-Passing Zone Markings

### Stop line width
`stop-line.width` · verified · Federal Highway Administration MUTCD 3B.19 para 05 · guidance

- width: 12 in (0.305 m) to 24 in (0.61 m)

> Stop lines should be 12 to 24 inches wide.

Plug-in: stop bar stroke 0.6 m (crossing.rs approach()): **conforms**. 23.6 in: within range, at the top.

### Stop and yield line setback from the crosswalk
`stop-line.setback` · verified · Federal Highway Administration MUTCD 3B.19 para 13 · guidance

- minimum in advance of the nearest crosswalk line: 4 ft (1.219 m)
- no marked crosswalk: distance from the nearest edge of the intersecting traveled way: 4 ft (1.219 m) to 30 ft (9.144 m)

> stop and yield lines should be placed a minimum of 4 feet in advance of the nearest crosswalk line at controlled intersections

Plug-in: stop bar: near edge STOP_LINE_SETBACK_MIN (1.22 m) beyond the crosswalk: **conforms**. Fixed: the bar now stands CW + 1.22 m + half its width (4.52 m) beyond the box edge plus the 1 m gap, so its near edge is 4 ft (1.22 m) past the crosswalk. Without a crosswalk the bar is 1.6 m from the box edge (5.2 ft): within 4 to 30 ft.

### Stop lines are not used where drivers yield
`stop-line.must-not-yield` · verified · Federal Highway Administration MUTCD 3B.19 para 03, 09 · standard

- rule: stop lines shall not be used at YIELD signs or uncontrolled approaches; yield lines shall not be used where drivers must stop

> Yield lines shall not be used at locations where drivers are required to stop in compliance with a STOP (R1-1) sign, a Stop Here for Pedestrians (R1-5b) sign, a Stop Here for School Crossing (R1-5c) sign, a Stop Here for Trail Crossing (R1-5e) sign, a traffic control signal, or some other traffic control device.

Plug-in: mid-block crossing draws a yield line of triangles (scenarios.rs midblock()): **conforms**. Fixed: an uncontrolled mid-block crossing now draws a yield line of triangles. A signal- or hybrid-beacon-controlled crossing would use a stop line (not modelled).

### Yield line triangles
`yield-line.triangles` · verified · Federal Highway Administration MUTCD 3B.19 para 10, 12 · guidance

- triangle base: 12 in (0.305 m) to 24 in (0.61 m)
- triangle height: 1.5 x the base
- space between triangles: 3 in (0.076 m) to 12 in (0.305 m)

> The individual triangles comprising the yield line should have a base of 12 to 24 inches wide and a height equal to 1.5 times the base. The space between the triangles should be 3 to 12 inches.

Plug-in: roundabout yield line: row of triangles (crossing.rs roundabout()): **conforms**. Fixed: drawn as isosceles triangles (0.45 m base, 0.675 m high, 0.15 m apart) pointing at approaching traffic.

### Stop or yield line before a crosswalk across an uncontrolled multi-lane approach
`stop-line.uncontrolled-multilane` · verified · Federal Highway Administration MUTCD 3B.19 para 14-16 · guidance

- distance in advance of the nearest crosswalk line: 20 ft (6.096 m) to 50 ft (15.24 m)

> the yield (stop) line should be placed 20 to 50 feet in advance of the nearest crosswalk line (see Drawing B in Figure 3B-16).

Plug-in: mid-block yield line 6.2 m before the crosswalk edge: **conforms**. Fixed: MIDBLOCK_YIELD_SETBACK = 6.2 m (20.3 ft), inside 20 to 50 ft.

### Transverse crosswalk line width
`crosswalk.transverse-width` · verified · Federal Highway Administration MUTCD 3C.03 para 03 · standard

- width: 6 in (0.152 m) to 24 in (0.61 m)

> When used, transverse lines shall not be less than 6 inches or greater than 24 inches in width.

### Minimum width of a marked crosswalk
`crosswalk.min-width` · verified · Federal Highway Administration MUTCD 3C.03 para 05-06 · standard

- minimum width: 6 ft (1.829 m)
- minimum width at a non-intersection crosswalk where the posted speed is 40 mph or more: 8 ft (2.438 m)

> Except as provided in Paragraph 6 of this Section, the minimum width of a marked crosswalk shall be 6 feet.

Plug-in: crosswalk depth CW = 3.0 m (crossing.rs): **conforms**. 9.8 ft.

### Longitudinal bar crosswalk
`crosswalk.longitudinal-bars` · verified · Federal Highway Administration MUTCD 3C.06 para 02-03; 3C.05 para 04 · standard

- bar width: 12 in (0.305 m) to 24 in (0.61 m)
- lateral spacing between bars: 12 in (0.305 m) to 60 in (1.524 m)
- spacing limit: not more than 2.5 x the bar width
- minimum number of bars: 3

> The lateral spacing between subsequent longitudinal bars shall not be less than 12 inches or greater than 60 inches. The lateral spacing of the longitudinal bars shall not exceed 2.5 times the width of a longitudinal bar.

Plug-in: bars 0.5 m wide every 1.2 m centre to centre (0.7 m gap): **conforms**. Bar 19.7 in; space 27.6 in = 1.4 x the bar width: within all limits.

### Bar pair crosswalk
`crosswalk.bar-pair` · verified · Federal Highway Administration MUTCD 3C.08 para 03-05 · standard

- width of each bar: 8 in (0.203 m) to 12 in (0.305 m)
- spacing between bar pairs: 24 in (0.61 m) to 60 in (1.524 m)

> The width of an individual longitudinal bar that establishes one-half of the bar pair shall not be less than 8 inches or greater than 12 inches.

### High-visibility bars lie parallel to approaching traffic
`crosswalk.bars-parallel` · verified · Federal Highway Administration MUTCD 3C.05 para 07 · guidance

- rule: bars angled parallel to the travel path of approaching traffic

> The individual longitudinal elements of a high-visibility crosswalk should be angled such that they are parallel to the travel path of approaching traffic.

### Crosswalks at roundabouts
`crosswalk.roundabout-setback` · verified · Federal Highway Administration MUTCD 3C.09 para 01-03 · guidance

- minimum distance from the edge of the circulatory roadway at a yield-controlled approach and an uncontrolled exit: 20 ft (6.096 m)
- central island: no crosswalk to or from the central island

> On an approach to a circular intersection controlled by a YIELD sign and at uncontrolled exits, crosswalks should be a minimum of 20 feet from the edge of the circulatory roadway.

Plug-in: roundabout crosswalk nearest edge 6.2 m from the ring: **conforms**. Fixed: ROUNDABOUT_CROSSWALK_SETBACK = 6.2 m (20.3 ft).

### Curb ramps lie within the crosswalk extension
`crosswalk.curb-ramps` · verified · Federal Highway Administration MUTCD 3C.03 para 09 · standard

- rule: where curb ramps are provided, crosswalk markings shall be located so that the ramps are within the extension of the markings

> Where curb ramps are provided, crosswalk markings shall be located so that the curb ramps are within the extension of the crosswalk markings.

### Pedestrian islands and medians
`crosswalk.refuge` · verified · Federal Highway Administration MUTCD 3C.12 para 01 · support

- rule: raised islands or medians of sufficient width can serve as a place of refuge for pedestrians; the minimum width for accessible refuge comes from the ADA standards (see PROWAG R302.2.1)

> can serve as a place of refuge for pedestrians who are attempting to cross at a midblock or intersection location.

### Chevron and diagonal markings in gores
`gore.chevron` · verified · Federal Highway Administration MUTCD 3B.25 para 02D, 05, 10 · guidance

- width at a speed limit of 45 mph or more, at least: 12 in (0.305 m)
- width at a speed limit under 45 mph, at least: 8 in (0.203 m)
- angle to the longitudinal lines: approximately 30 to 45 degrees
- colour and direction: white, the point of each chevron facing toward approaching traffic

> chevrons and diagonal markings should be at least 12 inches wide for roadways having a speed limit of 45 mph or greater, and at least 8 inches wide for roadways having a speed limit of less than 45 mph.

Plug-in: gore chevrons 0.31 m wide at 40 degrees, white, pointing toward approaching traffic (scenarios.rs gore()): **conforms**. 12.2 in wide, in the 30 to 45 degree range. The gore's length (30 m by default) and the chevron spacing (6 m) are judgment, not standard.

## Geometry

### Lane-reduction transition taper length
`taper.lane-reduction` · verified · Federal Highway Administration MUTCD 3B.12 para 08-10 · guidance

- speed limit 45 mph or more: L = W x S (L taper ft, W offset ft, S speed mph)
- speed limit under 45 mph: L = W x S^2 / 60
- minimum taper, urban: 100 ft (30.48 m)
- minimum taper, rural: 200 ft (60.96 m)

> the transition taper length for a lane-reduction transition should be computed by the formula L = WS, where L equals the taper length in feet, W equals the width of the offset distance in feet, and S equals the 85th-percentile speed or the speed limit in mph, whichever is higher.

Plug-in: pocket_taper default: not comparable. 15 m is 49 ft: equals FDOT's 50 ft approach-end taper for one turn lane; the MUTCD formula gives 12 ft x 35 mph^2/60 = 245 ft for a 12 ft shift at 35 mph, which is for lane reductions, not turn-lane approach tapers.

### Approach markings for obstructions: taper
`taper.approach-obstruction` · verified · Federal Highway Administration MUTCD 3B.13 para 02-03 · guidance

- 45 mph or more: L = W x S
- under 45 mph: L = W x S^2 / 60
- minimum taper, urban: 100 ft (30.48 m)
- minimum taper, rural: 200 ft (60.96 m)

> For roadways having a speed limit of 45 mph or greater, the taper length of the tapered line markings should be computed by the formula L = WS

## Bicycle

### Buffer-separated bicycle lane buffer
`buffer.bike` · verified · Federal Highway Administration MUTCD 9E.06 para 08-14 · guidance

- buffer width: at least 3 x the line width of the lines that mark it
- 2 to 3 ft wide: chevron or diagonal markings should be applied: 2 ft (0.61 m) to 3 ft (0.914 m)
- more than 3 ft wide: chevron or diagonal markings shall be applied: 3 ft (0.914 m)
- chevron or diagonal spacing: 10 ft (3.048 m)

> Where a buffer space is more than 3 feet wide, chevron or diagonal markings shall be applied within the buffer space.

Plug-in: bike-buffer element width 0.9 m (3.0 ft): **conforms**. Exactly 3 ft: diagonal markings are 'should' at 2 to 3 ft and 'shall' above 3 ft. The plug-in draws no diagonals yet.

### Bicycle lane width
`bicycle.lane-width` · unverified · Federal Highway Administration MUTCD 9E.01 · gap

- width: the MUTCD sets no width: 9E.01 only requires longitudinal lines plus symbol or word markings

> Longitudinal pavement markings and bicycle lane symbol or word markings (see Figure 9E-1) shall be used to define bicycle lanes.

Plug-in: bike-lane element 1.5 m (4.9 ft): not comparable. Width comes from the AASHTO bike guide (copyrighted) or the FHWA Bikeway Selection Guide: not retrieved.

## Signals

### Longitudinal position of primary signal faces
`signal.position` · verified · Federal Highway Administration MUTCD 4D.08 para 01 · standard

- distance beyond the stop line, minimum: 40 ft (12.192 m)
- distance beyond the stop line, maximum without a supplemental near-side face: 180 ft (54.864 m)

> No less than 40 feet beyond the stop line, and No more than 180 feet beyond the stop line unless a supplemental near-side signal face is provided.

Plug-in: signal heads: a primary head on the far side, at least 12.2 m (40 ft) beyond the stop line, and a supplemental near-side head at the stop line (crossing.rs, hub.rs, junction.rs): **conforms**. Fixed: the primary head now stands beyond the far edge of the intersection, never nearer than SIGNAL_MIN_BEYOND_STOP_LINE (12.2 m); the near-side head the plug-in drew before is kept as the supplemental face (4D.08 para 03).

### Mounting height of signal faces
`signal.height` · verified · Federal Highway Administration MUTCD 4D.09 para 01-02 · standard

- bottom of housing over a roadway, minimum: 15 ft (4.572 m)
- bottom of housing not over a roadway, above the sidewalk or pavement, minimum: 8 ft (2.438 m)
- above a median island on the near side, minimum: 4.5 ft (1.372 m)

> The bottom of the signal housing and any related attachments to a vehicular signal face located over any portion of a highway that can be used by motor vehicles shall be at least 15 feet above the pavement.

### Lateral offset of signal faces mounted at the roadside
`signal.offset` · verified · Federal Highway Administration MUTCD 4D.10 para 01 · guidance

- offset from the face of a vertical curb (or shoulder edge): 2 ft (0.61 m)

> should have a horizontal offset of not less than 2 feet from the face of a vertical curb, or if there is no curb, not less than 2 feet from the edge of a shoulder.

Plug-in: signal head 0.9 m from the lane edge: not comparable. Measured from the lane edge, not the kerb face; with an apron/bike lane/verge beside the lane the head stands past the kerb in most cases. Needs the real kerb position.

## Signs

### STOP sign sizes (Table 2B-1)
`sign.stop-size` · verified · Federal Highway Administration MUTCD Table 2B-1, sheet 1 · table

- conventional road, single lane: 30 in (0.762 m) 30 x 30 in
- conventional road, multi-lane: 36 in (0.914 m) 36 x 36 in
- expressway: 36 in (0.914 m) 36 x 36 in
- oversized: 48 in (1.219 m) 48 x 48 in

> Stop R1-1 2B.04 30 x 30 36 x 36 36 x 36 — 30 x 30* 48 x 48

Plug-in: stop sign octagon circumradius 0.413 m (30 in) or 0.495 m (36 in): **conforms**. Fixed: circumradius 0.413 m for a single-lane approach (30 in across the flats) and 0.495 m for two or more inbound lanes (36 in).

### YIELD sign sizes (Table 2B-1)
`sign.yield-size` · verified · Federal Highway Administration MUTCD Table 2B-1, sheet 1 · table

- conventional road, single lane: 36 in (0.914 m) 36 x 36 x 36 in
- conventional road, multi-lane: 48 in (1.219 m) 48 x 48 x 48 in
- freeway: 60 in (1.524 m) 60 x 60 x 60 in

> Yield R1-2 2B.05 36 x 36 x 36 48 x 48 x 48 48 x 48 x 48 60 x 60 x 60

### Sign mounting height
`sign.mount-height` · verified · Federal Highway Administration MUTCD 2A.15 para 04-05, 07 · standard

- rural: bottom of sign above the near pavement edge, minimum: 5 ft (1.524 m)
- business, commercial or residential area (and over a sidewalk), minimum: 7 ft (2.134 m)

> the minimum height, measured vertically from the bottom of the sign to the elevation of the near edge of the pavement, of signs installed at the side of the road shall be 5 feet

### Lateral offset of post-mounted signs
`sign.lateral-offset` · verified · Federal Highway Administration MUTCD 2A.16 para 03, 09-10 · guidance

- from the edge of the traveled way: 12 ft (3.658 m)
- from the edge of a shoulder wider than 6 ft: 6 ft (1.829 m)
- low-volume rural road where impractical: 2 ft (0.61 m)
- business, commercial or residential area: from the face of the curb: 1 ft (0.305 m)

> For post-mounted signs, the minimum lateral offset should be 12 feet from the edge of the traveled way.

## Median

### Median openings: separate intersections or one
`median.opening-signing` · verified · Federal Highway Administration MUTCD 2A.23 para 01-02 · guidance

- rule: a divided highway crossing should be signed and marked as separate intersections when the paths of opposing left turns cross each other and the interior approaches have adequate storage for the design vehicles; otherwise as one intersection

> A divided highway crossing should be signed and marked as separate intersections when both of the following conditions are present:

### Minimum median width by function
`median.min-width` · verified-secondary · Florida Department of Transportation Table 212.9.1 · state-standard

- separation of opposing traffic: 4 ft (1.219 m)
- pedestrian refuge: 6 ft (1.829 m)
- protection of vehicles crossing through lanes: 22 ft (6.706 m)
- U-turns from a left-turn lane to the outside lanes: 30 ft (9.144 m)
- dual left turn lanes and U-turns: 42 ft (12.802 m)

> Provision for pedestrian refuge 6 | Provision for protection of vehicles crossing through lanes 22 | Provision for U-turns, left turn lane to outside lanes 30

Plug-in: median element 3.0 m; margin 3.0 m: **conforms**. 9.8 ft: fine for opposing-traffic separation and pedestrian refuge; too narrow for a left-turn pocket (3.3 m) with no room: the code already reports that; or for a U-turn (30 ft = 9.1 m).

### Length of a full median opening
`median.opening-length` · verified-secondary · Florida Department of Transportation 212.9 · state-standard

- length: typically the same width as the intersecting road, including shoulders, to accommodate the swept path of left-turning vehicles; at an unsignalized intersection no longer than required

> The overall length of a full median opening is typically the same width as the intersecting road (including shoulders) which is sufficient to accommodate the swept path of left turning vehicles.

Plug-in: opening default: the cross road's width: **conforms**. Fixed: the default is now the cross road's own width (clamped to 8 to 40 m); an explicit length still wins.

### Offset left-turn lanes
`median.offset-left` · verified-secondary · Florida Department of Transportation 212.14.4 · state-standard

- curbed roadway: use offset left-turn lanes with median wider than: 18 ft (5.486 m)
- separator between the turn lane and opposing traffic, when possible: 4 ft (1.219 m)

> On curbed roadway designs, offset left-turn lanes should be used with median widths greater than 18 feet. A 4-foot traffic separator should be used when possible

### Refuge islands used by bicyclists
`island.refuge-bicycle` · verified-secondary · Florida Department of Transportation 212.13.3 · state-standard

- minimum width when bicyclists use the island: 6 ft (1.829 m)

> Refuge islands should be a minimum of 6 feet wide when they will be used for bicyclists.

## Railroad

### Stop line at a grade crossing
`rail.stop-line` · verified · Federal Highway Administration MUTCD 8C.03 para 03-06 · guidance

- passive crossing: no closer than, in advance of the nearest rail: 15 ft (4.572 m)
- active crossing with flashing lights: in advance of the flashing-light signal or gate, whichever is farther from the track: 8 ft (2.438 m)
- active crossing: no closer than, in advance of the nearest rail: 15 ft (4.572 m)

> it should be a transverse line at a right angle to the traveled way and should be placed approximately 8 feet in advance of the flashing-light signals or automatic gate (if present), whichever is farther from the track(s), but no closer than 15 feet in advance of the nearest rail

Plug-in: stop line 4.6 m from the nearest rail (scenarios.rs railroad()): **conforms**. 15.1 ft: just over the 15 ft (4.572 m) minimum. The pole should stand about 2.4 m (8 ft) beyond the stop line toward the track: the code puts it 1.6 m beyond; it is not a conformance gap, since the 8 ft is approximate.

### Automatic gate arm
`rail.gate-arm` · verified · Federal Highway Administration MUTCD 8D.03 para 02, 05, 07, 09, 14-15, 17 · standard

- stripes: vertical, alternately red and white at 16-inch intervals
- stripe interval: 16 in (0.406 m)
- retroreflective sheeting width, first 32 ft from the mast, at least: 4 in (0.102 m)
- height of the lowered arm above the crown of the road: 3.5 ft (1.067 m) to 4.5 ft (1.372 m)
- lights on the arm: at least three red lights, about 4 in diameter if circular
- median islands that discourage driving around gates, length where space allows: 60 ft (18.288 m)

> Gate arms shall be fully retroreflective on both sides and shall have vertical stripes alternately red and white at 16-inch intervals measured horizontally.

Plug-in: gate arm drawn as a plain red 0.2 m stroke across the inbound lane: not comparable. The arm should cover the approaching lanes up to the centre line (8D.03 para 14: so that vehicles cannot drive around it 'without crossing the center line'): the code reaches the far edge of the inbound lanes, which is the centre line when the road has no median. Draw 0.41 m (16 in) red/white stripes.

### Flashing-light signals
`rail.flashing-light` · verified · Federal Highway Administration MUTCD 8D.02 para 06-14 · standard

- lens: 8-inch or 12-inch nominal diameter
- centre-to-centre distance between the two red lights, approximately: 30 in (0.762 m)
- mounting height to the centre of the housing above the crown of the road: 8 ft (2.438 m) to 9 ft (2.743 m)
- placement: to the right-hand side of approaching traffic on all approaches; back-to-back across the tracks for two-way roads

> The mounting height of the flashing-light units, measured from the center of the flashing-light unit housing to the elevation of the crown of the roadway, should be between 8 feet and 9 feet .

### Crossbuck (R15-1) and Number of Tracks plaque (R15-2P) minimum sizes
`rail.crossbuck` · verified · Federal Highway Administration MUTCD Table 8B-1 · table

- Crossbuck R15-1: 48 x 9 in blades
- Number of Tracks plaque R15-2P: 27 x 18 in
- STOP sign at a passive crossing, single lane: 30 in (0.762 m) 30 x 30 in

> Grade Crossing (Crossbuck) R15-1 8B.03 48 x 9 48 x 9

Plug-in: crossbuck: two 1.4 m strokes at +-40 degrees: not comparable. Blade length 48 in = 1.22 m: 1.4 m is a little long. Use 1.22 m.

### Track gauge and tie spacing
`rail.gauge` · unverified · no source yet · typical

- standard gauge: 4 ft 8.5 in (1.435 m), from general knowledge; 49 CFR 213.53 sets track gauge limits
- tie spacing: about 0.5 m; the code uses 0.6 m

Plug-in: rails 1.435 m apart, ties every 0.6 m, ballast bed 4.0 m: not comparable. Verify against 49 CFR 213.53 (eCFR, public) and the FRA/AREMA references; not retrieved.

## Pedestrian

### Pedestrian access route clear width
`ped.clear-width` · verified · US Access Board (36 CFR Part 1190) R302.2, R302.3 · standard

- continuous clear width, minimum: 48 in (1.219 m)
- passing space interval if narrower than 60 in: 200 ft (60.96 m)
- passing space size: 60 in (1.524 m) 60 x 60 in

> the continuous clear width of pedestrian access routes shall be 48 inches (1220 mm) minimum

Plug-in: sidewalk element 1.5 m (59 in): **conforms**. Over the 48 in minimum; the 60 in passing space is met within 1 in.

### Medians and pedestrian refuge islands
`ped.refuge-width` · verified · US Access Board (36 CFR Part 1190) R302.2.1 · standard

- clear width of the route across a median or island, minimum: 60 in (1.524 m)

> The clear width of pedestrian access routes crossing medians and pedestrian refuge islands shall be 60 inches (1525 mm) minimum

Plug-in: median element 3.0 m (118 in); margin 3.0 m: **conforms**. Wide enough to be a refuge.

### Cross slope of pedestrian access routes
`ped.cross-slope` · verified · US Access Board (36 CFR Part 1190) R302.5.1, R302.5.2.1-4 · standard

- not within a crosswalk: 1:48 (2.1%) maximum
- crosswalk with yield or stop control: 1:48 (2.1%) maximum
- crosswalk at an uncontrolled approach: 1:20 (5.0%) maximum
- crosswalk at a signal or pedestrian hybrid beacon: 1:20 (5.0%) maximum
- midblock and roundabout crosswalks: not more than the street grade

> the cross slope of the pedestrian access route shall be 1:20 (5.0%) maximum.

### Curb ramps
`ped.curb-ramp` · verified · US Access Board (36 CFR Part 1190) R304.2.1, R304.2.2, R304.5.1.1, R304.2.4-5 · standard

- running slope: 1:12 (8.3%) maximum; where the ramp must exceed 15 ft (4.6 m) to get 1:12, at least 15 ft with a steeper slope
- cross slope of the ramp run: 1:48 (2.1%) maximum
- clear width of the run, excluding flares, minimum: 48 in (1.219 m)
- landing at the top: 48 in (1.219 m) 48 x 48 in minimum

> The running slope of the curb ramp shall be 1:12 (8.3%) maximum.

### Pedestrian clearance time
`ped.signal-timing` · verified · US Access Board (36 CFR Part 1190) R306.2 · standard

- walking speed for clearance time, at most: 3.5 ft/s (1.1 m/s)
- walk interval, minimum: 7 seconds

> pedestrian signal phase timing shall be based on a pedestrian clearance time that is calculated using a pedestrian walking speed of 3.5 ft/s (1.1 m/s) or less

## Turn lanes

### Approach-end taper of a turn lane
`turn-lane.taper` · verified-secondary · Florida Department of Transportation 212.14.3 · state-standard

- one turn lane: 50 ft (15.24 m)
- two or more turn lanes: 100 ft (30.48 m)
- design speed: applies to all design speeds

> The length of approach end tapers is 50 feet for a single turn lane and 100 feet for two or more turn lanes, as shown Exhibit 212-1.

Plug-in: pocket_taper default 15 m: **conforms**. 49.2 ft: matches the single-turn-lane 50 ft taper.

### Queue (storage) length of a turn lane
`turn-lane.queue` · verified-secondary · Florida Department of Transportation 212.14.2 · state-standard

- minimum for low volume rural, 2 vehicles: 50 ft (15.24 m)
- minimum for urbanized areas, 4 vehicles: 100 ft (30.48 m)
- truck traffic over 10%: accommodate at least one car and one truck

> a minimum 50-foot queue length (2 vehicles) should be provided for rural context classifications. A minimum 100-foot queue length (4 vehicles) should be provided in urbanized context classifications.

Plug-in: pocket_length default 30 m: **conforms**. 98 ft full-width length: meets the 100 ft urban queue length almost exactly; the deceleration length (Exhibit 212-1) comes on top of the queue, so a fast road needs a longer pocket.

### Auxiliary lane length
`turn-lane.auxiliary-length` · verified-secondary · Florida Department of Transportation 212.14 · state-standard

- length: the sum of the deceleration length, the queue length and the approach end taper

> The length of the auxiliary lanes is the sum of the deceleration length, queue length and approach end taper.

### Merging taper formula
`turn-lane.taper-formula` · verified-secondary · Florida Department of Transportation 212.6 · state-standard

- design speed 40 mph or less: L = W x S^2 / 60
- design speed 45 mph or more: L = W x S
- shifting taper: half the merging taper

> (a) For design speeds ≤ 40 mph: L = (W*S2)/60 (b) For design speeds ≥ 45 mph: L = W*S

## Kerb

### Control radii for minimum-speed turns (10 to 15 mph)
`corner.control-radii` · verified-secondary · Florida Department of Transportation Table 212.9.2 · state-standard

- control radius, ft: 50 (40 min) for predominantly passenger cars occasionally SU-30; 60 (50 min) for SU-30 occasionally SU-40 or WB-40; 75 for SU-40/WB-40 occasionally WB-62; 130 for WB-62FL occasionally WB-67

> Control Radius (feet) 50 (40 min) 60 (50 min) 75 130

Plug-in: kerb return default 6 m (19.7 ft); slip radius 15 m: not comparable. The 6 m default is a small urban corner for passenger cars. FDOT's table gives edge-of-traveled-way radii of 50 ft for SU-30 and 80 ft for SU-40 at 90 degrees (Table 212.12.1); the passenger-car row did not extract cleanly and was not used. The kerb radius is the edge of pavement, which is smaller than a control radius.

## Roundabout

### Typical inscribed circle diameter
`roundabout.icd` · verified-secondary · Wisconsin Department of Transportation Table 8.1 (citing NCHRP Report 1043) · state-standard

- mini: less than 80 ft
- compact: 80 to 120 ft
- single-lane: 120 to 160 ft
- multilane, 2-lane entry: 160 to 215 ft
- multilane, 3-lane entry: 215 to 275 ft

> Single-lane 120 -160 ft less than 25,000

Plug-in: roundabout diameter default 40 m (131 ft): **conforms**. In the single-lane range of 120 to 160 ft; also inside SCDOT's 105 to 150 ft.

### Inscribed circle diameter, single-lane and multilane (NCHRP 672 as summarised by SCDOT)
`roundabout.icd-scdot` · verified-secondary · South Carolina Department of Transportation 9.1.1 items 2 and 3 · state-standard

- single-lane typical: 105 to 150 ft
- single-lane for a WB-62 design vehicle, at least: 105 ft (32.004 m)
- single-lane for a WB-67, typically: 130 to 150 ft
- multilane typical: 150 to 300 ft
- two-lane common starting point: 160 to 180 ft
- desirable maximum entry design speed: single-lane 20 to 25 mph; multilane 25 to 30 mph

> The inscribed circle diameter typically needs to be at least 105 feet to accommodate a WB-62 design vehicle.

### Circulatory roadway width
`roundabout.circulatory-width` · verified-secondary · South Carolina Department of Transportation 9.1.1 item 5 · state-standard

- single-lane typical: 14 ft (4.267 m) to 18 ft (5.486 m)
- multilane, each lane: 14 ft (4.267 m) to 16 ft (4.877 m)
- tyre track to the curb line, minimum: 1 ft (0.305 m)
- tyre track to the curb line, preferred: 2 ft (0.61 m)

> Typical circulatory roadway widths range from 14 feet to 18 feet for single-lane roundabouts.

Plug-in: ring width min(5.5 m, 0.3 x radius) = 5.5 m: **conforms**. 18.0 ft: at the top of the single-lane range.

## Where the plug-in differs from the standard

| Entry | What the plug-in does | What to change |
|---|---|---|

## Not retrieved

- **FHWA Roundabouts: An Informational Guide (FHWA-SA-10-006) and the 2013 FHWA Signalized Intersections Informational Guide (FHWA-SA-13-027).** Tried: safety.fhwa.dot.gov and highways.dot.gov return HTTP 403 to automated downloads; nap.nationalacademies.org chapter pages serve page images with no text; onlinepubs.trb.org returned an HTML page instead of the PDF. Not worked around. Download by hand and drop the PDFs in a scratch folder, then add entries citing the section numbers.
- **AASHTO A Policy on Geometric Design of Highways and Streets (Green Book), Guide for the Development of Bicycle Facilities, Roadside Design Guide.** Tried: not fetched: copyrighted and sold by AASHTO. Cite by name and section only if the owner supplies a licensed copy. Values attributed to the Green Book elsewhere (lane width, kerb radius) stay unverified here.
- **NTSB.** Tried: https://www.ntsb.gov/about/Pages/default.aspx. The NTSB investigates accidents, studies safety and recommends changes; its mission page does not describe design standards. Its highway reports cite the MUTCD and AASHTO for the standards. Individual reports can be cited as case evidence, not as a source of dimensions.

## Adding an entry

Read the section in the primary text, copy a short verbatim quote, give the section and paragraph, convert units with `unit_us` (in, ft, mph) and let `node scripts/standards.mjs check` verify the metric figure, then regenerate this file. Mark a value `verified` only if it came from a federal source you read; a state manual makes it `verified-secondary`; anything from memory is `unverified` and must not be described to a user as a standard.
