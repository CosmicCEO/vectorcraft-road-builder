//! Plain 2-D geometry in metres. World frame: x east, y north. "Right" of the direction of travel is positive offset.

pub type P = [f64; 2];

pub fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}

pub fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}

pub fn scale(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}

pub fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

/// Centre of the bounding box of `pts`.
pub fn bounds_center(pts: &[P]) -> Option<P> {
    let first = *pts.first()?;
    let (mut lo, mut hi) = (first, first);
    for p in pts {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    Some([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0])
}

/// A polyline with cumulative arc length: the centerline. `s` is the "route metre".
pub struct Poly {
    pub pts: Vec<P>,
    cum: Vec<f64>,
}

impl Poly {
    pub fn new(raw: Vec<P>) -> Poly {
        let mut pts: Vec<P> = Vec::with_capacity(raw.len());
        for p in raw {
            if pts.last().is_none_or(|l| (l[0] - p[0]).hypot(l[1] - p[1]) > 1e-9) {
                pts.push(p);
            }
        }
        let mut cum = vec![0.0];
        for i in 1..pts.len() {
            let d = (pts[i][0] - pts[i - 1][0]).hypot(pts[i][1] - pts[i - 1][1]);
            cum.push(cum[i - 1] + d);
        }
        Poly { pts, cum }
    }

    pub fn len(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }

    pub fn is_empty(&self) -> bool {
        self.pts.len() < 2
    }

    /// Point and unit tangent at route metre `s` (clamped to the ends).
    pub fn at(&self, s: f64) -> (P, P) {
        let s = s.clamp(0.0, self.len());
        let i = self.cum.partition_point(|&c| c < s).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let span = self.cum[i] - self.cum[i - 1];
        let t = if span > 0.0 { (s - self.cum[i - 1]) / span } else { 0.0 };
        let d = sub(b, a);
        let mag = d[0].hypot(d[1]).max(1e-12);
        ([a[0] + d[0] * t, a[1] + d[1] * t], [d[0] / mag, d[1] / mag])
    }

    /// Unit normal pointing right of the direction of travel.
    pub fn right(t: P) -> P {
        [t[1], -t[0]]
    }

    /// The point `off` metres right of the centerline at `s`, and the tangent there.
    pub fn offset_point(&self, s: f64, off: f64) -> (P, P) {
        let (q, t) = self.at(s);
        (add(q, scale(Poly::right(t), off)), t)
    }

    /// Nearest point of the polyline to `p`: (route metre, signed offset, distance).
    pub fn project(&self, p: P) -> (f64, f64, f64) {
        self.project_in(p, 0.0, f64::INFINITY)
    }

    /// The inverse of "route metre `s`, offset `off`" for a point placed near route metre `hint`: projects onto the line of the
    /// segment holding `hint` (not clamped to the segment), which is exactly how the point was placed, even 8 m inside a sharp bend
    /// where the nearest point of the road would be a different segment. If that lands more than 0.6 m outside the segment
    /// (the point was dragged elsewhere), falls back to the nearest point of the whole road.
    pub fn project_hinted(&self, p: P, hint: f64) -> (f64, f64, f64) {
        let h = hint.clamp(0.0, self.len());
        let i = self.cum.partition_point(|&c| c < h).clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let d = sub(b, a);
        let len = d[0].hypot(d[1]).max(1e-12);
        let t = dot(sub(p, a), d) / (len * len);
        let s = self.cum[i - 1] + t * len;
        if s >= self.cum[i - 1] - 0.6 && s <= self.cum[i] + 0.6 {
            let n = [d[1] / len, -d[0] / len];
            let off = dot(sub(p, a), n);
            return (s, off, off.abs());
        }
        self.project(p)
    }

    fn project_in(&self, p: P, hint: f64, window: f64) -> (f64, f64, f64) {
        let mut best = (0.0, 0.0, f64::INFINITY);
        for i in 1..self.pts.len() {
            if self.cum[i] < hint - window || self.cum[i - 1] > hint + window {
                continue;
            }
            let (a, b) = (self.pts[i - 1], self.pts[i]);
            let d = sub(b, a);
            let len2 = dot(d, d);
            let t = if len2 > 0.0 { (dot(sub(p, a), d) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let q = [a[0] + d[0] * t, a[1] + d[1] * t];
            let dist = (p[0] - q[0]).hypot(p[1] - q[1]);
            if dist < best.2 {
                let mag = len2.sqrt().max(1e-12);
                let n = [d[1] / mag, -d[0] / mag];
                best = (self.cum[i - 1] + t * (self.cum[i] - self.cum[i - 1]), dot(sub(p, q), n), dist);
            }
        }
        best
    }
}

/// A function of route metre drawn as a polyline in (s, value) space, sorted by `s`.
pub struct Curve {
    pts: Vec<P>,
}

impl Curve {
    pub fn new(mut pts: Vec<P>) -> Curve {
        pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
        Curve { pts }
    }

    pub fn points(&self) -> &[P] {
        &self.pts
    }

    pub fn at(&self, s: f64) -> Option<f64> {
        let first = self.pts.first()?;
        let last = self.pts.last()?;
        if s <= first[0] {
            return Some(first[1]);
        }
        if s >= last[0] {
            return Some(last[1]);
        }
        let i = self.pts.partition_point(|p| p[0] < s).max(1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let span = b[0] - a[0];
        Some(if span > 0.0 { a[1] + (b[1] - a[1]) * (s - a[0]) / span } else { b[1] })
    }
}
