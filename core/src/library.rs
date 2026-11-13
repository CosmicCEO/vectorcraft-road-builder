//! The element library: the standard pieces a road cross-section is made of (lane, bike lane, apron, curbed margin, ...).
//!
//! The built-in list lives in `library/elements.json` (data, not code). A drawing can override widths or add elements with its own
//! entries, merged on top with `merge`. Widths are typical US values for illustrative reconstructions, not survey data.

use serde_json::Value;
use std::collections::BTreeMap;

/// What an element does, which decides how its edges are painted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Travel,
    Pocket,
    Bike,
    Buffer,
    Edge,
    Parking,
    Curbed,
    Flush,
    Walk,
}

impl Role {
    fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "travel" => Role::Travel,
            "pocket" => Role::Pocket,
            "bike" => Role::Bike,
            "buffer" => Role::Buffer,
            "edge" => Role::Edge,
            "parking" => Role::Parking,
            "curbed" => Role::Curbed,
            "flush" => Role::Flush,
            "walk" => Role::Walk,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ElementDef {
    pub id: String,
    pub label: String,
    pub width: f64,
    pub role: Role,
    pub directional: bool,
    pub fill: [f64; 3],
    pub source: String,
}

pub struct Library {
    elements: BTreeMap<String, ElementDef>,
    aliases: BTreeMap<String, String>,
}

impl Library {
    /// The library shipped with the plug-in.
    pub fn builtin() -> Library {
        let v: Value = serde_json::from_str(include_str!("../../library/elements.json")).expect("library/elements.json is valid JSON");
        Library::from_json(&v).expect("library/elements.json is a valid library")
    }

    pub fn from_json(v: &Value) -> Result<Library, String> {
        let mut lib = Library { elements: BTreeMap::new(), aliases: BTreeMap::new() };
        lib.merge(v)?;
        Ok(lib)
    }

    /// Adds or replaces elements and aliases from a library-shaped JSON value (`{"elements": {...}, "aliases": {...}}`).
    pub fn merge(&mut self, v: &Value) -> Result<(), String> {
        if let Some(els) = v["elements"].as_object() {
            for (id, e) in els {
                let role = e["role"].as_str().and_then(Role::parse).ok_or_else(|| format!("element {id}: missing or unknown role"))?;
                let width = e["width"].as_f64().filter(|w| *w > 0.0 && *w < 100.0).ok_or_else(|| format!("element {id}: width must be between 0 and 100 m"))?;
                let fill = e["fill"].as_array().map(|a| [0, 1, 2].map(|i| a.get(i).and_then(Value::as_f64).unwrap_or(0.85))).unwrap_or([0.85; 3]);
                self.elements.insert(
                    id.clone(),
                    ElementDef {
                        id: id.clone(),
                        label: e["label"].as_str().unwrap_or(id).to_string(),
                        width,
                        role,
                        directional: e["directional"].as_bool().unwrap_or(false),
                        fill,
                        source: e["source"].as_str().unwrap_or("").to_string(),
                    },
                );
            }
        }
        if let Some(al) = v["aliases"].as_object() {
            for (k, t) in al {
                if let Some(t) = t.as_str() {
                    self.aliases.insert(k.clone(), t.to_string());
                }
            }
        }
        Ok(())
    }

    /// An element by id or alias.
    pub fn get(&self, name: &str) -> Option<&ElementDef> {
        let key = name.to_ascii_lowercase();
        self.elements.get(&key).or_else(|| self.aliases.get(&key).and_then(|t| self.elements.get(t)))
    }

    pub fn names(&self) -> Vec<&str> {
        self.elements.keys().map(String::as_str).collect()
    }
}
