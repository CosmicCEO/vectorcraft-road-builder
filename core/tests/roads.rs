use road_builder_core::builder::{build, set_cross_section_text};
use serde_json::{Value, json};

const STORY: &str = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";

/// A filter-input path object with the given id and name; points are `[x, y]` in document space (y down).
fn line(id: i64, name: &str, pts: &[[f64; 2]]) -> Value {
    let anchors: Vec<Value> = pts.iter().map(|p| json!({"p": [p[0], p[1]]})).collect();
    json!({"id": id, "name": name, "type": "path", "path": {"subpaths": [{"anchors": anchors, "closed": false}]}})
}

fn params(spec: Value) -> Value {
    json!({"_context": {"mode": "filter", "documentData": spec}})
}

fn story_spec(id: &str) -> Value {
    json!({"version": 1, "segments": {id: {"cross_section": STORY, "anchor": "center"}}})
}

fn objects(out: &Value) -> Vec<&Value> {
    out["objects"].as_array().unwrap().iter().collect()
}

fn names<'a>(out: &'a Value, prefix: &str) -> Vec<&'a str> {
    objects(out).into_iter().filter_map(|o| o["name"].as_str()).filter(|n| n.starts_with(prefix)).collect()
}

fn points(o: &Value) -> Vec<[f64; 2]> {
    o["path"]["subpaths"].as_array().unwrap().iter().flat_map(|s| s["anchors"].as_array().unwrap().iter().map(|a| [a["p"][0].as_f64().unwrap(), a["p"][1].as_f64().unwrap()])).collect()
}

#[test]
fn a_road_gets_a_strip_per_element_and_lines_between_them() {
    let input = json!({"objects": [line(1, "road.a", &[[100.0, 500.0], [100.0, 100.0]])]});
    let out = build(&input, &params(story_spec("a"))).unwrap();
    assert_eq!(names(&out, "gen.road.a.1.surface.").len(), 9, "one strip per element");
    // outer edges (2) + apron|lane, lane|lane, lane|margin, margin|lane, lane|lane, lane|apron
    assert_eq!(names(&out, "gen.road.a.1.line.").len(), 8, "{:?}", names(&out, "gen.road.a.1.line."));
    assert!(objects(&out).iter().any(|o| o["id"] == 1), "the drawn line is kept");
    // Drawn north (up the page): the leftmost strip's outer edge is 10.8 m to the west of x = 100.
    let first = objects(&out).into_iter().find(|o| o["name"].as_str().is_some_and(|n| n.starts_with("gen.road.a.1.surface.0."))).unwrap();
    assert!(points(first).iter().any(|p| (p[0] - 89.2).abs() < 1e-6), "{:?}", points(first));
}

#[test]
fn a_90_degree_angle_point_comes_out_as_a_sharp_corner() {
    // Story 2: a road with an abrupt 90 degree turn. North 200 m, then east 200 m (document y runs down the page).
    let input = json!({"objects": [line(1, "road.a", &[[100.0, 500.0], [100.0, 300.0], [300.0, 300.0]])]});
    let out = build(&input, &params(story_spec("a"))).unwrap();
    let first = objects(&out).into_iter().find(|o| o["name"].as_str().is_some_and(|n| n.starts_with("gen.road.a.1.surface.0."))).unwrap();
    // The turn is to the right, so the road's left edge (10.8 m out) is the outside of the corner: a mitred point, not a cut corner.
    assert!(points(first).iter().any(|p| (p[0] - 89.2).abs() < 1e-2 && (p[1] - 289.2).abs() < 1e-2), "mitred outside corner: {:?}", points(first));
    // The right edge is the inside: trimmed at (110.8, 310.8), never running past it.
    let last = objects(&out).into_iter().find(|o| o["name"].as_str().is_some_and(|n| n.starts_with("gen.road.a.1.surface.8."))).unwrap();
    assert!(points(last).iter().any(|p| (p[0] - 110.8).abs() < 1e-2 && (p[1] - 310.8).abs() < 1e-2), "trimmed inside corner: {:?}", points(last));
}

#[test]
fn a_curve_stays_smooth() {
    let curved = json!({"id": 1, "name": "road.a", "type": "path", "path": {"subpaths": [{"anchors": [
        {"p": [100.0, 500.0], "out": [100.0, 400.0]},
        {"p": [200.0, 300.0], "in": [100.0, 300.0], "kind": "Smooth", "out": [300.0, 300.0]},
        {"p": [400.0, 100.0], "in": [400.0, 200.0]}
    ], "closed": false}]}});
    let out = build(&json!({"objects": [curved]}), &params(story_spec("a"))).unwrap();
    let first = objects(&out).into_iter().find(|o| o["name"].as_str().is_some_and(|n| n.starts_with("gen.road.a.1.surface.0."))).unwrap();
    assert!(points(first).len() > 30, "the curve is followed in many small steps, got {}", points(first).len());
    assert!(points(first).iter().all(|p| p[0].is_finite() && p[1].is_finite()));
}

#[test]
fn a_split_road_keeps_one_description_for_both_pieces() {
    let input = json!({"objects": [line(1, "road.a", &[[100.0, 500.0], [100.0, 300.0]]), line(2, "road.a", &[[100.0, 300.0], [100.0, 100.0]])]});
    let out = build(&input, &params(story_spec("a"))).unwrap();
    assert_eq!(names(&out, "gen.road.a.1.surface.").len(), 9);
    assert_eq!(names(&out, "gen.road.a.2.surface.").len(), 9);
}

#[test]
fn an_undescribed_road_is_two_lanes_and_a_bad_description_falls_back() {
    let input = json!({"objects": [line(1, "road.z", &[[0.0, 100.0], [0.0, 0.0]])]});
    let out = build(&input, &params(Value::Null)).unwrap();
    assert_eq!(names(&out, "gen.road.z.1.surface.").len(), 2, "default is two lanes");
    let bad = json!({"segments": {"z": {"cross_section": "lane | warp-drive"}}});
    let out = build(&input, &params(bad)).unwrap();
    assert_eq!(names(&out, "gen.road.z.1.surface.").len(), 2, "falls back to the default");
}

#[test]
fn a_rebuild_replaces_the_old_generated_shapes() {
    let mut objs = vec![line(1, "road.a", &[[100.0, 500.0], [100.0, 100.0]])];
    objs.push(line(9, "gen.road.a.1.surface.0.bike-lane", &[[0.0, 0.0], [1.0, 1.0]]));
    let out = build(&json!({"objects": objs}), &params(story_spec("a"))).unwrap();
    assert!(!objects(&out).iter().any(|o| o["id"] == 9), "the old generated shape is dropped");
}

#[test]
fn nothing_to_build_explains_itself() {
    let e = build(&json!({"objects": [line(1, "something", &[[0.0, 0.0], [1.0, 1.0]])]}), &params(Value::Null)).unwrap_err();
    assert!(e.contains("road.<id>"), "{e}");
}

#[test]
fn describing_a_plain_line_turns_it_into_a_road_and_remembers_the_description() {
    let input = json!({"objects": [line(7, "", &[[0.0, 100.0], [0.0, 0.0]])]});
    let p = json!({"cross_section": STORY, "_context": {"command": "set_cross_section", "documentData": null}});
    let out: Value = serde_json::from_str(&set_cross_section_text(&input, &p).unwrap()).unwrap();
    let first = &out["objects"][0];
    assert_eq!((first["id"].as_i64(), first["name"].as_str()), (Some(7), Some("road.r1")), "renamed in place");
    assert_eq!(out["documentData"]["segments"]["r1"]["cross_section"], STORY, "stored in canonical form");

    // Describe it again later: the new text replaces the old, other plug-in data is left alone.
    let mut data = out["documentData"].clone();
    data["other"] = json!({"keep": true});
    let input = json!({"objects": [line(7, "road.r1", &[[0.0, 100.0], [0.0, 0.0]])]});
    let p = json!({"cross_section": "lane x2", "_context": {"documentData": data}});
    let out: Value = serde_json::from_str(&set_cross_section_text(&input, &p).unwrap()).unwrap();
    assert_eq!(out["documentData"]["segments"]["r1"]["cross_section"], "lane x2");
    assert_eq!(out["documentData"]["other"], json!({"keep": true}));
    assert!(out["objects"][0].get("name").is_none(), "already a road: not renamed");
}

#[test]
fn describing_reports_problems_in_plain_words() {
    let two = json!({"objects": [line(1, "", &[[0.0, 1.0], [0.0, 0.0]]), line(2, "", &[[5.0, 1.0], [5.0, 0.0]])]});
    let p = json!({"cross_section": "lane", "_context": {}});
    assert!(set_cross_section_text(&two, &p).unwrap_err().contains("select the one line"));
    assert!(set_cross_section_text(&json!({"objects": []}), &p).unwrap_err().contains("Select the line"));
    let one = json!({"objects": [line(1, "", &[[0.0, 1.0], [0.0, 0.0]])]});
    let bad = json!({"cross_section": "lane | trolley", "_context": {}});
    assert!(set_cross_section_text(&one, &bad).unwrap_err().contains("unknown element \"trolley\""));
    let none = json!({"cross_section": "  ", "_context": {}});
    assert!(set_cross_section_text(&one, &none).unwrap_err().contains("Write the cross-section"));
}

#[test]
fn describing_selected_pieces_only_leaves_the_other_pieces_alone() {
    // The road r1 was split in two; only the north piece (id 2) is selected.
    let input = json!({"objects": [line(2, "road.r1", &[[100.0, 300.0], [100.0, 100.0]])]});
    let data = story_spec("r1");
    let p = json!({"cross_section": "lane back | lane fwd", "scope": "selected", "_context": {"documentData": data}});
    let out: Value = serde_json::from_str(&set_cross_section_text(&input, &p).unwrap()).unwrap();
    assert_eq!(out["objects"][0]["name"], "road.r2", "the selected piece moves to a new id");
    assert_eq!(out["documentData"]["segments"]["r2"]["cross_section"], "lane back | lane fwd");
    assert_eq!(out["documentData"]["segments"]["r1"]["cross_section"], STORY, "the other piece keeps its description");
}

#[test]
fn removing_a_side_element_does_not_move_the_median_off_the_line() {
    let margin_x = |text: &str| -> (f64, f64) {
        let spec = json!({"segments": {"a": {"cross_section": text}}});
        let out = build(&json!({"objects": [line(1, "road.a", &[[100.0, 500.0], [100.0, 100.0]])]}), &params(spec)).unwrap();
        let strip = objects(&out).into_iter().find(|o| o["name"].as_str().is_some_and(|n| n.ends_with(".margin"))).unwrap();
        let xs: Vec<f64> = points(strip).iter().map(|p| p[0]).collect();
        (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
    };
    let both = margin_x(STORY);
    let without_west_bike_lane = margin_x("facing N: apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane");
    assert!((both.0 - without_west_bike_lane.0).abs() < 1e-9 && (both.1 - without_west_bike_lane.1).abs() < 1e-9, "{both:?} vs {without_west_bike_lane:?}");
    assert!((both.0 - 98.5).abs() < 1e-9 && (both.1 - 101.5).abs() < 1e-9, "the 3 m margin straddles the drawn line: {both:?}");
}

#[test]
fn a_junction_line_is_built_and_described_through_the_commands() {
    use road_builder_core::builder::set_junction_text;
    // A road piece on each side of a connecting plain line.
    let objs = vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 420.0]]), line(2, "road.b", &[[200.0, 300.0], [200.0, 20.0]]), line(3, "", &[[200.0, 420.0], [200.0, 300.0]])];
    let desc = json!({"legs": [{"side": "west", "cross_section": "lane back | lane fwd"}, {"side": "east", "cross_section": "lane back | lane fwd"}], "control": "signal", "crosswalks": true}).to_string();
    let data = json!({"segments": {"a": {"cross_section": STORY}, "b": {"cross_section": STORY}}});
    let out: Value = serde_json::from_str(&set_junction_text(&json!({"objects": [objs[2].clone()]}), &json!({"description": desc, "_context": {"documentData": data}})).unwrap()).unwrap();
    assert_eq!((out["objects"][0]["id"].as_i64(), out["objects"][0]["name"].as_str()), (Some(3), Some("junction.j1")), "the plain line becomes a junction");
    assert_eq!(out["documentData"]["junctions"]["j1"]["kind"], "cross");
    assert_eq!(out["documentData"]["segments"]["a"]["cross_section"], STORY, "other stored data is kept");

    // Build everything: roads, then the junction on top.
    let objs = vec![objs[0].clone(), objs[1].clone(), line(3, "junction.j1", &[[200.0, 420.0], [200.0, 300.0]])];
    let built = build(&json!({"objects": objs}), &params(out["documentData"].clone())).unwrap();
    assert!(names(&built, "gen.junction.j1.box").len() == 1 && names(&built, "gen.junction.j1.leg.").len() >= 4, "{:?}", names(&built, "gen.junction"));
    assert_eq!(names(&built, "gen.road.a.1.surface.").len(), 9, "the roads on either side are still built");

    // Bad descriptions come back as plain sentences.
    let plain = json!({"objects": [line(3, "", &[[0.0, 1.0], [0.0, 0.0]])]});
    let bad = |d: &str| set_junction_text(&plain, &json!({"description": d, "_context": {}})).unwrap_err();
    assert!(bad("{not json").contains("not valid JSON"));
    assert!(bad(r#"{"control": "roundabout"}"#).contains("control must be"));
    assert!(set_junction_text(&json!({"objects": []}), &json!({"description": "{}", "_context": {}})).unwrap_err().contains("Select the connecting line"));
}

#[test]
fn the_drawn_line_stays_on_top_and_is_drawn_bold() {
    let input = json!({"objects": [line(1, "road.a", &[[100.0, 500.0], [100.0, 100.0]]), line(2, "other", &[[0.0, 0.0], [1.0, 1.0]])]});
    let out = build(&input, &params(story_spec("a"))).unwrap();
    let list = objects(&out);
    let first_kept = list.iter().position(|o| o.get("path").is_none()).unwrap();
    // New objects are listed before every kept object, so VectorCraft stacks them below the lines you drew.
    assert!(list[..first_kept].iter().all(|o| o.get("path").is_some()) && list[first_kept..].iter().all(|o| o.get("path").is_none()), "new objects first, kept objects after");
    let spine = list.iter().find(|o| o["id"] == 1).unwrap();
    assert_eq!(spine["strokes"][0]["width"], 1.0, "the road line is bold");
    assert_eq!(spine["strokes"][0]["paint"]["color"]["r"], 0.85);
    assert!(list.iter().find(|o| o["id"] == 2).unwrap().get("strokes").is_none(), "other objects are left alone");
}
