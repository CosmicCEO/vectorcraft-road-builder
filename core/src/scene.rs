//! The road as drawn in VectorCraft, recovered from named paths.
//!
//! Naming contract (the owner edits these paths; everything else is generated):
//! - `centerline`: the road's centre path; route metre `s` counts along it from its first anchor.
//! - `profile.zero`: baseline of the profile band; its first anchor is (s = 0, offset = 0).
//! - `profile.<key>`: a lane or edge line drawn in (s, offset) space: x = s, and the height above the baseline = offset (right of northbound is up).
//! - `flag.<name>.<n>`: a horizontal bar whose x-extent is a stretch of road where the flag is on.
//! - `<L><n>.<kind>.<side>[.<label>]` (e.g. `J7.signal.right.School`): a junction marker placed on the centerline (side is left, right or both).
//! - `arrow.<kind>.<N|S>.<s>`: a pavement arrow glyph (N = painted for northbound travel; `<s>` is its approximate route metre, a hint that keeps it
//!   on the right stretch on the inside of a bend). Its first two anchors are tail and tip, and its position is their midpoint.

use crate::geom::{Curve, P, Poly, bounds_center};
use crate::vcdoc::Item;
use std::collections::BTreeMap;

pub struct Junction {
    pub id: String,
    pub s: f64,
    pub kind: String,
    pub side: String,
    pub label: String,
}

pub struct Arrow {
    pub s: f64,
    pub off: f64,
    pub kind: String,
    pub south: bool,
}

pub struct Scene {
    /// Document coordinates of the centerline's first anchor (the route origin).
    pub origin: P,
    pub cl: Poly,
    pub curves: BTreeMap<String, Curve>,
    pub flags: BTreeMap<String, Vec<P>>,
    pub junctions: Vec<Junction>,
    pub arrows: Vec<Arrow>,
}

/// A junction marker's first name segment is one capital letter and a number (`J7`, `C12`).
pub fn is_junction_name(name: &str) -> bool {
    let id = name.split('.').next().unwrap_or("");
    id.len() > 1 && id.starts_with(|c: char| c.is_ascii_uppercase()) && id[1..].chars().all(|c| c.is_ascii_digit())
}

impl Scene {
    /// Document point -> world metres from the route origin (x east, y north).
    pub fn to_world(&self, d: P) -> P {
        [d[0] - self.origin[0], self.origin[1] - d[1]]
    }

    /// World metres -> document point.
    pub fn to_doc(&self, w: P) -> P {
        [self.origin[0] + w[0], self.origin[1] - w[1]]
    }

    pub fn from_items(items: &[Item]) -> Result<Scene, String> {
        let first = |name: &str| items.iter().find(|i| i.name == name && !i.subs.is_empty());
        let cl_item = first("centerline").ok_or("No path named \"centerline\" found. Use Select All before Build so the road is included.")?;
        let origin = *cl_item.subs[0].first().ok_or("The centerline path is empty.")?;
        let mut scene = Scene {
            origin,
            cl: Poly::new(vec![]),
            curves: BTreeMap::new(),
            flags: BTreeMap::new(),
            junctions: Vec::new(),
            arrows: Vec::new(),
        };
        scene.cl = Poly::new(cl_item.subs[0].iter().map(|&p| scene.to_world(p)).collect());
        if scene.cl.is_empty() {
            return Err("The centerline needs at least two distinct points.".into());
        }

        let zero = first("profile.zero").and_then(|i| i.subs[0].first().copied());
        for it in items {
            let parts: Vec<&str> = it.name.split('.').collect();
            let Some(sub) = it.subs.first() else { continue };
            match parts.as_slice() {
                ["profile", "zero"] => {}
                ["profile", key] => {
                    let z = zero.ok_or("Found profile curves but no \"profile.zero\" baseline.")?;
                    scene.curves.insert((*key).to_string(), Curve::new(sub.iter().map(|p| [p[0] - z[0], z[1] - p[1]]).collect()));
                }
                ["flag", name, ..] => {
                    let z = zero.ok_or("Found flag bars but no \"profile.zero\" baseline.")?;
                    for s in &it.subs {
                        let lo = s.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min) - z[0];
                        let hi = s.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max) - z[0];
                        scene.flags.entry((*name).to_string()).or_default().push([lo, hi]);
                    }
                }
                ["arrow", kind, dir, ..] => {
                    // The glyph's first two anchors are its tail and tip; its position is their midpoint (a bounding box would drift when the glyph rotates).
                    let centre = if sub.len() >= 2 { Some([(sub[0][0] + sub[1][0]) / 2.0, (sub[0][1] + sub[1][1]) / 2.0]) } else { bounds_center(sub) };
                    if let Some(c) = centre {
                        let at = scene.to_world(c);
                        let hint = parts.get(3).and_then(|h| h.parse::<f64>().ok());
                        let (s, off, _) = hint.map_or_else(|| scene.cl.project(at), |h| scene.cl.project_hinted(at, h));
                        scene.arrows.push(Arrow { s, off, kind: (*kind).to_string(), south: *dir == "S" });
                    }
                }
                _ if is_junction_name(&it.name) => {
                    if let Some(c) = bounds_center(sub) {
                        let (s, _, _) = scene.cl.project(scene.to_world(c));
                        scene.junctions.push(Junction {
                            id: parts[0].to_string(),
                            s,
                            kind: parts.get(1).unwrap_or(&"tee").to_string(),
                            side: parts.get(2).unwrap_or(&"right").to_string(),
                            label: parts.get(3..).map(|r| r.join(".")).unwrap_or_default(),
                        });
                    }
                }
                _ => {}
            }
        }
        scene.junctions.sort_by(|a, b| a.s.total_cmp(&b.s));
        scene.arrows.sort_by(|a, b| a.s.total_cmp(&b.s).then(a.off.total_cmp(&b.off)));
        for v in scene.flags.values_mut() {
            v.sort_by(|a, b| a[0].total_cmp(&b[0]));
        }
        Ok(scene)
    }

    pub fn flag_on(&self, name: &str, s: f64) -> bool {
        self.flags.get(name).is_some_and(|v| v.iter().any(|r| s >= r[0] && s <= r[1]))
    }
}
