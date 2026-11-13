//! Filter output written directly as JSON text (not as a JSON tree) because plug-ins run in an interpreter under an instruction budget.

use crate::geom::P;
use serde_json::Value;
use std::fmt::Write;

pub struct Out {
    /// New objects (they are listed first, so VectorCraft puts them just below the objects that are kept: the lines you drew stay on top).
    text: String,
    first: bool,
    /// Existing objects kept, in their original order.
    kept: String,
    first_kept: bool,
}

impl Default for Out {
    fn default() -> Out {
        Out::new()
    }
}

impl Out {
    pub fn new() -> Out {
        Out { text: String::new(), first: true, kept: String::new(), first_kept: true }
    }

    fn sep(&mut self) {
        if !self.first {
            self.text.push(',');
        }
        self.first = false;
    }

    fn sep_kept(&mut self) {
        if !self.first_kept {
            self.kept.push(',');
        }
        self.first_kept = false;
    }

    /// Keep an existing object as it is.
    pub fn keep(&mut self, id: i64) {
        self.sep_kept();
        let _ = write!(self.kept, "{{\"id\":{id}}}");
    }

    /// Keep an existing object and give it a new name.
    pub fn rename(&mut self, id: i64, name: &str) {
        self.sep_kept();
        let _ = write!(self.kept, "{{\"id\":{id},\"name\":{}}}", json_string(name));
    }

    /// Keep an existing line and give it a bold, bright stroke of the given colour and width (so it is easy to see and to click).
    pub fn keep_styled(&mut self, id: i64, rgb: [f64; 3], width: f64) {
        self.sep_kept();
        let _ = write!(self.kept, "{{\"id\":{id},\"strokes\":[{{\"paint\":");
        let mut paint = String::new();
        paint.push_str("{\"type\":\"solid\",\"color\":{\"model\":\"rgb\",\"r\":");
        num(&mut paint, rgb[0]);
        paint.push_str(",\"g\":");
        num(&mut paint, rgb[1]);
        paint.push_str(",\"b\":");
        num(&mut paint, rgb[2]);
        paint.push_str("}}");
        self.kept.push_str(&paint);
        self.kept.push_str(",\"width\":");
        num(&mut self.kept, width);
        self.kept.push_str(",\"cap\":\"round\",\"join\":\"round\",\"miterLimit\":4}]}");
    }

    fn open_object(&mut self, name: &str, subs: &[Vec<P>], map: &impl Fn(P) -> P, closed: bool) {
        self.sep();
        self.text.push_str("{\"name\":");
        self.text.push_str(&json_string(name));
        self.text.push_str(",\"path\":{\"subpaths\":[");
        for (i, sub) in subs.iter().enumerate() {
            if i > 0 {
                self.text.push(',');
            }
            self.text.push_str("{\"anchors\":[");
            for (k, &w) in sub.iter().enumerate() {
                let p = map(w);
                if k > 0 {
                    self.text.push(',');
                }
                self.text.push_str("{\"p\":[");
                num(&mut self.text, p[0]);
                self.text.push(',');
                num(&mut self.text, p[1]);
                self.text.push_str("]}");
            }
            self.text.push_str(if closed { "],\"closed\":true}" } else { "],\"closed\":false}" });
        }
        self.text.push_str("]}");
    }

    fn paint(&mut self, rgb: [f64; 3]) {
        self.text.push_str("{\"type\":\"solid\",\"color\":{\"model\":\"rgb\",\"r\":");
        num(&mut self.text, rgb[0]);
        self.text.push_str(",\"g\":");
        num(&mut self.text, rgb[1]);
        self.text.push_str(",\"b\":");
        num(&mut self.text, rgb[2]);
        self.text.push_str("}}");
    }

    /// A new stroked path; `map` converts each point to a document point.
    pub fn path(&mut self, name: &str, subs: &[Vec<P>], map: impl Fn(P) -> P, rgb: [f64; 3], width: f64, closed: bool) {
        self.open_object(name, subs, &map, closed);
        self.text.push_str(",\"strokes\":[{\"paint\":");
        self.paint(rgb);
        self.text.push_str(",\"width\":");
        num(&mut self.text, width);
        self.text.push_str(",\"cap\":\"butt\",\"join\":\"miter\",\"miterLimit\":4}]}");
    }

    /// Several closed filled shapes in one object (a row of triangles, say), with no outline.
    pub fn fills(&mut self, name: &str, rings: &[Vec<P>], map: impl Fn(P) -> P, rgb: [f64; 3]) {
        self.open_object(name, rings, &map, true);
        self.text.push_str(",\"fills\":[");
        self.paint(rgb);
        self.text.push_str("]}");
    }

    /// A new filled shape with no outline.
    pub fn polygon(&mut self, name: &str, ring: &[P], map: impl Fn(P) -> P, rgb: [f64; 3]) {
        self.open_object(name, std::slice::from_ref(&ring.to_vec()), &map, true);
        // A fill is the paint itself (a stroke wraps its paint in `paint` next to its width).
        self.text.push_str(",\"fills\":[");
        self.paint(rgb);
        self.text.push_str("]}");
    }

    pub fn finish(self) -> String {
        self.finish_with(None)
    }

    /// Ends the output, optionally replacing the data the plug-in keeps with the document. New objects come first, then the kept ones.
    pub fn finish_with(self, document_data: Option<&Value>) -> String {
        let mut out = String::from("{\"objects\":[");
        out.push_str(&self.text);
        if !self.text.is_empty() && !self.kept.is_empty() {
            out.push(',');
        }
        out.push_str(&self.kept);
        out.push(']');
        if let Some(d) = document_data {
            out.push_str(",\"documentData\":");
            out.push_str(&serde_json::to_string(d).unwrap_or_else(|_| "null".into()));
        }
        out.push('}');
        out
    }
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// A number to the nearest millimetre, without float-formatting machinery.
pub fn num(out: &mut String, x: f64) {
    let v = (x * 1000.0).round() as i64;
    if v < 0 {
        out.push('-');
    }
    let v = v.unsigned_abs();
    let _ = write!(out, "{}", v / 1000);
    let frac = v % 1000;
    if frac != 0 {
        let digits = format!("{frac:03}");
        out.push('.');
        out.push_str(digits.trim_end_matches('0'));
    }
}
