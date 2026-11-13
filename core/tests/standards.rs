//! The standards database is well formed, and every dimension the drawing code takes from a standard (`standards.rs`) lies within the limits
//! the database records for it.

use road_builder_core::standards::*;
use serde_json::Value;

fn db() -> Value {
    serde_json::from_str(include_str!("../../library/standards.json")).expect("library/standards.json is valid JSON")
}

fn entry(d: &Value, id: &str) -> Value {
    d["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap_or_else(|| panic!("no entry {id}")).clone()
}

/// The value of an entry whose name starts with `name`.
fn value(e: &Value, name: &str) -> Value {
    e["values"].as_array().unwrap().iter().find(|v| v["name"].as_str().is_some_and(|n| n.starts_with(name))).unwrap_or_else(|| panic!("no value {name} in {}", e["id"])).clone()
}

fn f(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or_else(|| panic!("{key} missing in {v}"))
}

fn within(x: f64, lo: f64, hi: f64, what: &str) {
    assert!(x >= lo - 1e-9 && x <= hi + 1e-9, "{what}: {x} m is outside {lo}..{hi} m");
}

#[test]
fn the_database_is_well_formed() {
    let d = db();
    let sources = d["sources"].as_object().unwrap();
    let mut ids = std::collections::HashSet::new();
    for e in d["entries"].as_array().unwrap() {
        let id = e["id"].as_str().unwrap();
        assert!(ids.insert(id.to_string()), "duplicate id {id}");
        let status = e["status"].as_str().unwrap();
        assert!(["verified", "verified-secondary", "unverified"].contains(&status), "{id}: status {status}");
        if status != "unverified" {
            let src = e["source"].as_str().unwrap_or_else(|| panic!("{id}: no source"));
            assert!(sources.contains_key(src), "{id}: unknown source {src}");
            assert!(e["section"].as_str().is_some_and(|s| !s.is_empty()) && e["quote"].as_str().is_some_and(|s| !s.is_empty()), "{id}: a verified entry names its section and quotes it");
            if status == "verified" {
                assert_eq!(sources[src]["kind"], "federal-standard", "{id}: only a federal source is 'verified'");
            }
        }
        for v in e["values"].as_array().unwrap_or(&vec![]) {
            for (us, si) in [("us", "si"), ("min_us", "min_si"), ("max_us", "max_si")] {
                let (Some(a), Some(b)) = (v[us].as_f64(), v[si].as_f64()) else { continue };
                let factor = match v["unit_us"].as_str().unwrap() { "in" => 0.0254, "ft" => 0.3048, "mph" => 0.44704, u => panic!("{id}: unit {u}") };
                assert!((a * factor - b).abs() <= (b.abs() * 0.002).max(0.0006), "{id} ({}): {a} is not {b}", v["name"]);
            }
        }
        for c in e["code"].as_array().unwrap_or(&vec![]) {
            assert!(c.get("conforms").is_some() && c["name"].is_string() && c["note"].is_string(), "{id}: a code entry needs name, conforms and note");
        }
    }
    assert!(ids.len() >= 50, "the database has {} entries", ids.len());
    for k in ["mutcd", "prowag"] {
        assert_eq!(sources[k]["kind"], "federal-standard");
    }
}

#[test]
fn the_database_records_no_known_gaps() {
    // Everything the plug-in draws that a standard covers now conforms; a new gap must be recorded as conforms: false and listed here on purpose.
    let d = db();
    let gaps: Vec<String> = d["entries"].as_array().unwrap().iter().flat_map(|e| e["code"].as_array().unwrap_or(&vec![]).iter().filter(|c| c["conforms"] == false).map(|_| e["id"].as_str().unwrap().to_string()).collect::<Vec<_>>()).collect();
    assert!(gaps.is_empty(), "{gaps:?}");
}

#[test]
fn marking_dimensions_are_within_the_mutcd() {
    let d = db();
    let w = value(&entry(&d, "stop-line.width"), "width");
    within(STOP_LINE_WIDTH, f(&w, "min_si"), f(&w, "max_si"), "stop line width");
    let s = value(&entry(&d, "stop-line.setback"), "minimum in advance");
    assert!(STOP_LINE_SETBACK_MIN >= f(&s, "si") - 1e-9, "stop line setback {STOP_LINE_SETBACK_MIN} < {}", f(&s, "si"));

    let bars = entry(&d, "crosswalk.longitudinal-bars");
    let bw = value(&bars, "bar width");
    within(CROSSWALK_BAR_WIDTH, f(&bw, "min_si"), f(&bw, "max_si"), "crosswalk bar width");
    let sp = value(&bars, "lateral spacing");
    let space = CROSSWALK_BAR_PITCH - CROSSWALK_BAR_WIDTH;
    within(space, f(&sp, "min_si"), f(&sp, "max_si"), "crosswalk bar spacing");
    assert!(space <= 2.5 * CROSSWALK_BAR_WIDTH + 1e-9, "spacing {space} over 2.5 bar widths");
    assert!(CROSSWALK_DEPTH >= f(&value(&entry(&d, "crosswalk.min-width"), "minimum width"), "si") - 1e-9);

    let y = entry(&d, "yield-line.triangles");
    within(YIELD_TRIANGLE_BASE, f(&value(&y, "triangle base"), "min_si"), f(&value(&y, "triangle base"), "max_si"), "yield triangle base");
    within(YIELD_TRIANGLE_SPACE, f(&value(&y, "space between"), "min_si"), f(&value(&y, "space between"), "max_si"), "yield triangle space");
    assert!((YIELD_TRIANGLE_HEIGHT - 1.5 * YIELD_TRIANGLE_BASE).abs() < 1e-9, "yield triangle height is 1.5 x the base");

    let dotted = entry(&d, "line.dotted-lane");
    assert!((DOTTED_LANE_ON - f(&value(&dotted, "segment"), "si")).abs() < 0.002 && (DOTTED_LANE_OFF - f(&value(&dotted, "gap"), "si")).abs() < 0.002, "dotted lane line is 3 ft on, 9 ft off");
    assert!(MARKING_LINE_MIN >= f(&value(&entry(&d, "line.normal-width"), "width"), "min_si") - 1e-9, "thinnest marking line");
    let gore = entry(&d, "gore.chevron");
    assert!(GORE_CHEVRON_WIDTH >= f(&value(&gore, "width at a speed limit of 45 mph or more"), "si") - 1e-9, "gore chevron width");
    assert!((30.0..=45.0).contains(&GORE_CHEVRON_ANGLE_DEG), "gore chevron angle");
}

#[test]
fn crossing_and_sign_dimensions_are_within_the_standards() {
    let d = db();
    let m = value(&entry(&d, "stop-line.uncontrolled-multilane"), "distance in advance");
    within(MIDBLOCK_YIELD_SETBACK, f(&m, "min_si"), f(&m, "max_si"), "mid-block yield line setback");
    assert!(ROUNDABOUT_CROSSWALK_SETBACK >= f(&value(&entry(&d, "crosswalk.roundabout-setback"), "minimum distance"), "si") - 1e-9, "roundabout crosswalk setback");
    assert!(RAIL_STOP_LINE_MIN >= f(&value(&entry(&d, "rail.stop-line"), "passive crossing"), "si") - 1e-9, "railroad stop line");
    assert!(SIGNAL_MIN_BEYOND_STOP_LINE >= f(&value(&entry(&d, "signal.position"), "distance beyond the stop line, minimum"), "si") - 1e-9, "primary signal head distance");
    // An octagon of circumradius r is 2 r cos(22.5 degrees) across its flats: 30 in (single lane) and 36 in (multi-lane).
    let flats = |r: f64| 2.0 * r * (std::f64::consts::PI / 8.0).cos();
    let sign = entry(&d, "sign.stop-size");
    assert!(flats(STOP_SIGN_RADIUS_SINGLE) >= f(&value(&sign, "conventional road, single lane"), "si") - 0.002, "single-lane STOP sign");
    assert!(flats(STOP_SIGN_RADIUS_MULTI) >= f(&value(&sign, "conventional road, multi-lane"), "si") - 0.002, "multi-lane STOP sign");
}
