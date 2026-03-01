//! Arc – Arc intersection.

use cadkit_types::Vec3;

use crate::{
    primitives::Arc,
    utils::{ccw_span, normalize_angle},
};

use super::{
    arc_circle::filter_for_arc, circle_circle::circle_circle_pts, Intersection, Intersects,
};

impl Intersects<Arc> for Arc {
    /// Intersect two arcs.
    ///
    /// Three cases are handled in order:
    /// 1. **Coincident circles** – both arcs lie on the same circle.
    ///    Intersection is their angular overlap (or a shared endpoint, or nothing).
    /// 2. **General case** – compute the two underlying circles' intersection
    ///    and filter each candidate point to lie on *both* arcs.
    fn intersect(&self, other: &Arc, tol: f64) -> Intersection {
        if self.is_degenerate(tol) || other.is_degenerate(tol) {
            return Intersection::None;
        }

        // Check whether both arcs lie on the same circle.
        let centres_close = self.center.distance_to(&other.center) < tol;
        let radii_close = (self.radius - other.radius).abs() < tol;

        if centres_close && radii_close {
            return same_circle_intersection(self, other, tol);
        }

        // General case: intersect the underlying circles, then apply both arc filters.
        let raw = circle_circle_pts(&self.as_circle(), &other.as_circle(), tol);

        // Filter by self's arc span
        let after_self = filter_for_arc(raw, self, tol);

        // Filter by other's arc span
        filter_for_arc(after_self, other, tol)
    }
}

/// Intersection of two arcs known to lie on the same circle.
fn same_circle_intersection(a1: &Arc, a2: &Arc, tol: f64) -> Intersection {
    let r = a1.radius;
    let angle_tol = if r > tol { tol / r } else { 1e-9 };

    // Check interior overlap using arc midpoints.
    // If the midpoint of a1 lies inside a2, the arcs share more than endpoints.
    let a1_mid_angle = a1.start_angle + ccw_span(a1.start_angle, a1.end_angle) / 2.0;
    let a2_mid_angle = a2.start_angle + ccw_span(a2.start_angle, a2.end_angle) / 2.0;

    if a2.contains_angle(a1_mid_angle, angle_tol) || a1.contains_angle(a2_mid_angle, angle_tol) {
        return Intersection::Coincident;
    }

    // No interior overlap – collect shared endpoint pairs.
    let eps1 = [
        (a1.start_angle, a1.start_point()),
        (a1.end_angle, a1.end_point()),
    ];
    let eps2 = [
        (a2.start_angle, a2.start_point()),
        (a2.end_angle, a2.end_point()),
    ];

    let mut shared: Vec<Vec3> = Vec::new();
    for &(angle1, pt1) in &eps1 {
        for &(angle2, _pt2) in &eps2 {
            // Two endpoint angles match when their angular difference ≈ 0 mod 2π.
            if normalize_angle(angle1 - angle2).min(normalize_angle(angle2 - angle1)) < angle_tol {
                // Only add once even if tolerances overlap
                if !shared.iter().any(|p: &Vec3| p.distance_to(&pt1) < tol) {
                    shared.push(pt1);
                }
            }
        }
    }

    if shared.is_empty() {
        Intersection::None
    } else {
        Intersection::Points(shared)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI, SQRT_2};

    const TOL: f64 = 1e-9;

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::xy(x, y)
    }
    fn arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Arc {
        Arc::new(v(cx, cy), r, a0, a1)
    }
    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    // ----- Same-circle tests ------------------------------------------------

    #[test]
    fn same_circle_no_overlap() {
        // Arc1: 0 to π/2, Arc2: π to 3π/2 — no overlap
        let a1 = arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2);
        let a2 = arc(0.0, 0.0, 1.0, PI, 3.0 * FRAC_PI_2);
        assert_eq!(a1.intersect(&a2, TOL), Intersection::None);
    }

    #[test]
    fn same_circle_shared_endpoint() {
        // Arc1: 0 to π/2,  Arc2: π/2 to π — share (0, 1) at π/2
        let a1 = arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2);
        let a2 = arc(0.0, 0.0, 1.0, FRAC_PI_2, PI);
        let r = a1.intersect(&a2, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(0.0, 1.0)));
    }

    #[test]
    fn same_circle_overlap_coincident() {
        // Arc1: 0 to π,  Arc2: π/2 to 3π/2 — overlap from π/2 to π
        let a1 = arc(0.0, 0.0, 1.0, 0.0, PI);
        let a2 = arc(0.0, 0.0, 1.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        assert_eq!(a1.intersect(&a2, TOL), Intersection::Coincident);
    }

    #[test]
    fn same_circle_a2_inside_a1() {
        // Arc1 is a full semicircle; Arc2 is a quarter inside it
        let a1 = arc(0.0, 0.0, 1.0, 0.0, PI);
        let a2 = arc(0.0, 0.0, 1.0, FRAC_PI_2 * 0.5, FRAC_PI_2);
        assert_eq!(a1.intersect(&a2, TOL), Intersection::Coincident);
    }

    #[test]
    fn same_circle_wrapping_overlap() {
        // Arc1 wraps through 0: 3π/2 to π/2
        // Arc2: 0 to π/4 — fully inside Arc1's wrap region
        let a1 = arc(0.0, 0.0, 1.0, 3.0 * FRAC_PI_2, FRAC_PI_2);
        let a2 = arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2 / 2.0);
        assert_eq!(a1.intersect(&a2, TOL), Intersection::Coincident);
    }

    // ----- Different-circle tests -------------------------------------------

    #[test]
    fn two_unit_circles_both_arcs_upper() {
        // Circles at (−0.5,0) and (0.5,0) each with r=1; upper arcs only
        let a1 = arc(-0.5, 0.0, 1.0, 0.0, PI); // upper semicircle left
        let a2 = arc(0.5, 0.0, 1.0, 0.0, PI); // upper semicircle right
        let r = a1.intersect(&a2, TOL);
        assert_eq!(r.point_count(), 1, "only upper crossing point on both arcs");
        assert!(r.points()[0].y > 0.0);
    }

    #[test]
    fn arcs_opposite_hemispheres_no_hit() {
        // Same circles but arcs on opposite halves
        let a1 = arc(-0.5, 0.0, 1.0, 0.0, PI); // upper
        let a2 = arc(0.5, 0.0, 1.0, PI, 0.0); // lower (π → 2π)
        // One circle-circle point is in upper half, one in lower.
        // Upper point is on a1 but is it on a2? a2 is the LOWER arc.
        let r = a1.intersect(&a2, TOL);
        assert_eq!(r.point_count(), 0);
    }

    #[test]
    fn external_tangent_both_arcs_reach_touch_point() {
        // Two unit circles: centres at (0,0) and (2,0), touching at (1,0)
        // Arc1: right quarter of left circle (−π/2 to π/2) → includes (1,0)
        // Arc2: left quarter of right circle (π/2 to 3π/2) → includes (1,0)
        let a1 = arc(0.0, 0.0, 1.0, -FRAC_PI_2, FRAC_PI_2);
        let a2 = arc(2.0, 0.0, 1.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        let r = a1.intersect(&a2, TOL);
        assert!(
            matches!(r, Intersection::Tangent(_)),
            "expected Tangent, got {:?}",
            r
        );
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn external_tangent_arc_misses() {
        // Same circles as above, but Arc1 doesn't reach (1,0)
        // Arc1: left semicircle of unit circle at origin (π/2 to 3π/2)
        let a1 = arc(0.0, 0.0, 1.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        let a2 = arc(2.0, 0.0, 1.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        assert_eq!(a1.intersect(&a2, TOL), Intersection::None);
    }

    #[test]
    fn arc_arc_two_crossing_points() {
        // Two circles of radius √2 centred at (0,0) and (2,0).
        // Crossing at (1, ±1).  Both arcs cover all angles.
        let sq2 = SQRT_2;
        // ccw_span(-PI, PI) = normalize_angle(2π) = 0 → degenerate full circle.
        // Use slightly-less-than-full-circle arcs instead.
        let a1 = arc(0.0, 0.0, sq2, -PI + 0.001, PI - 0.001);
        let a2 = arc(2.0, 0.0, sq2, -PI + 0.001, PI - 0.001);
        let r = a1.intersect(&a2, TOL);
        // Both points (1, 1) and (1, -1) should be on both arcs
        assert_eq!(r.point_count(), 2);
        for p in r.points() {
            assert!((p.x - 1.0).abs() < 1e-7);
            assert!((p.y.abs() - 1.0).abs() < 1e-7);
        }
    }

    #[test]
    fn arc_arc_far_apart() {
        let a1 = arc(0.0, 0.0, 1.0, 0.0, PI);
        let a2 = arc(10.0, 0.0, 1.0, 0.0, PI);
        assert_eq!(a1.intersect(&a2, TOL), Intersection::None);
    }
}
