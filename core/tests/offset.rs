use road_builder_core::geom::P;
use road_builder_core::offset::{MITRE_LIMIT, offset_polyline};

fn near(a: P, b: P, tol: f64) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) <= tol
}

fn has(pts: &[P], want: P) -> bool {
    pts.iter().any(|&p| near(p, want, 1e-6))
}

fn o(a: P, b: P, c: P) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn self_crossings(l: &[P]) -> usize {
    let mut n = 0;
    for i in 0..l.len() - 1 {
        for j in i + 2..l.len() - 1 {
            if o(l[i], l[i + 1], l[j]) * o(l[i], l[i + 1], l[j + 1]) < 0.0 && o(l[j], l[j + 1], l[i]) * o(l[j], l[j + 1], l[i + 1]) < 0.0 {
                n += 1;
            }
        }
    }
    n
}

fn length(l: &[P]) -> f64 {
    l.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum()
}

/// North 100 m, then a right turn east 100 m (x east, y north).
const RIGHT_ANGLE: [P; 3] = [[0.0, 0.0], [0.0, 100.0], [100.0, 100.0]];

#[test]
fn straight_line_moves_sideways_by_the_offset() {
    let e = offset_polyline(&[[0.0, 0.0], [0.0, 50.0]], 3.0, MITRE_LIMIT);
    assert_eq!(e.len(), 2);
    assert!(near(e[0], [3.0, 0.0], 1e-9) && near(e[1], [3.0, 50.0], 1e-9), "right of north is east: {e:?}");
    let l = offset_polyline(&[[0.0, 0.0], [0.0, 50.0]], -3.0, MITRE_LIMIT);
    assert!(near(l[0], [-3.0, 0.0], 1e-9), "left of north is west: {l:?}");
}

#[test]
fn a_90_degree_corner_has_a_sharp_outside_corner() {
    let outside = offset_polyline(&RIGHT_ANGLE, -7.0, MITRE_LIMIT);
    assert!(has(&outside, [-7.0, 107.0]), "the mitred corner point is on the edge: {outside:?}");
    assert_eq!(outside.len(), 3, "one point per vertex, nothing cut across");
    assert_eq!(self_crossings(&outside), 0);
    assert!((length(&outside) - (107.0 + 107.0)).abs() < 1e-6, "legs of 107 m each");
}

#[test]
fn a_90_degree_corner_is_trimmed_on_the_inside() {
    let inside = offset_polyline(&RIGHT_ANGLE, 7.0, MITRE_LIMIT);
    assert!(has(&inside, [7.0, 93.0]), "the trimmed corner point is on the edge: {inside:?}");
    assert_eq!(self_crossings(&inside), 0);
    assert!((length(&inside) - (93.0 + 93.0)).abs() < 1e-6, "legs of 93 m each, no overshoot and no spur");
}

#[test]
fn a_hairpin_is_bevelled_not_exploded() {
    let e = offset_polyline(&[[0.0, 0.0], [0.0, 100.0], [1.0, 0.0]], -5.0, MITRE_LIMIT);
    assert!(e.iter().all(|p| p[0].abs() < 20.0 && p[1] < 120.0), "no runaway mitre spike: {e:?}");
}

#[test]
fn a_flattened_curve_stays_at_the_offset_distance() {
    // A quarter circle of radius 50 around (0, 0) from (50, 0) to (0, 50), counter-clockwise: travel turns left, so "right" is outside.
    let arc: Vec<P> = (0..=64).map(|k| { let a = std::f64::consts::FRAC_PI_2 * k as f64 / 64.0; [50.0 * a.cos(), 50.0 * a.sin()] }).collect();
    for (off, radius) in [(7.0, 57.0), (-7.0, 43.0)] {
        let e = offset_polyline(&arc, off, MITRE_LIMIT);
        let worst = e.iter().map(|p| (p[0].hypot(p[1]) - radius).abs()).fold(0.0, f64::max);
        assert!(worst < 0.01, "offset {off}: worst deviation {worst} m from a circle of radius {radius}");
    }
}

#[test]
fn an_offset_larger_than_the_curve_radius_leaves_no_loop() {
    let arc: Vec<P> = (0..=32).map(|k| { let a = std::f64::consts::FRAC_PI_2 * k as f64 / 32.0; [20.0 * a.cos(), 20.0 * a.sin()] }).collect();
    let e = offset_polyline(&arc, -30.0, MITRE_LIMIT); // 30 m inside a 20 m radius: the offset curve inverts
    assert_eq!(self_crossings(&e), 0, "loops are cut out");
}

#[test]
fn an_empty_or_pointlike_path_gives_nothing() {
    assert!(offset_polyline(&[], 3.0, MITRE_LIMIT).is_empty());
    assert!(offset_polyline(&[[1.0, 1.0], [1.0, 1.0]], 3.0, MITRE_LIMIT).is_empty());
}
