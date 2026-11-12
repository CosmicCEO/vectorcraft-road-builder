//! The `.pmap` road file: a compact, diff-friendly JSON description of a road in metres from its start.
//!
//! `import` turns a pmap into VectorCraft paths (named per the contract in `scene.rs`); `from_scene` + `to_json` do the reverse.

use crate::geom::{P, Poly};
use crate::scene::{Arrow, Junction, Scene};
use crate::vcdoc::object;
use serde_json::Value;
use std::collections::BTreeMap;

pub const FORMAT: &str = "vectorcraft-road-builder/pmap";
/// Height (document points) of the profile baseline below the route origin.
pub const PROFILE_Y: f64 = 300.0;

pub struct PMap {
    pub centerline: Vec<P>,
    pub profile: BTreeMap<String, Vec<P>>,
    pub flags: BTreeMap<String, Vec<P>>,
    pub junctions: Vec<Junction>,
    pub arrows: Vec<Arrow>,
}

fn pts(v: &Value) -> Result<Vec<P>, String> {
    v.as_array()
        .ok_or("expected a list of [x, y] pairs")?
        .iter()
        .map(|p| Ok([p[0].as_f64().ok_or("bad number")?, p[1].as_f64().ok_or("bad number")?]))
        .collect()
}

fn text(v: &Value) -> String {
    v.as_str().unwrap_or("").to_string()
}

impl PMap {
    pub fn parse(bytes: &[u8]) -> Result<PMap, String> {
        let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("not valid JSON: {e}"))?;
        if v["format"] != FORMAT || v["version"] != 1 {
            return Err(format!("not a {FORMAT} version 1 file"));
        }
        let map_of = |key: &str| -> Result<BTreeMap<String, Vec<P>>, String> {
            v[key].as_object().map(|o| o.iter().map(|(k, val)| Ok((k.clone(), pts(val)?))).collect()).unwrap_or(Ok(BTreeMap::new()))
        };
        Ok(PMap {
            centerline: pts(&v["centerline"])?,
            profile: map_of("profile")?,
            flags: map_of("flags")?,
            junctions: v["junctions"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|j| Junction {
                            id: text(&j["id"]),
                            s: j["s"].as_f64().unwrap_or(0.0),
                            kind: text(&j["kind"]),
                            side: text(&j["side"]),
                            label: text(&j["label"]),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            arrows: v["arrows"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|x| Arrow {
                            s: x["s"].as_f64().unwrap_or(0.0),
                            off: x["off"].as_f64().unwrap_or(0.0),
                            kind: text(&x["kind"]),
                            south: x["south"].as_bool().unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    pub fn from_scene(sc: &Scene) -> PMap {
        PMap {
            centerline: sc.cl.pts.clone(),
            profile: sc.curves.iter().map(|(k, c)| (k.clone(), c.points().to_vec())).collect(),
            flags: sc.flags.clone(),
            junctions: sc
                .junctions
                .iter()
                .map(|j| Junction { id: j.id.clone(), s: j.s, kind: j.kind.clone(), side: j.side.clone(), label: j.label.clone() })
                .collect(),
            arrows: sc.arrows.iter().map(|a| Arrow { s: a.s, off: a.off, kind: a.kind.clone(), south: a.south }).collect(),
        }
    }

    /// Stable text: one list item per line, numbers rounded to a millimetre.
    pub fn to_json(&self) -> String {
        fn n(x: f64) -> String {
            let s = format!("{x:.3}");
            let s = s.trim_end_matches('0').trim_end_matches('.');
            if s == "-0" || s.is_empty() { "0".into() } else { s.into() }
        }
        fn q(s: &str) -> String {
            serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
        }
        fn rows(items: Vec<String>) -> String {
            if items.is_empty() { "[]".into() } else { format!("[\n{}\n]", items.join(",\n")) }
        }
        let pairs = |v: &[P]| rows(v.iter().map(|p| format!("[{}, {}]", n(p[0]), n(p[1]))).collect());
        let keyed = |m: &BTreeMap<String, Vec<P>>| {
            if m.is_empty() {
                return "{}".to_string();
            }
            let body: Vec<String> = m.iter().map(|(k, v)| format!("{}: {}", q(k), pairs(v))).collect();
            format!("{{\n{}\n}}", body.join(",\n"))
        };
        let length = Poly::new(self.centerline.clone()).len();
        let junctions = rows(
            self.junctions
                .iter()
                .map(|j| format!("{{\"id\": {}, \"s\": {}, \"kind\": {}, \"side\": {}, \"label\": {}}}", q(&j.id), n(j.s), q(&j.kind), q(&j.side), q(&j.label)))
                .collect(),
        );
        // Ordered by the millimetre-rounded values that are written, so equal-looking arrows never swap places between runs.
        let mut ordered: Vec<&Arrow> = self.arrows.iter().collect();
        let key = |a: &Arrow| ((a.s * 1000.0).round() as i64, (a.off * 1000.0).round() as i64, a.south, a.kind.clone());
        ordered.sort_by_key(|a| key(a));
        let arrows = rows(
            ordered
                .iter()
                .map(|a| format!("{{\"s\": {}, \"off\": {}, \"kind\": {}, \"south\": {}}}", n(a.s), n(a.off), q(&a.kind), a.south))
                .collect(),
        );
        format!(
            "{{\n\"format\": {},\n\"version\": 1,\n\"length\": {},\n\"centerline\": {},\n\"profile\": {},\n\"flags\": {},\n\"junctions\": {},\n\"arrows\": {}\n}}\n",
            q(FORMAT),
            n(length),
            pairs(&self.centerline),
            keyed(&self.profile),
            keyed(&self.flags),
            junctions,
            arrows
        )
    }
}

fn clean(label: &str) -> String {
    label.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect()
}

/// Importer output: `{"objects": [...]}` of named paths in document points (y down, 1 point = 1 metre).
pub fn import(bytes: &[u8]) -> Result<Value, String> {
    let m = PMap::parse(bytes)?;
    let cl = Poly::new(m.centerline.clone());
    if cl.is_empty() {
        return Err("The pmap has no centerline.".into());
    }
    let doc = |w: P| [w[0], -w[1]];
    let mut out: Vec<Value> = Vec::new();
    out.push(object("centerline", &[cl.pts.iter().map(|&p| doc(p)).collect()], false, [0.8, 0.0, 0.0], 0.5));
    out.push(object("profile.zero", &[vec![[0.0, PROFILE_Y], [cl.len(), PROFILE_Y]]], false, [0.6, 0.6, 0.6], 0.3));
    for (k, v) in &m.profile {
        out.push(object(&format!("profile.{k}"), &[v.iter().map(|p| [p[0], PROFILE_Y - p[1]]).collect()], false, [0.1, 0.3, 0.7], 0.2));
    }
    for (i, (name, ranges)) in m.flags.iter().enumerate() {
        let y = PROFILE_Y + 30.0 + 8.0 * i as f64;
        for (k, r) in ranges.iter().enumerate() {
            out.push(object(&format!("flag.{name}.{k}"), &[vec![[r[0], y], [r[1], y]]], false, [0.9, 0.5, 0.0], 2.0));
        }
    }
    for j in &m.junctions {
        let c = doc(cl.at(j.s).0);
        let sq = vec![[c[0] - 1.5, c[1] - 1.5], [c[0] + 1.5, c[1] - 1.5], [c[0] + 1.5, c[1] + 1.5], [c[0] - 1.5, c[1] + 1.5]];
        let label = clean(&j.label);
        let name = if label.is_empty() { format!("{}.{}.{}", j.id, j.kind, j.side) } else { format!("{}.{}.{}.{}", j.id, j.kind, j.side, label) };
        out.push(object(&name, &[sq], true, [0.0, 0.4, 0.8], 0.5));
    }
    for a in &m.arrows {
        let (q, t) = cl.at(a.s);
        let nrm = Poly::right(t);
        let d = if a.south { [-t[0], -t[1]] } else { t };
        let at = |along: f64, side: f64| doc([q[0] + nrm[0] * a.off + d[0] * along + nrm[0] * side, q[1] + nrm[1] * a.off + d[1] * along + nrm[1] * side]);
        let glyph = vec![at(-1.5, 0.0), at(1.5, 0.0), at(0.9, -0.6), at(1.5, 0.0), at(0.9, 0.6)];
        out.push(object(&format!("arrow.{}.{}.{}", a.kind, if a.south { "S" } else { "N" }, a.s.round() as i64), &[glyph], false, [0.25, 0.25, 0.25], 0.25));
    }
    Ok(serde_json::json!({"objects": out}))
}
