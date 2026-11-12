use road_builder_core::{builder, export, pmap};
use serde_json::{Value, json};

const SAMPLE: &str = r#"{
"format": "vectorcraft-road-builder/pmap",
"version": 1,
"centerline": [[0, 0], [0, 100], [50, 160], [50, 300]],
"profile": {
"sbEdge": [[0, -10], [300, -10]],
"nbEdge": [[0, 3.6], [150, 3.6], [200, 5.6], [300, 5.6]],
"nbLane": [[0, 0], [300, 0]],
"sbInner": [[0, -8], [300, -8]]
},
"flags": {"gap": [[100, 120]], "mouth": [[40, 50]]},
"junctions": [{"id": "J1", "s": 60, "kind": "tee", "side": "right", "label": "Deer-Valley-Dr"}, {"id": "J2", "s": 200, "kind": "cross", "side": "both", "label": ""}],
"arrows": [{"s": 30, "off": -1.8, "kind": "thru", "south": false}, {"s": 40, "off": -8.3, "kind": "left", "south": true}]
}"#;

/// Wraps the importer's objects the way a saved `.vectorcraft` document holds them.
fn native(objects: &Value) -> Value {
    let children: Vec<Value> = objects["objects"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, o)| json!({"id": i + 1, "name": o["name"], "kind": {"type": "path", "path": o["path"]}}))
        .collect();
    json!({"document": {"layers": [{"id": 0, "name": "L", "kind": {"type": "layer", "children": children}}]}})
}

fn parse(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn near(a: &Value, b: &Value, path: &str) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => assert!((x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 0.002, "{path}: {x} vs {y}"),
        (Value::Array(x), Value::Array(y)) => {
            assert_eq!(x.len(), y.len(), "{path} length");
            x.iter().zip(y).enumerate().for_each(|(i, (p, q))| near(p, q, &format!("{path}[{i}]")));
        }
        (Value::Object(x), Value::Object(y)) => {
            assert_eq!(x.keys().collect::<Vec<_>>(), y.keys().collect::<Vec<_>>(), "{path} keys");
            x.iter().for_each(|(k, v)| near(v, &y[k], &format!("{path}.{k}")));
        }
        _ => assert_eq!(a, b, "{path}"),
    }
}

#[test]
fn import_then_export_returns_the_same_road() {
    let objects = pmap::import(SAMPLE.as_bytes()).unwrap();
    let (text, _) = export::export(&native(&objects)).unwrap();
    let mut want = parse(SAMPLE);
    let got = parse(&text);
    want.as_object_mut().unwrap().insert("length".into(), got["length"].clone());
    near(&want, &got, "pmap");
}

#[test]
fn build_replaces_generated_paths_and_keeps_the_sources() {
    let objects = pmap::import(SAMPLE.as_bytes()).unwrap();
    let mut input: Vec<Value> = objects["objects"].as_array().unwrap().clone();
    for (i, o) in input.iter_mut().enumerate() {
        o["id"] = json!(i + 1);
    }
    input.push(json!({"id": 999, "name": "gen.line.nbEdge", "path": {"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [1, 1]}], "closed": false}]}}));
    let out = builder::build(&json!({"objects": input}), &json!({})).unwrap();
    let out = out["objects"].as_array().unwrap();
    let kept = out.iter().filter(|o| o.get("path").is_none()).count();
    let made: Vec<&str> = out.iter().filter_map(|o| o["name"].as_str()).collect();
    assert_eq!(kept, objects["objects"].as_array().unwrap().len(), "every source object is kept by id");
    assert!(!out.iter().any(|o| o["id"] == 999), "the old generated path is dropped");
    for want in ["gen.line.nbEdge", "gen.line.sbEdge", "gen.line.nbLane", "gen.stub.J1", "gen.zebra.J1", "gen.stop.J1", "gen.stub.J2r", "gen.stub.J2l"] {
        assert!(made.contains(&want), "missing {want}; made {made:?}");
    }
}

#[test]
fn build_without_a_centerline_explains_itself() {
    let err = builder::build(&json!({"objects": []}), &json!({})).unwrap_err();
    assert!(err.contains("centerline"), "{err}");
}

#[test]
fn base64_matches_known_values() {
    assert_eq!(export::base64(b""), "");
    assert_eq!(export::base64(b"f"), "Zg==");
    assert_eq!(export::base64(b"fo"), "Zm8=");
    assert_eq!(export::base64(b"foo"), "Zm9v");
    assert_eq!(export::base64(b"foobar"), "Zm9vYmFy");
}

fn with_ids(objects: &Value) -> Value {
    let mut input: Vec<Value> = objects["objects"].as_array().unwrap().clone();
    for (i, o) in input.iter_mut().enumerate() {
        o["id"] = json!(i + 1);
    }
    json!({"objects": input})
}

#[test]
fn add_junction_places_a_marker_the_exporter_reads_back() {
    let objects = pmap::import(SAMPLE.as_bytes()).unwrap();
    let params = json!({"s": 250.0, "kind": "tee", "side": "left", "label": "Test Street"});
    let out = builder::add_junction_text(&with_ids(&objects), &params).unwrap();
    let out: Value = serde_json::from_str(&out).unwrap();
    let new: Vec<Value> = out["objects"].as_array().unwrap().iter().filter(|o| o.get("path").is_some()).cloned().collect();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0]["name"], "J3.tee.left.Test-Street", "next free id after J1 and J2");
    let mut all = objects["objects"].as_array().unwrap().clone();
    all.extend(new);
    let (text, _) = export::export(&native(&json!({"objects": all}))).unwrap();
    let j = parse(&text)["junctions"].as_array().unwrap().iter().find(|j| j["id"] == "J3").cloned().unwrap();
    assert!((j["s"].as_f64().unwrap() - 250.0).abs() < 0.01, "{j}");
    assert_eq!(j["side"], "left");
}

#[test]
fn add_junction_refuses_a_spot_off_the_road() {
    let objects = pmap::import(SAMPLE.as_bytes()).unwrap();
    let err = builder::add_junction_text(&with_ids(&objects), &json!({"s": 9999.0})).unwrap_err();
    assert!(err.contains("off the road"), "{err}");
}
