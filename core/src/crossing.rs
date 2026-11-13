//! Crossing roads: two described roads (`road.<a>` and `road.<b>`) that cross each other. What happens where they cross is a *treatment*:
//! a lighted intersection (`signal`), a two-way stop (`two-way-stop`, the minor road stops), a four-way stop (`four-way-stop`), no control
//! (`uncontrolled`), an overpass or underpass (`overpass`: one road bridges the other), a `roundabout`, or (not built yet) a `cloverleaf`.
//! At-grade treatments pave the overlap, round the four kerb corners, and add crosswalks, stop bars and signals or stop signs on all four
//! approaches using each road's own lanes to decide which side traffic enters from.
//!
//! Stored in the document's plug-in data under `crossings.<id>`:
//! `{"roads": ["a", "b"], "treatment": "signal", "crosswalks": true, "radius": 6, "minor": "b", "over": "a", "diameter": 40}`
//! `"stub": "keep"|"trim"`, `"stub_length": 25`: a road that runs only a short way past the crossing (under `stub_length` metres from the crossing
//! centre) is a stub. `keep` (default) draws it as a dead end, making a four-way; `trim` stops the built road at the other road's travel lane, making
//! a three-way. The drawn line is never changed: only what is built from it.
//! (`minor` for a two-way stop, `over` for an overpass, `diameter` for a roundabout). The older `control` field (none, signal, stop) still reads.
//! Shortcut: the first crossing of the two roads is used; finding it tests every pair of segments, fine for hand-drawn roads.

use crate::crosssection::{Dir, Placed, layout};
use crate::geom::{P, Poly, add, dot, scale, sub};
use crate::junction::{polygon, travel};
use crate::library::Role;
use crate::offset::{MITRE_LIMIT, offset_polyline};
use crate::output::Out;
use crate::roads::{CURB, Spec, WHITE, emit as emit_road, flip, heading, segment_id};
use crate::vcdoc::Item;
use serde_json::{Value, json};

pub(crate) const PAVEMENT: [f64; 3] = [0.86, 0.86, 0.86];
pub(crate) const GRASS: [f64; 3] = [0.72, 0.85, 0.62];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Treatment {
    Uncontrolled,
    Signal,
    TwoWayStop,
    FourWayStop,
    /// A side entrance: a narrow road meeting the road with a small kerb return, no crosswalk, no control.
    Driveway,
    /// A gap in the median of a divided road for turning or U-turns; the other line only says where.
    MedianOpening,
    /// A one-way ramp joining the road at a sharp angle with an acceleration lane.
    OnRamp,
    /// A one-way ramp leaving the road with a deceleration lane.
    OffRamp,
    /// A pedestrian crossing in the middle of a road, with stop lines and optional flashing beacons.
    MidblockCrossing,
    /// A railway crossing the road: rails, ties, stop lines, crossbucks and gates.
    Railroad,
    Overpass,
    Roundabout,
    Cloverleaf,
}

impl Treatment {
    pub fn name(self) -> &'static str {
        match self {
            Treatment::Uncontrolled => "uncontrolled",
            Treatment::Signal => "signal",
            Treatment::TwoWayStop => "two-way-stop",
            Treatment::FourWayStop => "four-way-stop",
            Treatment::Driveway => "driveway",
            Treatment::MedianOpening => "median-opening",
            Treatment::OnRamp => "on-ramp",
            Treatment::OffRamp => "off-ramp",
            Treatment::MidblockCrossing => "midblock-crossing",
            Treatment::Railroad => "railroad",
            Treatment::Overpass => "overpass",
            Treatment::Roundabout => "roundabout",
            Treatment::Cloverleaf => "cloverleaf",
        }
    }

    /// Reads the usual spellings ("lighted intersection", "2 way stop", "underpass", "traffic circle", "exit ramp" ...).
    pub fn parse(word: &str) -> Result<Treatment, String> {
        let w = word.trim().to_ascii_lowercase().replace([' ', '_'], "-");
        Ok(match w.as_str() {
            "" | "none" | "uncontrolled" | "no-control" => Treatment::Uncontrolled,
            "signal" | "signals" | "signalised" | "signalized" | "lighted" | "lighted-intersection" | "traffic-light" | "traffic-lights" | "lights" => Treatment::Signal,
            "two-way-stop" | "2-way-stop" | "stop" | "stop-signs" | "minor-stop" => Treatment::TwoWayStop,
            "four-way-stop" | "4-way-stop" | "all-way-stop" | "all-stop" => Treatment::FourWayStop,
            "driveway" | "side-entrance" | "entrance" | "curb-cut" | "access" | "access-road" => Treatment::Driveway,
            "median-opening" | "median-crossover" | "crossover" | "u-turn" | "u-turn-crossover" | "median-gap" | "median-break" => Treatment::MedianOpening,
            "on-ramp" | "onramp" | "entrance-ramp" | "merge" | "acceleration-lane" => Treatment::OnRamp,
            "off-ramp" | "offramp" | "exit-ramp" | "exit" | "diverge" | "deceleration-lane" => Treatment::OffRamp,
            "midblock-crossing" | "mid-block-crossing" | "midblock" | "mid-block" | "pedestrian-crossing" | "ped-crossing" | "crosswalk" | "hawk" | "rrfb" => Treatment::MidblockCrossing,
            "railroad" | "railway" | "rail-crossing" | "railroad-crossing" | "level-crossing" | "grade-crossing" | "train-crossing" => Treatment::Railroad,
            "overpass" | "underpass" | "bridge" | "grade-separated" | "flyover" => Treatment::Overpass,
            "roundabout" | "round-about" | "traffic-circle" | "rotary" => Treatment::Roundabout,
            "cloverleaf" | "interchange" | "clover-leaf" => Treatment::Cloverleaf,
            other => return Err(format!("treatment must be signal, two-way-stop, four-way-stop, uncontrolled, driveway, median-opening, on-ramp, off-ramp, midblock-crossing, railroad, overpass, roundabout or cloverleaf (got \"{other}\")")),
        })
    }

    /// The treatments drawn as a paved box with kerb returns and markings on each approach.
    pub fn at_grade(self) -> bool {
        matches!(self, Treatment::Uncontrolled | Treatment::Signal | Treatment::TwoWayStop | Treatment::FourWayStop | Treatment::Driveway | Treatment::OnRamp | Treatment::OffRamp)
    }

    /// The kerb radius when none is given: small for a driveway, none for a ramp (it joins at a sharp angle), 6 m otherwise.
    fn default_radius(self) -> f64 {
        match self {
            Treatment::Driveway => 1.5,
            Treatment::OnRamp | Treatment::OffRamp => 0.0,
            _ => 6.0,
        }
    }
}

/// Where left- or right-turn pockets are cut into the approaches (a lane that starts `pocket_length` metres from the intersection, with a taper).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pocket {
    None,
    Left,
    Right,
    Both,
}

impl Pocket {
    fn parse(v: &Value) -> Result<Pocket, String> {
        if let Some(b) = v.as_bool() {
            return Ok(if b { Pocket::Both } else { Pocket::None });
        }
        Ok(match v.as_str().map(|w| w.trim().to_ascii_lowercase().replace([' ', '_'], "-")).as_deref() {
            None | Some("") | Some("none") | Some("no") | Some("false") => Pocket::None,
            Some("left") | Some("left-turn") | Some("left-pocket") => Pocket::Left,
            Some("right") | Some("right-turn") | Some("right-pocket") => Pocket::Right,
            Some("both") | Some("all") | Some("true") | Some("yes") => Pocket::Both,
            Some(other) => return Err(format!("pocket must be none, left, right or both (got \"{other}\")")),
        })
    }

    fn name(self) -> &'static str {
        match self {
            Pocket::None => "none",
            Pocket::Left => "left",
            Pocket::Right => "right",
            Pocket::Both => "both",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrossingSpec {
    pub roads: [String; 2],
    pub treatment: Treatment,
    pub crosswalks: bool,
    /// Kerb radius in metres, or None for the treatment's own (6 m, a driveway 1.5 m, a ramp none).
    pub radius: Option<f64>,
    /// For a two-way stop: the road that stops. Left out, the narrower road stops.
    pub minor: Option<String>,
    /// For an overpass: the road that goes over the other. Left out, the first road.
    pub over: Option<String>,
    /// For a median opening, mid-block crossing, railroad or ramp: the road that stays (the divided road, the road crossed, the through road).
    /// Left out, it is worked out from the cross-sections. The other road is only the line that says where, and is not drawn across it.
    pub main: Option<String>,
    /// For a roundabout: the outer diameter in metres.
    pub diameter: f64,
    /// What to do with a short stub of road past the crossing.
    pub stub: Stub,
    /// A side of a road shorter than this many metres, measured from the crossing centre, is a stub.
    pub stub_length: f64,
    /// Turn pockets on the approaches, and how long (full width) and how far the taper runs.
    pub pocket: Pocket,
    pub pocket_length: f64,
    pub pocket_taper: f64,
    /// Right-turn slip lanes with a triangular island at the corners, and their kerb radius.
    pub slip: bool,
    pub slip_radius: f64,
    /// Median opening length along the divided road.
    pub opening: f64,
    /// Ramp acceleration or deceleration lane: full-width length and taper.
    pub accel_length: f64,
    pub accel_taper: f64,
    /// Mid-block crossing: flashing beacons. Railroad: gates (crossbucks are always drawn).
    pub beacon: bool,
    pub gates: bool,
}

impl CrossingSpec {
    pub fn radius(&self) -> f64 {
        self.radius.unwrap_or_else(|| self.treatment.default_radius())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stub {
    /// Draw it as a dead end: a four-way intersection with one short arm.
    Keep,
    /// Stop the built road at the other road: a three-way intersection.
    Trim,
}

fn clean_id(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(16).collect()
}

impl CrossingSpec {
    /// Reads and checks a crossing description (what an agent returns, or what is stored).
    pub fn from_json(v: &Value) -> Result<CrossingSpec, String> {
        let ids: Vec<String> = v["roads"].as_array().map(|a| a.iter().filter_map(Value::as_str).map(clean_id).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
        let [a, b] = <[String; 2]>::try_from(ids).map_err(|_| "a crossing needs the ids of exactly two roads".to_string())?;
        if a == b {
            return Err("a crossing needs two different roads".into());
        }
        let pick = |key: &str| -> Result<Option<String>, String> {
            match v[key].as_str().map(clean_id).filter(|m| !m.is_empty()) {
                Some(m) if m != a && m != b => Err(format!("\"{key}\" must be one of the two roads ({a}, {b}), not \"{m}\"")),
                other => Ok(other),
            }
        };
        // The older `control` field (none, signal, stop) still reads: stop is a two-way stop if a minor road is named, else a four-way stop.
        let (minor, over, main) = (pick("minor")?, pick("over")?, pick("main")?);
        let treatment = match (v["treatment"].as_str(), v["control"].as_str()) {
            (Some(t), _) => Treatment::parse(t)?,
            (None, Some(c)) => match c.trim().to_ascii_lowercase().as_str() {
                "stop" | "stop-sign" | "stop-signs" => if minor.is_some() { Treatment::TwoWayStop } else { Treatment::FourWayStop },
                other => Treatment::parse(other).map_err(|_| format!("control must be none, signal or stop (got \"{other}\")"))?,
            },
            (None, None) => Treatment::Uncontrolled,
        };
        Ok(CrossingSpec {
            roads: [a, b],
            treatment,
            crosswalks: v["crosswalks"].as_bool().unwrap_or(false),
            radius: v["radius"].as_f64().map(|r| r.clamp(0.0, 20.0)),
            minor,
            over,
            main,
            diameter: v["diameter"].as_f64().unwrap_or(40.0).clamp(20.0, 120.0),
            stub: match v["stub"].as_str().map(|w| w.trim().to_ascii_lowercase().replace([' ', '_'], "-")).as_deref() {
                None | Some("") | Some("keep") | Some("dead-end") | Some("deadend") | Some("4-way") | Some("four-way") => Stub::Keep,
                Some("trim") | Some("cut") | Some("3-way") | Some("three-way") | Some("tee") | Some("t") => Stub::Trim,
                Some(other) => return Err(format!("stub must be keep or trim (got \"{other}\")")),
            },
            stub_length: v["stub_length"].as_f64().unwrap_or(25.0).clamp(5.0, 100.0),
            pocket: Pocket::parse(&v["pocket"])?,
            pocket_length: v["pocket_length"].as_f64().unwrap_or(30.0).clamp(5.0, 150.0),
            pocket_taper: v["pocket_taper"].as_f64().unwrap_or(15.0).clamp(0.0, 60.0),
            slip: v["slip"].as_bool().unwrap_or(false),
            slip_radius: v["slip_radius"].as_f64().unwrap_or(15.0).clamp(8.0, 40.0),
            opening: v["opening"].as_f64().unwrap_or(18.0).clamp(8.0, 40.0),
            accel_length: v["accel_length"].as_f64().unwrap_or(120.0).clamp(30.0, 400.0),
            accel_taper: v["accel_taper"].as_f64().unwrap_or(40.0).clamp(0.0, 150.0),
            beacon: v["beacon"].as_bool().unwrap_or(false),
            gates: v["gates"].as_bool().unwrap_or(true),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "roads": self.roads, "treatment": self.treatment.name(), "crosswalks": self.crosswalks, "radius": self.radius,
            "minor": self.minor, "over": self.over, "main": self.main, "diameter": self.diameter,
            "stub": if self.stub == Stub::Trim { "trim" } else { "keep" }, "stub_length": self.stub_length,
            "pocket": self.pocket.name(), "pocket_length": self.pocket_length, "pocket_taper": self.pocket_taper,
            "slip": self.slip, "slip_radius": self.slip_radius, "opening": self.opening,
            "accel_length": self.accel_length, "accel_taper": self.accel_taper, "beacon": self.beacon, "gates": self.gates
        })
    }

    pub fn summary(&self) -> String {
        let [a, b] = &self.roads;
        let what = match self.treatment {
            Treatment::Uncontrolled => "no control".to_string(),
            Treatment::Signal => "traffic signals".to_string(),
            Treatment::TwoWayStop => format!("two-way stop, {}", self.minor.as_ref().map_or("the narrower road stops".to_string(), |m| format!("road {m} stops"))),
            Treatment::FourWayStop => "four-way stop".to_string(),
            Treatment::Driveway => "driveway".to_string(),
            Treatment::MedianOpening => format!("median opening, {} m long", self.opening),
            Treatment::OnRamp => format!("on-ramp with a {} m acceleration lane", self.accel_length),
            Treatment::OffRamp => format!("off-ramp with a {} m deceleration lane", self.accel_length),
            Treatment::MidblockCrossing => if self.beacon { "mid-block pedestrian crossing with flashing beacons" } else { "mid-block pedestrian crossing" }.to_string(),
            Treatment::Railroad => if self.gates { "railroad crossing with gates" } else { "railroad crossing" }.to_string(),
            Treatment::Overpass => {
                let over = self.over.as_ref().unwrap_or(a);
                format!("road {over} passes over road {}", if over == a { b } else { a })
            }
            Treatment::Roundabout => format!("roundabout, {} m across", self.diameter),
            Treatment::Cloverleaf => "cloverleaf interchange (overpass and four loop ramps)".to_string(),
        };
        let grade = self.treatment.at_grade();
        let walks = if self.crosswalks && (grade || self.treatment == Treatment::Roundabout) { ", crosswalks" } else { "" };
        let stub = if self.stub == Stub::Trim && grade { ", a short stub is trimmed (three-way)" } else { "" };
        let pockets = if self.pocket != Pocket::None && grade { format!(", {} turn pockets ({} m)", self.pocket.name(), self.pocket_length) } else { String::new() };
        let slip = if self.slip && grade { ", right-turn slip lanes" } else { "" };
        format!("roads {a} and {b}: {what}{walks}{stub}{pockets}{slip}")
    }
}

/// Where two roads cross: the point and each road's unit direction there, and the piece of each road (world metres, y up) that crosses.
pub struct Crossing {
    pub p: P,
    pub ta: P,
    pub tb: P,
    pub path_a: Vec<P>,
    pub path_b: Vec<P>,
    /// Which items (by position) the two pieces are, so a trim can find them again.
    pub index_a: usize,
    pub index_b: usize,
    /// False when the crossing was found by extending that road's end: the road only touches the other (a side road drawn up to the kerb).
    pub reach_a: bool,
    pub reach_b: bool,
}

pub(crate) fn cross2(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

pub(crate) fn unit(v: P) -> P {
    let m = v[0].hypot(v[1]).max(1e-12);
    [v[0] / m, v[1] / m]
}

fn pieces(items: &[Item], road: &str) -> Vec<(usize, Vec<P>)> {
    items.iter().enumerate().filter(|(_, i)| segment_id(&i.name).as_deref() == Some(road)).filter_map(|(k, i)| i.subs.first().map(|s| (k, s.iter().map(|&p| flip(p)).collect()))).collect()
}

/// How far a road's end is extended to look for a road it only touches (a side road drawn up to the kerb, a driveway, a ramp).
const EXTEND: f64 = 30.0;

/// The line with each end extended straight on by `EXTEND` metres.
fn extended(path: &[P]) -> Vec<P> {
    let n = path.len();
    if n < 2 {
        return path.to_vec();
    }
    let (t0, t1) = (unit(sub(path[1], path[0])), unit(sub(path[n - 1], path[n - 2])));
    let mut out = vec![sub(path[0], scale(t0, EXTEND))];
    out.extend_from_slice(path);
    out.push(add(path[n - 1], scale(t1, EXTEND)));
    out
}

/// The first place the two lines cross: the point and each line's unit direction there.
fn first_cross(pa: &[P], pb: &[P]) -> Option<(P, P, P)> {
    for wa in pa.windows(2) {
        for wb in pb.windows(2) {
            let (r, s) = (sub(wa[1], wa[0]), sub(wb[1], wb[0]));
            let den = cross2(r, s);
            if den.abs() < 1e-12 {
                continue;
            }
            let q = sub(wb[0], wa[0]);
            let (t, u) = (cross2(q, s) / den, cross2(q, r) / den);
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                return Some((add(wa[0], scale(r, t)), unit(r), unit(s)));
            }
        }
    }
    None
}

fn on_path(path: &[P], p: P) -> bool {
    Poly::new(path.to_vec()).project(p).2 < 1e-6
}

/// The first place road `a` crosses road `b` (the lines as drawn, curves already flattened). Failing that, the place where one road's end,
/// carried straight on for `EXTEND` metres, meets the other: a road that only touches (`reach_*` is false for the road that stops short).
pub fn find(items: &[Item], a: &str, b: &str) -> Option<Crossing> {
    let (pa, pb) = (pieces(items, a), pieces(items, b));
    for (ia, la) in &pa {
        for (ib, lb) in &pb {
            if let Some((p, ta, tb)) = first_cross(la, lb) {
                return Some(Crossing { p, ta, tb, path_a: la.clone(), path_b: lb.clone(), index_a: *ia, index_b: *ib, reach_a: true, reach_b: true });
            }
        }
    }
    for (ia, la) in &pa {
        for (ib, lb) in &pb {
            if let Some((p, ta, tb)) = first_cross(&extended(la), &extended(lb)) {
                return Some(Crossing { p, ta, tb, path_a: la.clone(), path_b: lb.clone(), index_a: *ia, index_b: *ib, reach_a: on_path(la, p), reach_b: on_path(lb, p) });
            }
        }
    }
    None
}

/// For a road that stops short of `p`: which side of `p` its missing part is (-1 before, +1 after the end it stops at); 0 if it reaches `p`.
fn missing_side(path: &[P], p: P) -> f64 {
    let poly = Poly::new(path.to_vec());
    let (s0, _, d) = poly.project(p);
    if d < 1e-6 {
        return 0.0;
    }
    if s0 >= poly.len() - 1e-9 { 1.0 } else { -1.0 }
}

/// One road at the crossing: its direction, its sideways frame and the elements laid out across it.
pub(crate) struct Arm {
    pub(crate) id: String,
    pub(crate) t: P,
    pub(crate) n: P,
    pub(crate) lo: f64,
    pub(crate) hi: f64,
    pub(crate) placed: Vec<Placed>,
}

/// The circle that rounds the corner `c` between two edges leaving along `d1` and `d2` (unit vectors) with radius `r`: its centre, the angle of the
/// first tangent point, the signed sweep to the second, and the tangent length. None for a corner too sharp or too flat to round.
fn fillet_parts(c: P, d1: P, d2: P, r: f64) -> Option<(P, f64, f64, f64)> {
    let cos = dot(d1, d2).clamp(-1.0, 1.0);
    let theta = cos.acos();
    if r <= 0.0 || theta < 0.2 || theta > std::f64::consts::PI - 0.2 {
        return None;
    }
    let (t, to_centre) = (r / (theta / 2.0).tan(), r / (theta / 2.0).sin());
    let centre = add(c, scale(unit(add(d1, d2)), to_centre));
    let (t1, t2) = (add(c, scale(d1, t)), add(c, scale(d2, t)));
    let (a1, a2) = ((t1[1] - centre[1]).atan2(t1[0] - centre[0]), (t2[1] - centre[1]).atan2(t2[0] - centre[0]));
    let mut delta = a2 - a1;
    while delta > std::f64::consts::PI {
        delta -= std::f64::consts::TAU;
    }
    while delta < -std::f64::consts::PI {
        delta += std::f64::consts::TAU;
    }
    Some((centre, a1, delta, t))
}

/// Points along an arc of `radius` about `centre`, from angle `a1` sweeping `delta`.
pub(crate) fn arc_points(centre: P, radius: f64, a1: f64, delta: f64) -> Vec<P> {
    (0..=12).map(|i| { let a = a1 + delta * i as f64 / 12.0; [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()] }).collect()
}

/// A kerb return between two edges that meet at corner `c`, leaving along `d1` and `d2` (unit vectors): the arc and the paved fill behind it.
fn fillet(c: P, d1: P, d2: P, r: f64) -> Option<(Vec<P>, Vec<P>)> {
    let (centre, a1, delta, _) = fillet_parts(c, d1, d2, r)?;
    let arc = arc_points(centre, r, a1, delta);
    let mut fill = vec![c];
    fill.extend(arc.iter().copied());
    Some((arc, fill))
}

/// Builds every stored crossing. Roads must already have been emitted (the intersection is drawn over them). Returns notes for the user.
pub fn build(items: &[Item], spec: &Spec, out: &mut Out) -> Vec<String> {
    let mut notes = Vec::new();
    for (id, v) in spec.crossings() {
        match CrossingSpec::from_json(&v) {
            Ok(cs) => {
                match emit(items, spec, &id, &cs, out) {
                    Ok(ns) => notes.extend(ns.into_iter().map(|n| format!("intersection {id}: {n}"))),
                    Err(e) => notes.push(format!("intersection {id}: {e}")),
                }
            }
            Err(e) => notes.push(format!("intersection {id}: {e}")),
        }
    }
    notes
}

/// Lays out the two roads at the crossing: direction, sideways frame and the elements across each.
fn make_arms(spec: &Spec, cs: &CrossingSpec, x: &Crossing) -> Result<Vec<Arm>, String> {
    let mut arms: Vec<Arm> = Vec::new();
    for (road, t, path) in [(&cs.roads[0], x.ta, &x.path_a), (&cs.roads[1], x.tb, &x.path_b)] {
        let (parsed, anchor, _) = spec.for_segment(road);
        let placed = layout(&parsed, &spec.library, heading(path), anchor)?;
        let (lo, hi) = (placed.first().map_or(-3.0, |e| e.left), placed.last().map_or(3.0, |e| e.right));
        arms.push(Arm { id: road.clone(), t, n: Poly::right(t), lo, hi, placed });
    }
    if cross2(arms[0].n, arms[1].n).abs() < 0.17 {
        return Err("the roads are nearly parallel, so they do not form an intersection".into());
    }
    Ok(arms)
}

/// The short sides: `(arm, sign)` where the road runs less than `stub_length` past the crossing centre (sign -1 is the part before the
/// crossing, +1 the part after). A road with a short side on both ends is a short road, not a stub, and is left alone.
fn stub_sides(x: &Crossing, cs: &CrossingSpec) -> Vec<(usize, f64)> {
    let mut sides = Vec::new();
    for (k, path) in [&x.path_a, &x.path_b].into_iter().enumerate() {
        let poly = Poly::new(path.clone());
        let (s0, _, _) = poly.project(x.p);
        let (before, after) = (s0 < cs.stub_length, poly.len() - s0 < cs.stub_length);
        match (before, after) {
            (true, false) => sides.push((k, -1.0)),
            (false, true) => sides.push((k, 1.0)),
            _ => {}
        }
    }
    sides
}

/// The sides to trim away: every short stub if the stub choice is `trim`, and always a side of a road that stops short of the other road
/// (that road is extended to meet the pavement instead, see `clip`).
fn trimmed_sides(x: &Crossing, cs: &CrossingSpec) -> Vec<(usize, f64)> {
    let mut sides: Vec<(usize, f64)> = Vec::new();
    for (k, (path, reach)) in [(&x.path_a, x.reach_a), (&x.path_b, x.reach_b)].into_iter().enumerate() {
        if !reach {
            sides.push((k, missing_side(path, x.p)));
        }
    }
    if cs.stub == Stub::Trim {
        for side in stub_sides(x, cs) {
            if !sides.contains(&side) && !sides.iter().any(|(k, _)| *k == side.0) {
                sides.push(side);
            }
        }
    }
    sides
}

/// Where each road's paved band ends, as `[(lo, hi); 2]` per arm. Normally the road's whole width; where the *other* road is trimmed to a
/// stub-free end, the paving stops at this road's outer travel lane so its far kerb, bike lane and verge carry on (a three-way keeps them).
pub(crate) fn bounds(x: &Crossing, arms: &[Arm], trimmed: &[(usize, f64)]) -> [(f64, f64); 2] {
    let mut b = [(arms[0].lo, arms[0].hi), (arms[1].lo, arms[1].hi)];
    for &(k, sign) in trimmed {
        let o = 1 - k;
        let dir = scale(if k == 0 { x.ta } else { x.tb }, sign);
        let travels: Vec<&Placed> = arms[o].placed.iter().filter(|e| travel(e)).collect();
        if travels.is_empty() {
            continue;
        }
        if dot(dir, arms[o].n) > 0.0 {
            b[o].1 = travels.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max);
        } else {
            b[o].0 = travels.iter().map(|e| e.left).fold(f64::INFINITY, f64::min);
        }
    }
    b
}

/// What a trim does to one road piece: drop the part beyond `dist` metres from `p` on the `sign` side (sign -1 is before `p` along the line).
pub struct Trim {
    pub index: usize,
    pub p: P,
    pub sign: f64,
    pub dist: f64,
    /// When set, this is not a trim of one side but a gap: the road is cut out from this many metres before `p` to `dist` metres after it,
    /// leaving up to two pieces (the line that only says where, inside the road it crosses).
    pub gap_before: Option<f64>,
}

/// The cuts the stored crossings ask for, to be applied to the roads before they are drawn (see `clip`).
pub fn trims(items: &[Item], spec: &Spec) -> Vec<Trim> {
    let mut out = Vec::new();
    for (_, v) in spec.crossings() {
        let Ok(cs) = CrossingSpec::from_json(&v) else { continue };
        let Some(x) = find(items, &cs.roads[0], &cs.roads[1]) else { continue };
        let Ok(arms) = make_arms(spec, &cs, &x) else { continue };
        let index = |k: usize| if k == 0 { x.index_a } else { x.index_b };
        let tangent = |k: usize| if k == 0 { x.ta } else { x.tb };
        if cs.treatment.at_grade() {
            let sides = trimmed_sides(&x, &cs);
            let b = bounds(&x, &arms, &sides);
            let corners = [(b[0].0, b[1].0), (b[0].1, b[1].0), (b[0].1, b[1].1), (b[0].0, b[1].1)].map(|(ua, ub)| corner_at(&x, &arms, ua, ub));
            for &(k, sign) in &sides {
                let mut reach_of: Vec<f64> = corners.iter().map(|&c| dot(scale(tangent(k), sign), sub(c, x.p))).collect();
                reach_of.sort_by(|p, q| q.total_cmp(p));
                // The nearer of the two far corners: everything of the stub up to there lies on the other road's pavement or in the box.
                out.push(Trim { index: index(k), p: x.p, sign, dist: reach_of[1].max(0.0), gap_before: None });
            }
        } else if cross_road_only(cs.treatment) {
            // The line that only says where (the crossover, the footpath, the railway) is cut back to the edge of the road it crosses.
            let c = 1 - main_index(&cs, &arms);
            let corners = [(arms[0].lo, arms[1].lo), (arms[0].hi, arms[1].lo), (arms[0].hi, arms[1].hi), (arms[0].lo, arms[1].hi)].map(|(ua, ub)| corner_at(&x, &arms, ua, ub));
            let reach_of = |sign: f64| {
                let mut r: Vec<f64> = corners.iter().map(|&q| dot(scale(tangent(c), sign), sub(q, x.p))).collect();
                r.sort_by(|p, q| q.total_cmp(p));
                r[1].max(0.0)
            };
            out.push(Trim { index: index(c), p: x.p, sign: 1.0, dist: reach_of(1.0), gap_before: Some(reach_of(-1.0)) });
        }
    }
    out
}

/// Treatments where one road stays and the other is only a line saying where (and is not drawn across it).
pub(crate) fn cross_road_only(t: Treatment) -> bool {
    matches!(t, Treatment::MedianOpening | Treatment::MidblockCrossing | Treatment::Railroad)
}

/// The road that stays: named in `main`, else worked out from the cross-sections (the divided road, the road that is not the railway, the
/// road with more travel lanes).
pub(crate) fn main_index(cs: &CrossingSpec, arms: &[Arm]) -> usize {
    if let Some(m) = &cs.main {
        if let Some(k) = arms.iter().position(|a| a.id == *m) {
            return k;
        }
    }
    let lanes = |a: &Arm| a.placed.iter().filter(|e| travel(e)).count();
    let has = |a: &Arm, id: &str| a.placed.iter().any(|e| e.id == id);
    match cs.treatment {
        Treatment::Railroad => if has(&arms[0], "railway") { 1 } else { 0 },
        Treatment::MedianOpening => {
            let medians = |a: &Arm| a.placed.iter().enumerate().filter(|(i, e)| *i > 0 && *i + 1 < a.placed.len() && e.role == Role::Curbed).count();
            if medians(&arms[1]) > medians(&arms[0]) { 1 } else { 0 }
        }
        _ => if lanes(&arms[1]) > lanes(&arms[0]) { 1 } else { 0 },
    }
}

/// The road line with the part a trim drops cut off (the line itself in the document is not touched). A road that stops short of `p`
/// is carried straight on to the cut instead, so it meets the pavement it touches.
pub(crate) fn clip(path: &[P], t: &Trim) -> Vec<P> {
    let poly = Poly::new(path.to_vec());
    let len = poly.len();
    let n = path.len();
    if n < 2 || len <= 0.0 {
        return path.to_vec();
    }
    let (s0, _, _) = poly.project(t.p);
    let (t_start, t_end) = (unit(sub(path[1], path[0])), unit(sub(path[n - 1], path[n - 2])));
    // How far `p` lies beyond the end (or before the start) the cut is measured from, when the road does not reach it.
    let over = if s0 >= len - 1e-9 { dot(sub(t.p, path[n - 1]), t_end).max(0.0) } else if s0 <= 1e-9 { dot(sub(path[0], t.p), t_start).max(0.0) } else { 0.0 };
    let cut = if t.sign > 0.0 { s0 + over + t.dist } else { s0 - over - t.dist };
    let mut cum = vec![0.0];
    for w in path.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let mut kept: Vec<P> = Vec::new();
    if t.sign > 0.0 {
        if cut > len {
            kept.extend_from_slice(path);
            kept.push(add(path[n - 1], scale(t_end, cut - len)));
        } else {
            kept.extend(path.iter().zip(&cum).filter(|(_, c)| **c < cut - 1e-6).map(|(p, _)| *p));
            kept.push(poly.at(cut).0);
        }
    } else if cut < 0.0 {
        kept.push(sub(path[0], scale(t_start, -cut)));
        kept.extend_from_slice(path);
    } else {
        kept.push(poly.at(cut).0);
        kept.extend(path.iter().zip(&cum).filter(|(_, c)| **c > cut + 1e-6).map(|(p, _)| *p));
    }
    if kept.len() < 2 { path.to_vec() } else { kept }
}

/// The path between route metres `from` and `to`.
fn slice_path(path: &[P], from: f64, to: f64) -> Vec<P> {
    let poly = Poly::new(path.to_vec());
    let mut cum = vec![0.0];
    for w in path.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let mut out = vec![poly.at(from).0];
    out.extend(path.iter().zip(&cum).filter(|(_, c)| **c > from + 1e-6 && **c < to - 1e-6).map(|(p, _)| *p));
    out.push(poly.at(to).0);
    out
}

/// The pieces of a road line left to draw after the trims for it: one piece, or two where a gap was cut out of the middle.
pub(crate) fn apply_trims(path: &[P], trims: &[&Trim]) -> Vec<Vec<P>> {
    let mut pieces = vec![path.to_vec()];
    for t in trims {
        match t.gap_before {
            None => pieces = pieces.into_iter().map(|p| clip(&p, t)).collect(),
            Some(before) => {
                pieces = pieces
                    .into_iter()
                    .flat_map(|piece| {
                        let poly = Poly::new(piece.clone());
                        let (s0, _, d) = poly.project(t.p);
                        if d > 1.0 || poly.len() <= 0.0 {
                            return vec![piece];
                        }
                        let (left, right) = ((s0 - before, 0.0), (s0 + t.dist, poly.len()));
                        let mut out = Vec::new();
                        if left.0 - left.1 > 0.5 {
                            out.push(slice_path(&piece, left.1, left.0));
                        }
                        if right.1 - right.0 > 0.5 {
                            out.push(slice_path(&piece, right.0, right.1));
                        }
                        out
                    })
                    .collect();
            }
        }
    }
    pieces
}

fn emit(items: &[Item], spec: &Spec, id: &str, cs: &CrossingSpec, out: &mut Out) -> Result<Vec<String>, String> {
    let x = find(items, &cs.roads[0], &cs.roads[1]).ok_or_else(|| format!("roads {} and {} do not cross or meet", cs.roads[0], cs.roads[1]))?;
    let arms = make_arms(spec, cs, &x)?;
    let base = format!("gen.crossing.{id}");
    let mut notes = Vec::new();
    match cs.treatment {
        Treatment::Overpass => overpass(out, &base, cs, &x, &arms),
        Treatment::Roundabout => roundabout(out, &base, cs, &x, &arms),
        t if t.at_grade() => {
            let stubs = stub_sides(&x, cs);
            let trimmed = trimmed_sides(&x, cs);
            if cs.stub == Stub::Trim && trimmed.is_empty() {
                notes.push(format!("no stub to trim: both roads run more than {} m past the crossing", cs.stub_length));
            }
            notes.extend(at_grade(out, &base, cs, &x, &arms, &trimmed));
            if matches!(cs.treatment, Treatment::OnRamp | Treatment::OffRamp) {
                notes.extend(crate::scenarios::ramp_lane(out, &base, cs, &x, &arms, &trimmed));
            }
            // A stub that is kept ends in a kerb across its end: a dead end.
            if cs.stub == Stub::Keep {
                for &(k, sign) in stubs.iter().filter(|s| !trimmed.contains(s)) {
                    let path = if k == 0 { &x.path_a } else { &x.path_b };
                    let poly = Poly::new(path.clone());
                    let (p, t) = poly.at(if sign > 0.0 { poly.len() } else { 0.0 });
                    let n = Poly::right(t);
                    let tag = format!("{}.{}", arms[k].id, if sign < 0.0 { "before" } else { "after" });
                    out.path(&format!("{base}.deadend.{tag}"), &[vec![add(p, scale(n, arms[k].lo)), add(p, scale(n, arms[k].hi))]], flip, CURB, 0.3, false);
                }
            }
        }
        Treatment::Cloverleaf | Treatment::MedianOpening | Treatment::MidblockCrossing | Treatment::Railroad | _ => {
            notes.extend(crate::scenarios::emit(out, &base, cs, &x, &arms)?);
        }
    }
    Ok(notes)
}

/// Where a median, margin or verge in the middle of a road ends before an intersection: it is capped just short of the crosswalk (or a metre
/// from the paved box when there is none), so the crosswalk and the box lie on plain pavement. `from` and `to` are distances from the
/// centre of the intersection along the approach (start at 0: on an oblique crossing the box edge is nearer than its far corner); `at(along, across)` turns a distance along and a sideways offset into a point.
/// Pavement is laid over the median between them, with a kerb across the end (the nose).
pub(crate) fn cap_medians(out: &mut Out, name: &str, placed: &[Placed], from: f64, to: f64, at: &dyn Fn(f64, f64) -> P) {
    const OVER: f64 = 0.2; // laid a little past the median's own kerbs so they are covered
    for (k, e) in placed.iter().enumerate().filter(|(i, e)| *i > 0 && *i + 1 < placed.len() && e.role == Role::Curbed).map(|(_, e)| e).enumerate() {
        let ring = [at(from, e.left - OVER), at(to, e.left - OVER), at(to, e.right + OVER), at(from, e.right + OVER)];
        out.polygon(&format!("{name}.{}", k + 1), &ring, flip, PAVEMENT);
        out.path(&format!("{name}.{}.nose", k + 1), &[vec![at(to, e.left), at(to, e.right)]], flip, CURB, 0.3, false);
    }
}

/// Where the two roads' bands overlap, at offsets `ua` along road a's normal and `ub` along road b's.
pub(crate) fn corner_at(x: &Crossing, arms: &[Arm], ua: f64, ub: f64) -> P {
    let (a, b) = (&arms[0], &arms[1]);
    let det = cross2(a.n, b.n);
    add(x.p, [(ua * b.n[1] - ub * a.n[1]) / det, (a.n[0] * ub - b.n[0] * ua) / det])
}

/// How far the overlap reaches from the crossing point along `dir`.
pub(crate) fn reach(corners: &[P; 4], p: P, dir: P) -> f64 {
    corners.iter().map(|&c| dot(dir, sub(c, p))).fold(f64::NEG_INFINITY, f64::max).max(0.0)
}

/// What an approach is controlled by.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ctl {
    None,
    Signal,
    Stop,
}

/// Everything `approach` needs to mark one arm of an intersection.
pub(crate) struct Approach<'a> {
    pub base: &'a str,
    pub arm: &'a Arm,
    /// -1 is the part of the road before the intersection (along the line as drawn), +1 the part after.
    pub sign: f64,
    pub centre: P,
    /// How far the paved box reaches along this arm from the centre.
    pub ext: f64,
    pub ctl: Ctl,
    pub crosswalks: bool,
    pub pocket: Pocket,
    pub pocket_length: f64,
    pub pocket_taper: f64,
}

pub(crate) const GAP: f64 = 1.0;
pub(crate) const CW: f64 = 3.0;
const POCKET_WIDTH: f64 = 3.3;

/// The marks on one arm: the median capped short of the crosswalk, a turn pocket, the crosswalk, the stop bar and the signal head or stop sign.
/// Traffic enters from this side if it heads toward the intersection: before it that is the direction the road was drawn (fwd).
/// Returns notes (things that could not be drawn).
pub(crate) fn approach(out: &mut Out, ap: &Approach) -> Vec<String> {
    let (arm, sign, ext) = (ap.arm, ap.sign, ap.ext);
    let mut notes = Vec::new();
    let dir = scale(arm.t, sign);
    let tag = format!("{}.{}", arm.id, if sign < 0.0 { "before" } else { "after" });
    let at = |along: f64, across: f64| add(add(ap.centre, scale(dir, along)), scale(arm.n, across));
    cap_medians(out, &format!("{}.cap.{tag}", ap.base), &arm.placed, 0.0, ext + GAP + if ap.crosswalks { CW + 0.3 } else { 0.0 }, &at);

    let want = if sign < 0.0 { Dir::Fwd } else { Dir::Back };
    let lanes: Vec<&Placed> = arm.placed.iter().filter(|e| travel(e) && e.dir == Some(want)).collect();
    let lane_span = (!lanes.is_empty()).then(|| (lanes.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), lanes.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max)));
    // Which way is the driver's right in the arm's sideways coordinate?
    let heading_in = scale(dir, -1.0);
    let right_of_driver = [heading_in[1], -heading_in[0]];
    let right_is_plus = dot(right_of_driver, arm.n) > 0.0;

    if let (Some((lo, hi)), true) = (lane_span, ap.pocket != Pocket::None) {
        for (side_left, wanted) in [(true, matches!(ap.pocket, Pocket::Left | Pocket::Both)), (false, matches!(ap.pocket, Pocket::Right | Pocket::Both))] {
            if !wanted {
                continue;
            }
            // The pocket lies on the driver's left or right of the inbound lanes; `step` is +1 or -1 in the sideways coordinate.
            let step = if side_left == right_is_plus { -1.0 } else { 1.0 };
            let edge = if step > 0.0 { hi } else { lo };
            if side_left {
                // A left pocket is cut into what lies between the lanes and the opposing lanes: it needs room there.
                let room = room_beyond(&arm.placed, edge, step);
                if room < POCKET_WIDTH - 1e-6 {
                    notes.push(format!("road {}: no median or centre strip wide enough for a left-turn pocket on the {} approach", arm.id, if sign < 0.0 { "before" } else { "after" }));
                    continue;
                }
            }
            pocket_strip(out, &format!("{}.pocket.{tag}.{}", ap.base, if side_left { "left" } else { "right" }), &at, ap, edge, step, side_left);
        }
    }

    if ap.crosswalks {
        let centre = ext + GAP + CW / 2.0;
        let stripes: Vec<Vec<P>> = (0..((arm.hi - arm.lo) / 1.2).floor() as usize)
            .map(|i| { let u = arm.lo + 0.6 + 1.2 * i as f64; vec![at(centre - CW / 2.0, u), at(centre + CW / 2.0, u)] })
            .collect();
        out.path(&format!("{}.crosswalk.{tag}", ap.base), &stripes, flip, WHITE, 0.5, false);
    }
    if ap.ctl == Ctl::None {
        return notes;
    }
    let Some((lo, hi)) = lane_span else { return notes };
    let back = if ap.crosswalks { CW + 0.9 } else { 0.6 };
    let dist = ext + GAP + back;
    out.path(&format!("{}.stop.{tag}", ap.base), &[vec![at(dist, lo), at(dist, hi)]], flip, WHITE, 0.6, false);
    // The signal head or sign stands just past the bar, beside the driver's right-hand kerb.
    let pole = if right_is_plus { hi + 0.9 } else { lo - 0.9 };
    let c = at(dist + 0.9, pole);
    if ap.ctl == Ctl::Signal {
        out.polygon(&format!("{}.signal.{tag}", ap.base), &polygon(4, c, 0.8), flip, [0.15, 0.15, 0.17]);
        out.polygon(&format!("{}.signal.{tag}.lamp", ap.base), &polygon(8, c, 0.28), flip, [0.2, 0.85, 0.3]);
    } else {
        out.polygon(&format!("{}.sign.{tag}", ap.base), &polygon(8, c, 0.75), flip, [0.8, 0.12, 0.12]);
    }
    notes
}

/// How much non-travel width lies beyond `edge` in the direction `step` (+1 or -1) before the next travel lane or the end of the road.
fn room_beyond(placed: &[Placed], edge: f64, step: f64) -> f64 {
    let mut room = 0.0;
    let mut items: Vec<&Placed> = placed.iter().filter(|e| if step > 0.0 { e.left >= edge - 1e-6 } else { e.right <= edge + 1e-6 }).collect();
    items.sort_by(|a, b| if step > 0.0 { a.left.total_cmp(&b.left) } else { b.right.total_cmp(&a.right) });
    for e in items {
        if travel(e) {
            break;
        }
        room += e.width;
    }
    room
}

/// A turn pocket: pavement the width of a lane beside the inbound lanes from the box edge out to `pocket_length`, then a taper of
/// `pocket_taper` back to the lane edge. A dashed line separates it from the lane, a solid line closes its far side, and an arrow shows the turn.
fn pocket_strip(out: &mut Out, name: &str, at: &dyn Fn(f64, f64) -> P, ap: &Approach, edge: f64, step: f64, left: bool) {
    let (d0, d1) = (ap.ext, ap.ext + ap.pocket_length);
    let d2 = d1 + ap.pocket_taper;
    let far = edge + step * POCKET_WIDTH;
    let cover = edge - step * 0.15; // a little over the lane edge so the kerb or lane line underneath is covered
    out.polygon(name, &[at(d0, cover), at(d0, far), at(d1, far), at(d2, cover)], flip, PAVEMENT);
    out.path(&format!("{name}.lane-line"), &simplify_dashes(at, edge, d0, d1), flip, WHITE, 0.15, false);
    out.path(&format!("{name}.edge"), &[vec![at(d0, far), at(d1, far), at(d2, edge)]], flip, WHITE, 0.15, false);
    // The arrow sits in the middle of the full-width part, pointing at the box and turning toward the pocket's own side.
    let mid = ((d0 + d1) / 2.0).max(d0 + 6.0).min(d1 - 1.0);
    let ec = edge + step * POCKET_WIDTH / 2.0;
    let turn = if left { step } else { step }; // the pocket is already on the side the driver turns toward
    let tip = (mid - 1.4, ec + turn * 1.0);
    let v = {
        let (dx, dy) = (tip.0 - mid, tip.1 - ec);
        let m = dx.hypot(dy);
        (dx / m, dy / m)
    };
    let wing = |angle: f64| {
        let (c, s) = (angle.cos(), angle.sin());
        let back = (-(v.0 * c - v.1 * s), -(v.0 * s + v.1 * c));
        at(tip.0 + back.0 * 0.9, tip.1 + back.1 * 0.9)
    };
    out.path(&format!("{name}.arrow"), &[vec![at(mid + 2.0, ec), at(mid, ec), at(tip.0, tip.1)], vec![wing(0.5), at(tip.0, tip.1), wing(-0.5)]], flip, WHITE, 0.2, false);
}

/// Dashes (3 m on, 3 m off) along a line at sideways offset `u` from distance `d0` to `d1`.
pub(crate) fn simplify_dashes(at: &dyn Fn(f64, f64) -> P, u: f64, d0: f64, d1: f64) -> Vec<Vec<P>> {
    let mut dashes = Vec::new();
    let mut d = d0;
    while d < d1 {
        dashes.push(vec![at(d, u), at((d + 3.0).min(d1), u)]);
        d += 6.0;
    }
    dashes
}

/// A right-turn slip lane: the corner is rounded with a large radius, the carriageway between that kerb and a triangular island is paved, and the island is kerbed.
fn slip_corner(out: &mut Out, base: &str, k: usize, c: P, d1: P, d2: P, radius: f64) -> bool {
    const LANE: f64 = 4.0;
    let Some((centre, a1, delta, t)) = fillet_parts(c, d1, d2, radius) else { return false };
    // Where the island's kerb circle (radius + lane) meets the two road edges, measured back toward the corner from the kerb's tangent points.
    let m = ((radius + LANE).powi(2) - radius.powi(2)).sqrt();
    if t - m < 1.0 {
        return false;
    }
    let outer = arc_points(centre, radius, a1, delta);
    let mut fill = vec![c];
    fill.extend(outer.iter().copied());
    out.polygon(&format!("{base}.corner.{}", k + 1), &fill, flip, PAVEMENT);
    out.path(&format!("{base}.kerb.{}", k + 1), &[outer], flip, CURB, 0.3, false);
    // The island: bounded by the circle of radius + lane about the same centre, and by the two road edges back toward the corner.
    let (i1, i2) = (add(c, scale(d1, t - m)), add(c, scale(d2, t - m)));
    let ang = |q: P| (q[1] - centre[1]).atan2(q[0] - centre[0]);
    let (b1, b2) = (ang(i1), ang(i2));
    let mut bd = b2 - b1;
    while bd > std::f64::consts::PI {
        bd -= std::f64::consts::TAU;
    }
    while bd < -std::f64::consts::PI {
        bd += std::f64::consts::TAU;
    }
    let to_centre = unit(sub(centre, c));
    let mut island = vec![add(c, scale(to_centre, 2.5))];
    island.extend(arc_points(centre, radius + LANE, b1, bd));
    out.polygon(&format!("{base}.island.{}", k + 1), &island, flip, GRASS);
    out.path(&format!("{base}.island.{}.kerb", k + 1), &[island], flip, CURB, 0.25, true);
    true
}

/// Signals, stop signs or nothing: pave the overlap, round the corners, mark all four approaches. Returns notes.
fn at_grade(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm], trimmed: &[(usize, f64)]) -> Vec<String> {
    let (a, b) = (&arms[0], &arms[1]);
    let bd = bounds(x, arms, trimmed);
    let corners = [(bd[0].0, bd[1].0), (bd[0].1, bd[1].0), (bd[0].1, bd[1].1), (bd[0].0, bd[1].1)].map(|(ua, ub)| corner_at(x, arms, ua, ub));
    let centre = scale(corners.iter().fold([0.0, 0.0], |s, &c| add(s, c)), 0.25);
    // Paved a little beyond the overlap (a quarter metre on every side) so the paving also covers the roads' edge lines where they open onto each other.
    let m = 0.25;
    let inflated = [(bd[0].0 - m, bd[1].0 - m), (bd[0].1 + m, bd[1].0 - m), (bd[0].1 + m, bd[1].1 + m), (bd[0].0 - m, bd[1].1 + m)].map(|(ua, ub)| corner_at(x, arms, ua, ub));
    out.polygon(&format!("{base}.box"), &inflated, flip, PAVEMENT);

    // Rounded kerb corners: at each corner the two edges leave the box, and the arc rounds the space between them.
    for (k, &c) in corners.iter().enumerate() {
        let sign_away = |t: P| if dot(t, sub(centre, c)) > 0.0 { -1.0 } else { 1.0 };
        let away = |t: P| scale(t, sign_away(t));
        // No kerb return where one of the two roads meeting at this corner has been trimmed away.
        if trimmed.contains(&(0, sign_away(a.t))) || trimmed.contains(&(1, sign_away(b.t))) {
            continue;
        }
        if cs.slip && slip_corner(out, base, k, c, away(a.t), away(b.t), cs.slip_radius) {
            continue;
        }
        if let Some((arc, fill)) = fillet(c, away(a.t), away(b.t), cs.radius()) {
            out.polygon(&format!("{base}.corner.{}", k + 1), &fill, flip, PAVEMENT);
            out.path(&format!("{base}.kerb.{}", k + 1), &[arc], flip, CURB, 0.3, false);
        }
    }

    // Who stops: signals everywhere; a two-way stop on the minor road (the narrower one unless named); a four-way stop on every approach.
    let minor = cs.minor.clone().unwrap_or_else(|| if a.hi - a.lo <= b.hi - b.lo { a.id.clone() } else { b.id.clone() });
    let mut notes = Vec::new();
    for (k, arm) in arms.iter().enumerate() {
        for sign in [-1.0, 1.0] {
            if trimmed.contains(&(k, sign)) {
                continue; // this arm has been trimmed away
            }
            let ctl = match cs.treatment {
                Treatment::Signal => Ctl::Signal,
                Treatment::FourWayStop => Ctl::Stop,
                Treatment::TwoWayStop if arm.id == minor => Ctl::Stop,
                _ => Ctl::None,
            };
            let ap = Approach { base, arm, sign, centre: x.p, ext: reach(&corners, x.p, scale(arm.t, sign)), ctl, crosswalks: cs.crosswalks, pocket: cs.pocket, pocket_length: cs.pocket_length, pocket_taper: cs.pocket_taper };
            notes.extend(approach(out, &ap));
        }
    }
    notes
}

/// Overpass (or underpass): the `over` road is redrawn on top across the other as a bridge deck, with parapets and abutments.
/// The other road is left running underneath. There is no conflict, so no crosswalks, stop bars or signals.
pub(crate) fn overpass(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) {
    let (a, b) = (&arms[0], &arms[1]);
    let corners = [(a.lo, b.lo), (a.hi, b.lo), (a.hi, b.hi), (a.lo, b.hi)].map(|(ua, ub)| corner_at(x, arms, ua, ub));
    let over = cs.over.clone().unwrap_or_else(|| a.id.clone());
    let (ov, path) = if b.id == over { (b, &x.path_b) } else { (a, &x.path_a) };
    let span = 4.0; // abutments reach this far beyond the road underneath
    let (before, after) = (reach(&corners, x.p, scale(ov.t, -1.0)) + span, reach(&corners, x.p, ov.t) + span);
    let poly = Poly::new(path.clone());
    let (s0, _, _) = poly.project(x.p);
    let (s_lo, s_hi) = ((s0 - before).max(0.0), (s0 + after).min(poly.len()));
    let steps = ((s_hi - s_lo) / 2.0).ceil().max(2.0) as usize;
    let deck: Vec<P> = (0..=steps).map(|i| poly.at(s_lo + (s_hi - s_lo) * i as f64 / steps as f64).0).collect();
    emit_road(out, &format!("{base}.deck"), &deck, &ov.placed);
    let dark = [0.2, 0.2, 0.22];
    for (k, off) in [ov.lo, ov.hi].into_iter().enumerate() {
        let edge = offset_polyline(&deck, off, MITRE_LIMIT);
        if edge.len() >= 2 {
            out.path(&format!("{base}.parapet.{}", k + 1), &[edge], flip, dark, 0.5, false);
        }
    }
    for (k, s) in [s_lo, s_hi].into_iter().enumerate() {
        let (p, t) = poly.at(s);
        let n = Poly::right(t);
        out.path(&format!("{base}.abutment.{}", k + 1), &[vec![add(p, scale(n, ov.lo)), add(p, scale(n, ov.hi))]], flip, dark, 0.6, false);
    }
}

/// A single-lane roundabout centred on the crossing: the roads end at the ring, with splitter islands, yield lines and (if asked) crosswalks.
/// Circulation is counter-clockwise (US).
fn roundabout(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) {
    let r_out = cs.diameter / 2.0;
    let ring = 5.5_f64.min(r_out * 0.3);
    let apron = 2.0;
    let r_island = (r_out - ring - apron).max(2.0);
    let circle = |r: f64| -> Vec<P> { (0..48).map(|k| { let t = std::f64::consts::TAU * k as f64 / 48.0; add(x.p, [r * t.cos(), r * t.sin()]) }).collect() };
    out.polygon(&format!("{base}.disc"), &circle(r_out + 0.3), flip, PAVEMENT);
    out.polygon(&format!("{base}.apron"), &circle(r_island + apron), flip, [0.80, 0.80, 0.77]);
    out.polygon(&format!("{base}.island"), &circle(r_island), flip, GRASS);
    out.path(&format!("{base}.kerb.island"), &[circle(r_island)], flip, CURB, 0.3, true);

    // The outer kerb runs between the four openings where the roads join the ring.
    let tau = std::f64::consts::TAU;
    let mut gaps: Vec<(f64, f64)> = Vec::new();
    let cw = 3.0;
    let ang = |dir: P, n: P, u: f64| {
        let phi = dir[1].atan2(dir[0]);
        let s = if cross2(dir, n) > 0.0 { 1.0 } else { -1.0 };
        (phi + s * (u / r_out).clamp(-0.99, 0.99).asin()).rem_euclid(tau)
    };
    for arm in arms {
        for sign in [-1.0, 1.0] {
            let dir = scale(arm.t, sign);
            let (g0, g1) = (ang(dir, arm.n, arm.lo), ang(dir, arm.n, arm.hi));
            // An opening is narrower than half the circle: order its ends the short way round.
            gaps.push(if (g1 - g0).rem_euclid(tau) < std::f64::consts::PI { (g0, g1) } else { (g1, g0) });
        }
    }
    gaps.sort_by(|p, q| p.0.total_cmp(&q.0));
    for i in 0..gaps.len() {
        let (from, mut to) = (gaps[i].1, gaps[(i + 1) % gaps.len()].0);
        if to <= from {
            to += tau;
        }
        let steps = (((to - from) / 0.1).ceil() as usize).max(2);
        let arc: Vec<P> = (0..=steps).map(|k| { let t = from + (to - from) * k as f64 / steps as f64; add(x.p, [r_out * t.cos(), r_out * t.sin()]) }).collect();
        out.path(&format!("{base}.kerb.{}", i + 1), &[arc], flip, CURB, 0.3, false);
    }

    // Each approach: a splitter island, a yield line across the inbound lanes, a crosswalk a car length back.
    for arm in arms {
        for sign in [-1.0, 1.0] {
            let dir = scale(arm.t, sign);
            let tag = format!("{}.{}", arm.id, if sign < 0.0 { "before" } else { "after" });
            let at = |along: f64, across: f64| add(add(x.p, scale(dir, along)), scale(arm.n, across));
            let island = vec![at(r_out - 1.0, -1.5), at(r_out + 24.0, 0.0), at(r_out - 1.0, 1.5)];
            out.polygon(&format!("{base}.splitter.{tag}"), &island, flip, GRASS);
            out.path(&format!("{base}.splitter.{tag}.kerb"), &[island], flip, CURB, 0.25, true);
            let want = if sign < 0.0 { Dir::Fwd } else { Dir::Back };
            let lanes: Vec<&Placed> = arm.placed.iter().filter(|e| travel(e) && e.dir == Some(want)).collect();
            if !lanes.is_empty() {
                let (lo, hi) = (lanes.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), lanes.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max));
                let dashes: Vec<Vec<P>> = (0..((hi - lo) / 0.9).floor() as usize).map(|i| { let u = lo + 0.2 + 0.9 * i as f64; vec![at(r_out + 0.8, u), at(r_out + 0.8, u + 0.5)] }).collect();
                out.path(&format!("{base}.yield.{tag}"), &dashes, flip, WHITE, 0.5, false);
            }
            if cs.crosswalks {
                let c = r_out + 7.5;
                let stripes: Vec<Vec<P>> = (0..((arm.hi - arm.lo) / 1.2).floor() as usize).map(|i| { let u = arm.lo + 0.6 + 1.2 * i as f64; vec![at(c - cw / 2.0, u), at(c + cw / 2.0, u)] }).collect();
                out.path(&format!("{base}.crosswalk.{tag}"), &stripes, flip, WHITE, 0.5, false);
            }
        }
    }
    // Counter-clockwise arrows around the ring.
    let r_mid = (r_out + r_island + apron) / 2.0;
    let arrows: Vec<Vec<P>> = (0..8)
        .map(|k| {
            let t = tau * (k as f64 + 0.5) / 8.0;
            let (radial, tangent) = ([t.cos(), t.sin()], [-t.sin(), t.cos()]);
            let p = add(x.p, scale(radial, r_mid));
            vec![add(sub(p, scale(tangent, 0.8)), scale(radial, 0.5)), add(p, scale(tangent, 0.8)), sub(sub(p, scale(tangent, 0.8)), scale(radial, 0.5))]
        })
        .collect();
    out.path(&format!("{base}.arrows"), &arrows, flip, WHITE, 0.2, false);
}
