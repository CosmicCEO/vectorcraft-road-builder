use road_builder_core::builder::{build, set_crossing_text};
use road_builder_core::crossing::{self, CrossingSpec};
use road_builder_core::output::Out;
use road_builder_core::roads::Spec;
use road_builder_core::vcdoc::Item;
use serde_json::{Value, json};

const STORY: &str = "facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane";
const TWO_LANE: &str = "lane back | lane fwd";

fn line(id: i64, name: &str, pts: &[[f64; 2]]) -> Item {
    Item { id: Some(id), name: name.into(), subs: vec![pts.to_vec()] }
}

/// Road a runs north (document y runs down the page); road b crosses it. Perpendicular: b runs east along y = 400.
fn items(b: &[[f64; 2]]) -> Vec<Item> {
    vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]), line(2, "road.b", b)]
}

fn across() -> Vec<Item> {
    items(&[[-100.0, 400.0], [500.0, 400.0]])
}

fn spec(crossing: Value) -> Spec {
    Spec::from_document_data(&json!({"segments": {"a": {"cross_section": STORY}, "b": {"cross_section": TWO_LANE}}, "crossings": {"k1": crossing}}))
}

fn signals() -> Value {
    json!({"roads": ["a", "b"], "treatment": "signal", "crosswalks": true, "radius": 6})
}

fn run(items: &[Item], spec: &Spec) -> (Value, Vec<String>) {
    let mut out = Out::new();
    let notes = crossing::build(items, spec, &mut out);
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

fn centre(p: &[[f64; 2]]) -> [f64; 2] {
    let (lo, hi) = bbox(p);
    [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]
}

fn area(p: &[[f64; 2]]) -> f64 {
    (0..p.len()).map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1]).sum::<f64>().abs() / 2.0
}

/// Radius of the circle through three points.
fn circumradius(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    let (ab, bc, ca) = ((b[0] - a[0]).hypot(b[1] - a[1]), (c[0] - b[0]).hypot(c[1] - b[1]), (a[0] - c[0]).hypot(a[1] - c[1]));
    let twice_area = ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])).abs();
    ab * bc * ca / (2.0 * twice_area)
}

#[test]
fn the_crossing_is_found_where_the_lines_meet() {
    let x = crossing::find(&across(), "a", "b").unwrap();
    assert!((x.p[0] - 200.0).abs() < 1e-9 && (x.p[1] + 400.0).abs() < 1e-9, "world point (y is up): {:?}", x.p);
    assert!(crossing::find(&items(&[[-100.0, 800.0], [500.0, 800.0]]), "a", "b").is_none(), "a road that stops short does not cross");
}

#[test]
fn a_right_angle_intersection_has_every_part_in_the_right_place() {
    let (out, notes) = run(&across(), &spec(signals()));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    for want in [
        "box", "corner.1", "corner.2", "corner.3", "corner.4", "kerb.1", "kerb.4", "crosswalk.a.before", "crosswalk.a.after", "crosswalk.b.before", "crosswalk.b.after",
        "stop.a.before", "stop.a.after", "stop.b.before", "stop.b.after", "signal.a.before", "signal.b.after",
    ] {
        assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.{want}")), "missing {want}; have {n:?}");
    }
    // Road a is 21.6 m wide (x 189.2..210.8), road b 7.2 m (y 396.4..403.6); the box is the overlap, a hair larger to cover the edge lines.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.box"));
    assert!((lo[0] - 188.95).abs() < 0.01 && (hi[0] - 211.05).abs() < 0.01 && (lo[1] - 396.15).abs() < 0.01 && (hi[1] - 403.85).abs() < 0.01, "a quarter metre beyond the overlap on every side: {lo:?} {hi:?}");
    // Kerb corners are true 6 m arcs.
    for k in 1..=4 {
        let arc = pts(&out, &format!("gen.crossing.k1.kerb.{k}"));
        assert_eq!(arc.len(), 13);
        assert!((circumradius(arc[0], arc[6], arc[12]) - 6.0).abs() < 1e-2, "corner {k} is not a 6 m arc");
    }
}

#[test]
fn crosswalks_stop_bars_and_signals_are_on_the_side_traffic_enters() {
    let (out, _) = run(&across(), &spec(signals()));
    // Road a (north): northbound traffic (the east half) enters from the south, so its bar is south of the box, over x 201.5..208.7.
    let bar = pts(&out, "gen.crossing.k1.stop.a.before");
    assert!(bar.iter().all(|p| (p[1] - 408.5).abs() < 1e-3), "{bar:?}");
    let (lo, hi) = bbox(&bar);
    assert!((lo[0] - 201.5).abs() < 1e-3 && (hi[0] - 208.7).abs() < 1e-3, "northbound lanes only: {lo:?} {hi:?}");
    // Southbound traffic (west half) enters from the north.
    let bar = pts(&out, "gen.crossing.k1.stop.a.after");
    let (lo, hi) = bbox(&bar);
    assert!((lo[1] - 391.5).abs() < 1e-3 && (lo[0] - 191.3).abs() < 1e-3 && (hi[0] - 198.5).abs() < 1e-3, "{lo:?} {hi:?}");
    // Road b runs east: eastbound traffic (south half, y 400..403.6) enters from the west, westbound (north half) from the east.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.stop.b.before"));
    assert!((lo[0] - 184.3).abs() < 1e-3 && (lo[1] - 400.0).abs() < 1e-3 && (hi[1] - 403.6).abs() < 1e-3, "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.stop.b.after"));
    assert!((lo[0] - 215.7).abs() < 1e-3 && (lo[1] - 396.4).abs() < 1e-3 && (hi[1] - 400.0).abs() < 1e-3, "{lo:?} {hi:?}");
    // The signal head for northbound traffic stands beside the bar on the east (driver's right) side.
    let c = centre(&pts(&out, "gen.crossing.k1.signal.a.before"));
    assert!((c[0] - 209.6).abs() < 0.2 && (c[1] - 409.4).abs() < 0.2, "{c:?}");
    // The crosswalk across the south arm of road a lies between the box and the bar.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.crosswalk.a.before"));
    assert!((lo[1] - 404.6).abs() < 1e-3 && (hi[1] - 407.6).abs() < 1e-3, "{lo:?} {hi:?}");
}

#[test]
fn an_oblique_crossing_gives_a_parallelogram_and_true_arcs() {
    // Road b crosses road a at 60 degrees.
    let b = [[200.0 - 400.0 * 0.866, 400.0 + 400.0 * 0.5], [200.0 + 400.0 * 0.866, 400.0 - 400.0 * 0.5]];
    let (out, notes) = run(&items(&b), &spec(signals()));
    assert!(notes.is_empty(), "{notes:?}");
    let a = area(&pts(&out, "gen.crossing.k1.box"));
    // 21.6 m by 7.2 m at 60 degrees: 21.6 * 7.2 / sin 60 = 179.6 m2, a little more for the enlargement.
    assert!(a > 175.0 && a < 215.0, "area {a}");
    for k in 1..=4 {
        let arc = pts(&out, &format!("gen.crossing.k1.kerb.{k}"));
        assert!((circumradius(arc[0], arc[6], arc[12]) - 6.0).abs() < 0.05, "corner {k} at an oblique angle is still a 6 m arc");
    }
}

#[test]
fn stop_signs_go_on_the_minor_road_only() {
    let mut c = signals();
    c["treatment"] = json!("two-way-stop");
    c["minor"] = json!("b");
    let n = names(&run(&across(), &spec(c)).0);
    assert!(n.iter().any(|x| x == "gen.crossing.k1.sign.b.before") && n.iter().any(|x| x == "gen.crossing.k1.stop.b.after"));
    assert!(!n.iter().any(|x| x.contains(".stop.a.") || x.contains("signal") || x.contains(".sign.a.")), "road a is not stopped: {n:?}");
    let mut all = signals();
    all["treatment"] = json!("four-way-stop");
    let n = names(&run(&across(), &spec(all)).0);
    assert_eq!(n.iter().filter(|x| x.contains(".sign.")).count(), 4, "all-way stop: a sign on every approach");
    let mut none = signals();
    none["treatment"] = json!("uncontrolled");
    none["crosswalks"] = json!(false);
    let n = names(&run(&across(), &spec(none)).0);
    assert!(!n.iter().any(|x| x.contains("crosswalk") || x.contains(".stop.") || x.contains("signal")), "{n:?}");
}

#[test]
fn problems_are_reported_in_plain_words() {
    let (_, notes) = run(&items(&[[-100.0, 800.0], [500.0, 800.0]]), &spec(signals()));
    assert!(notes.iter().any(|n| n.contains("do not cross")), "{notes:?}");
    // Nearly parallel roads (3 degrees apart) do not make an intersection.
    let b = [[0.0, 700.0 + 0.0], [400.0, 100.0 + 30.0]];
    let almost = items(&[[190.0, 700.0], [210.0, 100.0]]);
    let (_, notes) = run(&almost, &spec(signals()));
    assert!(notes.iter().any(|n| n.contains("nearly parallel")), "{notes:?} {b:?}");
}

#[test]
fn a_crossing_description_is_checked() {
    let ok = CrossingSpec::from_json(&signals()).unwrap();
    assert_eq!((ok.roads.clone(), ok.radius), (["a".to_string(), "b".to_string()], 6.0));
    assert!(ok.summary().contains("traffic signals, crosswalks"));
    let err = |v: Value| CrossingSpec::from_json(&v).unwrap_err();
    assert!(err(json!({"roads": ["a"]})).contains("exactly two roads"));
    assert!(err(json!({"roads": ["a", "a"]})).contains("two different roads"));
    assert!(err(json!({"roads": ["a", "b"], "minor": "c"})).contains("must be one of the two roads"));
    assert!(err(json!({"roads": ["a", "b"], "over": "c"})).contains("must be one of the two roads"));
    assert!(err(json!({"roads": ["a", "b"], "treatment": "teleporter"})).contains("treatment must be"));
    assert!(err(json!({"roads": ["a", "b"], "control": "teleporter"})).contains("control must be"));
    for (word, want) in [("lighted intersection", "signal"), ("2 way stop", "two-way-stop"), ("4-way stop", "four-way-stop"), ("underpass", "overpass"), ("traffic circle", "roundabout"), ("cloverleaf", "cloverleaf"), ("none", "uncontrolled")] {
        assert_eq!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "treatment": word})).unwrap().treatment.name(), want, "{word}");
    }
    // The older control field still reads: stop means a two-way stop if a minor road is named, otherwise a four-way stop.
    assert_eq!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "control": "stop", "minor": "b"})).unwrap().treatment.name(), "two-way-stop");
    assert_eq!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "control": "stop"})).unwrap().treatment.name(), "four-way-stop");
    assert_eq!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "radius": 99})).unwrap().radius, 20.0);
}

#[test]
fn the_command_stores_the_intersection_and_build_draws_it() {
    let objs = across();
    let to_input = |items: &[Item]| json!({"objects": items.iter().map(|i| json!({"id": i.id, "name": i.name, "type": "path", "path": {"subpaths": [{"anchors": i.subs[0].iter().map(|p| json!({"p": p})).collect::<Vec<_>>(), "closed": false}]}})).collect::<Vec<_>>()});
    let data = json!({"segments": {"a": {"cross_section": STORY}, "b": {"cross_section": TWO_LANE}}});
    let p = |d: &str| json!({"description": d, "_context": {"documentData": data.clone()}});
    let out: Value = serde_json::from_str(&set_crossing_text(&to_input(&objs), &p(r#"{"treatment": "signal", "crosswalks": true}"#)).unwrap()).unwrap();
    assert_eq!(out["documentData"]["crossings"]["k1"]["roads"], json!(["a", "b"]));
    assert_eq!(out["documentData"]["crossings"]["k1"]["treatment"], "signal");
    assert_eq!(out["documentData"]["segments"]["a"]["cross_section"], STORY, "other stored data is kept");

    // Describing the same pair again updates the same intersection.
    let mut again = p(r#"{"treatment": "two-way-stop", "minor": "b"}"#);
    again["_context"]["documentData"] = out["documentData"].clone();
    let out2: Value = serde_json::from_str(&set_crossing_text(&to_input(&objs), &again).unwrap()).unwrap();
    assert_eq!(out2["documentData"]["crossings"].as_object().unwrap().len(), 1);
    assert_eq!(out2["documentData"]["crossings"]["k1"]["minor"], "b");

    // Build everything: roads, then the intersection over them.
    let built = build(&to_input(&objs), &json!({"_context": {"documentData": out["documentData"].clone()}})).unwrap();
    let n = names(&built);
    assert!(n.iter().any(|x| x == "gen.crossing.k1.box") && n.iter().filter(|x| x.starts_with("gen.road.a.1.surface.")).count() == 9, "{n:?}");

    // Plain-words problems.
    let one = to_input(&objs[..1]);
    assert!(set_crossing_text(&one, &p("{}")).unwrap_err().contains("exactly two roads"));
    let apart = to_input(&items(&[[-100.0, 800.0], [500.0, 800.0]]));
    assert!(set_crossing_text(&apart, &p("{}")).unwrap_err().contains("do not cross"));
    assert!(set_crossing_text(&to_input(&objs), &p("{nope")).unwrap_err().contains("not valid JSON"));
}

#[test]
fn a_two_way_stop_defaults_to_the_narrower_road_stopping() {
    let n = names(&run(&across(), &spec(json!({"roads": ["a", "b"], "treatment": "two-way-stop"}))).0);
    assert!(n.iter().any(|x| x == "gen.crossing.k1.sign.b.before") && !n.iter().any(|x| x.contains(".sign.a.")), "road b (7.2 m) is narrower than road a (21.6 m): {n:?}");
}

#[test]
fn an_overpass_puts_one_road_on_top_with_a_deck_and_no_conflict_marks() {
    let (out, notes) = run(&across(), &spec(json!({"roads": ["a", "b"], "treatment": "overpass", "over": "b", "crosswalks": true})));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    assert!(n.iter().any(|x| x.starts_with("gen.crossing.k1.deck.surface.")) && n.iter().any(|x| x == "gen.crossing.k1.parapet.1") && n.iter().any(|x| x == "gen.crossing.k1.abutment.2"), "{n:?}");
    assert!(!n.iter().any(|x| x.contains(".box") || x.contains("crosswalk") || x.contains(".stop.") || x.contains("signal")), "grade-separated: nothing to stop for: {n:?}");
    // Road b (east-west) is the bridge. It spans the 21.6 m road a plus 4 m abutments each side: x from 200 - 10.8 - 4 to 200 + 10.8 + 4.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.abutment.1").into_iter().chain(pts(&out, "gen.crossing.k1.abutment.2")).collect::<Vec<_>>());
    assert!((lo[0] - 185.2).abs() < 0.05 && (hi[0] - 214.8).abs() < 0.05, "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.abutment.1"));
    assert!((lo[1] - 396.4).abs() < 1e-3 && (hi[1] - 403.6).abs() < 1e-3, "the abutment spans the deck's 7.2 m width: {lo:?} {hi:?}");
}

#[test]
fn a_roundabout_has_a_ring_islands_yield_lines_and_crosswalks() {
    let (out, notes) = run(&across(), &spec(json!({"roads": ["a", "b"], "treatment": "roundabout", "diameter": 40, "crosswalks": true})));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    for want in ["disc", "apron", "island", "kerb.island", "arrows", "splitter.a.before", "splitter.b.after", "yield.a.after", "yield.b.before", "crosswalk.a.before"] {
        assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.{want}")), "missing {want}; have {n:?}");
    }
    assert_eq!(n.iter().filter(|x| x.starts_with("gen.crossing.k1.kerb.") && !x.ends_with("island")).count(), 4, "outer kerb arcs between the four openings");
    // 40 m across: the paved disc is 20.3 m in radius about the crossing point (200, 400); the central island is 20 - 5.5 - 2 = 12.5 m.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.disc"));
    assert!((hi[0] - lo[0] - 40.6).abs() < 0.1 && (lo[0] + hi[0] - 400.0).abs() < 0.1, "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.island"));
    assert!((hi[0] - lo[0] - 25.0).abs() < 0.1, "central island is 25 m across: {lo:?} {hi:?}");
    // Yield line for traffic on road b coming from the west (heading east, the south half, y 400..403.6), just outside the ring.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.yield.b.before"));
    assert!((hi[0] - 179.2).abs() < 1.0 && lo[1] >= 399.9 && hi[1] <= 403.7, "yield line for eastbound entry: {lo:?} {hi:?}");
}

#[test]
fn a_cloverleaf_says_it_is_not_built_yet() {
    let (_, notes) = run(&across(), &spec(json!({"roads": ["a", "b"], "treatment": "cloverleaf"})));
    assert!(notes.iter().any(|n| n.contains("cloverleaf") && n.contains("not built yet")), "{notes:?}");
}

#[test]
fn the_median_ends_short_of_the_crosswalk_with_a_nose() {
    let (out, _) = run(&across(), &spec(signals()));
    // Road a has a 3 m margin in the middle (x 198.5..201.5). South arm: box edge 3.6 m from the crossing, crosswalk 1 m beyond, 3 m deep,
    // so the median is capped 0.3 m past the crosswalk: 3.6 + 1 + 3 + 0.3 = 7.9 m south of the crossing (document y 407.9). The cap starts at the
    // crossing itself, so no median is left between the box edge and the cap on an oblique crossing.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.cap.a.before.1"));
    assert!((lo[0] - 198.3).abs() < 1e-3 && (hi[0] - 201.7).abs() < 1e-3 && (lo[1] - 400.0).abs() < 1e-3 && (hi[1] - 407.9).abs() < 1e-3, "{lo:?} {hi:?}");
    let nose = pts(&out, "gen.crossing.k1.cap.a.before.1.nose");
    assert!(nose.iter().all(|p| (p[1] - 407.9).abs() < 1e-3), "{nose:?}");
    let (lo, hi) = bbox(&nose);
    assert!((lo[0] - 198.5).abs() < 1e-3 && (hi[0] - 201.5).abs() < 1e-3);
    assert!(names(&out).iter().any(|n| n == "gen.crossing.k1.cap.a.after.1"), "both arms are capped");
    assert!(!names(&out).iter().any(|n| n.contains(".cap.b.")), "a two-lane road has no median to cap");
    // Without crosswalks the median stops a metre from the box.
    let mut c = signals();
    c["crosswalks"] = json!(false);
    let (out, _) = run(&across(), &spec(c));
    let (_, hi) = bbox(&pts(&out, "gen.crossing.k1.cap.a.before.1"));
    assert!((hi[1] - 404.6).abs() < 1e-3, "{hi:?}");
}
