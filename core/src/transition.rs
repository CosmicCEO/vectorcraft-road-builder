//! Lane transitions: a road that gains or loses lanes in one direction, on its left (toward the centre) or right (toward the edge) side.
//!
//! Stored in the document's plug-in data under `transitions.<road id>` as a list of
//! `{"piece": 1, "at": 120, "change": "add", "side": "right", "direction": "fwd", "count": 1, "length": null, "speed": 35, "to": null}`:
//! `piece` is which line of that road (1 is the first), `at` metres from the start of that line (left out: the middle), `change` add or drop,
//! `side` left or right as the driver sees it (right-hand traffic: left is toward the centre of the road, right toward its edge, in both
//! directions), `direction` fwd (with the line) or back, `count` 1 to 3 lanes, `length` the taper (left out: the MUTCD formula from `speed`), and
//! `to` the cross-section after the transition if you would rather write it than have it worked out.
//!
//! The road is cut into plain pieces before and after, and the taper between is drawn by laying the two cross-sections over each other: elements
//! that appear in both are matched in order, one that exists only after widens from nothing, one that exists only before narrows to nothing, and
//! every edge slides linearly along the taper.

use crate::crosssection::{Anchor, Dir, Parsed, Placed, format, layout, parse};
use crate::crossing::slice_path;
use crate::geom::{P, Poly, add, scale};
use crate::library::Library;
use crate::output::Out;
use crate::roads::{CURB, boundary, boundary_line, emit, flip, heading};
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Change {
    Add,
    Drop,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// Toward the centre of the road, as the driver sees it.
    Left,
    /// Toward the edge of the road.
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransitionSpec {
    pub piece: usize,
    pub at: Option<f64>,
    pub change: Change,
    pub side: Side,
    pub direction: Dir,
    pub count: usize,
    pub length: Option<f64>,
    pub speed: f64,
    pub to: Option<String>,
}

impl TransitionSpec {
    pub fn from_json(v: &Value) -> Result<TransitionSpec, String> {
        let word = |key: &str| v[key].as_str().map(|w| w.trim().to_ascii_lowercase().replace([' ', '_'], "-"));
        let change = match word("change").as_deref() {
            Some("add") | Some("expand") | Some("widen") | Some("gain") | Some("new-lane") | Some("lane-added") => Change::Add,
            Some("drop") | Some("contract") | Some("narrow") | Some("lose") | Some("merge") | Some("lane-drop") | Some("lane-ends") => Change::Drop,
            Some(other) => return Err(format!("change must be add or drop (got \"{other}\")")),
            None => return Err("a transition needs \"change\": add or drop".into()),
        };
        let side = match word("side").as_deref() {
            Some("left") | Some("inner") | Some("centre") | Some("center") => Side::Left,
            Some("right") | Some("outer") | Some("edge") => Side::Right,
            Some(other) => return Err(format!("side must be left or right (got \"{other}\")")),
            None => return Err("a transition needs \"side\": left or right".into()),
        };
        let direction = match word("direction").as_deref() {
            None | Some("") | Some("fwd") | Some("forward") | Some("with") | Some("along") => Dir::Fwd,
            Some("back") | Some("backward") | Some("against") | Some("reverse") | Some("opposite") => Dir::Back,
            Some(other) => return Err(format!("direction must be fwd or back (got \"{other}\")")),
        };
        let to = v["to"].as_str().map(str::trim).filter(|t| !t.is_empty()).map(String::from);
        Ok(TransitionSpec {
            piece: v["piece"].as_u64().unwrap_or(1).clamp(1, 99) as usize,
            at: v["at"].as_f64().map(|a| a.clamp(0.0, 100_000.0)),
            change,
            side,
            direction,
            count: v["count"].as_u64().unwrap_or(1).clamp(1, 3) as usize,
            length: v["length"].as_f64().map(|l| l.clamp(10.0, 600.0)),
            speed: v["speed"].as_f64().unwrap_or(35.0).clamp(15.0, 80.0),
            to,
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "piece": self.piece, "at": self.at, "change": if self.change == Change::Add { "add" } else { "drop" },
            "side": if self.side == Side::Left { "left" } else { "right" }, "direction": if self.direction == Dir::Fwd { "fwd" } else { "back" },
            "count": self.count, "length": self.length, "speed": self.speed, "to": self.to
        })
    }

    pub fn summary(&self) -> String {
        let n = self.count;
        let lanes = format!("{n} lane{}", if n == 1 { "" } else { "s" });
        let what = match self.change {
            Change::Add => format!("adds {lanes}"),
            Change::Drop => format!("drops {lanes}"),
        };
        let side = if self.side == Side::Left { "left (toward the centre)" } else { "right (toward the edge)" };
        let dir = if self.direction == Dir::Fwd { "with the line" } else { "against the line" };
        let at = self.at.map_or("the middle of the line".to_string(), |a| format!("{a} m from the start"));
        format!("{what} on the {side} of the lanes {dir}, at {at}, taper {}", self.length.map_or(format!("by the MUTCD formula at {} mph", self.speed), |l| format!("{l} m")))
    }

    /// The taper length in metres: given, or the MUTCD lane-reduction formula for a lane `lane_width` m wide (`L = W S` at 45 mph or more,
    /// `W S^2 / 60` below), never under the 100 ft urban minimum.
    pub fn taper(&self, lane_width: f64) -> f64 {
        if let Some(l) = self.length {
            return l;
        }
        let (w_ft, s) = (lane_width / 0.3048, self.speed);
        let ft = if s >= 45.0 { w_ft * s } else { w_ft * s * s / 60.0 };
        (ft.max(100.0)) * 0.3048
    }
}

/// What a derived transition did, in placed order (left to right across the road): `count` lanes appear at B index `at` (add) or vanish from A index
/// `at` (drop). With this, identical lanes still know which one tapers.
#[derive(Clone, Copy, Debug)]
pub struct Op {
    pub change: Change,
    pub at: usize,
    pub count: usize,
}

/// The cross-section after `spec` (lanes added or dropped on one side of one direction's lanes) and the exact operation, if it was worked out
/// rather than given as `to` text.
pub fn derive(from: &Parsed, lib: &Library, head: P, spec: &TransitionSpec) -> Result<(Parsed, Option<Op>), String> {
    if let Some(text) = &spec.to {
        return Ok((parse(text, lib)?, None));
    }
    let lanes_of = |p: &Parsed| -> Result<Vec<usize>, String> {
        let placed = layout(p, lib, head, Anchor::Auto)?;
        Ok(placed.iter().enumerate().filter(|(_, e)| matches!(e.role, crate::library::Role::Travel) && e.dir == Some(spec.direction)).map(|(j, _)| j).collect())
    };
    let first_group = lanes_of(from)?;
    if first_group.is_empty() {
        return Err(format!("the road has no lanes {} to change", if spec.direction == Dir::Fwd { "with the line" } else { "against the line" }));
    }
    // `layout` reads the list from the other end when the line runs against the direction it was written facing.
    let reversed = from.facing.and_then(|f| f.compass()).is_some_and(|f| crate::geom::dot(head, f) < 0.0);
    // The driver's left is toward the centre of the road: the left end of the group of lanes with the line, the right end of those against it.
    let at_left_end = (spec.side == Side::Left) == (spec.direction == Dir::Fwd);
    let mut p = from.clone();
    for _ in 0..spec.count {
        let group = lanes_of(&p)?;
        let edge_j = if at_left_end { *group.first().ok_or("no lanes left")? } else { *group.last().ok_or("no lanes left")? };
        let m = p.items.len();
        let idx = if reversed { m - 1 - edge_j } else { edge_j };
        match spec.change {
            Change::Drop => {
                if group.len() < 2 {
                    return Err("cannot drop the last lane in that direction".into());
                }
                p.delete(idx)?;
            }
            Change::Add => {
                let copy = p.items[idx].clone();
                let before = at_left_end != reversed;
                p.items.insert(if before { idx } else { idx + 1 }, copy);
            }
        }
    }
    let (first, last) = (*first_group.first().expect("not empty"), *first_group.last().expect("not empty"));
    let at = match (spec.change, at_left_end) {
        (Change::Add, true) => first,
        (Change::Add, false) => last + 1,
        (Change::Drop, true) => first,
        (Change::Drop, false) => last + 1 - spec.count,
    };
    Ok((p, Some(Op { change: spec.change, at, count: spec.count })))
}

/// The cross-section after `spec`, as parsed.
pub fn derive_to(from: &Parsed, lib: &Library, head: P, spec: &TransitionSpec) -> Result<Parsed, String> {
    derive(from, lib, head, spec).map(|(p, _)| p)
}

/// One stretch of a road: plain (one cross-section) or a taper between two.
pub(crate) enum Part {
    Plain { path: Vec<P>, placed: Vec<Placed>, tag: String },
    Morph { path: Vec<P>, from: Vec<Placed>, to: Vec<Placed>, slots: Vec<Slot>, tag: String },
}

fn shifted(mut placed: Vec<Placed>, by: f64) -> Vec<Placed> {
    for e in &mut placed {
        e.left += by;
        e.right += by;
    }
    placed
}

/// Cuts `path` at the transitions (sorted by position) into plain stretches and tapers. Elements the two sides of a transition share keep their
/// place at the joints, so the road does not drift sideways. Notes say what was skipped.
pub(crate) fn parts(path: &[P], parsed: &Parsed, anchor: &Anchor, lib: &Library, list: &[TransitionSpec], notes: &mut Vec<String>) -> Vec<Part> {
    let poly = Poly::new(path.to_vec());
    let len = poly.len();
    let mut list: Vec<&TransitionSpec> = list.iter().collect();
    list.sort_by(|a, b| a.at.unwrap_or(len / 2.0).total_cmp(&b.at.unwrap_or(len / 2.0)));
    let (mut cursor, mut cur, mut shift) = (0.0_f64, parsed.clone(), 0.0_f64);
    let mut out: Vec<Part> = Vec::new();
    let mut tag_n = 0;
    let plain = |from: f64, to: f64, cs: &Parsed, shift: f64, tag: String, out: &mut Vec<Part>| {
        if to - from < 0.3 {
            return;
        }
        let sub = slice_path(path, from, to);
        if let Ok(placed) = layout(cs, lib, heading(&sub), anchor.clone()) {
            out.push(Part::Plain { path: sub, placed: shifted(placed, shift), tag });
        }
    };
    for (k, t) in list.iter().enumerate() {
        let s0 = t.at.unwrap_or(len / 2.0);
        if s0 < cursor + 0.5 || s0 >= len - 1.0 {
            notes.push(format!("transition {} ({}) is outside the line or overlaps the one before: skipped", k + 1, t.summary()));
            continue;
        }
        let head = heading(&slice_path(path, s0, len));
        let (next, op) = match derive(&cur, lib, head, t) {
            Ok(n) => n,
            Err(e) => {
                notes.push(format!("transition {}: {e}: skipped", k + 1));
                continue;
            }
        };
        let lane_w = layout(&cur, lib, head, anchor.clone()).ok().and_then(|p| p.iter().find(|e| matches!(e.role, crate::library::Role::Travel)).map(|e| e.width)).unwrap_or(3.6);
        let mut l = t.taper(lane_w);
        if s0 + l > len {
            notes.push(format!("transition {}: the taper is longer than the line, shortened to {:.0} m", k + 1, len - s0));
            l = len - s0;
        }
        plain(cursor, s0, &cur, shift, if tag_n == 0 { String::new() } else { format!("after{tag_n}") }, &mut out);
        let sub = slice_path(path, s0, s0 + l);
        if let (Ok(a), Ok(b)) = (layout(&cur, lib, heading(&sub), anchor.clone()), layout(&next, lib, heading(&sub), anchor.clone())) {
            let (a, b) = (shifted(a, shift), b);
            let slots = align(&a, &b, op);
            // Slide the new cross-section so the first element both share keeps its place.
            let delta = slots.iter().find_map(|s| Some(a[s.a?].left - b[s.b?].left)).unwrap_or(0.0);
            let b = shifted(b, delta);
            shift = delta;
            tag_n += 1;
            out.push(Part::Morph { path: sub, from: a, to: b, slots, tag: format!("taper{tag_n}") });
        }
        cursor = s0 + l;
        cur = next;
    }
    plain(cursor, len, &cur, shift, if tag_n == 0 { String::new() } else { format!("after{tag_n}") }, &mut out);
    out
}

/// A slot of the overlaid cross-sections: its element in the first, in the second, or both.
pub(crate) struct Slot {
    a: Option<usize>,
    b: Option<usize>,
}

/// Overlays two cross-sections. With the operation known, the lanes that appear or vanish are exactly where it says; otherwise elements are
/// matched in order (same element and direction) by longest common subsequence and the rest appear or vanish.
fn align(a: &[Placed], b: &[Placed], op: Option<Op>) -> Vec<Slot> {
    if let Some(op) = op {
        let mut slots = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            let here = match op.change {
                Change::Add => j >= op.at && j < op.at + op.count,
                Change::Drop => i >= op.at && i < op.at + op.count,
            };
            if here {
                match op.change {
                    Change::Add => { slots.push(Slot { a: None, b: Some(j) }); j += 1; }
                    Change::Drop => { slots.push(Slot { a: Some(i), b: None }); i += 1; }
                }
            } else if i < a.len() && j < b.len() {
                slots.push(Slot { a: Some(i), b: Some(j) });
                i += 1;
                j += 1;
            } else {
                break;
            }
        }
        if slots.iter().filter(|s| s.a.is_some()).count() == a.len() && slots.iter().filter(|s| s.b.is_some()).count() == b.len() {
            return slots;
        }
    }
    let key = |e: &Placed| (e.id.clone(), e.dir);
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if key(&a[i]) == key(&b[j]) { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut slots = Vec::new();
    while i < n || j < m {
        if i < n && j < m && key(&a[i]) == key(&b[j]) {
            slots.push(Slot { a: Some(i), b: Some(j) });
            i += 1;
            j += 1;
        } else if j >= m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            slots.push(Slot { a: Some(i), b: None });
            i += 1;
        } else {
            slots.push(Slot { a: None, b: Some(j) });
            j += 1;
        }
    }
    slots
}

/// The edges of every slot in one cross-section, `slots + 1` offsets: a slot it does not have is a zero-width gap where it would stand.
fn edges(slots: &[Slot], placed: &[Placed], pick: impl Fn(&Slot) -> Option<usize>) -> Vec<f64> {
    let mut cursor = placed.first().map_or(0.0, |e| e.left);
    let mut out = vec![cursor];
    for s in slots {
        if let Some(i) = pick(s) {
            cursor = placed[i].right;
        }
        out.push(cursor);
    }
    out
}

/// Draws the taper over `path`: every edge slides linearly from its place in `from` to its place in `to`.
pub(crate) fn morph(out: &mut Out, base: &str, path: &[P], from: &[Placed], to: &[Placed], slots: &[Slot]) {
    let (ea, eb) = (edges(slots, from, |s| s.a), edges(slots, to, |s| s.b));
    let poly = Poly::new(path.to_vec());
    let len = poly.len();
    if len < 0.5 || slots.is_empty() {
        return;
    }
    // Sample often enough to look straight: every 2 m, and at every bend of the line.
    let steps = (len / 2.0).ceil().max(2.0) as usize;
    let mut s_list: Vec<f64> = (0..=steps).map(|i| len * i as f64 / steps as f64).collect();
    let mut acc = 0.0;
    for w in poly.pts.windows(2) {
        acc += (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        s_list.push(acc.min(len));
    }
    s_list.sort_by(f64::total_cmp);
    s_list.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    let edge = |k: usize| -> Vec<P> {
        s_list
            .iter()
            .map(|&s| {
                let (p, t) = poly.at(s);
                let off = ea[k] + (eb[k] - ea[k]) * (s / len);
                add(p, scale(Poly::right(t), off))
            })
            .collect()
    };
    let lines: Vec<Vec<P>> = (0..=slots.len()).map(edge).collect();
    let rep = |s: &Slot| -> &Placed { s.a.map(|i| &from[i]).or(s.b.map(|j| &to[j])).expect("a slot has an element") };
    for (k, s) in slots.iter().enumerate() {
        let fill = match (s.a, s.b) {
            (Some(i), Some(j)) => [0, 1, 2].map(|c| (from[i].fill[c] + to[j].fill[c]) / 2.0),
            _ => rep(s).fill,
        };
        let mut ring = lines[k].clone();
        ring.extend(lines[k + 1].iter().rev());
        out.polygon(&format!("{base}.surface.{k}.{}", rep(s).id), &ring, flip, fill);
    }
    for k in 0..=slots.len() {
        let style = match (k.checked_sub(1).map(|j| rep(&slots[j])), slots.get(k).map(rep)) {
            (Some(a), Some(b)) => boundary(a, b),
            _ => Some(crate::roads::Style { rgb: CURB, width: 0.15, dash: None, double: None }),
        };
        if let Some(style) = style {
            boundary_line(out, &format!("{base}.line.{k}"), &lines[k], &style);
        }
    }
}

/// The cross-section a road has at `at` metres, after the transitions stored before it (the ones at an earlier place, in order).
pub fn cross_section_at(parsed: &Parsed, lib: &Library, head: P, list: &[TransitionSpec], piece: usize, at: f64) -> Parsed {
    let mut earlier: Vec<&TransitionSpec> = list.iter().filter(|t| t.piece == piece && t.at.is_some_and(|a| a < at - 5.0)).collect();
    earlier.sort_by(|a, b| a.at.unwrap_or(0.0).total_cmp(&b.at.unwrap_or(0.0)));
    earlier.into_iter().fold(parsed.clone(), |cur, t| derive_to(&cur, lib, head, t).unwrap_or(cur))
}

/// The cross-section text after the transition, for showing the user (e.g. "lane back | lane fwd x2").
pub fn describe_to(from: &Parsed, lib: &Library, head: P, spec: &TransitionSpec) -> Result<String, String> {
    derive_to(from, lib, head, spec).map(|p| format(&p))
}

/// Draws one road line with its transitions: plain stretches and tapers. `base` is `gen.road.<id>.<piece>`.
pub(crate) fn emit_parts(out: &mut Out, base: &str, parts: &[Part]) {
    for part in parts {
        match part {
            Part::Plain { path, placed, tag } => emit(out, &if tag.is_empty() { base.to_string() } else { format!("{base}.{tag}") }, path, placed),
            Part::Morph { path, from, to, slots, tag } => morph(out, &format!("{base}.{tag}"), path, from, to, slots),
        }
    }
}
