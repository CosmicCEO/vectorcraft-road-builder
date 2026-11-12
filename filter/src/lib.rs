//! "Build Road": object filter. Select everything, run it, and the lane lines, edges and junction pieces are regenerated.

use road_builder_core::abi::Output;
use road_builder_core::{builder, vc_plugin};
use serde_json::Value;

const MANIFEST: &str = r#"{
  "id": "com.cosmicceo.roadbuilder.build",
  "name": "Build Road",
  "version": "0.1.0",
  "kind": "filter",
  "author": "CosmicCEO",
  "description": "Regenerates lane lines, pavement edges and junction pieces from the centerline, the profile curves and the junction markers. Select All first.",
  "params": {
    "stubLength": {"type": "number", "min": 5, "max": 100, "default": 28, "unit": "m", "label": "Side street length"},
    "stubWidth": {"type": "number", "min": 3, "max": 20, "default": 8, "unit": "m", "label": "Side street width"}
  }
}"#;

fn run(input: &[u8], params: &Value) -> Result<Output, String> {
    let input: Value = serde_json::from_slice(input).map_err(|e| e.to_string())?;
    builder::build_text(&input, params).map(Output::Raw)
}

vc_plugin!(MANIFEST, run);
