//! Exporter: reads the whole native VectorCraft document and writes the `.pmap` file.

use crate::pmap::PMap;
use crate::scene::Scene;
use crate::vcdoc::items_from_native;
use serde_json::Value;

/// The pmap text for a native document, and warnings for the user.
pub fn export(doc: &Value) -> Result<(String, Vec<String>), String> {
    let items = items_from_native(doc)?;
    let sc = Scene::from_items(&items)?;
    let mut warnings = Vec::new();
    if sc.curves.is_empty() {
        warnings.push("No profile.* curves found: the pmap has no lane lines.".to_string());
    }
    if sc.junctions.is_empty() {
        warnings.push("No junction markers (J1.kind.side) found.".to_string());
    }
    Ok((PMap::from_scene(&sc).to_json(), warnings))
}

/// Standard base64 (the host wants the file bytes encoded).
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() { out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char) } else { out.push('=') }
        }
    }
    out
}
