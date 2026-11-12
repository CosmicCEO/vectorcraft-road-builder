//! Offset a polyline sideways by a fixed distance, the way a road edge follows a centreline.
//!
//! At every vertex the two neighbouring offset segments are joined at their intersection (the "mitre point"), which is exactly right
//! for the gentle turns of a flattened curve, gives a sharp outside corner at an angle point, and trims the inside of a corner instead
//! of letting it run past. Very sharp outside corners fall back to a flat bevel (mitre limit); any loop left where the offset is larger
//! than the local radius is cut out. Offsets are positive to the right of the direction of travel.

use crate::geom::{P, add, dot, scale, sub};

/// Mitre limit as a multiple of the offset distance: a corner sharper than this is bevelled instead of mitred (4 is about 29 degrees).
pub const MITRE_LIMIT: f64 = 4.0;

fn unit(a: P, b: P) -> Option<P> {
    let d = sub(b, a);
    let m = d[0].hypot(d[1]);
    (m > 1e-9).then(|| [d[0] / m, d[1] / m])
}

fn right(t: P) -> P {
    [t[1], -t[0]]
}

/// Proper intersection point of segments ab and cd (not touching at an end), if any.
fn cross_point(a: P, b: P, c: P, d: P) -> Option<P> {
    let (r, s) = (sub(b, a), sub(d, c));
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let ca = sub(c, a);
    let t = (ca[0] * s[1] - ca[1] * s[0]) / den;
    let u = (ca[0] * r[1] - ca[1] * r[0]) / den;
    (t > 1e-9 && t < 1.0 - 1e-9 && u > 1e-9 && u < 1.0 - 1e-9).then(|| add(a, scale(r, t)))
}

/// Cuts out loops where a segment crosses another one a few segments later.
fn remove_loops(mut pts: Vec<P>, window: usize) -> Vec<P> {
    for _ in 0..64 {
        let n = pts.len();
        let mut found = None;
        'scan: for i in 0..n.saturating_sub(3) {
            for j in (i + 2)..(i + window).min(n - 1) {
                if let Some(x) = cross_point(pts[i], pts[i + 1], pts[j], pts[j + 1]) {
                    found = Some((i, j, x));
                    break 'scan;
                }
            }
        }
        let Some((i, j, x)) = found else { return pts };
        let mut next = pts[..=i].to_vec();
        next.push(x);
        next.extend_from_slice(&pts[j + 1..]);
        pts = next;
    }
    pts
}

/// `path` moved `off` metres to the right (negative: to the left), with proper joins. An empty result means the path has no length.
pub fn offset_polyline(path: &[P], off: f64, mitre_limit: f64) -> Vec<P> {
    let mut pts: Vec<P> = Vec::with_capacity(path.len());
    for &p in path {
        if pts.last().is_none_or(|l| unit(*l, p).is_some()) {
            pts.push(p);
        }
    }
    if pts.len() < 2 {
        return vec![];
    }
    let tangents: Vec<P> = pts.windows(2).map(|w| unit(w[0], w[1]).unwrap_or([1.0, 0.0])).collect();
    let normals: Vec<P> = tangents.iter().map(|&t| right(t)).collect();
    let mut out = vec![add(pts[0], scale(normals[0], off))];
    for i in 1..pts.len() - 1 {
        let (n1, n2) = (normals[i - 1], normals[i]);
        let denom = 1.0 + dot(n1, n2);
        let mitre = if denom > 1e-6 { [(n1[0] + n2[0]) / denom, (n1[1] + n2[1]) / denom] } else { [f64::INFINITY, 0.0] };
        if mitre[0].hypot(mitre[1]) > mitre_limit {
            out.push(add(pts[i], scale(n1, off)));
            out.push(add(pts[i], scale(n2, off)));
        } else {
            out.push(add(pts[i], scale(mitre, off)));
        }
    }
    let last = pts.len() - 1;
    out.push(add(pts[last], scale(normals[last - 1], off)));
    remove_loops(out, 24)
}
