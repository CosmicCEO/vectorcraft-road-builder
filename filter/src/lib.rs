//! "Build Road": object filter. Select everything, run it, and the roads are rebuilt from their descriptions.
//! Commands: Set Cross-section (describe the selected line), Add Junction (legacy centerline model).

use road_builder_core::abi::Output;
use road_builder_core::{builder, vc_plugin};
use serde_json::Value;

const MANIFEST: &str = r#"{
  "id": "com.cosmicceo.roadbuilder.build",
  "name": "Build Road",
  "version": "0.3.0",
  "kind": "filter",
  "author": "CosmicCEO",
  "description": "Builds roads from the lines you draw: a line named road.<id> becomes the road described by its cross-section (lanes, bike lanes, margins ...). Select All first, so every road is included.",
  "params": {
    "stubLength": {"type": "number", "min": 5, "max": 100, "default": 28, "unit": "m", "label": "Side street length"},
    "stubWidth": {"type": "number", "min": 3, "max": 20, "default": 8, "unit": "m", "label": "Side street width"}
  },
  "commands": [
    {
      "id": "set_cross_section",
      "label": "Describe Road (Cross-section)",
      "params": {
        "cross_section": {"type": "string", "default": "lane fwd | lane back", "maxLength": 1000, "label": "Cross-section", "help": "Left to right, separated by |. Each item: an element (lane, bike-lane, apron, shoulder, margin, median, turn-pocket, sidewalk ...), optional direction (N S E W fwd back), width (3, 3m, 10ft), repeat (x2). Start with 'facing N:' to say which way the list reads. Example: facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane"},
        "segment": {"type": "string", "default": "", "maxLength": 16, "label": "Road id", "help": "Leave empty to use the selected line (a plain line becomes a new road)."},
        "anchor": {"type": "string", "default": "", "maxLength": 24, "label": "Line follows", "help": "Which part of the road the drawn line follows: auto (the middle of the median or margin if there is one, else the middle of the road), center, left, right, or an element name such as margin. Leave empty to keep the current setting."},
        "scope": {"type": "choice", "options": ["segment", "selected"], "default": "segment", "help": "segment: every piece of this road. selected: only the selected pieces (they get an id of their own, e.g. after you broke the line and want to change one piece)."}
      }
    },
    {
      "id": "set_junction",
      "label": "Describe Junction",
      "params": {
        "description": {"type": "string", "default": "{\"legs\": [{\"side\": \"left\", \"cross_section\": \"lane fwd | lane back\"}, {\"side\": \"right\", \"cross_section\": \"lane fwd | lane back\"}], \"control\": \"signal\", \"crosswalks\": true}", "maxLength": 3000, "label": "Junction (JSON)", "help": "legs: up to two ({side: left/right or west/east/north/south, cross_section, length}); control: none, signal or stop; crosswalks: true/false; radius: kerb corner radius in metres. No legs = just carry the road across the gap."},
        "junction": {"type": "string", "default": "", "maxLength": 16, "label": "Junction id", "help": "Leave empty to use the selected connecting line (a plain line becomes a new junction)."}
      }
    },
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
        Some("set_cross_section") => builder::set_cross_section_text(&input, params),
        Some("set_junction") => builder::set_junction_text(&input, params),
        _ => builder::build_text(&input, params),
    }
    .map(Output::Raw)
}

vc_plugin!(MANIFEST, run);
