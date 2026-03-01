//! Line segment – Arc intersection.

use cadkit_types::Vec3;

use crate::{
    primitives::{Arc, Line},
    utils::angle_in_arc,
};

use super::{Intersection, Intersects};

impl Intersects<Arc> for Line {
    /// Intersect a finite line segment with an arc.
    ///
    /// Algorithm: first intersect with the underlying full circle, then filter
    /// each candidate point by whether its angle from the arc's centre falls
    /// within the arc's CCW span.
    ///
    /// Returns:
    /// - `None` – no intersection within the segment and arc bounds.
    /// - `Tangent(p)` – line is tangent to the circle and the tangent point is
    ///   on the arc.
    /// - `Points([…])` – one or two crossing points on the arc.
    fn intersect(&self, arc: &Arc, tol: f64) -> Intersection {
        if arc.is_degenerate(tol) {
            return Intersection::None;
        }

        // Intersect with the full circle first.
        let circle_result = self.intersect(&arc.as_circle(), tol);

        // Convert angle tolerance: arclength tol at the circumference → radians.
        let angle_tol = tol / arc.radius.max(tol);

        match circle_result {
            Intersection::None => Intersection::None,
            Intersection::Coincident => Intersection::None, // can't happen for line-circle
            Intersection::Tangent(p) => {
                let angle = arc_angle(&arc.center, p);
                if angle_in_arc(angle, arc.start_angle, arc.end_angle, angle_tol) {
                    Intersection::Tangent(p)
                } else {
                    Intersection::None
                }
            }
            Intersection::Points(pts) => {
                let filtered: Vec<Vec3> = pts
                    .into_iter()
                    .filter(|&p| {
                        let angle = arc_angle(&arc.center, p);
                        angle_in_arc(angle, arc.start_angle, arc.end_angle, angle_tol)
                    })
                    .collect();

                if filtered.is_empty() {
                    Intersection::None
                } else {
                    Intersection::Points(filtered)
                }
            }
        }
    }
}

/// Angle from `centre` to `point` (atan2 result, normalised to [−π, π]).
fn arc_angle(centre: &Vec3, point: Vec3) -> f64 {
    (point.y - centre.y).atan2(point.x - centre.x)
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
    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Line {
        Line::new(v(x0, y0), v(x1, y1))
    }
    fn arc(cx: f64, cy: f64, r: f64, a0: f64, a1: f64) -> Arc {
        Arc::new(v(cx, cy), r, a0, a1)
    }
    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    #[test]
    fn line_through_semicircle_two_points() {
        // Unit circle semicircle from 0 to π (upper half), line along x-axis
        let l = line(-2.0, 0.0, 2.0, 0.0);
        let a = arc(0.0, 0.0, 1.0, 0.0, PI);
        // Endpoints of semicircle: (1,0) and (-1,0) are exactly on the boundary
        let r = l.intersect(&a, TOL);
        assert!(r.point_count() >= 1, "should hit arc endpoints");
    }

    #[test]
    fn line_misses_arc_hits_circle() {
        // Line intersects the circle but only in the lower half,
        // while the arc only covers the upper half
        let l = line(-2.0, -0.5, 2.0, -0.5);
        let a = arc(0.0, 0.0, 1.0, 0.0, PI); // upper arc
        assert_eq!(l.intersect(&a, TOL), Intersection::None);
    }

    #[test]
    fn line_hits_one_arc_endpoint() {
        // Vertical line at x=1 hits the arc at the start point (1, 0)
        let l = line(1.0, -1.0, 1.0, 1.0);
        let a = arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2);
        let r = l.intersect(&a, TOL);
        assert!(r.is_some());
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn tangent_line_on_arc() {
        // Horizontal tangent at the top of a quarter arc
        let l = line(-2.0, 1.0, 2.0, 1.0);
        let a = arc(0.0, 0.0, 1.0, 0.0, PI); // semicircle includes top
        let r = l.intersect(&a, TOL);
        assert!(
            matches!(r, Intersection::Tangent(_)),
            "expected Tangent, got {:?}",
            r
        );
        assert!(approx_eq(r.points()[0], v(0.0, 1.0)));
    }

    #[test]
    fn tangent_line_but_arc_doesnt_reach_tangent_point() {
        // Tangent at top of circle (0, 1), but arc is only from 0 to π/2
        let l = line(-2.0, 1.0, 2.0, 1.0);
        let a = arc(0.0, 0.0, 1.0, 0.0, FRAC_PI_2); // quarter arc, ends at (0,1)
        let r = l.intersect(&a, TOL);
        // (0, 1) is the arc end-point so it should be found (boundary tolerance)
        assert!(r.is_some());
    }

    #[test]
    fn diagonal_line_two_arc_hits() {
        // y = x line, unit circle at origin, upper-right quarter arc
        let s = 1.0 / SQRT_2;
        let l = line(-2.0, -2.0, 2.0, 2.0);
        let a = arc(0.0, 0.0, 1.0, 0.0, PI); // upper semicircle
        let r = l.intersect(&a, TOL);
        // Only (s, s) is in the upper half (the other point is in the lower half)
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(s, s)));
    }

    #[test]
    fn arc_wrapping_around_zero() {
        // Arc from 3π/2 to π/2 (CCW, wrapping through 0°)
        // Line along the +X half-axis should hit the wrap-around region
        let l = line(0.0, 0.0, 2.0, 0.0);
        let a = arc(0.0, 0.0, 1.0, 3.0 * FRAC_PI_2, FRAC_PI_2);
        // The arc includes angle 0 (right side of circle)
        let r = l.intersect(&a, TOL);
        assert!(r.is_some());
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn line_segment_too_short() {
        // Line starts and ends before the circle
        let l = line(-0.5, 0.0, 0.0, 0.0);
        let a = arc(2.0, 0.0, 1.0, 0.0, PI);
        assert_eq!(l.intersect(&a, TOL), Intersection::None);
    }
}
