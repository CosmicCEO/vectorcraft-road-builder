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
  },
  "commands": [
    {
      "id": "add_junction",
      "label": "Add Junction",
      "params": {
        "s": {"type": "number", "min": 0, "max": 100000, "default": 0, "unit": "m", "label": "Route metre", "help": "Distance along the centerline from its first point."},
        "kind": {"type": "choice", "options": ["tee", "cross", "street", "drive", "signal", "school"], "default": "tee"},
        "side": {"type": "choice", "options": ["right", "left", "both"], "default": "right", "help": "Right and left are as seen driving north (along the centerline)."},
        "label": {"type": "string", "default": "", "maxLength": 40, "label": "Street name", "help": "Letters, numbers and dashes only."},
        "junction": {"type": "string", "default": "", "maxLength": 8, "label": "Junction id", "help": "Leave empty for the next free number."}
      }
    }
  ]
}"#;

fn run(input: &[u8], params: &Value) -> Result<Output, String> {
    let input: Value = serde_json::from_slice(input).map_err(|e| e.to_string())?;
    match params["_context"]["command"].as_str() {
        Some("add_junction") => builder::add_junction_text(&input, params),
        _ => builder::build_text(&input, params),
    }
    .map(Output::Raw)
}

vc_plugin!(MANIFEST, run);
