use road_builder_core::junction::{self, Control, JunctionSpec};
use road_builder_core::library::Library;
use road_builder_core::output::Out;
use road_builder_core::roads::Spec;
use road_builder_core::vcdoc::Item;
use serde_json::{Value, json};

const STORY: &str = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";

/// Road pieces a (south) and b (north) with a connecting line x between them, drawn north (document y runs down the page).
fn items() -> Vec<Item> {
    let line = |id, name: &str, pts: &[[f64; 2]]| Item { id: Some(id), name: name.into(), subs: vec![pts.to_vec()] };
    vec![
        line(1, "road.a", &[[200.0, 700.0], [200.0, 420.0]]),
        line(2, "road.b", &[[200.0, 300.0], [200.0, 20.0]]),
        line(3, "junction.x", &[[200.0, 420.0], [200.0, 300.0]]),
    ]
}

fn spec_with(junction: Value) -> Spec {
    Spec::from_document_data(&json!({"segments": {"a": {"cross_section": STORY}, "b": {"cross_section": STORY}}, "junctions": {"x": junction}}))
}

fn run(spec: &Spec) -> (Value, Vec<String>) {
    let mut out = Out::new();
    let notes = junction::build(&items(), spec, &mut out);
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

fn four_way() -> Value {
    json!({"legs": [{"side": "right", "cross_section": "lane fwd | lane back", "length": 40}, {"side": "left", "cross_section": "lane fwd | lane back", "length": 40}], "control": "signal", "crosswalks": true, "radius": 6})
}

#[test]
fn a_four_way_junction_has_every_part() {
    let (out, notes) = run(&spec_with(four_way()));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    let has = |name: &str| assert!(n.iter().any(|x| x == name), "missing {name}; have {n:?}");
    for want in [
        "gen.junction.x.box", "gen.junction.x.corner.1.f", "gen.junction.x.corner.1.b", "gen.junction.x.corner.2.f", "gen.junction.x.corner.2.b",
        "gen.junction.x.kerb.1.f", "gen.junction.x.kerb.2.b", "gen.junction.x.crosswalk.road", "gen.junction.x.crosswalk.leg.1", "gen.junction.x.crosswalk.leg.2",
        "gen.junction.x.stop.road.f", "gen.junction.x.stop.road.b", "gen.junction.x.stop.leg.1", "gen.junction.x.stop.leg.2",
        "gen.junction.x.signal.road.f", "gen.junction.x.signal.leg.1", "gen.junction.x.signal.leg.2",
    ] {
        has(want);
    }
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.junction.x.road.surface.")).count(), 9, "the road continues through with its 9 elements");
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.junction.x.leg.1.surface.")).count(), 2, "each leg has its own two lanes");
}

#[test]
fn the_box_and_legs_sit_where_the_roads_cross() {
    let (out, _) = run(&spec_with(four_way()));
    // Connector runs y 420..300, so the junction centre is y = 360. The road is 21.6 m wide about x = 200 (edges 189.2 and 210.8, box 0.2 m wider).
    let (lo, hi) = bbox(&pts(&out, "gen.junction.x.box"));
    assert!((lo[0] - 189.0).abs() < 1e-3 && (hi[0] - 211.0).abs() < 1e-3, "box spans the road: {lo:?} {hi:?}");
    assert!((lo[1] - 356.4).abs() < 1e-3 && (hi[1] - 363.6).abs() < 1e-3, "box is as wide as the legs (7.2 m): {lo:?} {hi:?}");
    // Right leg (east, since the line is drawn north) runs from the road edge 40 m outwards.
    let (lo, hi) = bbox(&pts(&out, "gen.junction.x.leg.1.surface.0.lane"));
    assert!((lo[0] - 210.8).abs() < 1e-3 && (hi[0] - 250.8).abs() < 1e-3, "{lo:?} {hi:?}");
    // Left leg (west) mirrors it.
    let (lo, hi) = bbox(&pts(&out, "gen.junction.x.leg.2.surface.0.lane"));
    assert!((hi[0] - 189.2).abs() < 1e-3 && (lo[0] - 149.2).abs() < 1e-3, "{lo:?} {hi:?}");
}

#[test]
fn kerb_corners_are_true_arcs() {
    let (out, _) = run(&spec_with(four_way()));
    // Right leg, forward (north) corner: centre of the arc is 6 m out from the road edge and 6 m beyond the leg's edge.
    let centre = [210.8 + 6.0, 360.0 - (3.6 + 6.0)];
    let arc = pts(&out, "gen.junction.x.kerb.1.f");
    assert_eq!(arc.len(), 13);
    for p in &arc {
        assert!(((p[0] - centre[0]).hypot(p[1] - centre[1]) - 6.0).abs() < 1e-2, "{p:?} is not 6 m from {centre:?}");
    }
}

#[test]
fn crosswalks_and_control_only_appear_when_asked_for() {
    let mut j = four_way();
    j["crosswalks"] = json!(false);
    j["control"] = json!("none");
    let n = names(&run(&spec_with(j)).0);
    assert!(!n.iter().any(|x| x.contains("crosswalk") || x.contains(".stop.") || x.contains("signal") || x.contains("sign.")), "{n:?}");
    // Stop signs go on the side legs only; the main road is not stopped.
    let mut j = four_way();
    j["control"] = json!("stop");
    let n = names(&run(&spec_with(j)).0);
    assert!(n.iter().any(|x| x == "gen.junction.x.sign.leg.1") && n.iter().any(|x| x == "gen.junction.x.stop.leg.2"));
    assert!(!n.iter().any(|x| x.contains(".stop.road") || x.contains("signal")), "{n:?}");
}

#[test]
fn a_tee_keeps_the_far_side_of_the_road() {
    let (out, _) = run(&spec_with(json!({"legs": [{"side": "east", "cross_section": "lane fwd | lane back"}], "control": "none"})));
    let (lo, hi) = bbox(&pts(&out, "gen.junction.x.box"));
    // On the west (no leg) side the box stops at the outer travel lane (200 - 8.7), so the bike lane and apron carry on across.
    assert!((lo[0] - 191.3).abs() < 1e-3, "{lo:?}");
    assert!((hi[0] - 211.0).abs() < 1e-3, "{hi:?}");
    let n = names(&out);
    assert!(n.iter().any(|x| x.starts_with("gen.junction.x.leg.1.")) && !n.iter().any(|x| x.starts_with("gen.junction.x.leg.2.")));
}

#[test]
fn compass_sides_follow_the_direction_the_line_is_drawn() {
    // The line runs north, so "west" is the left side: the leg goes to x < 189.
    let (out, notes) = run(&spec_with(json!({"legs": [{"side": "west", "cross_section": "lane fwd | lane back"}]})));
    assert!(notes.is_empty());
    let (lo, hi) = bbox(&pts(&out, "gen.junction.x.leg.1.surface.0.lane"));
    assert!(hi[0] < 190.0 && lo[0] < 160.0, "{lo:?} {hi:?}");
    // A leg that points along the road is refused politely.
    let (_, notes) = run(&spec_with(json!({"legs": [{"side": "north", "cross_section": "lane"}]})));
    assert!(notes.iter().any(|n| n.contains("runs along the road")), "{notes:?}");
}

#[test]
fn no_legs_just_carries_the_road_across_the_gap() {
    let (out, _) = run(&spec_with(json!({})));
    let n = names(&out);
    assert!(n.iter().all(|x| x.starts_with("gen.junction.x.road.")), "{n:?}");
    assert_eq!(n.iter().filter(|x| x.contains(".surface.")).count(), 9);
}

#[test]
fn a_junction_description_is_checked() {
    let lib = Library::builtin();
    let ok = JunctionSpec::from_json(&four_way(), &lib).unwrap();
    assert_eq!((ok.kind(), ok.control, ok.crosswalks), ("cross", Control::Signal, true));
    assert_eq!(ok.to_json()["legs"][0]["cross_section"], "lane fwd | lane back", "stored in canonical form");
    let err = |v: Value| JunctionSpec::from_json(&v, &lib).unwrap_err();
    assert!(err(json!({"legs": [{"side": "up"}]})).contains("side must be"));
    assert!(err(json!({"legs": [{"side": "left"}, {"side": "left"}]})).contains("same side"));
    assert!(err(json!({"legs": [{"side": "left"}, {"side": "right"}, {"side": "east"}]})).contains("at most two legs"));
    assert!(err(json!({"control": "roundabout"})).contains("control must be"));
    assert!(err(json!({"legs": [{"side": "left", "cross_section": "lane | warp-drive"}]})).contains("leg 1: unknown element"));
    let clamped = JunctionSpec::from_json(&json!({"radius": 99, "legs": [{"side": "e", "length": 1}]}), &lib).unwrap();
    assert_eq!((clamped.radius, clamped.legs[0].length, clamped.legs[0].side.as_str()), (20.0, 5.0, "east"));
}
