use road_builder_core::builder::{build_text, set_transition_text};
use road_builder_core::crosssection::{Anchor, Dir, format, layout, parse};
use road_builder_core::library::Library;
use road_builder_core::output::Out;
use road_builder_core::roads::{Spec, build_clipped};
use road_builder_core::transition::{Change, Side, TransitionSpec, derive_to};
use road_builder_core::vcdoc::Item;
use serde_json::{Value, json};

const TWO: &str = "lane back | lane fwd";

fn road(cross_section: &str, transitions: Value) -> (Vec<Item>, Spec) {
    // Road a runs north from (200, 700) to (200, 100): 600 m. Document y runs down the page.
    let items = vec![Item { id: Some(1), name: "road.a".into(), subs: vec![vec![[200.0, 700.0], [200.0, 100.0]]] }];
    let spec = Spec::from_document_data(&json!({"segments": {"a": {"cross_section": cross_section}}, "transitions": {"a": transitions}}));
    (items, spec)
}

fn build(items: &[Item], spec: &Spec) -> (Value, Vec<String>) {
    let mut out = Out::new();
    let notes = build_clipped(items, spec, &[], &mut out);
    (serde_json::from_str(&out.finish()).unwrap(), notes)
}

fn names(v: &Value) -> Vec<String> {
    v["objects"].as_array().unwrap().iter().filter_map(|o| o["name"].as_str().map(String::from)).collect()
}

fn pts(v: &Value, name: &str) -> Vec<[f64; 2]> {
    let o = v["objects"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no object {name}: {:?}", names(v)));
    o["path"]["subpaths"].as_array().unwrap().iter().flat_map(|s| s["anchors"].as_array().unwrap().iter().map(|a| [a["p"][0].as_f64().unwrap(), a["p"][1].as_f64().unwrap()])).collect()
}

fn bbox(p: &[[f64; 2]]) -> ([f64; 2], [f64; 2]) {
    (
        [p.iter().map(|q| q[0]).fold(f64::INFINITY, f64::min), p.iter().map(|q| q[1]).fold(f64::INFINITY, f64::min)],
        [p.iter().map(|q| q[0]).fold(f64::NEG_INFINITY, f64::max), p.iter().map(|q| q[1]).fold(f64::NEG_INFINITY, f64::max)],
    )
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.02
}

fn spec_of(v: Value) -> TransitionSpec {
    TransitionSpec::from_json(&v).unwrap()
}

#[test]
fn adding_a_lane_on_the_right_widens_the_edge_and_keeps_the_other_lanes_in_place() {
    let (items, spec) = road(TWO, json!([{"piece": 1, "at": 200, "length": 60, "change": "add", "side": "right"}]));
    let (v, notes) = build(&items, &spec);
    assert!(notes.is_empty(), "{notes:?}");
    // Before the taper (document y 700 down to 500): two lanes, x 196.4..200 (back) and 200..203.6 (fwd).
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.surface.1.lane"));
    assert!(near(lo[0], 200.0) && near(hi[0], 203.6) && near(lo[1], 500.0) && near(hi[1], 700.0), "{lo:?} {hi:?}");
    // The taper (y 500 to 440): the new lane grows from nothing at the edge of the old road (x 203.6) to a full lane (to x 207.2).
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.taper1.surface.2.lane"));
    assert!(near(lo[0], 203.6) && near(hi[0], 207.2) && near(lo[1], 440.0) && near(hi[1], 500.0), "{lo:?} {hi:?}");
    // The opposing lane did not move.
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.taper1.surface.0.lane"));
    assert!(near(lo[0], 196.4) && near(hi[0], 200.0), "{lo:?} {hi:?}");
    // After the taper (y 440 to 100): three lanes.
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.after1.surface.2.lane"));
    assert!(near(lo[0], 203.6) && near(hi[0], 207.2) && near(lo[1], 100.0) && near(hi[1], 440.0), "{lo:?} {hi:?}");
    assert!(names(&v).iter().any(|n| n == "gen.road.a.1.after1.line.3"), "the outer edge is drawn after the taper");
}

#[test]
fn dropping_the_lane_nearest_the_centre_narrows_the_outer_edge_toward_it() {
    // One lane against the line and three with it: x -7.2..-3.6 (back), then three fwd lanes of 3.6 m each.
    let (items, spec) = road("lane back | lane fwd x3", json!([{"piece": 1, "at": 100, "length": 50, "change": "drop", "side": "left"}]));
    let (v, _) = build(&items, &spec);
    // The lane that ends is the first with the line (the driver's left is toward the centre): x 196.4..200 over the taper (y 600 to 550).
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.taper1.surface.1.lane"));
    assert!(near(lo[0], 196.4) && near(hi[0], 200.0) && near(lo[1], 550.0) && near(hi[1], 600.0), "ends: {lo:?} {hi:?}");
    // The two lanes beyond it slide 3.6 m toward the centre: after the taper they are x 196.4..200 and 200..203.6.
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.after1.surface.1.lane"));
    assert!(near(lo[0], 196.4) && near(hi[0], 200.0), "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.after1.surface.2.lane"));
    assert!(near(lo[0], 200.0) && near(hi[0], 203.6), "{lo:?} {hi:?}");
    // The lane against the line never moved.
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.after1.surface.0.lane"));
    assert!(near(lo[0], 192.8) && near(hi[0], 196.4), "{lo:?} {hi:?}");
}

#[test]
fn a_lane_against_the_line_is_added_on_its_outer_edge_for_a_right_change() {
    // For lanes against the line the driver's right is the left edge of the road (west, for a line drawn north).
    let (items, spec) = road(TWO, json!([{"piece": 1, "at": 200, "length": 60, "change": "add", "side": "right", "direction": "back"}]));
    let (v, _) = build(&items, &spec);
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.taper1.surface.0.lane"));
    assert!(near(lo[0], 192.8) && near(hi[0], 196.4) && near(lo[1], 440.0) && near(hi[1], 500.0), "{lo:?} {hi:?}");
    // Both lanes with the line and the old one against it stay where they were.
    let (lo, hi) = bbox(&pts(&v, "gen.road.a.1.after1.surface.2.lane"));
    assert!(near(lo[0], 200.0) && near(hi[0], 203.6), "{lo:?} {hi:?}");
}

#[test]
fn one_to_four_lanes_and_back_in_a_series() {
    // 1 lane each way, then +2 on the right, then -1 toward the centre, then -2: tapers and plain stretches alternate and are all named.
    let list = json!([
        {"piece": 1, "at": 100, "length": 40, "change": "add", "side": "right", "count": 2},
        {"piece": 1, "at": 250, "length": 40, "change": "drop", "side": "left"},
        {"piece": 1, "at": 400, "length": 40, "change": "drop", "side": "right"}
    ]);
    let (items, spec) = road(TWO, list);
    let (v, notes) = build(&items, &spec);
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&v);
    for tag in ["taper1", "after1", "taper2", "after2", "taper3", "after3"] {
        assert!(n.iter().any(|x| x.starts_with(&format!("gen.road.a.1.{tag}."))), "missing {tag}");
    }
    // Three lanes with the line after the first change, two after the second, one after the third.
    let lanes = |tag: &str| n.iter().filter(|x| x.starts_with(&format!("gen.road.a.1.{tag}.surface.")) && x.ends_with(".lane")).count();
    assert_eq!((lanes("after1"), lanes("after2"), lanes("after3")), (4, 3, 2), "back lane plus 3, 2, 1 lanes with the line");
}

#[test]
fn the_taper_follows_the_mutcd_formula_unless_a_length_is_given() {
    let t = |v: Value| spec_of(json!({"change": "add", "side": "right"})).taper(v.as_f64().unwrap());
    let _ = t;
    let base = |speed: f64, length: Option<f64>| {
        let mut j = json!({"change": "add", "side": "right", "speed": speed});
        if let Some(l) = length {
            j["length"] = json!(l);
        }
        spec_of(j)
    };
    // 3.6 m is 11.81 ft: at 35 mph L = 11.81 x 35^2 / 60 = 241.1 ft = 73.5 m; at 55 mph L = 11.81 x 55 = 649.6 ft = 198.0 m.
    assert!((base(35.0, None).taper(3.6) - 73.5).abs() < 0.2, "{}", base(35.0, None).taper(3.6));
    assert!((base(55.0, None).taper(3.6) - 198.0).abs() < 0.3);
    // Never under 100 ft (30.5 m).
    assert!((base(15.0, None).taper(3.6) - 30.48).abs() < 0.01);
    assert_eq!(base(55.0, Some(120.0)).taper(3.6), 120.0);
}

#[test]
fn what_is_derived_and_what_is_refused() {
    let lib = Library::builtin();
    let head = [0.0, 1.0];
    let from = parse("bike-lane | lane back | lane fwd | bike-lane", &lib).unwrap();
    let add = |v: Value| derive_to(&from, &lib, head, &spec_of(v)).map(|p| format(&p));
    assert_eq!(add(json!({"change": "add", "side": "right"})).unwrap(), "bike-lane | lane back | lane fwd x2 | bike-lane", "the lane goes between the road and the bike lane");
    assert_eq!(add(json!({"change": "add", "side": "left", "count": 2})).unwrap(), "bike-lane | lane back | lane fwd x3 | bike-lane");
    assert!(add(json!({"change": "drop", "side": "left"})).unwrap_err().contains("last lane"));
    assert!(add(json!({"change": "add", "side": "left", "direction": "back"})).is_ok());
    let one_way = parse("lane fwd x2", &lib).unwrap();
    assert!(derive_to(&one_way, &lib, head, &spec_of(json!({"change": "add", "side": "left", "direction": "back"}))).unwrap_err().contains("no lanes against the line"));
    // A cross-section given outright is used as it is.
    assert_eq!(format(&derive_to(&from, &lib, head, &spec_of(json!({"change": "add", "side": "left", "to": "lane back | lane fwd x4"}))).unwrap()), "lane back | lane fwd x4");
    // The layout of the result is a valid road.
    let after = derive_to(&from, &lib, head, &spec_of(json!({"change": "add", "side": "right"}))).unwrap();
    let placed = layout(&after, &lib, head, Anchor::Auto).unwrap();
    assert!((placed.last().unwrap().right - placed[0].left - (1.5 * 2.0 + 3.6 * 3.0)).abs() < 1e-9);
    assert_eq!(placed.iter().filter(|e| e.dir == Some(Dir::Fwd)).count(), 2);
}

#[test]
fn a_transition_description_is_checked_in_plain_words() {
    let err = |v: Value| TransitionSpec::from_json(&v).unwrap_err();
    assert!(err(json!({"side": "left"})).contains("needs \"change\""));
    assert!(err(json!({"change": "add"})).contains("needs \"side\""));
    assert!(err(json!({"change": "grow", "side": "left"})).contains("change must be add or drop"));
    assert!(err(json!({"change": "add", "side": "up"})).contains("side must be left or right"));
    assert!(err(json!({"change": "add", "side": "left", "direction": "sideways"})).contains("direction must be fwd or back"));
    let t = spec_of(json!({"change": "expand", "side": "outer", "direction": "with", "count": 9, "speed": 5}));
    assert_eq!((t.change, t.side, t.direction, t.count, t.speed), (Change::Add, Side::Right, Dir::Fwd, 3, 15.0));
    assert!(t.summary().contains("adds 3 lanes on the right (toward the edge)"));
}

#[test]
fn problems_while_building_are_notes_not_failures() {
    let (items, spec) = road(TWO, json!([{"piece": 1, "at": 5000, "length": 40, "change": "add", "side": "right"}, {"piece": 1, "at": 500, "length": 300, "change": "add", "side": "left"}]));
    let (v, notes) = build(&items, &spec);
    assert!(notes.iter().any(|n| n.contains("outside the line")), "{notes:?}");
    assert!(notes.iter().any(|n| n.contains("shortened to 100 m")), "the taper cannot run past the end of the line: {notes:?}");
    assert!(names(&v).iter().any(|n| n.starts_with("gen.road.a.1.taper")));
    // A transition for another piece of the road does nothing here.
    let (items, spec) = road(TWO, json!([{"piece": 2, "at": 100, "length": 40, "change": "add", "side": "right"}]));
    let (v, _) = build(&items, &spec);
    assert!(!names(&v).iter().any(|n| n.contains("taper")));
}

#[test]
fn the_command_stores_replaces_and_removes_a_transition() {
    let its = vec![Item { id: Some(1), name: "road.a".into(), subs: vec![vec![[200.0, 700.0], [200.0, 100.0]]] }];
    let input = json!({"objects": its.iter().map(|i| json!({"id": i.id, "name": i.name, "type": "path", "path": {"subpaths": [{"anchors": i.subs[0].iter().map(|p| json!({"p": p})).collect::<Vec<_>>(), "closed": false}]}})).collect::<Vec<_>>()});
    let data = json!({"segments": {"a": {"cross_section": TWO}}});
    let run = |d: &str, data: &Value| set_transition_text(&input, &json!({"description": d, "_context": {"documentData": data.clone()}}));
    let out: Value = serde_json::from_str(&run(r#"{"change": "add", "side": "right", "at": 200, "length": 60}"#, &data).unwrap()).unwrap();
    assert_eq!(out["documentData"]["transitions"]["a"].as_array().unwrap().len(), 1);
    assert_eq!(out["documentData"]["transitions"]["a"][0]["change"], "add");
    assert_eq!(out["documentData"]["segments"]["a"]["cross_section"], TWO, "other data is kept");
    // Describing one within 5 m of the same place replaces it; one elsewhere is added.
    let again: Value = serde_json::from_str(&run(r#"{"change": "add", "side": "right", "at": 203, "count": 2}"#, &out["documentData"]).unwrap()).unwrap();
    assert_eq!(again["documentData"]["transitions"]["a"].as_array().unwrap().len(), 1);
    assert_eq!(again["documentData"]["transitions"]["a"][0]["count"], 2);
    let more: Value = serde_json::from_str(&run(r#"{"change": "drop", "side": "right", "at": 400}"#, &again["documentData"]).unwrap()).unwrap();
    assert_eq!(more["documentData"]["transitions"]["a"].as_array().unwrap().len(), 2);
    // Remove.
    let gone: Value = serde_json::from_str(&run(r#"{"change": "drop", "side": "right", "at": 400, "remove": true}"#, &more["documentData"]).unwrap()).unwrap();
    assert_eq!(gone["documentData"]["transitions"]["a"].as_array().unwrap().len(), 1);
    // Plain-words problems.
    assert!(run(r#"{"change": "drop", "side": "left"}"#, &data).unwrap_err().contains("last lane"));
    assert!(run("{nope", &data).unwrap_err().contains("not valid JSON"));
    let two = json!({"objects": [input["objects"][0].clone(), {"id": 2, "name": "road.b", "type": "path", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [1, 1]}], "closed": false}]}}]});
    assert!(set_transition_text(&two, &json!({"description": "{}", "_context": {"documentData": data}})).unwrap_err().contains("one line of one road"));
    // A build with a stored transition draws the taper and keeps the transitions in the document.
    let built: Value = serde_json::from_str(&build_text(&input, &json!({"_context": {"documentData": out["documentData"].clone()}})).unwrap()).unwrap();
    assert!(names(&built).iter().any(|n| n.starts_with("gen.road.a.1.taper1.")));
    assert_eq!(built["documentData"]["transitions"]["a"][0]["at"], 200.0);
}
