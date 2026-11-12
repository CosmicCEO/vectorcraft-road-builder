//! Reading and writing VectorCraft's path JSON (the filter/importer object form and the native `.vectorcraft` node form).

use crate::geom::P;
use serde_json::{Value, json};

/// A named path read from VectorCraft: every subpath flattened to points (document points, y down).
pub struct Item {
    pub id: Option<i64>,
    pub name: String,
    pub subs: Vec<Vec<P>>,
}

fn pt(v: &Value) -> Option<P> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
}

fn bezier(a: P, c1: P, c2: P, b: P, t: f64) -> P {
    let u = 1.0 - t;
    let k = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    [
        k[0] * a[0] + k[1] * c1[0] + k[2] * c2[0] + k[3] * b[0],
        k[0] * a[1] + k[1] * c1[1] + k[2] * c2[1] + k[3] * b[1],
    ]
}

/// Points of a subpath's anchors, with curved segments (anchor handles) sampled 16 times.
fn flatten(subpath: &Value) -> Vec<P> {
    let anchors = subpath["anchors"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut out: Vec<P> = Vec::new();
    for (i, a) in anchors.iter().enumerate() {
        let Some(p) = pt(&a["p"]) else { continue };
        if i > 0 {
            let prev = &anchors[i - 1];
            let from = pt(&prev["p"]).unwrap_or(p);
            let (out_h, in_h) = (pt(&prev["out"]), pt(&a["in"]));
            if out_h.is_some() || in_h.is_some() {
                for k in 1..16 {
                    out.push(bezier(from, out_h.unwrap_or(from), in_h.unwrap_or(p), p, k as f64 / 16.0));
                }
            }
        }
        out.push(p);
    }
    out
}

fn subpaths_of(path: &Value) -> Vec<Vec<P>> {
    path["subpaths"].as_array().map(|a| a.iter().map(flatten).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

/// Items from a filter's `{"objects": [...]}` input (paths and compound paths).
pub fn items_from_filter_input(input: &Value) -> Vec<Item> {
    input["objects"]
        .as_array()
        .map(|objs| {
            objs.iter()
                .map(|o| Item {
                    id: o["id"].as_i64(),
                    name: o["name"].as_str().unwrap_or("").to_string(),
                    subs: subpaths_of(&o["path"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Items from a native `.vectorcraft` document (the exporter's input): every named path in every layer and group.
pub fn items_from_native(doc: &Value) -> Result<Vec<Item>, String> {
    let layers = doc["document"]["layers"].as_array().ok_or("not a VectorCraft document (no document.layers)")?;
    fn walk(node: &Value, out: &mut Vec<Item>) {
        if node["kind"]["type"] == "path" {
            out.push(Item {
                id: node["id"].as_i64(),
                name: node["name"].as_str().unwrap_or("").to_string(),
                subs: subpaths_of(&node["kind"]["path"]),
            });
        }
        if let Some(children) = node["kind"]["children"].as_array() {
            children.iter().for_each(|c| walk(c, out));
        }
    }
    let mut out = Vec::new();
    layers.iter().for_each(|l| walk(l, &mut out));
    Ok(out)
}

pub fn path_value(subs: &[Vec<P>], closed: bool) -> Value {
    json!({"subpaths": subs.iter().map(|s| json!({
        "anchors": s.iter().map(|p| json!({"p": [p[0], p[1]]})).collect::<Vec<_>>(),
        "closed": closed,
    })).collect::<Vec<_>>()})
}

pub fn stroke(rgb: [f64; 3], width: f64) -> Value {
    json!({"paint": {"type": "solid", "color": {"model": "rgb", "r": rgb[0], "g": rgb[1], "b": rgb[2]}}, "width": width, "cap": "butt", "join": "miter", "miterLimit": 4})
}

/// A new stroked path object (no `id`: the host adds it).
pub fn object(name: &str, subs: &[Vec<P>], closed: bool, rgb: [f64; 3], width: f64) -> Value {
    json!({"name": name, "path": path_value(subs, closed), "strokes": [stroke(rgb, width)]})
}
