//! "Build Road": regenerates the lane lines, edges and junction pieces from the centerline, profile curves, flags and junction markers.
//! Everything it creates is named `gen.*`, so a rebuild replaces the previous result and never touches the paths the owner edits.

use crate::geom::{P, Poly, add, scale};
use crate::output::Out;
use crate::crossing::{self, CrossingSpec};
use crate::junction::{self, JunctionSpec, junction_id};
use crate::roads::{self, Spec, anchor_from, segment_id};
use crate::scene::{Scene, is_junction_name};
use crate::vcdoc::{Item, items_from_filter_input};
use serde_json::{Value, json};

pub const GEN_PREFIX: &str = "gen.";
/// The drawn line of a road or junction: bright and bold (metres of stroke), so it can be seen and clicked among the generated shapes.
const SPINE_ROAD: [f64; 3] = [0.85, 0.1, 0.1];
const SPINE_JUNCTION: [f64; 3] = [0.1, 0.35, 0.9];
const SPINE_WIDTH: f64 = 1.0;

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
pub(crate) fn simplify(pts: &[P], tol: f64) -> Vec<P> {
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

/// Filter output text: every source object kept by id, the old `gen.*` paths dropped, fresh `gen.*` paths added.
pub fn build_text(input: &Value, params: &Value) -> Result<String, String> {
    let items = items_from_filter_input(input);
    let spec = Spec::from_document_data(&params["_context"]["documentData"]);
    let has_road = items.iter().any(|i| segment_id(&i.name).is_some() || junction_id(&i.name).is_some());
    let has_legacy = items.iter().any(|i| i.name == "centerline");
    if !has_road && !has_legacy {
        return Err("Nothing to build. Draw a line and name it road.<id> (or open a pmap that has a centerline), then Select All so the road is included.".into());
    }

    let mut out = Out::new();
    for it in items.iter().filter(|i| !i.name.starts_with(GEN_PREFIX)) {
        let Some(id) = it.id else { continue };
        // The line you drew stays on top of what is built from it, and is drawn bold so it is easy to find and click.
        if segment_id(&it.name).is_some() {
            out.keep_styled(id, SPINE_ROAD, SPINE_WIDTH);
        } else if junction_id(&it.name).is_some() {
            out.keep_styled(id, SPINE_JUNCTION, SPINE_WIDTH);
        } else {
            out.keep(id);
        }
    }
    if has_road {
        roads::build(&items, &spec, &mut out);
        junction::build(&items, &spec, &mut out); // junctions are drawn over the roads they join
        crossing::build(&items, &spec, &mut out); // and so are intersections where two roads cross
    }
    if has_legacy {
        build_legacy(&mut out, &items, params)?;
    }
    Ok(out.finish())
}

/// The original model: one `centerline` plus profile curves, flags and junction markers (see `scene`).
fn build_legacy(out: &mut Out, items: &[Item], params: &Value) -> Result<(), String> {
    let sc = Scene::from_items(items)?;
    let stub_len = params["stubLength"].as_f64().unwrap_or(28.0);
    let stub_w = params["stubWidth"].as_f64().unwrap_or(8.0);
    let to_doc = |p: P| sc.to_doc(p);

    for (key, rgb, width, dash) in STYLE {
        let r = runs(&sc, key, dash);
        if !r.is_empty() {
            out.path(&format!("{GEN_PREFIX}line.{key}"), &r, to_doc, rgb, width, false);
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
            out.path(&format!("{GEN_PREFIX}stub.{tag}"), &edges, to_doc, [0.13, 0.13, 0.13], 0.3, false);
            let stripes: Vec<Vec<P>> = (0..5).map(|k| { let w = -hw + 0.8 + 1.6 * k as f64; vec![along(2.0, w), along(5.0, w)] }).collect();
            out.path(&format!("{GEN_PREFIX}zebra.{tag}"), &stripes, to_doc, [0.9, 0.9, 0.9], 0.6, false);
            out.path(&format!("{GEN_PREFIX}stop.{tag}"), &[vec![along(7.0, -hw), along(7.0, hw)]], to_doc, [0.95, 0.95, 0.95], 0.6, false);
        }
    }
    Ok(())
}

/// The same as a JSON value (for tests and callers that want one).
pub fn build(input: &Value, params: &Value) -> Result<Value, String> {
    serde_json::from_str(&build_text(input, params)?).map_err(|e| e.to_string())
}

/// Command "Add Junction": keeps every selected object and adds one junction marker at route metre `s` (needs the centerline selected too).
pub fn add_junction_text(input: &Value, params: &Value) -> Result<String, String> {
    let items = items_from_filter_input(input);
    let sc = Scene::from_items(&items)?;
    let s = params["s"].as_f64().ok_or("Give the route metre `s` where the junction goes.")?;
    if !(0.0..=sc.cl.len()).contains(&s) {
        return Err(format!("s = {s} is off the road (0 to {:.1} m).", sc.cl.len()));
    }
    let word = |key: &str, default: &str| params[key].as_str().filter(|v| !v.is_empty()).unwrap_or(default).to_string();
    let clean = |t: String| t.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect::<String>();
    // Next free id: the id the caller gave, else the letter of the existing junctions (C by default) and the next number.
    let id = match params["junction"].as_str().filter(|v| !v.is_empty()) {
        Some(id) => clean(id.to_string()),
        None => {
            let letter = sc.junctions.first().and_then(|j| j.id.chars().next()).unwrap_or('C');
            let top = sc.junctions.iter().filter_map(|j| j.id[1..].parse::<u32>().ok()).max().unwrap_or(0);
            format!("{letter}{}", top + 1)
        }
    };
    if sc.junctions.iter().any(|j| j.id == id) {
        return Err(format!("A junction {id} already exists."));
    }
    let (kind, side, label) = (word("kind", "tee"), word("side", "right"), clean(word("label", "")));
    let name = if label.is_empty() { format!("{id}.{kind}.{side}") } else { format!("{id}.{kind}.{side}.{label}") };
    let c = sc.cl.at(s).0;
    let square = vec![[c[0] - 1.5, c[1] - 1.5], [c[0] + 1.5, c[1] - 1.5], [c[0] + 1.5, c[1] + 1.5], [c[0] - 1.5, c[1] + 1.5]];
    let mut out = Out::new();
    for it in &items {
        if let Some(id) = it.id {
            out.keep(id);
        }
    }
    out.path(&name, &[square], |p| sc.to_doc(p), [0.0, 0.4, 0.8], 0.5, true);
    Ok(out.finish())
}

/// Names that already mean something else, so a plain line must not be mistaken for them.
fn is_reserved_name(name: &str) -> bool {
    name == "centerline" || ["gen.", "profile.", "flag.", "arrow."].iter().any(|p| name.starts_with(p)) || is_junction_name(name) || junction_id(name).is_some()
}

/// Command "Set Cross-section": stores the road's cross-section (see `crosssection`) in the document's plug-in data. Applies to the
/// selected `road.*` lines, or to the segment named in `segment`; a single plain line is turned into a road with a fresh id.
pub fn set_cross_section_text(input: &Value, params: &Value) -> Result<String, String> {
    let items = items_from_filter_input(input);
    let mut spec = Spec::from_document_data(&params["_context"]["documentData"]);
    let text = params["cross_section"].as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or("Write the cross-section, for example: bike-lane | lane back x2 | margin 3 | lane fwd x2 | bike-lane")?;
    let anchor = params["anchor"].as_str().and_then(anchor_from);
    let clean = |t: &str| t.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(16).collect::<String>();

    let mut targets: Vec<String> = Vec::new();
    let mut renames: Vec<(i64, String)> = Vec::new();
    if let Some(seg) = params["segment"].as_str().map(clean).filter(|s| !s.is_empty()) {
        targets.push(seg);
    } else {
        for it in &items {
            if let Some(id) = segment_id(&it.name).filter(|id| !targets.contains(id)) {
                targets.push(id);
            }
        }
        // "Selected pieces only": give them an id of their own, starting from a copy of the old description, so the other pieces keep theirs.
        if params["scope"].as_str() == Some("selected") && !targets.is_empty() {
            let mut fresh: Vec<String> = Vec::new();
            for old in &targets {
                let new = (1..).map(|n| format!("r{n}")).find(|id| !spec.segments.contains_key(id) && !fresh.contains(id) && !targets.contains(id)).unwrap_or_default();
                for it in items.iter().filter(|i| segment_id(&i.name).as_deref() == Some(old.as_str())) {
                    if let Some(iid) = it.id {
                        renames.push((iid, format!("road.{new}")));
                    }
                }
                if let Some(prev) = spec.segments.get(old).cloned() {
                    spec.segments.insert(new.clone(), prev);
                }
                fresh.push(new);
            }
            targets = fresh;
        }
        if targets.is_empty() {
            let plain: Vec<&Item> = items.iter().filter(|i| i.id.is_some() && !i.subs.is_empty() && !is_reserved_name(&i.name)).collect();
            match plain.as_slice() {
                [] => return Err("Select the line to describe (or give its segment id).".into()),
                [one] => {
                    let taken = |id: &str| spec.segments.contains_key(id);
                    let id = (1..).map(|n| format!("r{n}")).find(|id| !taken(id)).unwrap_or_default();
                    renames.push((one.id.unwrap_or_default(), format!("road.{id}")));
                    targets.push(id);
                }
                _ => return Err("More than one line is selected: select the one line to describe.".into()),
            }
        }
    }
    for id in &targets {
        spec.set_cross_section(id, text, anchor.clone())?;
    }
    let mut out = Out::new();
    for it in &items {
        if let Some(id) = it.id {
            match renames.iter().find(|r| r.0 == id) {
                Some((_, name)) => out.rename(id, name),
                None => out.keep(id),
            }
        }
    }
    Ok(out.finish_with(Some(&spec.to_value())))
}

/// Command "Set Junction": stores a junction description (see `junction`) in the document's plug-in data. Applies to the selected
/// `junction.*` line, or to the one selected plain line, which is renamed `junction.<id>`; `junction` names the id explicitly.
pub fn set_junction_text(input: &Value, params: &Value) -> Result<String, String> {
    let items = items_from_filter_input(input);
    let mut spec = Spec::from_document_data(&params["_context"]["documentData"]);
    let raw = params["description"].as_str().map(str::trim).filter(|t| !t.is_empty()).ok_or("Give the junction description (JSON), for example {\"legs\": [{\"side\": \"left\", \"cross_section\": \"lane back | lane fwd\"}], \"control\": \"signal\", \"crosswalks\": true}")?;
    let value: Value = serde_json::from_str(raw).map_err(|e| format!("the junction description is not valid JSON: {e}"))?;
    let js = JunctionSpec::from_json(&value, &spec.library)?;
    let clean = |t: &str| t.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(16).collect::<String>();

    let mut renames: Vec<(i64, String)> = Vec::new();
    let id = if let Some(id) = params["junction"].as_str().map(clean).filter(|s| !s.is_empty()) {
        id
    } else if let Some(id) = items.iter().find_map(|i| junction_id(&i.name)) {
        id
    } else {
        let plain: Vec<&Item> = items.iter().filter(|i| i.id.is_some() && !i.subs.is_empty() && !is_reserved_name(&i.name) && segment_id(&i.name).is_none()).collect();
        match plain.as_slice() {
            [] => return Err("Select the connecting line to describe (or give its junction id).".into()),
            [one] => {
                let id = (1..).map(|n| format!("j{n}")).find(|id| spec.junction_value(id).is_none()).unwrap_or_default();
                renames.push((one.id.unwrap_or_default(), format!("junction.{id}")));
                id
            }
            _ => return Err("More than one line is selected: select the one connecting line to describe.".into()),
        }
    };
    spec.set_junction(&id, &js);
    let mut out = Out::new();
    for it in &items {
        if let Some(iid) = it.id {
            match renames.iter().find(|r| r.0 == iid) {
                Some((_, name)) => out.rename(iid, name),
                None => out.keep(iid),
            }
        }
    }
    Ok(out.finish_with(Some(&spec.to_value())))
}

/// Command "Set Crossing": the two selected roads (one line of each) cross; stores the intersection's description (see `crossing`) in the
/// document's plug-in data. `description` is JSON (control, crosswalks, radius, minor); the two road ids come from the selection.
pub fn set_crossing_text(input: &Value, params: &Value) -> Result<String, String> {
    let items = items_from_filter_input(input);
    let mut spec = Spec::from_document_data(&params["_context"]["documentData"]);
    let mut roads: Vec<String> = Vec::new();
    for it in &items {
        if let Some(id) = segment_id(&it.name).filter(|id| !roads.contains(id)) {
            roads.push(id);
        }
    }
    let [a, b] = <[String; 2]>::try_from(roads).map_err(|r: Vec<String>| format!("Select exactly two roads (one line of each) that cross; {} road{} selected.", r.len(), if r.len() == 1 { " is" } else { "s are" }))?;
    let mut value: Value = match params["description"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
        Some(raw) => serde_json::from_str(raw).map_err(|e| format!("the intersection description is not valid JSON: {e}"))?,
        None => json!({}),
    };
    value["roads"] = json!([a, b]);
    let cs = CrossingSpec::from_json(&value)?;
    if crossing::find(&items, &a, &b).is_none() {
        return Err(format!("Roads {a} and {b} do not cross within the selected lines. Draw one across the other, then select both."));
    }
    let id = spec.crossing_between(&a, &b).unwrap_or_else(|| (1..).map(|n| format!("k{n}")).find(|id| !spec.crossings().iter().any(|(k, _)| k == id)).unwrap_or_default());
    spec.set_crossing(&id, &cs);
    let mut out = Out::new();
    for it in &items {
        if let Some(iid) = it.id {
            out.keep(iid);
        }
    }
    Ok(out.finish_with(Some(&spec.to_value())))
}
