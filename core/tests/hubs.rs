use road_builder_core::builder::set_hub_text;
use road_builder_core::hub::{self, HubSpec};
use road_builder_core::output::Out;
use road_builder_core::roads::Spec;
use road_builder_core::vcdoc::Item;
use serde_json::{Value, json};

const TWO_LANE: &str = "lane back | lane fwd";
const STORY: &str = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";

fn line(id: i64, name: &str, pts: &[[f64; 2]]) -> Item {
    Item { id: Some(id), name: name.into(), subs: vec![pts.to_vec()] }
}

fn spec(segments: Value, hub: Value) -> Spec {
    Spec::from_document_data(&json!({"segments": segments, "hubs": {"h1": hub}}))
}

fn two_lane_segments() -> Value {
    json!({"a": {"cross_section": TWO_LANE}, "b": {"cross_section": TWO_LANE}, "c": {"cross_section": TWO_LANE}})
}

fn run(items: &[Item], sp: &Spec) -> (Value, Vec<String>) {
    let mut out = Out::new();
    let notes = hub::build(items, sp, &mut out);
    (serde_json::from_str(&out.finish()).unwrap(), notes)
}

fn names(v: &Value) -> Vec<String> {
    v["objects"].as_array().unwrap().iter().filter_map(|o| o["name"].as_str().map(String::from)).collect()
}

fn pts(v: &Value, name: &str) -> Vec<[f64; 2]> {
    let o = v["objects"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no object {name}"));
    o["path"]["subpaths"].as_array().unwrap().iter().flat_map(|s| s["anchors"].as_array().unwrap().iter().map(|a| [a["p"][0].as_f64().unwrap(), a["p"][1].as_f64().unwrap()])).collect()
}

fn bbox(p: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    (
        [p.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min), p.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min)],
        [p.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max), p.iter().map(|q| q[1]).fold(f64::NEG_INFINITY, f64::max)],
    )
}

fn has(v: &Value, name: &str) -> bool {
    names(v).iter().any(|n| n == name)
}

/// Three roads through (200, 400): a runs north-south, b east-west, c at 45 degrees (rising to the north-east).
fn through() -> Vec<Item> {
    vec![
        line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]),
        line(2, "road.b", &[[-100.0, 400.0], [500.0, 400.0]]),
        line(3, "road.c", &[[-100.0, 700.0], [500.0, 100.0]]),
    ]
}

fn signal(extra: Value) -> Value {
    let mut h = json!({"roads": ["a", "b", "c"], "treatment": "signal", "crosswalks": false});
    for (k, v) in extra.as_object().unwrap() {
        h[k] = v.clone();
    }
    h
}

#[test]
fn three_roads_through_one_point_make_six_arms_round_one_core() {
    let (out, notes) = run(&through(), &spec(two_lane_segments(), signal(json!({}))));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    assert!(has(&out, "gen.hub.h1.core"));
    // Six arms, each with a stop bar and a signal (three roads, two ends each).
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.hub.h1.stop.")).count(), 6, "{n:?}");
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.hub.h1.signal.") && !x.ends_with(".lamp") && !x.ends_with(".far")).count(), 6, "a near-side head at each stop bar");
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.hub.h1.signal.") && x.ends_with(".far")).count(), 6, "and a primary head on the far side of each");
    // Kerb returns at the corners between neighbouring arms (all six gaps are wide enough to round).
    assert!((1..=6).all(|k| has(&out, &format!("gen.hub.h1.kerb.{k}"))), "{n:?}");
    // The north arm of road a: its two neighbouring corners are (3.6, 8.7) with the 45-degree arm and (-3.6, 3.6) with the west arm, so the core
    // reaches 8.7 m along it; the stop bar is 1.6 m beyond (1 m gap and 0.6 m), over the southbound lanes (x 196.4..200), at document y 389.7.
    let (lo, hi) = bbox(&pts(&out, "gen.hub.h1.stop.a.after"));
    assert!((lo[0] - 196.4).abs() < 0.01 && (hi[0] - 200.0).abs() < 0.01 && (lo[1] - 389.7).abs() < 0.01, "{lo:?} {hi:?}");
    // Its primary signal head is on the far (south) side, at the driver's right (west) kerb: the bar is 10.3 m out and the head 20 m beyond it (over
    // the 12.2 m minimum), so 9.7 m south of the middle.
    let c = bbox(&pts(&out, "gen.hub.h1.signal.a.after.far"));
    let (cx, cy) = ((c.0[0] + c.1[0]) / 2.0, (c.0[1] + c.1[1]) / 2.0);
    assert!((cx - 195.5).abs() < 0.15 && (cy - 409.7).abs() < 0.15, "{cx} {cy}");
}

#[test]
fn touching_roads_make_a_four_arm_hub_and_are_carried_on_to_the_core() {
    // a runs north-south; b stops at x = 190 and c starts at x = 211, either side of it, both 10 m short of its middle.
    let its = vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]), line(2, "road.b", &[[100.0, 400.0], [190.0, 400.0]]), line(3, "road.c", &[[210.0, 400.0], [300.0, 400.0]])];
    let sp = spec(two_lane_segments(), signal(json!({"crosswalks": true})));
    let (out, notes) = run(&its, &sp);
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.hub.h1.crosswalk.")).count(), 4, "four arms: {n:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.hub.h1.core"));
    // A square of side 7.2 m, each corner pushed out 0.25 m along its diagonal (0.177 m on each axis).
    assert!((lo[0] - 196.223).abs() < 0.01 && (hi[0] - 203.777).abs() < 0.01 && (lo[1] - 396.223).abs() < 0.01 && (hi[1] - 403.777).abs() < 0.01, "{lo:?} {hi:?}");
    // The two roads that stop short are carried on past the middle to the far edge of the core (3.6 m beyond it).
    let trims = hub::trims(&its, &sp);
    assert_eq!(trims.len(), 2);
    assert!(trims.iter().all(|t| (t.dist - 3.6).abs() < 0.01), "{:?}", trims.iter().map(|t| t.dist).collect::<Vec<_>>());
    let mut drawn = Out::new();
    road_builder_core::roads::build_clipped(&its, &sp, &trims, &mut drawn);
    let v: Value = serde_json::from_str(&drawn.finish()).unwrap();
    let (_, hi) = bbox(&pts(&v, "gen.road.b.1.surface.0.lane"));
    // Each is carried across to the far edge of the core, under the core's paving (as a stop that is carried to the other road's travel lane).
    assert!((hi[0] - 203.6).abs() < 0.01, "road b now meets the core: {hi:?}");
    let (lo, _) = bbox(&pts(&v, "gen.road.c.1.surface.0.lane"));
    assert!((lo[0] - 196.4).abs() < 0.01, "{lo:?}");
}

#[test]
fn a_two_way_stop_stops_the_narrow_roads_only() {
    let segs = json!({"a": {"cross_section": STORY}, "b": {"cross_section": TWO_LANE}, "c": {"cross_section": TWO_LANE}});
    let its = vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]), line(2, "road.b", &[[-100.0, 400.0], [500.0, 400.0]]), line(3, "road.c", &[[-100.0, 700.0], [500.0, 100.0]])];
    let (out, _) = run(&its, &spec(segs.clone(), signal(json!({"treatment": "two-way-stop"}))));
    let n = names(&out);
    assert!(n.iter().any(|x| x.starts_with("gen.hub.h1.sign.b.")) && n.iter().any(|x| x.starts_with("gen.hub.h1.sign.c.")));
    assert!(!n.iter().any(|x| x.starts_with("gen.hub.h1.sign.a.")), "the wide road is not stopped: {n:?}");
    let (out, _) = run(&its, &spec(segs, signal(json!({"treatment": "two-way-stop", "minor": "a"}))));
    assert!(names(&out).iter().any(|x| x.starts_with("gen.hub.h1.sign.a.")) && !names(&out).iter().any(|x| x.starts_with("gen.hub.h1.sign.b.")));
}

#[test]
fn trimmed_stubs_drop_an_arm_and_kept_stubs_get_a_dead_end() {
    // a and b cross at (200, 400); c is a 15 m stub to the north-east of the point (it starts at the point and runs 15 m).
    let its = vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]), line(2, "road.b", &[[-100.0, 400.0], [500.0, 400.0]]), line(3, "road.c", &[[-100.0, 700.0], [210.6, 389.4]])];
    let keep = spec(two_lane_segments(), signal(json!({})));
    let (out, _) = run(&its, &keep);
    assert!(has(&out, "gen.hub.h1.deadend.c.after"), "{:?}", names(&out));
    let trim = spec(two_lane_segments(), signal(json!({"stub": "trim"})));
    let (out, _) = run(&its, &trim);
    assert!(!names(&out).iter().any(|x| x.contains(".c.after") || x.contains("deadend")), "{:?}", names(&out));
    assert_eq!(hub::trims(&its, &trim).len(), 1);
}

#[test]
fn a_hub_description_is_checked_in_plain_words() {
    let err = |v: Value| HubSpec::from_json(&v).unwrap_err();
    assert!(err(json!({"roads": ["a", "b"]})).contains("3 to 8 different roads"));
    assert!(err(json!({"roads": ["a", "a", "b"]})).contains("3 to 8 different roads"));
    assert!(err(json!({"roads": ["a", "b", "c"], "treatment": "roundabout"})).contains("a hub can be signal"));
    assert!(err(json!({"roads": ["a", "b", "c"], "minor": "z"})).contains("must be one of the roads"));
    assert!(err(json!({"roads": ["a", "b", "c"], "stub": "fold"})).contains("stub must be keep or trim"));
    let ok = HubSpec::from_json(&json!({"roads": ["a", "b", "c"], "treatment": "lighted", "crosswalks": true})).unwrap();
    assert!(ok.summary().contains("3 roads meet (a, b, c): traffic signals, crosswalks"));
    // Roads that do not meet, or too far apart.
    let apart = vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]), line(2, "road.b", &[[-100.0, 400.0], [500.0, 400.0]]), line(3, "road.c", &[[400.0, 700.0], [400.0, 500.0]])];
    let (_, notes) = run(&apart, &spec(two_lane_segments(), signal(json!({}))));
    assert!(notes.iter().any(|n| n.contains("does not pass through the meeting point")), "{notes:?}");
}

#[test]
fn the_command_stores_a_hub_for_three_selected_roads() {
    let its = through();
    let input = json!({"objects": its.iter().map(|i| json!({"id": i.id, "name": i.name, "type": "path", "path": {"subpaths": [{"anchors": i.subs[0].iter().map(|p| json!({"p": p})).collect::<Vec<_>>(), "closed": false}]}})).collect::<Vec<_>>()});
    let data = json!({"segments": two_lane_segments()});
    let params = |d: &str| json!({"description": d, "_context": {"documentData": data.clone()}});
    let out: Value = serde_json::from_str(&set_hub_text(&input, &params(r#"{"treatment": "four-way-stop", "crosswalks": true}"#)).unwrap()).unwrap();
    assert_eq!(out["documentData"]["hubs"]["h1"]["roads"], json!(["a", "b", "c"]));
    assert_eq!(out["documentData"]["hubs"]["h1"]["treatment"], "four-way-stop");
    // Two roads is a crossing, not a hub.
    let two = json!({"objects": input["objects"].as_array().unwrap()[..2].to_vec()});
    assert!(set_hub_text(&two, &params("{}")).unwrap_err().contains("three or more roads"));
}
