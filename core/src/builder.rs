//! "Build Road": regenerates the lane lines, edges and junction pieces from the centerline, profile curves, flags and junction markers.
//! Everything it creates is named `gen.*`, so a rebuild replaces the previous result and never touches the paths the owner edits.

use crate::geom::{P, Poly, add, scale};
use crate::scene::Scene;
use crate::vcdoc::items_from_filter_input;
use serde_json::Value;
use std::fmt::Write;

pub const GEN_PREFIX: &str = "gen.";

/// (profile key, colour, stroke width, dash on/off in metres or None for solid)
const STYLE: [(&str, [f64; 3], f64, Option<(f64, f64)>); 12] = [
    ("sbEdge", [0.13, 0.13, 0.13], 0.3, None),
    ("nbEdge", [0.13, 0.13, 0.13], 0.3, None),
    ("sbOuter", [0.35, 0.35, 0.35], 0.15, None),
    ("nbOuter", [0.35, 0.35, 0.35], 0.15, None),
    ("sbBuf", [0.55, 0.55, 0.55], 0.12, Some((2.0, 2.0))),
    ("nbBuf", [0.55, 0.55, 0.55], 0.12, Some((2.0, 2.0))),
    ("sbLane", [0.2, 0.2, 0.2], 0.2, Some((3.0, 9.0))),
    ("nbLane", [0.2, 0.2, 0.2], 0.2, Some((3.0, 9.0))),
    ("sbInner", [0.8, 0.6, 0.0], 0.25, None),
    ("nbInner", [0.8, 0.6, 0.0], 0.25, None),
    ("sbTurn", [0.2, 0.2, 0.2], 0.2, Some((3.0, 3.0))),
    ("nbTurn", [0.2, 0.2, 0.2], 0.2, Some((3.0, 3.0))),
];

/// Is the line `key` painted at route metre `s`? Median openings hide the median edges; driveway mouths hide the right edge.
fn painted(sc: &Scene, key: &str, s: f64) -> bool {
    match key {
        "sbInner" | "nbInner" | "sbTurn" | "nbTurn" => !sc.flag_on("gap", s),
        "nbEdge" | "nbOuter" | "nbBuf" => !sc.flag_on("mouth", s),
        _ => true,
    }
}

/// Drops points that lie within `tol` metres of the straight line between their kept neighbours (iterative Douglas-Peucker).
fn simplify(pts: &[P], tol: f64) -> Vec<P> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (p, q) = (pts[a], pts[b]);
        let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
        let len = dx.hypot(dy).max(1e-9);
        let (mut far, mut at) = (0.0, a);
        for (i, r) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = ((r[0] - p[0]) * dy - (r[1] - p[1]) * dx).abs() / len;
            if d > far {
                (far, at) = (d, i);
            }
        }
        if far > tol {
            keep[at] = true;
            stack.push((a, at));
            stack.push((at, b));
        }
    }
    pts.iter().zip(keep).filter_map(|(p, k)| k.then_some(*p)).collect()
}

/// The line `key` as separate world-space runs (broken wherever it is not painted; dashed lines get one run per dash).
/// Kept cheap on purpose: plug-ins run under a fixed instruction budget.
fn runs(sc: &Scene, key: &str, dash: Option<(f64, f64)>) -> Vec<Vec<P>> {
    let Some(curve) = sc.curves.get(key) else { return vec![] };
    let total = sc.cl.len();
    let point = |s: f64| curve.at(s).map(|off| sc.cl.offset_point(s, off).0);
    let mut out: Vec<Vec<P>> = Vec::new();
    match dash {
        Some((on, off)) => {
            let mut a = 0.0_f64;
            while a < total {
                let b = (a + on).min(total);
                let mid = (a + b) / 2.0;
                if painted(sc, key, a) && painted(sc, key, b) {
                    if let (Some(p), Some(m), Some(q)) = (point(a), point(mid), point(b)) {
                        out.push(simplify(&[p, m, q], 0.02));
                    }
                }
                a += on + off;
            }
        }
        None => {
            let mut run: Vec<P> = Vec::new();
            let mut s = 0.0_f64;
            loop {
                let here = s.min(total);
                match (painted(sc, key, here), point(here)) {
                    (true, Some(p)) => run.push(p),
                    _ => {
                        if run.len() >= 2 {
                            out.push(simplify(&std::mem::take(&mut run), 0.02));
                        }
                        run.clear();
                    }
                }
                if here >= total {
                    break;
                }
                s += 2.0;
            }
            if run.len() >= 2 {
                out.push(simplify(&run, 0.02));
            }
        }
    }
    out
}

fn side_edge(sc: &Scene, side: f64, s: f64) -> f64 {
    let keys: [&str; 2] = if side > 0.0 { ["nbEdge", "nbOuter"] } else { ["sbEdge", "sbOuter"] };
    keys.iter().find_map(|k| sc.curves.get(*k).and_then(|c| c.at(s))).unwrap_or(if side > 0.0 { 3.6 } else { -17.2 })
}

/// Output text under construction: written directly (not as a JSON tree) because plug-ins run in an interpreter under an instruction budget.
struct Out {
    text: String,
    first: bool,
}

impl Out {
    fn new() -> Out {
        Out { text: String::from("{\"objects\":["), first: true }
    }

    fn sep(&mut self) {
        if !self.first {
            self.text.push(',');
        }
        self.first = false;
    }

    fn keep(&mut self, id: i64) {
        self.sep();
        let _ = write!(self.text, "{{\"id\":{id}}}");
    }

    /// A new stroked path object; `map` converts each world point to a document point.
    fn path(&mut self, name: &str, subs: &[Vec<P>], map: impl Fn(P) -> P, rgb: [f64; 3], width: f64) {
        self.sep();
        self.text.push_str("{\"name\":");
        self.text.push_str(&serde_json::to_string(name).unwrap_or_default());
        self.text.push_str(",\"path\":{\"subpaths\":[");
        for (i, sub) in subs.iter().enumerate() {
            if i > 0 {
                self.text.push(',');
            }
            self.text.push_str("{\"anchors\":[");
            for (k, &w) in sub.iter().enumerate() {
                let p = map(w);
                if k > 0 {
                    self.text.push(',');
                }
                self.text.push_str("{\"p\":[");
                num(&mut self.text, p[0]);
                self.text.push(',');
                num(&mut self.text, p[1]);
                self.text.push_str("]}");
            }
            self.text.push_str("],\"closed\":false}");
        }
        self.text.push_str("]},\"strokes\":[{\"paint\":{\"type\":\"solid\",\"color\":{\"model\":\"rgb\",\"r\":");
        num(&mut self.text, rgb[0]);
        self.text.push_str(",\"g\":");
        num(&mut self.text, rgb[1]);
        self.text.push_str(",\"b\":");
        num(&mut self.text, rgb[2]);
        self.text.push_str("}},\"width\":");
        num(&mut self.text, width);
        self.text.push_str(",\"cap\":\"butt\",\"join\":\"miter\",\"miterLimit\":4}]}");
    }

    fn finish(mut self) -> String {
        self.text.push_str("]}");
        self.text
    }
}

/// A number to the nearest millimetre, without float-formatting machinery.
fn num(out: &mut String, x: f64) {
    let v = (x * 1000.0).round() as i64;
    if v < 0 {
        out.push('-');
    }
    let v = v.unsigned_abs();
    let _ = write!(out, "{}", v / 1000);
    let frac = v % 1000;
    if frac != 0 {
        let digits = format!("{frac:03}");
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
}

/// Filter output text: every source object kept by id, the old `gen.*` paths dropped, fresh `gen.*` paths added.
pub fn build_text(input: &Value, params: &Value) -> Result<String, String> {
    let items = items_from_filter_input(input);
    let sc = Scene::from_items(&items)?;
    let stub_len = params["stubLength"].as_f64().unwrap_or(28.0);
    let stub_w = params["stubWidth"].as_f64().unwrap_or(8.0);
    let to_doc = |p: P| sc.to_doc(p);

    let mut out = Out::new();
    for it in items.iter().filter(|i| !i.name.starts_with(GEN_PREFIX)) {
        if let Some(id) = it.id {
            out.keep(id);
        }
    }

    for (key, rgb, width, dash) in STYLE {
        let r = runs(&sc, key, dash);
        if !r.is_empty() {
            out.path(&format!("{GEN_PREFIX}line.{key}"), &r, to_doc, rgb, width);
        }
    }

    for j in &sc.junctions {
        let (q, t) = sc.cl.at(j.s);
        let n = Poly::right(t);
        let sides: &[f64] = match j.side.as_str() {
            "left" => &[-1.0],
            "both" => &[1.0, -1.0],
            _ => &[1.0],
        };
        for &side in sides {
            let base = add(q, scale(n, side_edge(&sc, side, j.s)));
            let axis = scale(n, side);
            let along = |d: f64, w: f64| add(add(base, scale(axis, d)), scale(t, w));
            let hw = stub_w / 2.0;
            let tag = if sides.len() > 1 { format!("{}{}", j.id, if side > 0.0 { "r" } else { "l" }) } else { j.id.clone() };
            let edges = vec![vec![along(0.0, -hw), along(stub_len, -hw)], vec![along(0.0, hw), along(stub_len, hw)]];
            out.path(&format!("{GEN_PREFIX}stub.{tag}"), &edges, to_doc, [0.13, 0.13, 0.13], 0.3);
            let stripes: Vec<Vec<P>> = (0..5).map(|k| { let w = -hw + 0.8 + 1.6 * k as f64; vec![along(2.0, w), along(5.0, w)] }).collect();
            out.path(&format!("{GEN_PREFIX}zebra.{tag}"), &stripes, to_doc, [0.9, 0.9, 0.9], 0.6);
            out.path(&format!("{GEN_PREFIX}stop.{tag}"), &[vec![along(7.0, -hw), along(7.0, hw)]], to_doc, [0.95, 0.95, 0.95], 0.6);
        }
    }
    Ok(out.finish())
}

/// The same as a JSON value (for tests and callers that want one).
pub fn build(input: &Value, params: &Value) -> Result<Value, String> {
    serde_json::from_str(&build_text(input, params)?).map_err(|e| e.to_string())
}
