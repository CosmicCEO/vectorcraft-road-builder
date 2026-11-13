//! Three or more roads that meet at one point: a hub. Each road contributes an arm on each side of the meeting point where it runs on (a road
//! that stops there contributes one). The arms are sorted by bearing; where two neighbouring arms' kerb lines meet is a corner of the paved
//! core, each corner is rounded, and every arm gets the usual crosswalk, stop bar and signal head or stop sign.
//!
//! Stored under `hubs.<id>`: `{"roads": ["a", "b", "c"], "treatment": "signal", "crosswalks": true, "radius": 6, "minor": null, "stub": "keep"}`.
//! Treatments: signal, two-way-stop (the narrowest road or `minor` stops), four-way-stop (every arm stops; "all-way"), uncontrolled.
//! Where two neighbouring arms point opposite ways (the side of a road that has no leg) the core stops at that road's outer travel lane, so
//! its far kerb, bike lane and verge carry on, as in a three-way.

use crate::crossing::{Approach, Arm, Crossing, Ctl, Stub, Treatment, Trim, approach, clean_id, cross2, fillet, find, pieces, unit, PAVEMENT};
use crate::crosssection::layout;
use crate::geom::{P, Poly, add, dot, scale, sub};
use crate::junction::travel;
use crate::output::Out;
use crate::roads::{CURB, Spec, flip, heading};
use crate::vcdoc::Item;
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct HubSpec {
    pub roads: Vec<String>,
    pub treatment: Treatment,
    pub crosswalks: bool,
    pub radius: Option<f64>,
    pub minor: Option<String>,
    pub stub: Stub,
    pub stub_length: f64,
}

impl HubSpec {
    pub fn from_json(v: &Value) -> Result<HubSpec, String> {
        let mut roads: Vec<String> = Vec::new();
        for id in v["roads"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(clean_id).filter(|s| !s.is_empty()).collect::<Vec<_>>()).unwrap_or_default() {
            if !roads.contains(&id) {
                roads.push(id);
            }
        }
        if roads.len() < 3 || roads.len() > 8 {
            return Err(format!("a hub needs 3 to 8 different roads (got {})", roads.len()));
        }
        let treatment = Treatment::parse(v["treatment"].as_str().unwrap_or("uncontrolled"))?;
        if !matches!(treatment, Treatment::Uncontrolled | Treatment::Signal | Treatment::TwoWayStop | Treatment::FourWayStop) {
            return Err(format!("a hub can be signal, two-way-stop, four-way-stop (all-way) or uncontrolled, not {}", treatment.name()));
        }
        let minor = v["minor"].as_str().map(clean_id).filter(|m| !m.is_empty());
        if let Some(m) = &minor {
            if !roads.contains(m) {
                return Err(format!("\"minor\" must be one of the roads ({}), not \"{m}\"", roads.join(", ")));
            }
        }
        let stub = match v["stub"].as_str().map(|w| w.trim().to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("keep") => Stub::Keep,
            Some("trim") | Some("cut") => Stub::Trim,
            Some(other) => return Err(format!("stub must be keep or trim (got \"{other}\")")),
        };
        Ok(HubSpec {
            roads,
            treatment,
            crosswalks: v["crosswalks"].as_bool().unwrap_or(false),
            radius: v["radius"].as_f64().map(|r| r.clamp(0.0, 20.0)),
            minor,
            stub,
            stub_length: v["stub_length"].as_f64().unwrap_or(25.0).clamp(5.0, 100.0),
        })
    }

    pub fn radius(&self) -> f64 {
        self.radius.unwrap_or(6.0)
    }

    pub fn to_json(&self) -> Value {
        json!({"roads": self.roads, "treatment": self.treatment.name(), "crosswalks": self.crosswalks, "radius": self.radius, "minor": self.minor,
               "stub": if self.stub == Stub::Trim { "trim" } else { "keep" }, "stub_length": self.stub_length})
    }

    pub fn summary(&self) -> String {
        let what = match self.treatment {
            Treatment::Signal => "traffic signals".to_string(),
            Treatment::TwoWayStop => format!("two-way stop, {}", self.minor.as_ref().map_or("the narrowest road stops".to_string(), |m| format!("road {m} stops"))),
            Treatment::FourWayStop => "all-way stop".to_string(),
            _ => "no control".to_string(),
        };
        format!("{} roads meet ({}): {what}{}{}", self.roads.len(), self.roads.join(", "), if self.crosswalks { ", crosswalks" } else { "" }, if self.stub == Stub::Trim { ", short stubs trimmed" } else { "" })
    }
}

/// One road's arm on one side of the meeting point.
struct HubArm {
    arm: Arm,
    sign: f64,
    /// Metres of road from the meeting point to the end of the line on this side.
    remaining: f64,
    path: Vec<P>,
}

struct Layout {
    centre: P,
    arms: Vec<HubArm>,
    /// Corners of the paved core, in order round it, with the kerb return to round at each (corner, outward direction of each of the two arms).
    verts: Vec<P>,
    returns: Vec<(P, P, P)>,
    /// How far the core reaches along each arm.
    ext: Vec<f64>,
    /// Sides of roads with no arm (the line stops short, the stub is trimmed): `(item, sign, direction, road)`.
    absent: Vec<(usize, f64, P)>,
}

/// Where the ray from `c` along `d` leaves a convex polygon.
fn ray_exit(verts: &[P], c: P, d: P) -> f64 {
    let mut best: f64 = 0.0;
    for k in 0..verts.len() {
        let (a, b) = (verts[k], verts[(k + 1) % verts.len()]);
        let e = sub(b, a);
        let den = cross2(d, e);
        if den.abs() < 1e-12 {
            continue;
        }
        let q = sub(a, c);
        let (s, u) = (cross2(q, e) / den, cross2(q, d) / den);
        if s > 0.0 && (-1e-9..=1.0 + 1e-9).contains(&u) {
            best = best.max(s);
        }
    }
    best
}

fn layout_hub(items: &[Item], spec: &Spec, hs: &HubSpec) -> Result<Layout, String> {
    // The meeting point: the mean of where the roads meet in pairs.
    let mut points: Vec<P> = Vec::new();
    for i in 0..hs.roads.len() {
        for j in i + 1..hs.roads.len() {
            if let Some(Crossing { p, .. }) = find(items, &hs.roads[i], &hs.roads[j]) {
                points.push(p);
            }
        }
    }
    if points.is_empty() {
        return Err("the roads do not meet anywhere".into());
    }
    let centre = scale(points.iter().fold([0.0, 0.0], |s, &p| add(s, p)), 1.0 / points.len() as f64);

    let mut arms: Vec<HubArm> = Vec::new();
    let mut absent: Vec<(usize, f64, P)> = Vec::new();
    for road in &hs.roads {
        let Some((index, path, s0, d, len)) = pieces(items, road)
            .into_iter()
            .map(|(i, p)| {
                let poly = Poly::new(p.clone());
                let (s0, _, d) = poly.project(centre);
                (i, p, s0, d, poly.len())
            })
            .min_by(|a, b| a.3.total_cmp(&b.3))
        else {
            return Err(format!("road {road} is not in the document"));
        };
        if d > 12.0 {
            return Err(format!("road {road} does not pass through the meeting point (it is {d:.0} m away)"));
        }
        let poly = Poly::new(path.clone());
        let t = poly.at(s0).1;
        let (parsed, anchor, _) = spec.for_segment(road);
        let placed = layout(&parsed, &spec.library, heading(&path), anchor)?;
        let (lo, hi) = (placed.first().map_or(-3.0, |e| e.left), placed.last().map_or(3.0, |e| e.right));
        let sides = [(-1.0, s0), (1.0, len - s0)];
        let long_side = sides.iter().any(|(_, rem)| *rem >= hs.stub_length);
        for (sign, remaining) in sides {
            // A side exists if the road runs on at least 3 m past the point; a short side is a stub, dropped when stubs are trimmed and the road runs on the other way.
            let stub = remaining < hs.stub_length;
            if remaining < 3.0 || (stub && hs.stub == Stub::Trim && long_side) {
                absent.push((index, sign, scale(t, sign)));
                continue;
            }
            arms.push(HubArm { arm: Arm { id: road.clone(), t, n: Poly::right(t), lo, hi, placed: placed.clone() }, sign, remaining, path: path.clone() });
        }
    }
    if arms.len() < 3 {
        return Err(format!("only {} arm{} left at the meeting point: use two crossing roads instead", arms.len(), if arms.len() == 1 { "" } else { "s" }));
    }
    // Sort the arms round the point by the bearing they leave in.
    let bearing = |h: &HubArm| scale(h.arm.t, h.sign);
    arms.sort_by(|a, b| {
        let (da, db) = (bearing(a), bearing(b));
        da[1].atan2(da[0]).total_cmp(&db[1].atan2(db[0]))
    });

    let k = arms.len();
    // The kerb line on the left (counter-clockwise side) and on the right of an arm, as a point at the meeting point's frame and its direction.
    let edge = |h: &HubArm, left: bool, travel_only: bool| -> P {
        let (lo, hi) = if travel_only {
            let lanes: Vec<_> = h.arm.placed.iter().filter(|e| travel(e)).collect();
            if lanes.is_empty() { (h.arm.lo, h.arm.hi) } else { (lanes.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), lanes.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max)) }
        } else {
            (h.arm.lo, h.arm.hi)
        };
        // Facing outward along the line (sign +) the left is the road's -n side; facing back (sign -) it is the +n side.
        let u = if (h.sign > 0.0) == left { lo } else { hi };
        add(centre, scale(h.arm.n, u))
    };
    let mut verts: Vec<P> = Vec::new();
    let mut returns: Vec<(P, P, P)> = Vec::new();
    let mut pair_pts: Vec<Vec<P>> = Vec::new();
    for i in 0..k {
        let (a, b) = (&arms[i], &arms[(i + 1) % k]);
        let (da, db) = (bearing(a), bearing(b));
        let den = cross2(da, db);
        let straight = den.abs() < 0.05 && dot(da, db) < 0.0;
        if straight {
            // The side of a road with no leg: the core stops at the outer travel lane of each arm.
            let (pa, pb) = (edge(a, true, true), edge(b, false, true));
            let pts: Vec<P> = if (pa[0] - pb[0]).hypot(pa[1] - pb[1]) < 0.01 { vec![pa] } else { vec![pa, pb] };
            verts.extend(pts.iter().copied());
            pair_pts.push(pts);
            continue;
        }
        let (pa, pb) = (edge(a, true, false), edge(b, false, false));
        let corner = if den.abs() < 0.1 {
            // Two arms almost the same way: no sharp corner; join their edges a little way out.
            scale(add(add(pa, scale(da, 10.0)), add(pb, scale(db, 10.0))), 0.5)
        } else {
            // pa + s*da = pb + u*db
            let s = cross2(sub(pb, pa), db) / den;
            add(pa, scale(da, s.clamp(-1.0, 60.0)))
        };
        verts.push(corner);
        pair_pts.push(vec![corner]);
        if den.abs() >= 0.1 {
            returns.push((corner, da, db));
        }
    }
    let ext = (0..k)
        .map(|i| {
            let d = bearing(&arms[i]);
            let (prev, next) = (&pair_pts[(i + k - 1) % k], &pair_pts[i]);
            prev.iter().chain(next.iter()).map(|&p| dot(d, sub(p, centre))).fold(0.0_f64, f64::max)
        })
        .collect();
    Ok(Layout { centre, arms, verts, returns, ext, absent })
}

/// Builds every stored hub. Returns notes.
pub fn build(items: &[Item], spec: &Spec, out: &mut Out) -> Vec<String> {
    let mut notes = Vec::new();
    for (id, v) in spec.hubs() {
        match HubSpec::from_json(&v).and_then(|hs| emit(items, spec, &id, &hs, out)) {
            Ok(ns) => notes.extend(ns.into_iter().map(|n| format!("hub {id}: {n}"))),
            Err(e) => notes.push(format!("hub {id}: {e}")),
        }
    }
    notes
}

fn emit(items: &[Item], spec: &Spec, id: &str, hs: &HubSpec, out: &mut Out) -> Result<Vec<String>, String> {
    let lay = layout_hub(items, spec, hs)?;
    let base = format!("gen.hub.{id}");
    let centre_of_core = scale(lay.verts.iter().fold([0.0, 0.0], |s, &p| add(s, p)), 1.0 / lay.verts.len() as f64);
    // The core a quarter metre larger on every side so it covers the roads' edge lines where they open onto each other.
    let inflated: Vec<P> = lay.verts.iter().map(|&p| add(p, scale(unit(sub(p, centre_of_core)), 0.25))).collect();
    out.polygon(&format!("{base}.core"), &inflated, flip, PAVEMENT);
    for (k, &(c, d1, d2)) in lay.returns.iter().enumerate() {
        if let Some((arc, fill)) = fillet(c, d1, d2, hs.radius()) {
            out.polygon(&format!("{base}.corner.{}", k + 1), &fill, flip, PAVEMENT);
            out.path(&format!("{base}.kerb.{}", k + 1), &[arc], flip, CURB, 0.3, false);
        }
    }
    let width = |h: &HubArm| h.arm.hi - h.arm.lo;
    let narrowest = lay.arms.iter().map(|h| width(h)).fold(f64::INFINITY, f64::min);
    let mut notes = Vec::new();
    for (i, h) in lay.arms.iter().enumerate() {
        let ctl = match hs.treatment {
            Treatment::Signal => Ctl::Signal,
            Treatment::FourWayStop => Ctl::Stop,
            Treatment::TwoWayStop => {
                let stops = match &hs.minor {
                    Some(m) => h.arm.id == *m,
                    None => (width(h) - narrowest).abs() < 0.01,
                };
                if stops { Ctl::Stop } else { Ctl::None }
            }
            _ => Ctl::None,
        };
        let ap = Approach { base: &base, arm: &h.arm, sign: h.sign, centre: lay.centre, ext: lay.ext[i], ctl, crosswalks: hs.crosswalks, pocket: crate::crossing::Pocket::None, pocket_length: 30.0, pocket_taper: 15.0 };
        notes.extend(approach(out, &ap));
        // A short arm that is kept ends in a kerb across its end.
        if h.remaining < hs.stub_length && hs.stub == Stub::Keep {
            let poly = Poly::new(h.path.clone());
            let (p, t) = poly.at(if h.sign > 0.0 { poly.len() } else { 0.0 });
            let n = Poly::right(t);
            out.path(&format!("{base}.deadend.{}.{}", h.arm.id, if h.sign < 0.0 { "before" } else { "after" }), &[vec![add(p, scale(n, h.arm.lo)), add(p, scale(n, h.arm.hi))]], flip, CURB, 0.3, false);
        }
    }
    Ok(notes)
}

/// The cuts the stored hubs ask for: the drawn road is cut (or carried on, for a line that stops short) to the edge of the core on every side
/// that has no arm.
pub fn trims(items: &[Item], spec: &Spec) -> Vec<Trim> {
    let mut out = Vec::new();
    for (_, v) in spec.hubs() {
        let Ok(hs) = HubSpec::from_json(&v) else { continue };
        let Ok(lay) = layout_hub(items, spec, &hs) else { continue };
        for &(index, sign, dir) in &lay.absent {
            out.push(Trim { index, p: lay.centre, sign, dist: ray_exit(&lay.verts, lay.centre, dir), gap_before: None });
        }
    }
    out
}
