//! Circle – Circle intersection.
//!
//! Exposed as a public helper (`circle_circle_pts`) so that arc-based modules
//! can reuse the underlying circle math without duplicating it.

use cadkit_types::Vec3;

use crate::{primitives::Circle, utils::rot90};

use super::{Intersection, Intersects};

// ---------------------------------------------------------------------------
// Public helper: raw circle-circle computation (used by arc modules)
// ---------------------------------------------------------------------------

/// Compute the intersection of two full circles.
///
/// This is a free function so `arc_circle` and `arc_arc` can call it without
/// having to go through the trait dispatch path.
pub(crate) fn circle_circle_pts(c1: &Circle, c2: &Circle, tol: f64) -> Intersection {
    let dx = c2.center.x - c1.center.x;
    let dy = c2.center.y - c1.center.y;
    let d_sq = dx * dx + dy * dy;
    let d = d_sq.sqrt();
    let (r1, r2) = (c1.radius, c2.radius);

    // Concentric circles
    if d < tol {
        return if (r1 - r2).abs() < tol {
            Intersection::Coincident
        } else {
            Intersection::None
        };
    }

    let sum_r = r1 + r2;
    let diff_r = (r1 - r2).abs();

    // Completely separate (external) or one strictly inside the other
    if d > sum_r + tol || d < diff_r - tol {
        return Intersection::None;
    }

    // External tangent: circles touch from outside
    if (d - sum_r).abs() <= tol {
        let pt = Vec3::xy(c1.center.x + (r1 / d) * dx, c1.center.y + (r1 / d) * dy);
        return Intersection::Tangent(pt);
    }

    // Internal tangent: one circle internally tangent to the other
    if (d - diff_r).abs() <= tol {
        // Touch point is at distance r1 from c1 toward c2 if r1 ≥ r2,
        // or away from c2 if r2 > r1 (c1 inside c2).
        let sign: f64 = if r1 >= r2 { 1.0 } else { -1.0 };
        let pt = Vec3::xy(
            c1.center.x + sign * (r1 / d) * dx,
            c1.center.y + sign * (r1 / d) * dy,
        );
        return Intersection::Tangent(pt);
    }

    // Two intersection points
    // Standard formula: a = (r1² − r2² + d²) / (2d)
    //                   h = √(r1² − a²)
    let a = (r1 * r1 - r2 * r2 + d_sq) / (2.0 * d);
    let h_sq = (r1 * r1 - a * a).max(0.0);
    let h = h_sq.sqrt();

    let mid_x = c1.center.x + (a / d) * dx;
    let mid_y = c1.center.y + (a / d) * dy;

    let (perp_x, perp_y) = rot90((dx, dy));
    let px = (h / d) * perp_x;
    let py = (h / d) * perp_y;

    Intersection::Points(vec![
        Vec3::xy(mid_x + px, mid_y + py),
        Vec3::xy(mid_x - px, mid_y - py),
    ])
}

// ---------------------------------------------------------------------------
// Trait impl
// ---------------------------------------------------------------------------

impl Intersects<Circle> for Circle {
    fn intersect(&self, other: &Circle, tol: f64) -> Intersection {
        circle_circle_pts(self, other, tol)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-9;

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::xy(x, y)
    }
    fn c(cx: f64, cy: f64, r: f64) -> Circle {
        Circle::new(v(cx, cy), r)
    }
    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    #[test]
    fn two_crossing_circles() {
        // Circles of radius 1 centred at (-0.5, 0) and (0.5, 0)
        let c1 = c(-0.5, 0.0, 1.0);
        let c2 = c(0.5, 0.0, 1.0);
        let r = c1.intersect(&c2, TOL);
        assert_eq!(r.point_count(), 2);
        // By symmetry both points have x = 0
        let pts = r.points();
        assert!((pts[0].x).abs() < 1e-7);
        assert!((pts[1].x).abs() < 1e-7);
        // y coordinates are symmetric
        assert!((pts[0].y + pts[1].y).abs() < 1e-7);
    }

    #[test]
    fn external_tangent() {
        // Unit circles centred at (0,0) and (2,0): d = r1+r2 = 2
        let c1 = c(0.0, 0.0, 1.0);
        let c2 = c(2.0, 0.0, 1.0);
        let r = c1.intersect(&c2, TOL);
        assert!(
            matches!(r, Intersection::Tangent(_)),
            "expected Tangent, got {:?}",
            r
        );
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn internal_tangent_small_inside_large() {
        // Circle of radius 3 at (0,0), circle of radius 1 at (2,0).
        // d = 2 = r1 − r2 = 3 − 1.  Touch from inside.
        let big = c(0.0, 0.0, 3.0);
        let small = c(2.0, 0.0, 1.0);
        let r = big.intersect(&small, TOL);
        assert!(
            matches!(r, Intersection::Tangent(_)),
            "expected Tangent, got {:?}",
            r
        );
        // Touch point is at (3, 0): distance 3 from big centre, distance 1 from small centre
        assert!(approx_eq(r.points()[0], v(3.0, 0.0)));
    }

    #[test]
    fn internal_tangent_small_inside_large_opposite_side() {
        // Circle of radius 3 at (0,0), circle of radius 1 at (-2,0).
        // Small circle inside large, touching at (-3, 0).
        let big = c(0.0, 0.0, 3.0);
        let small = c(-2.0, 0.0, 1.0);
        let r = big.intersect(&small, TOL);
        assert!(matches!(r, Intersection::Tangent(_)));
        assert!(approx_eq(r.points()[0], v(-3.0, 0.0)));
    }

    #[test]
    fn circles_too_far_apart() {
        let c1 = c(0.0, 0.0, 1.0);
        let c2 = c(5.0, 0.0, 1.0);
        assert_eq!(c1.intersect(&c2, TOL), Intersection::None);
    }

    #[test]
    fn one_circle_inside_other_no_touch() {
        let c1 = c(0.0, 0.0, 5.0);
        let c2 = c(1.0, 0.0, 1.0);
        assert_eq!(c1.intersect(&c2, TOL), Intersection::None);
    }

    #[test]
    fn concentric_same_radius() {
        let c1 = c(0.0, 0.0, 2.0);
        let c2 = c(0.0, 0.0, 2.0);
        assert_eq!(c1.intersect(&c2, TOL), Intersection::Coincident);
    }

    #[test]
    fn concentric_different_radii() {
        let c1 = c(0.0, 0.0, 1.0);
        let c2 = c(0.0, 0.0, 2.0);
        assert_eq!(c1.intersect(&c2, TOL), Intersection::None);
    }

    #[test]
    fn intersection_points_on_both_circles() {
        // Verify points lie on both circles
        let c1 = c(0.0, 0.0, 2.0);
        let c2 = c(2.0, 0.0, 2.0);
        let r = c1.intersect(&c2, TOL);
        assert_eq!(r.point_count(), 2);
        for p in r.points() {
            let d1 = p.distance_to(&c1.center);
            let d2 = p.distance_to(&c2.center);
            assert!((d1 - c1.radius).abs() < 1e-7, "point not on c1: dist={d1}");
            assert!((d2 - c2.radius).abs() < 1e-7, "point not on c2: dist={d2}");
        }
    }

    #[test]
    fn intersection_points_on_both_circles_offset() {
        let c1 = c(1.0, 2.0, 3.0);
        let c2 = c(4.0, 2.0, 3.0);
        let r = c1.intersect(&c2, TOL);
        assert_eq!(r.point_count(), 2);
        for p in r.points() {
            let d1 = p.distance_to(&c1.center);
            let d2 = p.distance_to(&c2.center);
            assert!((d1 - c1.radius).abs() < 1e-7);
            assert!((d2 - c2.radius).abs() < 1e-7);
        }
    }
}
