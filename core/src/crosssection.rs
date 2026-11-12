//! A road's cross-section: an ordered list of elements, written as a short line of text and laid out as offsets from the path.
//!
//! ```text
//! facing N: bike-lane | apron | lane S x2 | margin 3 | lane N x2 | apron | bike-lane
//! ```
//! In right-hand traffic, facing along the line the lane on the right goes the way the line goes (`fwd`) and the lane on the left comes toward you
//! (`back`): a two-lane road is `lane back | lane fwd`. Items are separated by `|` or `,`. An item is an element name (or alias) with optional pieces in any order: a direction
//! (`N S E W`, or `fwd`/`back`), a width (`3`, `3m`, `10ft`), and a repeat (`x2`). The list reads left to right as seen facing the
//! compass direction after `facing` (or facing along the path when that is left out).

use crate::geom::{P, dot};
use crate::library::{Library, Role};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Way {
    North,
    South,
    East,
    West,
    Fwd,
    Back,
}

impl Way {
    fn parse(t: &str) -> Option<Way> {
        Some(match t {
            "n" | "north" | "northbound" => Way::North,
            "s" | "south" | "southbound" => Way::South,
            "e" | "east" | "eastbound" => Way::East,
            "w" | "west" | "westbound" => Way::West,
            "fwd" | "forward" => Way::Fwd,
            "back" | "backward" => Way::Back,
            _ => return None,
        })
    }

    /// The short written form: N, S, E, W, fwd, back.
    pub fn letter(self) -> &'static str {
        match self {
            Way::North => "N",
            Way::South => "S",
            Way::East => "E",
            Way::West => "W",
            Way::Fwd => "fwd",
            Way::Back => "back",
        }
    }

    fn compass(self) -> Option<P> {
        match self {
            Way::North => Some([0.0, 1.0]),
            Way::South => Some([0.0, -1.0]),
            Way::East => Some([1.0, 0.0]),
            Way::West => Some([-1.0, 0.0]),
            _ => None,
        }
    }
}

/// One item as written.
#[derive(Clone, PartialEq, Debug)]
pub struct Item {
    pub kind: String,
    pub way: Option<Way>,
    pub width: Option<f64>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Parsed {
    pub facing: Option<Way>,
    pub items: Vec<Item>,
    /// Things the parser had to guess, for the user to see.
    pub notes: Vec<String>,
}

impl Parsed {
    fn check(&self, i: usize) -> Result<(), String> {
        if i < self.items.len() { Ok(()) } else { Err(format!("there is no item {} (the cross-section has {} items, numbered from 0)", i, self.items.len())) }
    }

    /// Removes item `i`. A cross-section must keep at least one element.
    pub fn delete(&mut self, i: usize) -> Result<(), String> {
        self.check(i)?;
        if self.items.len() == 1 {
            return Err("cannot delete the only element".into());
        }
        self.items.remove(i);
        Ok(())
    }

    /// Moves item `from` so that it ends up at position `to`.
    pub fn move_item(&mut self, from: usize, to: usize) -> Result<(), String> {
        self.check(from)?;
        self.check(to)?;
        let it = self.items.remove(from);
        self.items.insert(to, it);
        Ok(())
    }

    pub fn set_width(&mut self, i: usize, width: f64) -> Result<(), String> {
        self.check(i)?;
        if !(width > 0.0 && width < 100.0) {
            return Err("a width must be between 0 and 100 metres".into());
        }
        self.items[i].width = Some(width);
        Ok(())
    }

    /// Inserts the element(s) written in `text` (same shorthand, one item or several) before position `at` (`items.len()` appends).
    pub fn insert(&mut self, at: usize, text: &str, lib: &Library) -> Result<(), String> {
        if at > self.items.len() {
            return Err(format!("cannot insert at {at}: the cross-section has {} items", self.items.len()));
        }
        let more = parse(text, lib)?;
        self.notes.extend(more.notes);
        for (k, it) in more.items.into_iter().enumerate() {
            self.items.insert(at + k, it);
        }
        Ok(())
    }
}

/// Which way along the path a travel element runs, once the compass words are resolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    Fwd,
    Back,
}

/// An element placed across the road: `left` and `right` are offsets from the reference (negative = left of the direction of travel).
#[derive(Clone, Debug)]
pub struct Placed {
    pub id: String,
    pub role: Role,
    pub dir: Option<Dir>,
    pub width: f64,
    pub left: f64,
    pub right: f64,
    pub fill: [f64; 3],
}

/// Which part of the road the drawn line follows.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Anchor {
    /// The middle of the median, margin or centre turn lane if the road has one (so editing a side element does not move the road),
    /// otherwise the middle of the whole road.
    Auto,
    Center,
    Left,
    Right,
    /// The middle of the first element of this kind (an element id or alias).
    Element(String),
}

fn width_of(t: &str) -> Option<f64> {
    let (num, scale) = if let Some(n) = t.strip_suffix("ft") { (n, 0.3048) } else if let Some(n) = t.strip_suffix('m') { (n, 1.0) } else { (t, 1.0) };
    num.parse::<f64>().ok().filter(|w| *w > 0.0 && *w < 100.0).map(|w| w * scale)
}

fn word_number(t: &str) -> Option<usize> {
    ["one", "two", "three", "four", "five", "six"].iter().position(|w| *w == t).map(|i| i + 1)
}

fn repeat_of(t: &str) -> Option<usize> {
    t.strip_prefix('x').or_else(|| t.strip_prefix('×')).and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=12).contains(n))
}

pub fn parse(text: &str, lib: &Library) -> Result<Parsed, String> {
    let mut out = Parsed::default();
    let mut body = text.trim();
    if let Some(rest) = body.strip_prefix("facing").or_else(|| body.strip_prefix("Facing")) {
        let (head, tail) = rest.split_once(':').ok_or("after \"facing\" write a compass direction and a colon, like \"facing N:\"")?;
        let way = Way::parse(&head.trim().to_ascii_lowercase()).filter(|w| w.compass().is_some()).ok_or_else(|| format!("\"{}\" is not a compass direction (use N, S, E or W)", head.trim()))?;
        out.facing = Some(way);
        body = tail;
    }
    for raw in body.split(['|', ',']) {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (mut words, mut way, mut width, mut times) = (Vec::new(), None, None, 1);
        let toks: Vec<String> = raw.split_whitespace().map(str::to_ascii_lowercase).collect();
        let mut i = 0;
        while i < toks.len() {
            let tok = &toks[i];
            let unit_next = toks.get(i + 1).is_some_and(|u| matches!(u.as_str(), "m" | "meter" | "meters" | "metre" | "metres" | "ft" | "feet" | "foot"));
            if let Some(n) = repeat_of(tok) {
                times = n;
            } else if let (true, Ok(num)) = (unit_next, tok.parse::<f64>()) {
                let feet = matches!(toks[i + 1].as_str(), "ft" | "feet" | "foot");
                width = Some(num * if feet { 0.3048 } else { 1.0 }).filter(|w| *w > 0.0 && *w < 100.0);
                i += 1;
            } else if let Some(w) = width_of(tok) {
                // A whole number before the element name is a count ("2 lanes"); anywhere else a number is a width ("margin 3").
                if words.is_empty() && w.fract() == 0.0 && !tok.ends_with('m') && !tok.ends_with("ft") {
                    times = w as usize;
                } else {
                    width = Some(w);
                }
            } else if let Some(n) = word_number(tok) {
                times = n;
            } else if let Some(w) = Way::parse(tok) {
                way = Some(w);
            } else if !matches!(tok.as_str(), "a" | "an" | "the" | "and" | "with" | "of" | "one-way") {
                words.push(tok.clone());
            }
            i += 1;
        }
        if words.is_empty() {
            return Err(format!("\"{raw}\" has no element name"));
        }
        let joined = words.join("-");
        let singular = |w: &str| w.strip_suffix('s').unwrap_or(w).to_string();
        let last_singular = words.last().map(|w| singular(w));
        let kind = if let Some(def) = lib.get(&joined).or_else(|| lib.get(&format!("{}-{}", words[..words.len() - 1].join("-"), last_singular.clone().unwrap_or_default()).trim_start_matches('-'))) {
            def.id.clone()
        } else if let Some(def) = words.last().and_then(|w| lib.get(w)).or_else(|| last_singular.as_deref().and_then(|w| lib.get(w))) {
            out.notes.push(format!("read \"{}\" as \"{}\"", words.join(" "), def.id));
            def.id.clone()
        } else {
            let near: Vec<&str> = lib.names().into_iter().filter(|n| words.iter().any(|w| n.contains(w.as_str()) || w.contains(n))).collect();
            let hint = if near.is_empty() { format!("known elements: {}", lib.names().join(", ")) } else { format!("did you mean {}?", near.join(" or ")) };
            return Err(format!("unknown element \"{}\" ({hint})", words.join(" ")));
        };
        for _ in 0..times {
            out.items.push(Item { kind: kind.clone(), way, width });
        }
    }
    if out.items.is_empty() {
        return Err("the cross-section has no elements".into());
    }
    Ok(out)
}

/// The same cross-section as canonical text: equal neighbours collapse to `xN`.
pub fn format(p: &Parsed) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    while i < p.items.len() {
        let it = &p.items[i];
        let mut n = 1;
        while i + n < p.items.len() && p.items[i + n] == *it {
            n += 1;
        }
        let mut s = it.kind.clone();
        if let Some(w) = it.way {
            s = format!("{s} {}", w.letter());
        }
        if let Some(w) = it.width {
            s = format!("{s} {}", (w * 1000.0).round() / 1000.0);
        }
        if n > 1 {
            s = format!("{s} x{n}");
        }
        parts.push(s);
        i += n;
    }
    let body = parts.join(" | ");
    match p.facing {
        Some(f) => format!("facing {}: {body}", f.letter()),
        None => body,
    }
}

/// Lays the elements across the road for a path whose general heading is `heading` (a unit vector, world frame: x east, y north).
pub fn layout(p: &Parsed, lib: &Library, heading: P, anchor: Anchor) -> Result<Vec<Placed>, String> {
    // The list reads left to right facing `facing`; if the path runs the other way, read it from the other end.
    let reversed = p.facing.and_then(Way::compass).is_some_and(|f| dot(heading, f) < 0.0);
    let mut items: Vec<&Item> = p.items.iter().collect();
    if reversed {
        items.reverse();
    }
    let dir_of = |w: Option<Way>| -> Option<Dir> {
        match w? {
            Way::Fwd => Some(if reversed { Dir::Back } else { Dir::Fwd }),
            Way::Back => Some(if reversed { Dir::Fwd } else { Dir::Back }),
            compass => Some(if dot(heading, compass.compass()?) >= 0.0 { Dir::Fwd } else { Dir::Back }),
        }
    };
    let mut placed: Vec<Placed> = Vec::with_capacity(items.len());
    let mut x = 0.0;
    for it in items {
        let def = lib.get(&it.kind).ok_or_else(|| format!("unknown element \"{}\"", it.kind))?;
        let width = it.width.unwrap_or(def.width);
        placed.push(Placed { id: def.id.clone(), role: def.role, dir: if def.directional { dir_of(it.way) } else { None }, width, left: x, right: x + width, fill: def.fill });
        x += width;
    }
    let middle_of = |pred: &dyn Fn(&Placed) -> bool| placed.iter().find(|e| pred(e)).map(|e| -(e.left + e.right) / 2.0);
    let shift = match &anchor {
        Anchor::Center => -x / 2.0,
        Anchor::Left => 0.0,
        Anchor::Right => -x,
        Anchor::Auto => middle_of(&|e| ["median", "margin", "center-turn-lane", "painted-median"].contains(&e.id.as_str())).unwrap_or(-x / 2.0),
        Anchor::Element(name) => {
            let id = lib.get(name).map(|d| d.id.clone()).ok_or_else(|| format!("the anchor \"{name}\" is not an element"))?;
            middle_of(&|e| e.id == id).ok_or_else(|| format!("the anchor \"{name}\" is not in this cross-section"))?
        }
    };
    for e in &mut placed {
        e.left += shift;
        e.right += shift;
    }
    Ok(placed)
}
