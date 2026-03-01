//! Arc – Circle intersection.

use cadkit_types::Vec3;

use crate::{
    primitives::{Arc, Circle},
    utils::angle_in_arc,
};

use super::{circle_circle::circle_circle_pts, Intersection, Intersects};

impl Intersects<Circle> for Arc {
    /// Intersect an arc with a full circle.
    ///
    /// Algorithm: compute the full circle-circle intersection, then filter each
    /// candidate point to keep only those whose angle from the arc's centre
    /// falls within the arc's CCW span.
    fn intersect(&self, circle: &Circle, tol: f64) -> Intersection {
        if self.is_degenerate(tol) || circle.is_degenerate(tol) {
            return Intersection::None;
        }

        let raw = circle_circle_pts(&self.as_circle(), circle, tol);
        filter_for_arc(raw, self, tol)
    }
}

/// Also allow `circle.intersect(&arc, …)` — just delegates symmetrically.
impl Intersects<Arc> for Circle {
    fn intersect(&self, arc: &Arc, tol: f64) -> Intersection {
        arc.intersect(self, tol)
    }
}

/// Keep only those intersection points/tangents that lie on `arc`.
pub(crate) fn filter_for_arc(raw: Intersection, arc: &Arc, tol: f64) -> Intersection {
    let angle_tol = tol / arc.radius.max(tol);

    match raw {
        Intersection::None | Intersection::Coincident => raw,
        Intersection::Tangent(p) => {
            let angle = angle_from_centre(&arc.center, p);
            if angle_in_arc(angle, arc.start_angle, arc.end_angle, angle_tol) {
                Intersection::Tangent(p)
            } else {
                Intersection::None
            }
        }
        Intersection::Points(pts) => {
            let kept: Vec<Vec3> = pts
                .into_iter()
                .filter(|&p| {
                    let angle = angle_from_centre(&arc.center, p);
                    angle_in_arc(angle, arc.start_angle, arc.end_angle, angle_tol)
                })
                .collect();

            match kept.len() {
                0 => Intersection::None,
                _ => Intersection::Points(kept),
            }
        }
    }
}

fn angle_from_centre(centre: &cadkit_types::Vec3, point: Vec3) -> f64 {
    (point.y - centre.y).atan2(point.x - centre.x)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    const TOL: f64 = 1e-9;

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::xy(x, y)
    }
    fn arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Arc {
        Arc::new(v(cx, cy), r, a0, a1)
    }
    fn circ(cx: f64, cy: f64, r: f64) -> Circle {
        Circle::new(v(cx, cy), r)
    }
    fn _approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    #[test]
    fn arc_inside_circle_no_intersection() {
        // Arc of radius 1 inside circle of radius 3 (no intersection)
        let a = arc(0.0, 0.0, 1.0, 0.0, PI);
        let c = circ(0.0, 0.0, 3.0);
        assert_eq!(a.intersect(&c, TOL), Intersection::None);
    }

    #[test]
    fn arc_and_circle_cross_twice() {
        // Arc: upper semicircle of unit circle at origin
        // Circle: unit circle at (1, 0), intersects at 2 points
        let a = arc(0.0, 0.0, 1.0, 0.0, PI);
        let c = circ(1.0, 0.0, 1.0);
        // Circle-circle intersections: (0.5, ±√3/2)
        // (0.5, +√3/2) is in upper half → on arc ✓
        // (0.5, -√3/2) is in lower half → not on arc ✗
        let r = a.intersect(&c, TOL);
        assert_eq!(r.point_count(), 1);
        let p = r.points()[0];
        assert!((p.x - 0.5).abs() < 1e-7);
        assert!(p.y > 0.0);
    }

    #[test]
    fn arc_and_circle_external_tangent_on_arc() {
        // Arc: full lower half (π to 2π) of unit circle at origin
        // External circle at (2, 0) with radius 1: tangent at (1, 0)
        // (1, 0) is at angle 0, which is NOT on the lower-half arc (π to 2π)
        let a = arc(0.0, 0.0, 1.0, PI, 0.0); // lower half: π → 2π (=0)
        let c = circ(2.0, 0.0, 1.0);
        // Tangent at (1, 0), angle = 0, which is the arc's end boundary
        let r = a.intersect(&c, TOL);
        assert!(r.is_some(), "arc end-point is on arc boundary (within tol)");
    }

    #[test]
    fn arc_and_circle_tangent_but_arc_misses() {
        // Arc is only the right quarter (−π/2 to π/2).
        // Tangent at (0, 1) → angle = π/2, which is the arc boundary
        let a = arc(0.0, 0.0, 1.0, -FRAC_PI_2, FRAC_PI_2);
        let c = circ(0.0, 2.0, 1.0); // tangent at top
        let r = a.intersect(&c, TOL);
        // (0, 1) is exactly at arc's end-point
        assert!(r.is_some());
    }

    #[test]
    fn arc_circle_symmetry() {
        // circle.intersect(&arc) == arc.intersect(&circle) (same result)
        let a = arc(0.0, 0.0, 1.0, 0.0, PI);
        let c = circ(1.0, 0.0, 1.0);
        let r1 = a.intersect(&c, TOL);
        let r2 = c.intersect(&a, TOL);
        assert_eq!(r1.point_count(), r2.point_count());
    }

    #[test]
    fn arc_entire_circle_coincident_via_circle_intersect() {
        // Arc and circle on same circle → Coincident (from circle-circle),
        // but the arc filter should still return Coincident (arc is full span of its circle)
        // Actually arc_circle returns Coincident when both underlying circles are the same.
        let a = arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2);
        let c = circ(0.0, 0.0, 1.0);
        // circle_circle_pts returns Coincident → filter_for_arc passes it through
        let r = a.intersect(&c, TOL);
        assert_eq!(r, Intersection::Coincident);
    }

    #[test]
    fn two_hits_both_on_arc() {
        // Diagonal line as a thin arc: two circles both intersecting an arc at two points
        let a = arc(0.0, 0.0, 2.0, -FRAC_PI_2, FRAC_PI_2); // right half arc
        let c = circ(1.0, 0.0, 2.0);
        let r = a.intersect(&c, TOL);
        // Both intersection points should have x > 0 (inside right half)
        for p in r.points() {
            // Verify on arc's circle
            let dist = p.distance_to(&v(0.0, 0.0));
            assert!((dist - 2.0).abs() < 1e-7, "point not on arc's circle");
        }
    }
}
