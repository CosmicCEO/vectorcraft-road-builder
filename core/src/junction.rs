//! Junctions: a line named `junction.<id>` joins two road pieces. The road continues through it (same cross-section as the pieces it
//! touches), and its description adds side legs (each with its own cross-section), a paved box where they cross, rounded curb corners,
//! crosswalks, stop bars and a traffic signal or stop signs.
//!
//! Stored in the document's plug-in data under `junctions.<id>`:
//! `{"legs": [{"side": "left", "cross_section": "lane back | lane fwd", "length": 40}], "control": "signal", "crosswalks": true, "radius": 6}`
//! No legs means "match and continue": the line is just the road carried across the gap.

use crate::crosssection::{Anchor, Dir, Placed, format, layout, parse};
use crate::geom::{P, Poly, add, dot, scale};
use crate::library::{Library, Role};
use crate::output::Out;
use crate::roads::{CURB, DEFAULT_CROSS_SECTION, Spec, WHITE, emit, flip, heading, segment_id};
use crate::vcdoc::Item;
use serde_json::{Value, json};

pub const JUNCTION_PREFIX: &str = "junction.";
const PAVEMENT: [f64; 3] = [0.86, 0.86, 0.86];

/// The junction id of a junction line (`junction.a` is `a`; plain `junction` is `main`), or None if the name is not a junction.
pub fn junction_id(name: &str) -> Option<String> {
    if name == "junction" {
        return Some("main".into());
    }
    let id = name.strip_prefix(JUNCTION_PREFIX)?.split('.').next().unwrap_or("");
    Some(if id.is_empty() { "main".into() } else { id.into() })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Control {
    None,
    Signal,
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Leg {
    /// `left` or `right` as seen travelling along the connecting line, or a compass word (west, east, north, south).
    pub side: String,
    pub cross_section: String,
    pub length: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JunctionSpec {
    pub legs: Vec<Leg>,
    pub control: Control,
    pub crosswalks: bool,
    pub radius: f64,
}

impl Default for JunctionSpec {
    fn default() -> JunctionSpec {
        JunctionSpec { legs: vec![], control: Control::None, crosswalks: false, radius: 6.0 }
    }
}

/// `none`, `signal` (traffic lights) or `stop` (stop signs), with the usual spellings.
pub(crate) fn parse_control(word: &str) -> Result<Control, String> {
    match word.trim().to_ascii_lowercase().as_str() {
        "none" | "" | "uncontrolled" => Ok(Control::None),
        "signal" | "signals" | "traffic-light" | "traffic-lights" | "lights" => Ok(Control::Signal),
        "stop" | "stop-sign" | "stop-signs" => Ok(Control::Stop),
        other => Err(format!("control must be none, signal or stop (got \"{other}\")")),
    }
}

pub(crate) fn control_name(c: Control) -> &'static str {
    match c {
        Control::None => "none",
        Control::Signal => "signal",
        Control::Stop => "stop",
    }
}

const SIDES: [&str; 8] = ["left", "right", "west", "east", "north", "south", "w", "e"];

impl JunctionSpec {
    /// "match" (no legs), "tee" (one leg) or "cross" (two legs).
    pub fn kind(&self) -> &'static str {
        match self.legs.len() {
            0 => "match",
            1 => "tee",
            _ => "cross",
        }
    }

    /// Reads and checks a junction description (what an agent returns, or what is stored). Cross-sections are stored in canonical form.
    pub fn from_json(v: &Value, lib: &Library) -> Result<JunctionSpec, String> {
        let mut spec = JunctionSpec::default();
        if let Some(legs) = v["legs"].as_array() {
            if legs.len() > 2 {
                return Err("a junction has at most two legs (one on each side of the road)".into());
            }
            for (i, l) in legs.iter().enumerate() {
                let side = l["side"].as_str().unwrap_or("").trim().to_ascii_lowercase();
                if !SIDES.contains(&side.as_str()) {
                    return Err(format!("leg {}: side must be left, right, west, east, north or south (got \"{side}\")", i + 1));
                }
                let side = match side.as_str() {
                    "w" => "west".to_string(),
                    "e" => "east".to_string(),
                    _ => side,
                };
                let text = l["cross_section"].as_str().filter(|t| !t.trim().is_empty()).unwrap_or(DEFAULT_CROSS_SECTION);
                let parsed = parse(text, lib).map_err(|e| format!("leg {}: {e}", i + 1))?;
                let length = l["length"].as_f64().unwrap_or(40.0).clamp(5.0, 300.0);
                spec.legs.push(Leg { side, cross_section: format(&parsed), length });
            }
            if spec.legs.len() == 2 && spec.legs[0].side == spec.legs[1].side {
                return Err("both legs are on the same side: a junction has one leg on each side".into());
            }
        }
        spec.control = parse_control(v["control"].as_str().unwrap_or("none"))?;
        spec.crosswalks = v["crosswalks"].as_bool().unwrap_or(false);
        spec.radius = v["radius"].as_f64().unwrap_or(6.0).clamp(0.0, 20.0);
        Ok(spec)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "kind": self.kind(),
            "legs": self.legs.iter().map(|l| json!({"side": l.side, "cross_section": l.cross_section, "length": l.length})).collect::<Vec<_>>(),
            "control": control_name(self.control),
            "crosswalks": self.crosswalks,
            "radius": self.radius,
        })
    }
}

/// A frame at the junction centre: `at(u, v)` is `u` metres to the right of the connecting line and `v` metres along it.
struct Frame {
    m: P,
    t: P,
    n: P,
}

impl Frame {
    fn at(&self, u: f64, v: f64) -> P {
        add(add(self.m, scale(self.n, u)), scale(self.t, v))
    }
}

pub(crate) fn polygon(n: usize, centre: P, radius: f64) -> Vec<P> {
    (0..n).map(|k| { let a = std::f64::consts::TAU * (k as f64 + 0.5) / n as f64; [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()] }).collect()
}

pub(crate) fn travel(e: &Placed) -> bool {
    matches!(e.role, Role::Travel | Role::Pocket)
}

/// The nearest end of a road piece to `p` within 3 m: the segment id of that piece.
fn neighbour(ends: &[(String, P)], p: P) -> Option<String> {
    ends.iter().map(|(id, q)| (id, (q[0] - p[0]).hypot(q[1] - p[1]))).filter(|(_, d)| *d < 3.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(id, _)| id.clone())
}

/// Builds every `junction.*` line in `items`. Roads must already have been emitted (junctions are drawn over them). Returns notes for the user.
pub fn build(items: &[Item], spec: &Spec, out: &mut Out) -> Vec<String> {
    let mut notes = Vec::new();
    let lib = &spec.library;
    let ends: Vec<(String, P)> = items
        .iter()
        .filter_map(|it| Some((segment_id(&it.name)?, it.subs.first()?)))
        .flat_map(|(id, pts)| [pts.first().map(|&p| (id.clone(), flip(p))), pts.last().map(|&p| (id.clone(), flip(p)))])
        .flatten()
        .collect();

    for it in items {
        let Some(id) = junction_id(&it.name) else { continue };
        let Some(sub) = it.subs.first() else { continue };
        let path: Vec<P> = sub.iter().map(|&p| flip(p)).collect();
        let poly = Poly::new(path.clone());
        if poly.is_empty() {
            notes.push(format!("junction {id}: the line has no length, skipped"));
            continue;
        }
        let jspec = match spec.junction_value(&id) {
            Some(v) => JunctionSpec::from_json(&v, lib).unwrap_or_else(|e| {
                notes.push(format!("junction {id}: {e}; drawn as a plain connection"));
                JunctionSpec::default()
            }),
            None => JunctionSpec::default(),
        };

        // The road continues through: same cross-section as the pieces this line touches.
        let seg = neighbour(&ends, path[0]).or_else(|| neighbour(&ends, *path.last().unwrap_or(&path[0])));
        let (main_parsed, anchor, note) = spec.for_segment(seg.as_deref().unwrap_or("-"));
        notes.extend(note);
        let main = match layout(&main_parsed, lib, heading(&path), anchor) {
            Ok(p) => p,
            Err(e) => {
                notes.push(format!("junction {id}: {e}"));
                continue;
            }
        };
        let base = format!("gen.junction.{id}");
        emit(out, &format!("{base}.road"), &path, &main);
        if jspec.legs.is_empty() {
            continue;
        }

        let (mid, t) = poly.at(poly.len() / 2.0);
        let fr = Frame { m: mid, t, n: Poly::right(t) };
        let main_left = main.first().map_or(-5.0, |e| e.left);
        let main_right = main.last().map_or(5.0, |e| e.right);
        let travel_left = main.iter().filter(|e| travel(e)).map(|e| e.left).fold(f64::INFINITY, f64::min);
        let travel_right = main.iter().filter(|e| travel(e)).map(|e| e.right).fold(f64::NEG_INFINITY, f64::max);

        // Resolve each leg to a side of the connecting line and lay out its cross-section.
        struct Placement {
            su: f64,
            placed: Vec<Placed>,
            width: f64,
            length: f64,
        }
        let mut legs: Vec<Placement> = Vec::new();
        for (k, l) in jspec.legs.iter().enumerate() {
            let compass = match l.side.as_str() {
                "west" => Some([-1.0, 0.0]),
                "east" => Some([1.0, 0.0]),
                "north" => Some([0.0, 1.0]),
                "south" => Some([0.0, -1.0]),
                _ => None,
            };
            let su = match (l.side.as_str(), compass) {
                ("right", _) => 1.0,
                ("left", _) => -1.0,
                (_, Some(c)) if dot(fr.n, c) > 0.5 => 1.0,
                (_, Some(c)) if dot(fr.n, c) < -0.5 => -1.0,
                _ => {
                    notes.push(format!("junction {id}: the {} leg runs along the road, not across it; skipped", l.side));
                    continue;
                }
            };
            let axis = scale(fr.n, su);
            let parsed = match parse(&l.cross_section, lib) {
                Ok(p) => p,
                Err(e) => {
                    notes.push(format!("junction {id}, leg {}: {e}", k + 1));
                    continue;
                }
            };
            let Ok(placed) = layout(&parsed, lib, axis, Anchor::Auto) else { continue };
            let width = placed.last().map_or(0.0, |e| e.right) - placed.first().map_or(0.0, |e| e.left);
            legs.push(Placement { su, placed, width, length: l.length });
        }
        if legs.is_empty() {
            continue;
        }
        let has = |su: f64| legs.iter().any(|l| l.su == su);
        // The paved box spans the road from one leg's edge to the other's, or, where there is no leg, to the outer travel lane (kerb, bike lane and
        // verge on that side carry on across the junction).
        let box_l = if has(-1.0) { main_left - 0.2 } else { travel_left };
        let box_r = if has(1.0) { main_right + 0.2 } else { travel_right };
        let hw = legs.iter().map(|l| l.width / 2.0).fold(0.0, f64::max);

        // The legs, each from the edge of the road outwards.
        for (k, l) in legs.iter().enumerate() {
            let u_e = if l.su > 0.0 { main_right } else { main_left };
            let leg_path = vec![fr.at(u_e, 0.0), fr.at(u_e + l.su * l.length, 0.0)];
            emit(out, &format!("{base}.leg.{}", k + 1), &leg_path, &l.placed);
        }
        // The box covers the road's lines and median where the roads cross.
        out.polygon(&format!("{base}.box"), &[fr.at(box_l, -hw), fr.at(box_r, -hw), fr.at(box_r, hw), fr.at(box_l, hw)], flip, PAVEMENT);

        // Rounded kerb corners where each leg meets the road.
        for (k, l) in legs.iter().enumerate() {
            let (u_e, r) = (if l.su > 0.0 { main_right } else { main_left }, jspec.radius);
            let hwl = l.width / 2.0;
            if r <= 0.0 {
                continue;
            }
            for sv in [1.0, -1.0] {
                let arc: Vec<P> = (0..=12)
                    .map(|i| {
                        let a = std::f64::consts::FRAC_PI_2 * i as f64 / 12.0;
                        fr.at(u_e + l.su * r - l.su * r * a.cos(), sv * (hwl + r) - sv * r * a.sin())
                    })
                    .collect();
                let mut fill = vec![fr.at(u_e, sv * hwl)];
                fill.extend(arc.iter().copied());
                out.polygon(&format!("{base}.corner.{}.{}", k + 1, if sv > 0.0 { "f" } else { "b" }), &fill, flip, PAVEMENT);
                out.path(&format!("{base}.kerb.{}.{}", k + 1, if sv > 0.0 { "f" } else { "b" }), &[arc], flip, CURB, 0.3, false);
            }
        }

        // Crosswalks: across the road on both sides of the junction, and across each leg.
        let (gap, cw) = (1.0, 3.0);
        // The median ends short of the crosswalk (or a metre from the box when there is none).
        for sv in [-1.0, 1.0] {
            let at = |along: f64, across: f64| fr.at(across, sv * along);
            crate::crossing::cap_medians(out, &format!("{base}.cap.{}", if sv > 0.0 { "after" } else { "before" }), &main, hw - 0.3, hw + gap + if jspec.crosswalks { cw + 0.3 } else { 0.0 }, &at);
        }
        if jspec.crosswalks {
            let mut stripes_main: Vec<Vec<P>> = Vec::new();
            let width = main_right - main_left;
            for sv in [-1.0, 1.0] {
                let vc = sv * (hw + gap + cw / 2.0);
                for i in 0..(width / 1.2).floor() as usize {
                    let u = main_left + 0.6 + 1.2 * i as f64;
                    stripes_main.push(vec![fr.at(u, vc - cw / 2.0), fr.at(u, vc + cw / 2.0)]);
                }
            }
            out.path(&format!("{base}.crosswalk.road"), &stripes_main, flip, WHITE, 0.5, false);
            for (k, l) in legs.iter().enumerate() {
                let u_e = if l.su > 0.0 { main_right } else { main_left };
                let uc = u_e + l.su * (gap + cw / 2.0);
                let stripes: Vec<Vec<P>> = (0..(l.width / 1.2).floor() as usize)
                    .map(|i| {
                        let v = -l.width / 2.0 + 0.6 + 1.2 * i as f64;
                        vec![fr.at(uc - l.su * cw / 2.0, v), fr.at(uc + l.su * cw / 2.0, v)]
                    })
                    .collect();
                out.path(&format!("{base}.crosswalk.leg.{}", k + 1), &stripes, flip, WHITE, 0.5, false);
            }
        }

        // Stop bars and the signal heads or stop signs.
        if jspec.control != Control::None {
            let back = if jspec.crosswalks { cw + crate::standards::STOP_LINE_SETBACK_MIN + crate::standards::STOP_LINE_WIDTH / 2.0 } else { 0.6 };
            if jspec.control == Control::Signal {
                for (sv, want) in [(-1.0, Dir::Fwd), (1.0, Dir::Back)] {
                    let lanes: Vec<&Placed> = main.iter().filter(|e| travel(e) && e.dir == Some(want)).collect();
                    if lanes.is_empty() {
                        continue;
                    }
                    let v = sv * (hw + gap + back);
                    let (lo, hi) = (lanes.iter().map(|e| e.left).fold(f64::INFINITY, f64::min), lanes.iter().map(|e| e.right).fold(f64::NEG_INFINITY, f64::max));
                    out.path(&format!("{base}.stop.road.{}", if sv < 0.0 { "f" } else { "b" }), &[vec![fr.at(lo, v), fr.at(hi, v)]], flip, WHITE, 0.6, false);
                    // Signal head at the driver's right-hand kerb.
                    let (pole_u, pole_v) = if sv < 0.0 { (main_right + 1.3, v - 0.9) } else { (main_left - 1.3, v + 0.9) };
                    let c = fr.at(pole_u, pole_v);
                    out.polygon(&format!("{base}.signal.road.{}", if sv < 0.0 { "f" } else { "b" }), &polygon(4, c, 0.8), flip, [0.15, 0.15, 0.17]);
                    out.polygon(&format!("{base}.signal.road.{}.lamp", if sv < 0.0 { "f" } else { "b" }), &polygon(8, c, 0.28), flip, [0.2, 0.85, 0.3]);
                }
            }
            for (k, l) in legs.iter().enumerate() {
                // Inbound lanes run against the leg's outward direction.
                let lanes: Vec<&Placed> = l.placed.iter().filter(|e| travel(e) && e.dir == Some(Dir::Back)).collect();
                if lanes.is_empty() {
                    continue;
                }
                let u_e = if l.su > 0.0 { main_right } else { main_left };
                let ub = u_e + l.su * (gap + back);
                let to_v = |o: f64| -l.su * o;
                let vs: Vec<f64> = lanes.iter().flat_map(|e| [to_v(e.left), to_v(e.right)]).collect();
                let (v0, v1) = (vs.iter().cloned().fold(f64::INFINITY, f64::min), vs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
                out.path(&format!("{base}.stop.leg.{}", k + 1), &[vec![fr.at(ub, v0), fr.at(ub, v1)]], flip, WHITE, 0.6, false);
                // The sign or signal stands just beyond the inbound lanes on the driver's right-hand side, on the same side as the bar.
                let pole_v = if l.su > 0.0 { v1 + 0.9 } else { v0 - 0.9 };
                let c = fr.at(ub + l.su * 0.9, pole_v);
                if jspec.control == Control::Stop {
                    let radius = if lanes.len() >= 2 { crate::standards::STOP_SIGN_RADIUS_MULTI } else { crate::standards::STOP_SIGN_RADIUS_SINGLE };
                    out.polygon(&format!("{base}.sign.leg.{}", k + 1), &polygon(8, c, radius), flip, [0.8, 0.12, 0.12]);
                } else {
                    out.polygon(&format!("{base}.signal.leg.{}", k + 1), &polygon(4, c, 0.8), flip, [0.15, 0.15, 0.17]);
                    out.polygon(&format!("{base}.signal.leg.{}.lamp", k + 1), &polygon(8, c, 0.28), flip, [0.2, 0.85, 0.3]);
                }
            }
        }
    }
    notes
}
