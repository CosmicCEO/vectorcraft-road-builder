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
    assert_eq!((ok.roads.clone(), ok.radius()), (["a".to_string(), "b".to_string()], 6.0));
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
    assert_eq!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "radius": 99})).unwrap().radius(), 20.0);
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

// A road that runs only 15 m past the crossing: road b ends at x = 215, road a runs on both sides.
fn stubbed() -> Vec<Item> {
    items(&[[-100.0, 400.0], [215.0, 400.0]])
}

fn with_stub(word: &str) -> Spec {
    let mut c = signals();
    c["stub"] = json!(word);
    spec(c)
}

#[test]
fn a_kept_stub_is_a_dead_end_with_a_kerb_across_its_end() {
    let (out, notes) = run(&stubbed(), &with_stub("keep"));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    for want in ["deadend.b.after", "crosswalk.b.after", "stop.b.before", "kerb.1", "kerb.2", "kerb.3", "kerb.4"] {
        assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.{want}")), "missing {want}; have {n:?}");
    }
    let end = pts(&out, "gen.crossing.k1.deadend.b.after");
    let (lo, hi) = bbox(&end);
    assert!((lo[0] - 215.0).abs() < 1e-3 && (hi[0] - 215.0).abs() < 1e-3 && (lo[1] - 396.4).abs() < 1e-3 && (hi[1] - 403.6).abs() < 1e-3, "the kerb closes the road's end: {lo:?} {hi:?}");
    assert!(!n.iter().any(|x| x.contains("deadend.a.") || x.contains("deadend.b.before")), "only the short end is capped: {n:?}");
    // Long roads have no stub and no dead end.
    assert!(!names(&run(&across(), &with_stub("keep")).0).iter().any(|x| x.contains("deadend")));
}

#[test]
fn a_trimmed_stub_makes_a_three_way_with_the_far_kerb_kept() {
    let (out, notes) = run(&stubbed(), &with_stub("trim"));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    assert!(!n.iter().any(|x| x.contains(".b.after") || x.contains("deadend")), "nothing is drawn for the trimmed arm: {n:?}");
    for want in ["crosswalk.b.before", "stop.b.before", "crosswalk.a.before", "crosswalk.a.after", "kerb.1", "kerb.4"] {
        assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.{want}")), "missing {want}; have {n:?}");
    }
    assert!(!n.iter().any(|x| x == "gen.crossing.k1.kerb.2" || x == "gen.crossing.k1.kerb.3"), "no kerb returns on the side where road b was trimmed: {n:?}");
    // Road a's east travel lanes end at x = 208.7 (1.5 + 2 x 3.6 m from its middle); its apron and bike lane beyond are not paved over.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.box"));
    assert!((lo[0] - 188.95).abs() < 0.01 && (hi[0] - 208.95).abs() < 0.01, "{lo:?} {hi:?}");
}

#[test]
fn trimming_cuts_the_built_road_but_not_the_line() {
    let items = stubbed();
    let sp = with_stub("trim");
    let trims = crossing::trims(&items, &sp);
    assert_eq!(trims.len(), 1);
    assert_eq!((trims[0].index, trims[0].sign), (1, 1.0));
    assert!((trims[0].dist - 8.7).abs() < 1e-3, "cut where road a's travel lanes end: {}", trims[0].dist);
    let mut out = Out::new();
    road_builder_core::roads::build_clipped(&items, &sp, &trims, &mut out);
    let v: Value = serde_json::from_str(&out.finish()).unwrap();
    let (lo, hi) = bbox(&pts(&v, "gen.road.b.1.surface.0.lane"));
    assert!((lo[0] + 100.0).abs() < 1e-3 && (hi[0] - 208.7).abs() < 0.01, "road b's drawing now stops at the through road's travel lane: {lo:?} {hi:?}");
    // Without trims it runs the whole line.
    let mut full = Out::new();
    road_builder_core::roads::build(&items, &sp, &mut full);
    let v: Value = serde_json::from_str(&full.finish()).unwrap();
    assert!((bbox(&pts(&v, "gen.road.b.1.surface.0.lane")).1[0] - 215.0).abs() < 0.01);
    assert_eq!(items[1].subs[0][1], [215.0, 400.0], "the document's line is untouched");
    // Keep, or an overpass, trims nothing.
    assert!(crossing::trims(&items, &with_stub("keep")).is_empty());
}

#[test]
fn trimming_with_nothing_to_trim_says_so_and_changes_nothing() {
    let (out, notes) = run(&across(), &with_stub("trim"));
    assert!(notes.iter().any(|n| n.contains("no stub to trim")), "{notes:?}");
    assert!(names(&out).iter().any(|n| n == "gen.crossing.k1.crosswalk.b.after"), "a four-way is drawn as usual");
    assert!(crossing::trims(&across(), &with_stub("trim")).is_empty());
}

#[test]
fn stub_words_are_read_and_checked() {
    let word = |w: &str| CrossingSpec::from_json(&json!({"roads": ["a", "b"], "stub": w})).map(|c| c.stub);
    use road_builder_core::crossing::Stub;
    for w in ["trim", "3-way", "tee", "cut"] {
        assert_eq!(word(w).unwrap(), Stub::Trim, "{w}");
    }
    for w in ["keep", "dead end", "4-way", ""] {
        assert_eq!(word(w).unwrap(), Stub::Keep, "{w}");
    }
    assert!(word("fold").unwrap_err().contains("stub must be keep or trim"));
    assert!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "stub": "trim"})).unwrap().summary().contains("short stub is trimmed (three-way)"));
    assert_eq!(CrossingSpec::from_json(&json!({"roads": ["a", "b"], "stub_length": 500})).unwrap().stub_length, 100.0);
}

// ---- Groundwork: roads that only touch, and the new modifiers ----

const DIVIDED: &str = "lane S x2 | median 4 | lane N x2";

fn spec_a(a_cross_section: &str, crossing: Value) -> Spec {
    Spec::from_document_data(&json!({"segments": {"a": {"cross_section": a_cross_section}, "b": {"cross_section": TWO_LANE}}, "crossings": {"k1": crossing}}))
}

fn crossing_json(extra: Value) -> Value {
    let mut c = json!({"roads": ["a", "b"], "treatment": "uncontrolled"});
    for (k, v) in extra.as_object().unwrap() {
        c[k] = v.clone();
    }
    c
}

#[test]
fn a_road_that_only_touches_is_found_and_extended_to_the_pavement() {
    // Road b stops at x = 190, ten metres short of road a's middle (x = 200), inside its pavement.
    let touching = items(&[[-100.0, 400.0], [190.0, 400.0]]);
    let x = crossing::find(&touching, "a", "b").expect("a road that stops at the other is still an intersection");
    assert!((x.p[0] - 200.0).abs() < 1e-9 && (x.p[1] + 400.0).abs() < 1e-9);
    assert!(x.reach_a && !x.reach_b, "only road b stops short");
    let sp = spec(json!({"roads": ["a", "b"], "treatment": "signal", "crosswalks": true}));
    let (out, notes) = run(&touching, &sp);
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    assert!(!n.iter().any(|x| x.contains(".b.after") || x.contains("deadend")), "a touching road is a three-way, whatever the stub choice: {n:?}");
    assert!(n.iter().any(|x| x == "gen.crossing.k1.crosswalk.b.before") && !n.iter().any(|x| x == "gen.crossing.k1.kerb.2" || x == "gen.crossing.k1.kerb.3"));
    // Its drawing is carried on to the far travel lane of road a (x = 208.7), under the box.
    let trims = crossing::trims(&touching, &sp);
    assert_eq!(trims.len(), 1);
    assert!((trims[0].dist - 8.7).abs() < 1e-3);
    let mut out = Out::new();
    road_builder_core::roads::build_clipped(&touching, &sp, &trims, &mut out);
    let v: Value = serde_json::from_str(&out.finish()).unwrap();
    let (lo, hi) = bbox(&pts(&v, "gen.road.b.1.surface.0.lane"));
    assert!((lo[0] + 100.0).abs() < 1e-3 && (hi[0] - 208.7).abs() < 0.01, "{lo:?} {hi:?}");
    // A road that stops more than 30 m short does not meet the other.
    assert!(crossing::find(&items(&[[-100.0, 400.0], [100.0, 400.0]]), "a", "b").is_none());
}

#[test]
fn pocket_words_and_the_new_spec_fields_are_read() {
    use road_builder_core::crossing::{Pocket, Treatment};
    let read = |v: Value| CrossingSpec::from_json(&crossing_json(v)).unwrap();
    for (w, want) in [("left", Pocket::Left), ("right turn", Pocket::Right), ("both", Pocket::Both), ("none", Pocket::None)] {
        assert_eq!(read(json!({"pocket": w})).pocket, want, "{w}");
    }
    assert_eq!(read(json!({"pocket": true})).pocket, Pocket::Both);
    assert!(CrossingSpec::from_json(&crossing_json(json!({"pocket": "sideways"}))).unwrap_err().contains("pocket must be"));
    let c = read(json!({"pocket": "left"}));
    assert_eq!((c.pocket_length, c.pocket_taper, c.slip, c.slip_radius), (30.0, 15.0, false, 15.0));
    assert_eq!(read(json!({"pocket_length": 5000})).pocket_length, 150.0);
    // The kerb radius follows the treatment unless given.
    assert_eq!(read(json!({})).radius(), 6.0);
    assert_eq!(read(json!({"treatment": "driveway"})).radius(), 1.5);
    assert_eq!(read(json!({"treatment": "on-ramp"})).radius(), 0.0);
    assert_eq!(read(json!({"treatment": "driveway", "radius": 4})).radius(), 4.0);
    assert_eq!(read(json!({})).to_json()["radius"], Value::Null, "an unset radius stays unset when stored");
    for (w, want) in [("driveway", Treatment::Driveway), ("U-turn", Treatment::MedianOpening), ("exit ramp", Treatment::OffRamp), ("merge", Treatment::OnRamp), ("pedestrian crossing", Treatment::MidblockCrossing), ("level crossing", Treatment::Railroad)] {
        assert_eq!(Treatment::parse(w).unwrap(), want, "{w}");
    }
    assert!(read(json!({"pocket": "left", "pocket_length": 40})).summary().contains("left turn pockets (40 m)"));
}

#[test]
fn left_pockets_are_cut_into_the_median_and_say_when_there_is_no_room() {
    let (out, notes) = run(&across(), &spec_a(DIVIDED, crossing_json(json!({"pocket": "left"}))));
    // Road a has a 4 m median (x 198..202) with northbound lanes to its east. The south arm's inbound traffic is northbound: its left pocket
    // takes the median's edge (x 198.7..202.15, taking the 0.15 m cover over the lane edge) from the box edge (3.6 m) out 30 m, then tapers 15 m.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.pocket.a.before.left"));
    assert!((lo[0] - 198.7).abs() < 1e-3 && (hi[0] - 202.15).abs() < 1e-3 && (lo[1] - 403.6).abs() < 1e-3 && (hi[1] - 448.6).abs() < 1e-3, "{lo:?} {hi:?}");
    let n = names(&out);
    for part in ["lane-line", "edge", "arrow"] {
        assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.pocket.a.before.left.{part}")), "{part}");
    }
    assert!(n.iter().any(|x| x == "gen.crossing.k1.pocket.a.after.left"), "both ends of the road");
    // Road b has no median: a note per approach, and nothing drawn for it.
    assert_eq!(notes.iter().filter(|n| n.contains("no median or centre strip wide enough")).count(), 2, "{notes:?}");
    assert!(!n.iter().any(|x| x.contains("pocket.b.")));
}

#[test]
fn right_pockets_widen_the_road_outward() {
    let (out, notes) = run(&across(), &spec_a(DIVIDED, crossing_json(json!({"pocket": "right"}))));
    assert!(notes.is_empty(), "{notes:?}");
    // Southern arm of road a, northbound traffic: its right is east. The pocket lies beyond the lane edge (x 9.2 from the middle): x 209.05..212.5.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.pocket.a.before.right"));
    assert!((lo[0] - 209.05).abs() < 1e-3 && (hi[0] - 212.5).abs() < 1e-3, "{lo:?} {hi:?}");
    // Road b's arms get right pockets too (always room: the road is widened).
    assert!(names(&out).iter().any(|x| x == "gen.crossing.k1.pocket.b.before.right"));
}

#[test]
fn a_pocket_is_marked_with_a_dashed_lane_line_and_stays_clear_of_the_crosswalk_order() {
    let (out, _) = run(&across(), &spec_a(DIVIDED, crossing_json(json!({"pocket": "left", "crosswalks": true}))));
    let n = names(&out);
    let at = |name: &str| n.iter().position(|x| x == name).unwrap_or_else(|| panic!("no {name}"));
    assert!(at("gen.crossing.k1.pocket.a.before.left") < at("gen.crossing.k1.crosswalk.a.before"), "the crosswalk is drawn over the pocket, not under it");
}

#[test]
fn slip_lanes_round_each_corner_with_a_big_radius_and_an_island() {
    let (out, notes) = run(&across(), &spec(json!({"roads": ["a", "b"], "treatment": "uncontrolled", "slip": true, "slip_radius": 15})));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    for k in 1..=4 {
        for part in [format!("kerb.{k}"), format!("island.{k}"), format!("island.{k}.kerb"), format!("corner.{k}")] {
            assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.{part}")), "missing {part}");
        }
        let arc = pts(&out, &format!("gen.crossing.k1.kerb.{k}"));
        assert!((circumradius(arc[0], arc[6], arc[12]) - 15.0).abs() < 0.02, "corner {k}: the outer kerb is a 15 m arc");
        // The island lies inside the corner: smaller than the fillet it sits in.
        assert!(area(&pts(&out, &format!("gen.crossing.k1.island.{k}"))) < area(&pts(&out, &format!("gen.crossing.k1.corner.{k}"))));
    }
    // Slip lanes off: the ordinary 6 m return.
    let (plain, _) = run(&across(), &spec(json!({"roads": ["a", "b"], "treatment": "uncontrolled"})));
    assert!(!names(&plain).iter().any(|x| x.contains("island")));
}

#[test]
fn a_driveway_meets_the_road_with_a_small_return_and_no_marks() {
    // A 39 m driveway runs up to road a's west edge (it ends at x = 189; road a's middle is x = 200).
    let drive = items(&[[150.0, 400.0], [189.0, 400.0]]);
    let (out, notes) = run(&drive, &spec(json!({"roads": ["a", "b"], "treatment": "driveway", "crosswalks": true, "pocket": "none"})));
    assert!(notes.is_empty(), "{notes:?}");
    let n = names(&out);
    for want in ["box", "kerb.1", "kerb.4", "corner.1", "corner.4"] {
        assert!(n.iter().any(|x| *x == format!("gen.crossing.k1.{want}")), "missing {want}: {n:?}");
    }
    assert!(!n.iter().any(|x| x.contains("kerb.2") || x.contains("kerb.3") || x.contains("stop") || x.contains("signal") || x.contains("sign.")), "a driveway has no control and no return on the far side: {n:?}");
    // The crosswalk option is honoured on its own (a driveway can have one across it); nothing else is added.
    assert!(n.iter().any(|x| x == "gen.crossing.k1.crosswalk.b.before"));
    for k in [1, 4] {
        let arc = pts(&out, &format!("gen.crossing.k1.kerb.{k}"));
        assert!((circumradius(arc[0], arc[6], arc[12]) - 1.5).abs() < 0.01, "a driveway's kerb return is a 1.5 m arc");
    }
    // Its paving covers road a's west edge, bike lane and apron across the driveway's width, but not the far (east) kerb and bike lane.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.box"));
    assert!((lo[0] - 188.95).abs() < 0.01 && (hi[0] - 208.95).abs() < 0.01, "{lo:?} {hi:?}");
}

#[test]
fn offset_tees_are_two_separate_three_ways_on_one_road() {
    // Road a runs north; side road b meets it from the west at y = 400 and side road c from the east at y = 450 (50 m apart).
    let line = |id, name: &str, pts: &[[f64; 2]]| Item { id: Some(id), name: name.into(), subs: vec![pts.to_vec()] };
    let its = vec![line(1, "road.a", &[[200.0, 700.0], [200.0, 100.0]]), line(2, "road.b", &[[150.0, 400.0], [189.0, 400.0]]), line(3, "road.c", &[[211.0, 450.0], [250.0, 450.0]])];
    let sp = Spec::from_document_data(&json!({
        "segments": {"a": {"cross_section": STORY}, "b": {"cross_section": TWO_LANE}, "c": {"cross_section": TWO_LANE}},
        "crossings": {"k1": {"roads": ["a", "b"], "treatment": "uncontrolled"}, "k2": {"roads": ["a", "c"], "treatment": "uncontrolled"}}
    }));
    let mut out = Out::new();
    let notes = crossing::build(&its, &sp, &mut out);
    assert!(notes.is_empty(), "{notes:?}");
    let v: Value = serde_json::from_str(&out.finish()).unwrap();
    let (lo1, hi1) = bbox(&pts(&v, "gen.crossing.k1.box"));
    let (lo2, hi2) = bbox(&pts(&v, "gen.crossing.k2.box"));
    assert!((lo1[1] - 396.15).abs() < 0.01 && (hi1[1] - 403.85).abs() < 0.01, "k1 sits at y 400: {lo1:?} {hi1:?}");
    assert!((lo2[1] - 446.15).abs() < 0.01 && (hi2[1] - 453.85).abs() < 0.01, "k2 sits at y 450: {lo2:?} {hi2:?}");
    // Road b comes from the west so the box reaches road a's east travel edge; road c from the east, so it reaches the west one.
    assert!((hi1[0] - 208.95).abs() < 0.01 && (lo1[0] - 188.95).abs() < 0.01);
    assert!((lo2[0] - 191.05).abs() < 0.01 && (hi2[0] - 211.05).abs() < 0.01, "{lo2:?} {hi2:?}");
    // Two roads meeting the same road are two intersections: both are stored and drawn.
    assert_eq!(sp.crossings().len(), 2);
}

#[test]
fn a_skewed_tee_still_makes_a_box_and_true_arcs_where_they_fit() {
    // Side road b meets road a at 30 degrees (it heads north-east and stops just short of road a's middle).
    let s = 0.5_f64; // sin 30
    let c = 0.866_f64;
    let b = [[200.0 - 120.0 * c, 400.0 + 120.0 * s], [200.0 - 5.0 * c, 400.0 + 5.0 * s]];
    let (out, notes) = run(&items(&b), &spec(json!({"roads": ["a", "b"], "treatment": "uncontrolled", "radius": 6})));
    assert!(notes.is_empty(), "{notes:?}");
    assert!(area(&pts(&out, "gen.crossing.k1.box")) > 100.0);
    // The obtuse corner gets its 6 m arc; the acute one is too sharp to round (the road simply runs out along the kerb).
    let arcs: Vec<String> = names(&out).into_iter().filter(|n| n.starts_with("gen.crossing.k1.kerb.")).collect();
    assert!(!arcs.is_empty() && arcs.len() < 4, "{arcs:?}");
    for name in arcs {
        let arc = pts(&out, &name);
        assert!((circumradius(arc[0], arc[6], arc[12]) - 6.0).abs() < 0.05, "{name}");
    }
}

// ---- Median opening, mid-block crossing, railroad, ramps, cloverleaf ----

fn subpaths(v: &Value, name: &str) -> usize {
    let o = v["objects"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no object {name}"));
    o["path"]["subpaths"].as_array().unwrap().len()
}

fn has(out: &Value, name: &str) -> bool {
    names(out).iter().any(|n| n == name)
}

fn drawn_road(items: &[Item], sp: &Spec) -> Value {
    let trims = crossing::trims(items, sp);
    let mut out = Out::new();
    road_builder_core::roads::build_clipped(items, sp, &trims, &mut out);
    serde_json::from_str(&out.finish()).unwrap()
}

#[test]
fn a_median_opening_cuts_the_median_and_the_marker_road_is_cut_out() {
    let sp = spec_a(DIVIDED, crossing_json(json!({"treatment": "median-opening", "opening": 18})));
    let (out, notes) = run(&across(), &sp);
    assert!(notes.is_empty(), "{notes:?}");
    // The 4 m median (x 198..202, laid 0.2 m wider each side) is opened 9 m each way from the crossing (document y 391..409), with a nose at each end.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.opening.before.1"));
    assert!((lo[0] - 197.8).abs() < 1e-3 && (hi[0] - 202.2).abs() < 1e-3 && (lo[1] - 400.0).abs() < 1e-3 && (hi[1] - 409.0).abs() < 1e-3, "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.opening.after.1"));
    assert!((lo[1] - 391.0).abs() < 1e-3 && (hi[1] - 400.0).abs() < 1e-3, "{lo:?} {hi:?}");
    assert!(has(&out, "gen.crossing.k1.opening.after.1.nose") && has(&out, "gen.crossing.k1.opening.before.1.nose"));
    assert!(!names(&out).iter().any(|n| n.contains(".box") || n.contains("crosswalk") || n.contains("stop")), "nothing else is drawn");
    // The crossover line (road b) is cut out across the divided road's 18.4 m: two pieces are left, outside it.
    let trims = crossing::trims(&across(), &sp);
    assert_eq!(trims.len(), 1);
    assert!((trims[0].dist - 9.2).abs() < 1e-3 && (trims[0].gap_before.unwrap() - 9.2).abs() < 1e-3);
    let v = drawn_road(&across(), &sp);
    let (lo, hi) = bbox(&pts(&v, "gen.road.b.1.surface.0.lane"));
    assert!((lo[0] + 100.0).abs() < 1e-3 && (hi[0] - 190.8).abs() < 0.01, "west piece: {lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&v, "gen.road.b.1.part2.surface.0.lane"));
    assert!((lo[0] - 209.2).abs() < 0.01 && (hi[0] - 500.0).abs() < 1e-3, "east piece, named .part2 so a click still finds the road: {lo:?} {hi:?}");
    // A road with no median cannot have one opened.
    let (_, notes) = run(&across(), &spec_a(TWO_LANE, crossing_json(json!({"treatment": "median-opening"}))));
    assert!(notes.iter().any(|n| n.contains("has no median to open")), "{notes:?}");
}

#[test]
fn a_midblock_crossing_has_bars_over_the_lanes_not_the_median_and_a_stop_line_each_way() {
    let sp = spec_a(DIVIDED, crossing_json(json!({"treatment": "midblock-crossing", "beacon": true})));
    let (out, notes) = run(&across(), &sp);
    assert!(notes.is_empty(), "{notes:?}");
    // Four 3.6 m lanes, three bars each (1.2 m apart), none over the median.
    assert_eq!(subpaths(&out, "gen.crossing.k1.crosswalk"), 12);
    let bars = pts(&out, "gen.crossing.k1.crosswalk");
    assert!(bars.iter().all(|p| !(p[0] > 198.0 && p[0] < 202.0)), "the median is a refuge");
    let (lo, hi) = bbox(&bars);
    assert!((lo[1] - 398.5).abs() < 1e-3 && (hi[1] - 401.5).abs() < 1e-3, "3 m deep, centred on the crossing: {lo:?} {hi:?}");
    // Stop lines 6 m (half the crosswalk plus 4.5 m) from the middle, over the lanes that approach: northbound from the south, southbound from the north.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.stop.before"));
    assert!((lo[0] - 202.0).abs() < 1e-3 && (hi[0] - 209.2).abs() < 1e-3 && (lo[1] - 406.0).abs() < 1e-3, "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.stop.after"));
    assert!((lo[0] - 190.8).abs() < 1e-3 && (hi[0] - 198.0).abs() < 1e-3 && (lo[1] - 394.0).abs() < 1e-3, "{lo:?} {hi:?}");
    // Beacons stand 0.9 m beyond each kerb.
    let c1 = centre(&pts(&out, "gen.crossing.k1.beacon.1"));
    let c2 = centre(&pts(&out, "gen.crossing.k1.beacon.2"));
    assert!((c1[0] - 189.9).abs() < 0.1 && (c2[0] - 210.1).abs() < 0.1 && (c1[1] - 400.0).abs() < 0.1, "{c1:?} {c2:?}");
    // Without beacons there are none; the footpath line is cut out across the road.
    let (plain, _) = run(&across(), &spec_a(DIVIDED, crossing_json(json!({"treatment": "midblock-crossing"}))));
    assert!(!names(&plain).iter().any(|n| n.contains("beacon")));
    assert_eq!(crossing::trims(&across(), &sp).len(), 1);
}

fn rail_spec(crossing: Value) -> Spec {
    Spec::from_document_data(&json!({"segments": {"a": {"cross_section": TWO_LANE}, "b": {"cross_section": "railway"}}, "crossings": {"k1": crossing}}))
}

#[test]
fn a_railroad_crossing_has_rails_ties_stop_lines_crossbucks_and_gates() {
    let sp = rail_spec(json!({"roads": ["a", "b"], "treatment": "railroad"}));
    let (out, notes) = run(&across(), &sp);
    assert!(notes.is_empty(), "{notes:?}");
    // The 7.2 m road is the one that stays (the other has the railway element). Rails 1.435 m apart run 6 m past the 3.6 m crossing panel.
    let rails = pts(&out, "gen.crossing.k1.rails");
    let (lo, hi) = bbox(&rails);
    assert!((lo[0] - 190.4).abs() < 1e-3 && (hi[0] - 209.6).abs() < 1e-3 && (lo[1] - 399.28).abs() < 1e-3 && (hi[1] - 400.72).abs() < 1e-3, "{lo:?} {hi:?}");
    assert_eq!(subpaths(&out, "gen.crossing.k1.ties"), 12, "a tie every 0.6 m across the 7.2 m road");
    // Stop lines 4.6 m from the nearest rail (2 m from the track's middle): 6.6 m from the crossing, over the approaching lane only.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.stop.before"));
    assert!((lo[0] - 200.0).abs() < 1e-3 && (hi[0] - 203.6).abs() < 1e-3 && (lo[1] - 406.6).abs() < 1e-3, "{lo:?} {hi:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.stop.after"));
    assert!((lo[0] - 196.4).abs() < 1e-3 && (hi[0] - 200.0).abs() < 1e-3 && (lo[1] - 393.4).abs() < 1e-3, "{lo:?} {hi:?}");
    // The southern pole stands at the driver's right-hand kerb, 5 m from the crossing; a gate arm reaches across the lane to the middle.
    let lamp = centre(&pts(&out, "gen.crossing.k1.lamp.before"));
    assert!((lamp[0] - 205.1).abs() < 0.1 && (lamp[1] - 405.0).abs() < 0.1, "{lamp:?}");
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.gate.before"));
    assert!((lo[0] - 200.0).abs() < 1e-3 && (hi[0] - 205.1).abs() < 1e-3, "{lo:?} {hi:?}");
    assert!(has(&out, "gen.crossing.k1.crossbuck.after") && has(&out, "gen.crossing.k1.panel"));
    // No gates when asked not to; the railway's ballast is cut out across the road.
    let (open, _) = run(&across(), &rail_spec(json!({"roads": ["a", "b"], "treatment": "railroad", "gates": false})));
    assert!(!names(&open).iter().any(|n| n.contains("gate")) && has(&open, "gen.crossing.k1.crossbuck.before"));
    let trims = crossing::trims(&across(), &sp);
    assert_eq!(trims.len(), 1);
    assert!((trims[0].dist - 3.6).abs() < 1e-3);
    // A railway that nearly runs along the road is not a crossing.
    let along = items(&[[190.0, 700.0], [210.0, 100.0]]);
    let (_, notes) = run(&along, &sp);
    assert!(notes.iter().any(|n| n.contains("nearly parallel")), "{notes:?}");
}

// A one-way ramp joining road a (divided, running north) from the south-east at 25 degrees, stopping 11 m short of the road's middle.
fn ramp_items() -> Vec<Item> {
    items(&[[250.7, 508.8], [204.65, 409.97]])
}

fn ramp_spec(treatment: &str) -> Spec {
    Spec::from_document_data(&json!({
        "segments": {"a": {"cross_section": DIVIDED}, "b": {"cross_section": "lane fwd"}},
        "crossings": {"k1": {"roads": ["a", "b"], "treatment": treatment}}
    }))
}

#[test]
fn an_on_ramp_adds_an_acceleration_lane_downstream_and_an_off_ramp_a_deceleration_lane_upstream() {
    let (out, notes) = run(&ramp_items(), &ramp_spec("on-ramp"));
    assert!(notes.is_empty(), "{notes:?}");
    // Northbound lanes are on the east: the ramp is on the right and traffic runs north. The lane starts at the crossing (y 400), is 3.6 m wide
    // beside the outer travel lane edge (x 9.2 from the middle), runs 80 m full width and tapers over 40 m: y 280..400, x 209.05..212.8.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.accel"));
    assert!((lo[0] - 209.05).abs() < 1e-3 && (hi[0] - 212.8).abs() < 1e-3 && (lo[1] - 280.0).abs() < 0.05 && (hi[1] - 400.0).abs() < 0.05, "{lo:?} {hi:?}");
    assert!(has(&out, "gen.crossing.k1.accel.lane-line") && has(&out, "gen.crossing.k1.accel.edge"));
    // A ramp joins at a sharp angle with no kerb return, no crosswalk, no control.
    assert!(!names(&out).iter().any(|n| n.contains("kerb") || n.contains("crosswalk") || n.contains("stop") || n.contains("signal") || n.contains("sign.")), "{:?}", names(&out));
    let (off, _) = run(&ramp_items(), &ramp_spec("off-ramp"));
    let (lo, hi) = bbox(&pts(&off, "gen.crossing.k1.decel"));
    assert!((lo[1] - 400.0).abs() < 0.05 && (hi[1] - 520.0).abs() < 0.05, "upstream of the exit: {lo:?} {hi:?}");
    // The ramp road runs on to the pavement it joins; the line it was drawn with is untouched.
    let trims = crossing::trims(&ramp_items(), &ramp_spec("on-ramp"));
    assert_eq!(trims.len(), 1);
    assert_eq!(trims[0].index, 1);
}

#[test]
fn a_cloverleaf_is_an_overpass_with_four_loop_ramps() {
    let two = |over: Value| Spec::from_document_data(&json!({"segments": {"a": {"cross_section": TWO_LANE}, "b": {"cross_section": TWO_LANE}}, "crossings": {"k1": over}}));
    let sp = two(json!({"roads": ["a", "b"], "treatment": "cloverleaf", "over": "b"}));
    let (out, notes) = run(&across(), &sp);
    assert!(notes.iter().any(|n| n.contains("only the four loop ramps")), "{notes:?}");
    let n = names(&out);
    assert!(n.iter().any(|x| x.starts_with("gen.crossing.k1.deck.surface.")) && has(&out, "gen.crossing.k1.abutment.1"), "the overpass");
    for q in 1..=4 {
        for part in [format!("loop.{q}"), format!("loop.{q}.edges"), format!("loop.{q}.infield"), format!("loop.{q}.strip.1"), format!("loop.{q}.strip.2")] {
            assert!(has(&out, &format!("gen.crossing.k1.{part}")), "missing {part}");
        }
    }
    // Quadrant 1 (north-east): the ring's centre is 43.6 m east and north of the crossing (document 243.6, 356.4), its outer edge 42.5 m from it.
    // It runs 270 degrees away from the crossing, so it reaches west to x 201.1, east to 286.1, north to y 313.9 and south to 398.9.
    let (lo, hi) = bbox(&pts(&out, "gen.crossing.k1.loop.1"));
    assert!((lo[0] - 201.1).abs() < 0.02 && (hi[0] - 286.1).abs() < 0.02 && (lo[1] - 313.9).abs() < 0.02 && (hi[1] - 398.9).abs() < 0.02, "{lo:?} {hi:?}");
    // Roads that cross at 60 degrees cannot have a cloverleaf.
    let b = [[200.0 - 400.0 * 0.866, 400.0 + 400.0 * 0.5], [200.0 + 400.0 * 0.866, 400.0 - 400.0 * 0.5]];
    let (_, notes) = run(&items(&b), &sp);
    assert!(notes.iter().any(|n| n.contains("close to a right angle")), "{notes:?}");
}
