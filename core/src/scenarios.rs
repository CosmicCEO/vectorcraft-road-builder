//! The crossing treatments that are not a plain paved box: median opening, ramps (speed-change lane), mid-block crossing, railroad, cloverleaf.
//! Each draws into the same `gen.crossing.<id>.*` names as the other treatments. Shapes are typical, not surveyed.

use crate::crossing::{Arm, CW, Crossing, CrossingSpec, GRASS, PAVEMENT, Treatment, cap_medians, yield_triangles, corner_at, cross2, main_index, overpass, simplify_dashes, unit};
use crate::geom::{P, add, dot, scale, sub};
use crate::junction::{polygon, travel};
use crate::crosssection::{Dir, Placed};
use crate::library::Role;
use crate::output::Out;
use crate::standards::*;
use crate::roads::{CURB, WHITE, flip};

const DARK: [f64; 3] = [0.2, 0.2, 0.22];
const RED: [f64; 3] = [0.85, 0.1, 0.1];
const YELLOW: [f64; 3] = [1.0, 0.8, 0.0];

pub(crate) fn emit(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) -> Result<Vec<String>, String> {
    match cs.treatment {
        Treatment::MedianOpening => median_opening(out, base, cs, x, arms),
        Treatment::MidblockCrossing => midblock(out, base, cs, x, arms),
        Treatment::Railroad => railroad(out, base, cs, x, arms),
        Treatment::Cloverleaf => cloverleaf(out, base, cs, x, arms),
        t => Err(format!("{} is not built yet", t.name())),
    }
}

/// A point `d` metres along `dir` from `origin` and `u` metres sideways along `side`.
fn frame(origin: P, dir: P, side: P) -> impl Fn(f64, f64) -> P {
    move |d, u| add(add(origin, scale(dir, d)), scale(side, u))
}

/// Interior curbed elements (median, margin, verge in the middle of the road): their sideways ranges.
fn medians(placed: &[Placed]) -> Vec<(f64, f64)> {
    placed.iter().enumerate().filter(|(i, e)| *i > 0 && *i + 1 < placed.len() && e.role == Role::Curbed).map(|(_, e)| (e.left, e.right)).collect()
}

/// The sideways span of the travel lanes that run `dir` (fwd or back), if there are any.
fn lane_span(placed: &[Placed], dir: Dir) -> Option<(f64, f64)> {
    let lanes: Vec<&Placed> = placed.iter().filter(|e| travel(e) && e.dir == Some(dir)).collect();
    (!lanes.is_empty()).then(|| (lanes.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), lanes.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max)))
}

/// A gap in the median of a divided road, `opening` metres long centred where the other line crosses, with a kerbed nose at each end.
/// The other line is only the marker: it is cut back to the edge of the divided road and nothing is drawn across the carriageways.
fn median_opening(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) -> Result<Vec<String>, String> {
    let m = &arms[main_index(cs, arms)];
    if medians(&m.placed).is_empty() {
        return Err(format!("road {} has no median to open: give it a median or margin (for example \"lane back | median 4 | lane fwd\")", m.id));
    }
    // A full median opening is typically as long as the road crossing it is wide.
    let cross = &arms[1 - main_index(cs, arms)];
    let opening = cs.opening.unwrap_or(cross.hi - cross.lo).clamp(8.0, 40.0);
    for sign in [-1.0, 1.0] {
        let at = frame(x.p, scale(m.t, sign), m.n);
        cap_medians(out, &format!("{base}.opening.{}", if sign < 0.0 { "before" } else { "after" }), &m.placed, 0.0, opening / 2.0, &at);
    }
    Ok(vec![])
}

/// A pedestrian crossing in the middle of the road: crosswalk bars over the carriageway (not over a median, which is a refuge), a stop line
/// 4.5 m before it for each direction, and flashing beacons at both kerbs if asked.
fn midblock(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) -> Result<Vec<String>, String> {
    let m = &arms[main_index(cs, arms)];
    let at = frame(x.p, m.t, m.n);
    let paved: Vec<&Placed> = m.placed.iter().filter(|e| !matches!(e.role, Role::Curbed | Role::Walk)).collect();
    if paved.is_empty() {
        return Err(format!("road {} has nothing to cross", m.id));
    }
    let mut stripes: Vec<Vec<P>> = Vec::new();
    for e in &paved {
        let mut u = e.left + 0.6;
        while u + 0.6 <= e.right + 1e-9 {
            stripes.push(vec![at(-CW / 2.0, u), at(CW / 2.0, u)]);
            u += 1.2;
        }
    }
    out.path(&format!("{base}.crosswalk"), &stripes, flip, WHITE, 0.5, false);
    for (sign, dir, tag) in [(-1.0, Dir::Fwd, "before"), (1.0, Dir::Back, "after")] {
        if let Some((lo, hi)) = lane_span(&m.placed, dir) {
            // An uncontrolled crossing gets a yield line (triangles pointing at the approaching traffic), 20 to 50 ft before the crosswalk.
            let d = sign * (CW / 2.0 + MIDBLOCK_YIELD_SETBACK);
            out.fills(&format!("{base}.yield.{tag}"), &yield_triangles(&at, lo, hi, d, sign), flip, WHITE);
        }
    }
    if cs.beacon {
        let (lo, hi) = (paved.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), paved.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max));
        for (k, u) in [lo - 0.9, hi + 0.9].into_iter().enumerate() {
            out.polygon(&format!("{base}.beacon.{}", k + 1), &polygon(4, at(0.0, u), 0.5), flip, YELLOW);
        }
    }
    Ok(vec![])
}

/// A railway across the road: a rubber crossing panel over the overlap, ties and two rails along the track, and for each direction of the road
/// a stop line 4.6 m (15 ft) from the nearest rail, a crossbuck, flashing lamps and, if asked, a gate arm across the inbound lanes.
fn railroad(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) -> Result<Vec<String>, String> {
    let mi = main_index(cs, arms);
    let (m, r) = (&arms[mi], &arms[1 - mi]);
    let sin = cross2(m.t, r.t).abs();
    if sin < 0.17 {
        return Err("the railway runs nearly along the road, so it does not cross it".into());
    }
    let corners = [(arms[0].lo, arms[1].lo), (arms[0].hi, arms[1].lo), (arms[0].hi, arms[1].hi), (arms[0].lo, arms[1].hi)].map(|(ua, ub)| corner_at(x, arms, ua, ub));
    let along_track = |q: P| dot(r.t, sub(q, x.p));
    let reach = corners.iter().map(|&q| along_track(q).abs()).fold(0.0, f64::max);
    out.polygon(&format!("{base}.panel"), &corners, flip, [0.72, 0.72, 0.72]);
    let on_track = frame(x.p, r.t, r.n);
    // Ties across the crossing, every 0.6 m, 2.6 m long; rails 1.435 m apart run 6 m past the panel at each end.
    let ties: Vec<Vec<P>> = (0..).map(|i| -reach + 0.3 + 0.6 * i as f64).take_while(|d| *d <= reach).map(|d| vec![on_track(d, -1.3), on_track(d, 1.3)]).collect();
    out.path(&format!("{base}.ties"), &ties, flip, [0.45, 0.35, 0.25], 0.25, false);
    let rail_len = reach + 6.0;
    out.path(&format!("{base}.rails"), &[vec![on_track(-rail_len, -0.72), on_track(rail_len, -0.72)], vec![on_track(-rail_len, 0.72), on_track(rail_len, 0.72)]], flip, DARK, 0.15, false);
    // The nearest rail edge, measured along the road from the track centre, is 2 m (half the ballast) over sin of the crossing angle.
    let clear = 2.0 / sin;
    let at = frame(x.p, m.t, m.n);
    for (sign, dir, tag) in [(-1.0, Dir::Fwd, "before"), (1.0, Dir::Back, "after")] {
        let Some((lo, hi)) = lane_span(&m.placed, dir) else { continue };
        let stop = sign * (clear + 4.6_f64.max(RAIL_STOP_LINE_MIN));
        out.path(&format!("{base}.stop.{tag}"), &[vec![at(stop, lo), at(stop, hi)]], flip, WHITE, 0.6, false);
        // The pole stands at the driver's right-hand kerb, between the stop line and the track.
        let heading_in = scale(m.t, -sign);
        let right_is_plus = dot([heading_in[1], -heading_in[0]], m.n) > 0.0;
        let pole_u = if right_is_plus { hi + 1.5 } else { lo - 1.5 };
        let pole = at(sign * (clear + 3.0), pole_u);
        let cross: Vec<Vec<P>> = [0.7_f64, -0.7].iter().map(|a| { let (c, s) = (a.cos(), a.sin()); vec![sub(pole, [0.7 * s, 0.7 * c]), add(pole, [0.7 * s, 0.7 * c])] }).collect();
        out.path(&format!("{base}.crossbuck.{tag}"), &cross, flip, WHITE, 0.25, false);
        out.polygon(&format!("{base}.lamp.{tag}"), &polygon(8, pole, 0.3), flip, RED);
        if cs.gates {
            let far = if right_is_plus { lo } else { hi };
            out.path(&format!("{base}.gate.{tag}"), &[vec![at(sign * (clear + 3.0), pole_u), at(sign * (clear + 3.0), far)]], flip, RED, 0.2, false);
        }
    }
    Ok(vec![])
}

/// A speed-change lane beside the through road at a ramp: on-ramp, an acceleration lane downstream of the join; off-ramp, a deceleration lane
/// upstream of the exit. Right-hand traffic: lanes on the right of the line run with it, so downstream is along the line on that side.
pub(crate) fn ramp_lane(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm], trimmed: &[(usize, f64)]) -> Vec<String> {
    let mi = main_index(cs, arms);
    let (m, r) = (&arms[mi], &arms[1 - mi]);
    let ramp_k = 1 - mi;
    // The ramp lies on the side of the road where its long part is.
    let long = match trimmed.iter().find(|(k, _)| *k == ramp_k) {
        Some((_, sign)) => -sign,
        None => {
            let path = if ramp_k == 0 { &x.path_a } else { &x.path_b };
            let poly = crate::geom::Poly::new(path.clone());
            let (s0, _, _) = poly.project(x.p);
            if poly.len() - s0 > s0 { 1.0 } else { -1.0 }
        }
    };
    let side = if dot(scale(r.t, long), m.n) > 0.0 { 1.0 } else { -1.0 };
    let travels: Vec<&Placed> = m.placed.iter().filter(|e| travel(e)).collect();
    if travels.is_empty() {
        return vec![format!("road {} has no travel lanes for a ramp to join", m.id)];
    }
    let edge = if side > 0.0 { travels.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max) } else { travels.iter().map(|e| e.left).fold(f64::INFINITY, f64::min) };
    let downstream = scale(m.t, side);
    let (dir, name) = if cs.treatment == Treatment::OnRamp { (downstream, "accel") } else { (scale(downstream, -1.0), "decel") };
    let at = frame(x.p, dir, m.n);
    let (d1, d2) = ((cs.accel_length - cs.accel_taper).max(1.0), cs.accel_length);
    let w = 3.6 * side;
    out.polygon(&format!("{base}.{name}"), &[at(0.0, edge - 0.15 * side), at(0.0, edge + w), at(d1, edge + w), at(d2, edge - 0.15 * side)], flip, PAVEMENT);
    out.path(&format!("{base}.{name}.lane-line"), &simplify_dashes(&at, edge, 0.0, d1), flip, WHITE, 0.15, false);
    out.path(&format!("{base}.{name}.edge"), &[vec![at(0.0, edge + w), at(d1, edge + w), at(d2, edge)]], flip, WHITE, 0.15, false);
    if cs.gore {
        return gore(out, base, cs, x, m, r, side, edge, long);
    }
    vec![]
}

/// The gore where the ramp and the through road part (an off-ramp: it widens downstream of the nose) or come together (an on-ramp: it widens upstream
/// of the nose): the paved neutral area between the road's travel edge and the ramp's edge, with white chevrons pointing toward approaching traffic.
/// The nose is where the ramp's edge nearer the road meets the road's travel edge. Skipped when the ramp meets the road at a steep angle.
#[allow(clippy::too_many_arguments)]
fn gore(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, m: &Arm, r: &Arm, side: f64, edge: f64, long: f64) -> Vec<String> {
    let downstream = scale(m.t, side);
    // Where each edge of the ramp meets the travel edge of the road, as a distance along the road in the downstream direction.
    let meets = |u: f64| -> Option<f64> {
        let rhs = sub(scale(m.n, edge), scale(r.n, u));
        let det = cross2(r.t, scale(m.t, -1.0));
        (det.abs() > 1e-9).then(|| cross2(r.t, rhs) / det)
    };
    let (Some(sa), Some(sb)) = (meets(r.lo), meets(r.hi)) else { return vec![] };
    // Positions along `downstream`: the road's own coordinate is `m.t * s`, so flip the sign when the road is read the other way.
    let (da, db) = (sa * side, sb * side);
    let diverge = cs.treatment == Treatment::OffRamp;
    let near_edge_u = if (diverge && db >= da) || (!diverge && db < da) { r.hi } else { r.lo };
    let Some(s_nose) = meets(near_edge_u) else { return vec![] };
    let nose = add(x.p, add(scale(m.n, edge), scale(m.t, s_nose)));
    let dir_g = if diverge { downstream } else { scale(downstream, -1.0) };
    let rd = scale(r.t, long);
    let along = crate::geom::dot(rd, dir_g);
    if along < 0.25 {
        return vec![];
    }
    let perp = unit(sub(rd, scale(dir_g, along)));
    let tan_theta = crate::geom::dot(rd, perp) / along;
    let g = cs.gore_length;
    let at = |u: f64, v: f64| add(add(nose, scale(dir_g, u)), scale(perp, v));
    out.polygon(&format!("{base}.gore"), &[at(0.0, 0.0), at(g, 0.0), at(g, g * tan_theta)], flip, PAVEMENT);
    // Chevrons point toward the approaching traffic: at the nose for a diverge (traffic comes from upstream), away from it for a merge.
    let p = if diverge { -1.0 } else { 1.0 };
    let tan_phi = GORE_CHEVRON_ANGLE_DEG.to_radians().tan();
    let mut chevrons: Vec<Vec<P>> = Vec::new();
    let mut u = 3.0 / tan_theta.max(0.05);
    while u < g - 1.0 {
        let vmax = u * tan_theta;
        if vmax >= 1.2 {
            let (vc, l) = (vmax / 2.0, if p < 0.0 { 0.9 * vmax / 2.0 / tan_phi } else { 0.9 * (vmax / 2.0) / (tan_phi + tan_theta) });
            chevrons.push(vec![at(u - p * l, vc - l * tan_phi), at(u, vc), at(u - p * l, vc + l * tan_phi)]);
        }
        u += 6.0;
    }
    if !chevrons.is_empty() {
        out.path(&format!("{base}.gore.chevrons"), &chevrons, flip, WHITE, GORE_CHEVRON_WIDTH, false);
    }
    vec![]
}

/// A partial cloverleaf: the overpass (the road named `over`, else the first) and four one-lane loop ramps, one in each quadrant, each a
/// 270-degree ring tangent to the outer edge of both roads, with a straight speed-change strip along each road. The diagonal connectors that
/// complete a full interchange are not drawn. Needs roads that cross at close to a right angle.
fn cloverleaf(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) -> Result<Vec<String>, String> {
    let (a, b) = (&arms[0], &arms[1]);
    if cross2(a.t, b.t).abs() < 0.94 {
        return Err("a cloverleaf needs two roads that cross at close to a right angle (within about 20 degrees)".into());
    }
    overpass(out, base, cs, x, arms);
    let (ha, hb) = (a.lo.abs().max(a.hi.abs()), b.lo.abs().max(b.hi.abs()));
    let (radius, width, strip, taper) = (40.0, 5.0, 70.0, 30.0);
    let (f1, f2) = (a.t, a.n);
    for (qi, (s1, s2)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
        let tag = qi + 1;
        let c = add(add(x.p, scale(f1, s1 * (hb + radius))), scale(f2, s2 * (ha + radius)));
        // Tangent points: toward road a (opposite f2) and toward road b (opposite f1). The loop goes the long way round, away from the crossing.
        let ang = |v: P| v[1].atan2(v[0]);
        let (th_a, th_b) = (ang(scale(f2, -s2)), ang(scale(f1, -s1)));
        let mut raw = th_b - th_a;
        while raw > std::f64::consts::PI {
            raw -= std::f64::consts::TAU;
        }
        while raw < -std::f64::consts::PI {
            raw += std::f64::consts::TAU;
        }
        let sweep = raw - raw.signum() * std::f64::consts::TAU;
        let ring = |r: f64| (0..=24).map(|i| { let t = th_a + sweep * i as f64 / 24.0; [c[0] + r * t.cos(), c[1] + r * t.sin()] }).collect::<Vec<P>>();
        let (outer, inner) = (ring(radius + width / 2.0), ring(radius - width / 2.0));
        let mut poly = outer.clone();
        poly.extend(inner.iter().rev());
        out.polygon(&format!("{base}.loop.{tag}"), &poly, flip, PAVEMENT);
        out.path(&format!("{base}.loop.{tag}.edges"), &[outer, inner.clone()], flip, WHITE, 0.15, false);
        // The infield.
        let r_in = radius - width / 2.0 - 0.4;
        let infield: Vec<P> = (0..48).map(|i| { let t = std::f64::consts::TAU * i as f64 / 48.0; [c[0] + r_in * t.cos(), c[1] + r_in * t.sin()] }).collect();
        out.polygon(&format!("{base}.loop.{tag}.infield"), &infield, flip, GRASS);
        // A straight strip along each road from where the loop touches its outer edge, outward, tapering at the far end.
        for (k, (arm, edge_sign, along_dir, start)) in [(a, s2, scale(f1, s1), hb + radius), (b, s1, scale(f2, s2), ha + radius)].into_iter().enumerate() {
            let side = if k == 0 { f2 } else { f1 };
            let edge = edge_sign * if k == 0 { ha } else { hb };
            let at = frame(x.p, along_dir, side);
            let _ = arm;
            let (d0, d1, d2) = (start, start + strip - taper, start + strip);
            let h = width / 2.0;
            out.polygon(&format!("{base}.loop.{tag}.strip.{}", k + 1), &[at(d0, edge - h), at(d0, edge + h), at(d1, edge + h), at(d2, edge)], flip, PAVEMENT);
            out.path(&format!("{base}.loop.{tag}.strip.{}.edge", k + 1), &[vec![at(d0, edge + h), at(d1, edge + h), at(d2, edge)]], flip, WHITE, 0.15, false);
        }
        let _ = (CURB, unit([1.0, 0.0]));
    }
    Ok(vec!["cloverleaf: only the four loop ramps are drawn, not the diagonal connectors".into()])
}
