//! Road Builder exporter: writes the document's road as a `.pmap` file.

use road_builder_core::abi::Output;
use road_builder_core::{export, vc_plugin};
use serde_json::{Value, json};

const MANIFEST: &str = r#"{
  "id": "com.cosmicceo.roadbuilder.export",
  "name": "Road Builder road (.pmap)",
  "version": "0.1.0",
  "kind": "exporter",
  "author": "CosmicCEO",
  "description": "Writes the centerline, lane profile, flags, junctions and arrows as a pmap road file for the simulation.",
  "extensions": ["pmap"]
}"#;

fn run(input: &[u8], _params: &Value) -> Result<Output, String> {
    let doc: Value = serde_json::from_slice(input).map_err(|e| e.to_string())?;
    let (text, warnings) = export::export(&doc)?;
    Ok(Output::Json(json!({"dataBase64": export::base64(text.as_bytes()), "warnings": warnings})))
}

vc_plugin!(MANIFEST, run);
