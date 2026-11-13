//! `road-builder-cli`: parse, validate and edit road cross-sections from the command line. Always prints one line of JSON.
//!
//! ```text
//! road-builder-cli parse "facing N: bike-lane | lane S x2 | margin 3 | lane N x2 | bike-lane"
//! road-builder-cli edit  "<text>" delete 0 | move 0 5 | width 2 1.5 | insert 3 "turn-pocket N"
//! road-builder-cli library
//! ```
//! `parse` and `edit` print `{"ok": true, "canonical", "facing", "items": [...], "total_width", "notes"}` or `{"ok": false, "error"}`.

use road_builder_core::crosssection::{Item, Parsed, format, parse};
use road_builder_core::junction::{Control, JunctionSpec};
use road_builder_core::library::Library;
use serde_json::{Value, json};

fn describe(p: &Parsed, lib: &Library) -> Value {
    let mut total = 0.0;
    let items: Vec<Value> = p
        .items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let def = lib.get(&it.kind);
            let width = it.width.or(def.map(|d| d.width)).unwrap_or(0.0);
            total += width;
            json!({
                "index": i,
                "kind": it.kind,
                "label": def.map_or(it.kind.as_str(), |d| d.label.as_str()),
                "way": it.way.map(|w| w.letter()),
                "width": (width * 1000.0).round() / 1000.0,
                "explicit_width": it.width.is_some(),
                "directional": def.is_some_and(|d| d.directional),
                "fill": def.map(|d| d.fill),
            })
        })
        .collect();
    json!({"ok": true, "canonical": format(p), "facing": p.facing.map(|w| w.letter()), "items": items, "total_width": (total * 1000.0).round() / 1000.0, "notes": p.notes})
}

fn label(it: &Item, lib: &Library) -> String {
    lib.get(&it.kind).map_or(it.kind.clone(), |d| d.label.to_lowercase())
}

fn width(it: &Item, lib: &Library) -> f64 {
    it.width.or(lib.get(&it.kind).map(|d| d.width)).unwrap_or(0.0)
}

fn way_word(w: Option<road_builder_core::crosssection::Way>) -> &'static str {
    use road_builder_core::crosssection::Way::*;
    match w {
        Some(North) => "northbound",
        Some(South) => "southbound",
        Some(East) => "eastbound",
        Some(West) => "westbound",
        Some(Fwd) => "along the line",
        Some(Back) => "against the line",
        None => "no direction",
    }
}

/// The differences between two cross-sections as plain sentences: elements are matched in order by kind (longest common subsequence),
/// so a removal, an addition, a change of direction or width, and a different reading side each show up on their own line.
fn changes(a: &Parsed, b: &Parsed, lib: &Library) -> Vec<String> {
    let (n, m) = (a.items.len(), b.items.len());
    let mut t = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            t[i][j] = if a.items[i].kind == b.items[j].kind { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    let mut out = Vec::new();
    if a.facing != b.facing {
        out.push(format!("reading side changed from {} to {}", a.facing.map_or("the line", |w| w.letter()), b.facing.map_or("the line", |w| w.letter())));
    }
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a.items[i].kind == b.items[j].kind && t[i][j] == t[i + 1][j + 1] + 1 {
            let (x, y) = (&a.items[i], &b.items[j]);
            if x.way != y.way {
                out.push(format!("{}: direction {} to {}", label(x, lib), way_word(x.way), way_word(y.way)));
            }
            if (width(x, lib) - width(y, lib)).abs() > 1e-9 {
                out.push(format!("{}: width {} m to {} m", label(x, lib), width(x, lib), width(y, lib)));
            }
            i += 1;
            j += 1;
        } else if j < m && (i == n || t[i][j + 1] >= t[i + 1][j]) {
            out.push(format!("added {} ({} m{})", label(&b.items[j], lib), width(&b.items[j], lib), if b.items[j].way.is_some() { format!(", {}", way_word(b.items[j].way)) } else { String::new() }));
            j += 1;
        } else {
            out.push(format!("removed {} ({} m{})", label(&a.items[i], lib), width(&a.items[i], lib), if a.items[i].way.is_some() { format!(", {}", way_word(a.items[i].way)) } else { String::new() }));
            i += 1;
        }
    }
    out
}

fn fail(msg: impl Into<String>) -> Value {
    json!({"ok": false, "error": msg.into()})
}

fn index(args: &[String], at: usize, what: &str) -> Result<usize, String> {
    args.get(at).and_then(|a| a.parse::<usize>().ok()).ok_or_else(|| format!("{what}: give a whole number (items are numbered from 0)"))
}

fn run(args: &[String], lib: &Library) -> Value {
    match args.first().map(String::as_str) {
        Some("parse") => match args.get(1) {
            Some(text) => parse(text, lib).map_or_else(fail, |p| describe(&p, lib)),
            None => fail("usage: parse \"<cross-section text>\""),
        },
        Some("edit") => {
            let Some(text) = args.get(1) else { return fail("usage: edit \"<text>\" delete|move|width|insert ...") };
            let mut p = match parse(text, lib) {
                Ok(p) => p,
                Err(e) => return fail(e),
            };
            let done = match args.get(2).map(String::as_str) {
                Some("delete") => index(args, 3, "delete").and_then(|i| p.delete(i)),
                Some("move") => index(args, 3, "move from").and_then(|a| index(args, 4, "move to").and_then(|b| p.move_item(a, b))),
                Some("width") => index(args, 3, "width").and_then(|i| match args.get(4).and_then(|w| w.parse::<f64>().ok()) {
                    Some(w) => p.set_width(i, w),
                    None => Err("width: give the new width in metres".into()),
                }),
                Some("insert") => index(args, 3, "insert at").and_then(|i| match args.get(4) {
                    Some(t) => p.insert(i, t, lib),
                    None => Err("insert: give the element text, like \"turn-pocket N\"".into()),
                }),
                _ => Err("edit needs one of: delete <i>, move <from> <to>, width <i> <metres>, insert <i> \"<text>\"".into()),
            };
            done.map_or_else(fail, |()| describe(&p, lib))
        }
        Some("diff") => match (args.get(1).map(|t| parse(t, lib)), args.get(2).map(|t| parse(t, lib))) {
            (Some(Ok(a)), Some(Ok(b))) => json!({"ok": true, "changes": changes(&a, &b, lib)}),
            (Some(Err(e)), _) => fail(format!("current: {e}")),
            (_, Some(Err(e))) => fail(format!("proposed: {e}")),
            _ => fail("usage: diff \"<current>\" \"<proposed>\""),
        },
        Some("parse-junction") => {
            let Some(text) = args.get(1) else { return fail("usage: parse-junction '<junction JSON>'") };
            let value: Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(e) => return fail(format!("not valid JSON: {e}")),
            };
            JunctionSpec::from_json(&value, lib).map_or_else(fail, |j| {
                let legs: Vec<String> = j.legs.iter().map(|l| format!("{}: {} ({} m)", l.side, l.cross_section, l.length)).collect();
                let control = match j.control { Control::None => "no control", Control::Signal => "traffic signals", Control::Stop => "stop signs on the legs" };
                let summary = format!("{}, {control}{}{}", match j.kind() { "tee" => "T-junction", "cross" => "four-way junction", _ => "road carried across the gap" }, if j.crosswalks { ", crosswalks" } else { "" }, if legs.is_empty() { String::new() } else { format!("; legs {}", legs.join("; ")) });
                json!({"ok": true, "junction": j.to_json(), "summary": summary})
            })
        }
        Some("parse-crossing") => {
            let Some(text) = args.get(1) else { return fail("usage: parse-crossing '<crossing JSON>'") };
            let value: Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(e) => return fail(format!("not valid JSON: {e}")),
            };
            road_builder_core::crossing::CrossingSpec::from_json(&value).map_or_else(fail, |c| json!({"ok": true, "crossing": c.to_json(), "summary": c.summary()}))
        }
        Some("parse-hub") => {
            let Some(text) = args.get(1) else { return fail("usage: parse-hub '<hub JSON>'") };
            let value: Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(e) => return fail(format!("not valid JSON: {e}")),
            };
            road_builder_core::hub::HubSpec::from_json(&value).map_or_else(fail, |h| json!({"ok": true, "hub": h.to_json(), "summary": h.summary()}))
        }
        Some("parse-transition") => {
            let Some(text) = args.get(1) else { return fail("usage: parse-transition '<transition JSON>' [\"current cross-section\"]") };
            let value: Value = match serde_json::from_str(text) {
                Ok(v) => v,
                Err(e) => return fail(format!("not valid JSON: {e}")),
            };
            match road_builder_core::transition::TransitionSpec::from_json(&value) {
                Err(e) => fail(e),
                Ok(t) => {
                    let after = args.get(2).map(|cs| parse(cs, lib).and_then(|p| road_builder_core::transition::describe_to(&p, lib, [0.0, 1.0], &t)));
                    match after {
                        Some(Err(e)) => fail(e),
                        Some(Ok(text)) => json!({"ok": true, "transition": t.to_json(), "summary": t.summary(), "after": text}),
                        None => json!({"ok": true, "transition": t.to_json(), "summary": t.summary()}),
                    }
                }
            }
        }
        Some("library") => {
            let els: Vec<Value> = lib
                .names()
                .into_iter()
                .filter_map(|n| lib.get(n))
                .map(|d| json!({"id": d.id, "label": d.label, "width": d.width, "directional": d.directional, "fill": d.fill, "source": d.source}))
                .collect();
            json!({"ok": true, "elements": els})
        }
        _ => fail("usage: road-builder-cli parse|edit|diff|parse-junction|parse-crossing|parse-hub|parse-transition|library ..."),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = run(&args, &Library::builtin());
    println!("{out}");
}
