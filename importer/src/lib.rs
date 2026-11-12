//! Road Builder importer: opens a `.pmap` road file as a VectorCraft document of named paths.

use road_builder_core::abi::Output;
use road_builder_core::{pmap, vc_plugin};
use serde_json::Value;

const MANIFEST: &str = r#"{
  "id": "com.cosmicceo.roadbuilder.import",
  "name": "Road Builder road (.pmap)",
  "version": "0.1.0",
  "kind": "importer",
  "author": "CosmicCEO",
  "description": "Opens a pmap road file: the centerline, the lane profile curves, flag bars, junction markers and arrows become named paths.",
  "extensions": ["pmap"]
}"#;

fn run(input: &[u8], _params: &Value) -> Result<Output, String> {
    pmap::import(input).map(Output::Json)
}

vc_plugin!(MANIFEST, run);
