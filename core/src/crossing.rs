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
const GRASS: [f64; 3] = [0.72, 0.85, 0.62];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Treatment {
    Uncontrolled,
    Signal,
    TwoWayStop,
    FourWayStop,
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
            Treatment::Overpass => "overpass",
            Treatment::Roundabout => "roundabout",
            Treatment::Cloverleaf => "cloverleaf",
        }
    }

    /// Reads the usual spellings ("lighted intersection", "2 way stop", "underpass", "traffic circle" ...).
    pub fn parse(word: &str) -> Result<Treatment, String> {
        let w = word.trim().to_ascii_lowercase().replace([' ', '_'], "-");
        Ok(match w.as_str() {
            "" | "none" | "uncontrolled" | "no-control" => Treatment::Uncontrolled,
            "signal" | "signals" | "signalised" | "signalized" | "lighted" | "lighted-intersection" | "traffic-light" | "traffic-lights" | "lights" => Treatment::Signal,
            "two-way-stop" | "2-way-stop" | "stop" | "stop-signs" | "minor-stop" => Treatment::TwoWayStop,
            "four-way-stop" | "4-way-stop" | "all-way-stop" | "all-stop" => Treatment::FourWayStop,
            "overpass" | "underpass" | "bridge" | "grade-separated" | "flyover" => Treatment::Overpass,
            "roundabout" | "round-about" | "traffic-circle" | "rotary" => Treatment::Roundabout,
            "cloverleaf" | "interchange" | "clover-leaf" => Treatment::Cloverleaf,
            other => return Err(format!("treatment must be signal, two-way-stop, four-way-stop, uncontrolled, overpass, roundabout or cloverleaf (got \"{other}\")")),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CrossingSpec {
    pub roads: [String; 2],
    pub treatment: Treatment,
    pub crosswalks: bool,
    pub radius: f64,
    /// For a two-way stop: the road that stops. Left out, the narrower road stops.
    pub minor: Option<String>,
    /// For an overpass: the road that goes over the other. Left out, the first road.
    pub over: Option<String>,
    /// For a roundabout: the outer diameter in metres.
    pub diameter: f64,
    /// What to do with a short stub of road past the crossing.
    pub stub: Stub,
    /// A side of a road shorter than this many metres, measured from the crossing centre, is a stub.
    pub stub_length: f64,
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
        let (minor, over) = (pick("minor")?, pick("over")?);
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
            radius: v["radius"].as_f64().unwrap_or(6.0).clamp(0.0, 20.0),
            minor,
            over,
            diameter: v["diameter"].as_f64().unwrap_or(40.0).clamp(20.0, 120.0),
            stub: match v["stub"].as_str().map(|w| w.trim().to_ascii_lowercase().replace([' ', '_'], "-")).as_deref() {
                None | Some("") | Some("keep") | Some("dead-end") | Some("deadend") | Some("4-way") | Some("four-way") => Stub::Keep,
                Some("trim") | Some("cut") | Some("3-way") | Some("three-way") | Some("tee") | Some("t") => Stub::Trim,
                Some(other) => return Err(format!("stub must be keep or trim (got \"{other}\")")),
            },
            stub_length: v["stub_length"].as_f64().unwrap_or(25.0).clamp(5.0, 100.0),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({"roads": self.roads, "treatment": self.treatment.name(), "crosswalks": self.crosswalks, "radius": self.radius, "minor": self.minor, "over": self.over, "diameter": self.diameter, "stub": if self.stub == Stub::Trim { "trim" } else { "keep" }, "stub_length": self.stub_length})
    }

    pub fn summary(&self) -> String {
        let [a, b] = &self.roads;
        let what = match self.treatment {
            Treatment::Uncontrolled => "no control".to_string(),
            Treatment::Signal => "traffic signals".to_string(),
            Treatment::TwoWayStop => format!("two-way stop, {}", self.minor.as_ref().map_or("the narrower road stops".to_string(), |m| format!("road {m} stops"))),
            Treatment::FourWayStop => "four-way stop".to_string(),
            Treatment::Overpass => {
                let over = self.over.as_ref().unwrap_or(a);
                format!("road {over} passes over road {}", if over == a { b } else { a })
            }
            Treatment::Roundabout => format!("roundabout, {} m across", self.diameter),
            Treatment::Cloverleaf => "cloverleaf interchange (not built yet)".to_string(),
        };
        let walks = if self.crosswalks && !matches!(self.treatment, Treatment::Overpass | Treatment::Cloverleaf) { ", crosswalks" } else { "" };
        let stub = if self.stub == Stub::Trim && !matches!(self.treatment, Treatment::Overpass | Treatment::Roundabout | Treatment::Cloverleaf) { ", a short stub is trimmed (three-way)" } else { "" };
        format!("roads {a} and {b}: {what}{walks}{stub}")
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
}

fn cross2(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn unit(v: P) -> P {
    let m = v[0].hypot(v[1]).max(1e-12);
    [v[0] / m, v[1] / m]
}

fn pieces(items: &[Item], road: &str) -> Vec<(usize, Vec<P>)> {
    items.iter().enumerate().filter(|(_, i)| segment_id(&i.name).as_deref() == Some(road)).filter_map(|(k, i)| i.subs.first().map(|s| (k, s.iter().map(|&p| flip(p)).collect()))).collect()
}

/// The first place road `a` crosses road `b` (the lines as drawn, curves already flattened), if they cross at all.
pub fn find(items: &[Item], a: &str, b: &str) -> Option<Crossing> {
    for (ia, pa) in pieces(items, a) {
        for (ib, pb) in pieces(items, b) {
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
                        return Some(Crossing { p: add(wa[0], scale(r, t)), ta: unit(r), tb: unit(s), path_a: pa.clone(), path_b: pb.clone(), index_a: ia, index_b: ib });
                    }
                }
            }
        }
    }
    None
}

/// One road at the crossing: its direction, its sideways frame and the elements laid out across it.
struct Arm {
    id: String,
    t: P,
    n: P,
    lo: f64,
    hi: f64,
    placed: Vec<Placed>,
}

/// A kerb return between two edges that meet at corner `c`, leaving along `d1` and `d2` (unit vectors): the arc and the paved fill behind it.
fn fillet(c: P, d1: P, d2: P, r: f64) -> Option<(Vec<P>, Vec<P>)> {
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
    let arc: Vec<P> = (0..=12).map(|i| { let a = a1 + delta * i as f64 / 12.0; [centre[0] + r * a.cos(), centre[1] + r * a.sin()] }).collect();
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
                    Ok(Some(n)) => notes.push(format!("intersection {id}: {n}")),
                    Ok(None) => {}
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

fn at_grade_treatment(t: Treatment) -> bool {
    matches!(t, Treatment::Uncontrolled | Treatment::Signal | Treatment::TwoWayStop | Treatment::FourWayStop)
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

/// Where each road's paved band ends, as `[(lo, hi); 2]` per arm. Normally the road's whole width; where the *other* road is trimmed to a
/// stub-free end, the paving stops at this road's outer travel lane so its far kerb, bike lane and verge carry on (a three-way keeps them).
fn bounds(x: &Crossing, arms: &[Arm], trimmed: &[(usize, f64)]) -> [(f64, f64); 2] {
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
}

/// The cuts the stored crossings ask for, to be applied to the roads before they are drawn (see `clip`).
pub fn trims(items: &[Item], spec: &Spec) -> Vec<Trim> {
    let mut out = Vec::new();
    for (_, v) in spec.crossings() {
        let Ok(cs) = CrossingSpec::from_json(&v) else { continue };
        if cs.stub != Stub::Trim || !at_grade_treatment(cs.treatment) {
            continue;
        }
        let Some(x) = find(items, &cs.roads[0], &cs.roads[1]) else { continue };
        let Ok(arms) = make_arms(spec, &cs, &x) else { continue };
        let sides = stub_sides(&x, &cs);
        let b = bounds(&x, &arms, &sides);
        let corners = [(b[0].0, b[1].0), (b[0].1, b[1].0), (b[0].1, b[1].1), (b[0].0, b[1].1)].map(|(ua, ub)| corner_at(&x, &arms, ua, ub));
        for &(k, sign) in &sides {
            let dir = scale(if k == 0 { x.ta } else { x.tb }, sign);
            let mut reach_of: Vec<f64> = corners.iter().map(|&c| dot(dir, sub(c, x.p))).collect();
            reach_of.sort_by(|p, q| q.total_cmp(p));
            // The nearer of the two far corners: everything of the stub up to there lies on the other road's pavement or in the box.
            out.push(Trim { index: if k == 0 { x.index_a } else { x.index_b }, p: x.p, sign, dist: reach_of[1].max(0.0) });
        }
    }
    out
}

/// The road line with the part a trim drops cut off (the line itself in the document is not touched).
pub(crate) fn clip(path: &[P], t: &Trim) -> Vec<P> {
    let poly = Poly::new(path.to_vec());
    let (s0, _, _) = poly.project(t.p);
    let len = poly.len();
    let cut = (s0 + t.sign * t.dist).clamp(0.0, len);
    let mut cum = vec![0.0];
    for w in path.windows(2) {
        cum.push(cum.last().copied().unwrap_or(0.0) + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    let mut kept: Vec<P> = Vec::new();
    if t.sign > 0.0 {
        kept.extend(path.iter().zip(&cum).filter(|(_, c)| **c < cut - 1e-6).map(|(p, _)| *p));
        kept.push(poly.at(cut).0);
    } else {
        kept.push(poly.at(cut).0);
        kept.extend(path.iter().zip(&cum).filter(|(_, c)| **c > cut + 1e-6).map(|(p, _)| *p));
    }
    if kept.len() < 2 { path.to_vec() } else { kept }
}

fn emit(items: &[Item], spec: &Spec, id: &str, cs: &CrossingSpec, out: &mut Out) -> Result<Option<String>, String> {
    let x = find(items, &cs.roads[0], &cs.roads[1]).ok_or_else(|| format!("roads {} and {} do not cross", cs.roads[0], cs.roads[1]))?;
    let arms = make_arms(spec, cs, &x)?;
    let base = format!("gen.crossing.{id}");
    let mut note = None;
    match cs.treatment {
        Treatment::Overpass => overpass(out, &base, cs, &x, &arms),
        Treatment::Roundabout => roundabout(out, &base, cs, &x, &arms),
        Treatment::Cloverleaf => return Err("a cloverleaf interchange is not built yet; use an overpass for now".into()),
        _ => {
            let stubs = stub_sides(&x, cs);
            let trimmed: &[(usize, f64)] = if cs.stub == Stub::Trim { &stubs } else { &[] };
            if cs.stub == Stub::Trim && stubs.is_empty() {
                note = Some(format!("no stub to trim: both roads run more than {} m past the crossing", cs.stub_length));
            }
            at_grade(out, &base, cs, &x, &arms, trimmed);
            // A stub that is kept ends in a kerb across its end: a dead end.
            if cs.stub == Stub::Keep {
                for &(k, sign) in &stubs {
                    let path = if k == 0 { &x.path_a } else { &x.path_b };
                    let poly = Poly::new(path.clone());
                    let (p, t) = poly.at(if sign > 0.0 { poly.len() } else { 0.0 });
                    let n = Poly::right(t);
                    let tag = format!("{}.{}", arms[k].id, if sign < 0.0 { "before" } else { "after" });
                    out.path(&format!("{base}.deadend.{tag}"), &[vec![add(p, scale(n, arms[k].lo)), add(p, scale(n, arms[k].hi))]], flip, CURB, 0.3, false);
                }
            }
        }
    }
    Ok(note)
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
fn corner_at(x: &Crossing, arms: &[Arm], ua: f64, ub: f64) -> P {
    let (a, b) = (&arms[0], &arms[1]);
    let det = cross2(a.n, b.n);
    add(x.p, [(ua * b.n[1] - ub * a.n[1]) / det, (a.n[0] * ub - b.n[0] * ua) / det])
}

/// How far the overlap reaches from the crossing point along `dir`.
fn reach(corners: &[P; 4], p: P, dir: P) -> f64 {
    corners.iter().map(|&c| dot(dir, sub(c, p))).fold(f64::NEG_INFINITY, f64::max).max(0.0)
}

/// Signals, stop signs or nothing: pave the overlap, round the corners, mark all four approaches.
fn at_grade(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm], trimmed: &[(usize, f64)]) {
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
        if let Some((arc, fill)) = fillet(c, away(a.t), away(b.t), cs.radius) {
            out.polygon(&format!("{base}.corner.{}", k + 1), &fill, flip, PAVEMENT);
            out.path(&format!("{base}.kerb.{}", k + 1), &[arc], flip, CURB, 0.3, false);
        }
    }

    // Who stops: signals everywhere; a two-way stop on the minor road (the narrower one unless named); a four-way stop on every approach.
    let minor = cs.minor.clone().unwrap_or_else(|| if a.hi - a.lo <= b.hi - b.lo { a.id.clone() } else { b.id.clone() });
    let (gap, cw) = (1.0, 3.0);
    let back = if cs.crosswalks { cw + 0.9 } else { 0.6 };
    for (k, arm) in arms.iter().enumerate() {
        for sign in [-1.0, 1.0] {
            if trimmed.contains(&(k, sign)) {
                continue; // this arm has been trimmed away
            }
            let dir = scale(arm.t, sign);
            let ext = reach(&corners, x.p, dir);
            let tag = format!("{}.{}", arm.id, if sign < 0.0 { "before" } else { "after" });
            let at = |along: f64, across: f64| add(add(x.p, scale(dir, along)), scale(arm.n, across));
            cap_medians(out, &format!("{base}.cap.{tag}"), &arm.placed, 0.0, ext + gap + if cs.crosswalks { cw + 0.3 } else { 0.0 }, &at);
            if cs.crosswalks {
                let centre = ext + gap + cw / 2.0;
                let stripes: Vec<Vec<P>> = (0..((arm.hi - arm.lo) / 1.2).floor() as usize)
                    .map(|i| { let u = arm.lo + 0.6 + 1.2 * i as f64; vec![at(centre - cw / 2.0, u), at(centre + cw / 2.0, u)] })
                    .collect();
                out.path(&format!("{base}.crosswalk.{tag}"), &stripes, flip, WHITE, 0.5, false);
            }
            let controlled = match cs.treatment {
                Treatment::Signal | Treatment::FourWayStop => true,
                Treatment::TwoWayStop => arm.id == minor,
                _ => false,
            };
            if !controlled {
                continue;
            }
            // Traffic enters from this side if it heads toward the crossing: before the crossing that is the direction the road was drawn (fwd).
            let want = if sign < 0.0 { Dir::Fwd } else { Dir::Back };
            let lanes: Vec<&Placed> = arm.placed.iter().filter(|e| travel(e) && e.dir == Some(want)).collect();
            if lanes.is_empty() {
                continue;
            }
            let (lo, hi) = (lanes.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), lanes.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max));
            let dist = ext + gap + back;
            out.path(&format!("{base}.stop.{tag}"), &[vec![at(dist, lo), at(dist, hi)]], flip, WHITE, 0.6, false);
            // The signal head or sign stands just past the bar, beside the driver's right-hand kerb.
            let heading_in = scale(dir, -1.0);
            let right_of_driver = [heading_in[1], -heading_in[0]];
            let pole = if dot(right_of_driver, arm.n) > 0.0 { hi + 0.9 } else { lo - 0.9 };
            let c = at(dist + 0.9, pole);
            if cs.treatment == Treatment::Signal {
                out.polygon(&format!("{base}.signal.{tag}"), &polygon(4, c, 0.8), flip, [0.15, 0.15, 0.17]);
                out.polygon(&format!("{base}.signal.{tag}.lamp"), &polygon(8, c, 0.28), flip, [0.2, 0.85, 0.3]);
            } else {
                out.polygon(&format!("{base}.sign.{tag}"), &polygon(8, c, 0.75), flip, [0.8, 0.12, 0.12]);
            }
        }
    }
}

/// Overpass (or underpass): the `over` road is redrawn on top across the other as a bridge deck, with parapets and abutments.
/// The other road is left running underneath. There is no conflict, so no crosswalks, stop bars or signals.
fn overpass(out: &mut Out, base: &str, cs: &CrossingSpec, x: &Crossing, arms: &[Arm]) {
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
