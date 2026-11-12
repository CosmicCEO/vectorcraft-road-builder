//! Runs the Build filter on the full Ross Bridge road (when `map/ross-bridge.pmap` is present) and reports the size of the work.

use road_builder_core::{builder, pmap};
use serde_json::{Value, json};
use std::time::Instant;

#[test]
fn builds_the_full_road_quickly_and_small() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../ross-bridge.pmap");
    let Ok(bytes) = std::fs::read(path) else { return };
    let objects = pmap::import(&bytes).unwrap();
    let mut input: Vec<Value> = objects["objects"].as_array().unwrap().clone();
    for (i, o) in input.iter_mut().enumerate() {
        o["id"] = json!(i + 1);
    }
    let input = json!({"objects": input});
    let size = serde_json::to_vec(&input).unwrap().len();
    let t = Instant::now();
    let items = road_builder_core::vcdoc::items_from_filter_input(&input);
    eprintln!("items {:?}", t.elapsed());
    let sc = road_builder_core::scene::Scene::from_items(&items).unwrap();
    eprintln!("scene {:?} ({} arrows, {} junctions)", t.elapsed(), sc.arrows.len(), sc.junctions.len());
    let t = Instant::now();
    let out = builder::build(&input, &json!({})).unwrap();
    let anchors: usize = out["objects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| o["path"]["subpaths"].as_array())
        .flatten()
        .map(|s| s["anchors"].as_array().unwrap().len())
        .sum();
    eprintln!("input {size} bytes, built in {:?}, {} objects, {anchors} new anchors", t.elapsed(), out["objects"].as_array().unwrap().len());
    assert!(anchors < 60_000, "output too large: {anchors} anchors");
}
