//! Roads drawn as lines named `road.<id>`. Each id has a cross-section (the road's elements, see `crosssection`) kept in the document's
//! plug-in data; every line with that name (a split gives several) is built from it: a filled strip per element and a painted line at
//! every boundary, following the line exactly as drawn (curves stay smooth, angle points stay sharp).
//!
//! Plug-in data shape: `{"version": 1, "segments": {"<id>": {"cross_section": "<text>", "anchor": "center"}}, "library": {...overrides}}`.

use crate::builder::simplify;
use crate::crosssection::{Anchor, Dir, Parsed, Placed, format, layout, parse};
use crate::geom::{P, dash_polyline};
use crate::library::{Library, Role};
use crate::offset::{MITRE_LIMIT, offset_polyline};
use crate::output::Out;
use crate::vcdoc::Item;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const ROAD_PREFIX: &str = "road.";
/// What a road gets before anyone has described it: two lanes, one each way.
pub const DEFAULT_CROSS_SECTION: &str = "lane back | lane fwd";

/// The segment id of a road line (`road.a` is `a`; plain `road` is `main`), or None if the name is not a road.
pub fn segment_id(name: &str) -> Option<String> {
    if name == "road" {
        return Some("main".into());
    }
    let rest = name.strip_prefix(ROAD_PREFIX)?;
    let id = rest.split('.').next().unwrap_or("");
    Some(if id.is_empty() { "main".into() } else { id.into() })
}

fn anchor_name(a: &Anchor) -> String {
    match a {
        Anchor::Auto => "auto".into(),
        Anchor::Center => "center".into(),
        Anchor::Left => "left".into(),
        Anchor::Right => "right".into(),
        Anchor::Element(e) => e.clone(),
    }
}

/// `auto`, `center`, `left`, `right`, or the name of an element to follow (checked when the road is laid out). Empty means unset.
pub fn anchor_from(s: &str) -> Option<Anchor> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "auto" => Some(Anchor::Auto),
        "center" | "centre" => Some(Anchor::Center),
        "left" => Some(Anchor::Left),
        "right" => Some(Anchor::Right),
        other => Some(Anchor::Element(other.to_string())),
    }
}

/// Everything the plug-in keeps with the document.
pub struct Spec {
    raw: Value,
    pub segments: BTreeMap<String, (String, Anchor)>,
    pub library: Library,
}

impl Spec {
    /// Reads the plug-in data leniently: a missing or odd entry is skipped, never an error.
    pub fn from_document_data(dd: &Value) -> Spec {
        let mut library = Library::builtin();
        if dd["library"].is_object() {
            let _ = library.merge(&dd["library"]);
        }
        let mut segments = BTreeMap::new();
        if let Some(map) = dd["segments"].as_object() {
            for (id, s) in map {
                if let Some(text) = s["cross_section"].as_str() {
                    segments.insert(id.clone(), (text.to_string(), s["anchor"].as_str().and_then(anchor_from).unwrap_or(Anchor::Auto)));
                }
            }
        }
        Spec { raw: if dd.is_object() { dd.clone() } else { json!({}) }, segments, library }
    }

    /// The data to store: the original with our parts updated (other keys are left alone).
    pub fn to_value(&self) -> Value {
        let mut v = self.raw.clone();
        v["version"] = json!(1);
        v["segments"] = Value::Object(self.segments.iter().map(|(id, (t, a))| (id.clone(), json!({"cross_section": t, "anchor": anchor_name(a)}))).collect());
        v
    }

    /// Checks the text, then stores it in its canonical form. Returns what the parser had to guess.
    pub fn set_cross_section(&mut self, segment: &str, text: &str, anchor: Option<Anchor>) -> Result<Vec<String>, String> {
        let parsed = parse(text, &self.library)?;
        let anchor = anchor.or_else(|| self.segments.get(segment).map(|s| s.1.clone())).unwrap_or(Anchor::Auto);
        // An anchor on an element must name one that is really in this cross-section.
        layout(&parsed, &self.library, [0.0, 1.0], anchor.clone())?;
        self.segments.insert(segment.to_string(), (format(&parsed), anchor));
        Ok(parsed.notes)
    }

    /// The stored description of junction `id`, as JSON.
    pub fn junction_value(&self, id: &str) -> Option<Value> {
        self.raw["junctions"].get(id).filter(|v| v.is_object()).cloned()
    }

    /// Stores a junction description (already checked).
    pub fn set_junction(&mut self, id: &str, spec: &crate::junction::JunctionSpec) {
        if !self.raw["junctions"].is_object() {
            self.raw["junctions"] = json!({});
        }
        self.raw["junctions"][id] = spec.to_json();
    }

    /// All stored crossings (id and description), where two described roads cross.
    pub fn crossings(&self) -> Vec<(String, Value)> {
        self.raw["crossings"].as_object().map(|m| m.iter().filter(|(_, v)| v.is_object()).map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
    }

    /// The id of the stored crossing between these two roads, in either order.
    pub fn crossing_between(&self, a: &str, b: &str) -> Option<String> {
        self.crossings().into_iter().find(|(_, v)| {
            let roads: Vec<&str> = v["roads"].as_array().map(|r| r.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            roads.len() == 2 && ((roads[0] == a && roads[1] == b) || (roads[0] == b && roads[1] == a))
        }).map(|(id, _)| id)
    }

    /// Stores a crossing description (already checked).
    pub fn set_crossing(&mut self, id: &str, spec: &crate::crossing::CrossingSpec) {
        if !self.raw["crossings"].is_object() {
            self.raw["crossings"] = json!({});
        }
        self.raw["crossings"][id] = spec.to_json();
    }

    /// The cross-section and anchor for a segment, falling back to the default.
    pub(crate) fn for_segment(&self, id: &str) -> (Parsed, Anchor, Option<String>) {
        let (text, anchor) = self.segments.get(id).cloned().unwrap_or((DEFAULT_CROSS_SECTION.to_string(), Anchor::Auto));
        match parse(&text, &self.library) {
            Ok(p) => (p, anchor, None),
            Err(e) => (parse(DEFAULT_CROSS_SECTION, &self.library).expect("default is valid"), anchor, Some(format!("road {id}: {e}; using the default two lanes"))),
        }
    }
}

struct Style {
    rgb: [f64; 3],
    width: f64,
    dash: Option<(f64, f64)>,
    /// Two parallel lines this far apart (a double line).
    double: Option<f64>,
}

pub(crate) const WHITE: [f64; 3] = [0.98, 0.98, 0.98];
const YELLOW: [f64; 3] = [0.92, 0.72, 0.05];
pub(crate) const CURB: [f64; 3] = [0.25, 0.25, 0.25];

fn solid(rgb: [f64; 3], width: f64) -> Option<Style> {
    Some(Style { rgb, width, dash: None, double: None })
}

/// How the line between two neighbouring elements is painted, if at all.
fn boundary(a: &Placed, b: &Placed) -> Option<Style> {
    use Role::*;
    let travelish = |r: Role| matches!(r, Travel | Pocket);
    let hard = |r: Role| matches!(r, Curbed | Walk);
    let (ra, rb) = (a.role, b.role);
    if travelish(ra) && travelish(rb) {
        return match (a.dir, b.dir) {
            (Some(x), Some(y)) if x == y => Some(Style { rgb: WHITE, width: 0.12, dash: Some((3.0, 9.0)), double: None }),
            (Some(_), Some(_)) => Some(Style { rgb: YELLOW, width: 0.12, dash: None, double: Some(0.3) }),
            _ => Some(Style { rgb: YELLOW, width: 0.12, dash: Some((3.0, 3.0)), double: None }),
        };
    }
    if (hard(ra) && hard(rb)) || (ra == Edge && rb == Edge) {
        return if hard(ra) && hard(rb) && ra != rb { solid(CURB, 0.12) } else { None };
    }
    if hard(ra) || hard(rb) {
        return solid(CURB, 0.3);
    }
    match (ra, rb) {
        (Buffer, _) | (_, Buffer) => solid(WHITE, 0.1),
        (Flush, x) | (x, Flush) if travelish(x) => solid(YELLOW, 0.12),
        (x, Bike) | (Bike, x) if travelish(x) => solid(WHITE, 0.15),
        (x, Edge) | (Edge, x) if travelish(x) => solid(WHITE, 0.15),
        (x, Parking) | (Parking, x) if travelish(x) => solid(WHITE, 0.12),
        _ => None,
    }
}

pub(crate) fn heading(path: &[P]) -> P {
    path.windows(2)
        .find_map(|w| {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let m = d[0].hypot(d[1]);
            (m > 1e-9).then(|| [d[0] / m, d[1] / m])
        })
        .unwrap_or([0.0, 1.0])
}

/// Document points are y-down; the geometry works y-up. Flipping y converts either way.
pub(crate) fn flip(p: P) -> P {
    [p[0], -p[1]]
}

/// Draws one road piece: a filled strip per placed element and a painted line at every boundary, following `path` (world metres,
/// y up) with the elements laid out by `layout`. Shared by roads, junction legs and junction connectors.
pub(crate) fn emit(out: &mut Out, base: &str, path: &[P], placed: &[Placed]) {
    let mut bounds: Vec<f64> = placed.iter().map(|e| e.left).collect();
    bounds.push(placed.last().map_or(0.0, |e| e.right));
    let edges: Vec<Vec<P>> = bounds.iter().map(|&o| simplify(&offset_polyline(path, o, MITRE_LIMIT), 0.02)).collect();

    for (i, e) in placed.iter().enumerate() {
        if edges[i].len() < 2 || edges[i + 1].len() < 2 {
            continue;
        }
        let mut ring = edges[i].clone();
        ring.extend(edges[i + 1].iter().rev());
        out.polygon(&format!("{base}.surface.{i}.{}", e.id), &ring, flip, e.fill);
    }
    for (k, edge) in edges.iter().enumerate() {
        let style = match (k.checked_sub(1).map(|j| &placed[j]), placed.get(k)) {
            (Some(a), Some(b)) => boundary(a, b),
            _ => solid(CURB, 0.15), // the outer edge of the whole road
        };
        let Some(style) = style else { continue };
        let lines: Vec<Vec<P>> = match style.double {
            Some(gap) => [gap / 2.0, -gap / 2.0].iter().map(|&o| simplify(&offset_polyline(edge, o, MITRE_LIMIT), 0.02)).collect(),
            None => vec![edge.clone()],
        };
        let lines: Vec<Vec<P>> = match style.dash {
            Some((on, off)) => lines.iter().flat_map(|l| dash_polyline(l, on, off)).collect(),
            None => lines,
        };
        if lines.iter().any(|l| l.len() >= 2) {
            out.path(&format!("{base}.line.{k}"), &lines, flip, style.rgb, style.width, false);
        }
    }
}

/// Builds every `road.*` line in `items`. Returns notes for the user (things guessed or skipped).
pub fn build(items: &[Item], spec: &Spec, out: &mut Out) -> Vec<String> {
    build_clipped(items, spec, &[], out)
}

/// `build`, with the parts of road pieces that crossings trim away left out (the lines in the document are not changed).
pub fn build_clipped(items: &[Item], spec: &Spec, trims: &[crate::crossing::Trim], out: &mut Out) -> Vec<String> {
    let mut notes = Vec::new();
    let mut pieces: BTreeMap<String, usize> = BTreeMap::new();
    let mut resolved: BTreeMap<String, (Parsed, Anchor)> = BTreeMap::new();
    for (index, it) in items.iter().enumerate() {
        let Some(id) = segment_id(&it.name) else { continue };
        let Some(sub) = it.subs.first() else { continue };
        let path: Vec<P> = sub.iter().map(|&p| flip(p)).collect();
        if heading(&path) == [0.0, 1.0] && path.windows(2).all(|w| w[0] == w[1]) {
            notes.push(format!("road {id}: the line has no length, skipped"));
            continue;
        }
        let (parsed, anchor) = resolved
            .entry(id.clone())
            .or_insert_with(|| {
                let (p, a, note) = spec.for_segment(&id);
                notes.extend(note);
                (p, a)
            })
            .clone();
        let placed = match layout(&parsed, &spec.library, heading(&path), anchor) {
            Ok(p) => p,
            Err(e) => {
                notes.push(format!("road {id}: {e}"));
                continue;
            }
        };
        let piece = pieces.entry(id.clone()).and_modify(|n| *n += 1).or_insert(1);
        let base = format!("gen.road.{id}.{piece}");

        // The elements are laid out for the whole line; only the drawing is cut short.
        let drawn = trims.iter().filter(|t| t.index == index).fold(path.clone(), |p, t| crate::crossing::clip(&p, t));
        emit(out, &base, &drawn, &placed);
        let _ = Dir::Fwd; // directions are resolved in the layout; arrows come with the junction work
    }
    notes
}
