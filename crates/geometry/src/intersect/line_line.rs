//! Line segment – Line segment intersection.

use cadkit_types::Vec3;

use crate::{
    primitives::Line,
    utils::{cross2, dot2},
};

use super::{Intersection, Intersects};

impl Intersects<Line> for Line {
    /// Intersect two finite line segments.
    ///
    /// Returns:
    /// - `None` – no contact within the segments.
    /// - `Points([p])` – one crossing point (including shared endpoints).
    /// - `Coincident` – the segments are collinear and overlap in a 1-D span.
    fn intersect(&self, other: &Line, tol: f64) -> Intersection {
        let p1 = (self.start.x, self.start.y);
        let p2 = (self.end.x, self.end.y);
        let p3 = (other.start.x, other.start.y);
        let p4 = (other.end.x, other.end.y);

        let d1 = (p2.0 - p1.0, p2.1 - p1.1); // direction of self
        let d2 = (p4.0 - p3.0, p4.1 - p3.1); // direction of other

        let len1_sq = dot2(d1, d1);
        let len2_sq = dot2(d2, d2);

        // Degenerate segment checks
        if len1_sq < tol * tol || len2_sq < tol * tol {
            return Intersection::None;
        }

        let len1 = len1_sq.sqrt();
        let len2 = len2_sq.sqrt();

        let denom = cross2(d1, d2); // |d1|·|d2|·sin(θ)
        let sin_angle = denom.abs() / (len1 * len2);

        // --- Parallel (or coincident) ---
        if sin_angle < 1e-10 {
            // Perpendicular distance from p3 to the infinite line through L1.
            // cross(d1_unit, p3-p1) = cross(d1, p3-p1) / len1
            let w = (p3.0 - p1.0, p3.1 - p1.1);
            let perp_dist = cross2(d1, w).abs() / len1;

            if perp_dist > tol {
                return Intersection::None; // parallel, not collinear
            }

            // Collinear: project L2 endpoints onto L1's axis and find overlap.
            // t = dot(pt - p1, d1) / |d1|²
            let t3 = dot2((p3.0 - p1.0, p3.1 - p1.1), d1) / len1_sq;
            let t4 = dot2((p4.0 - p1.0, p4.1 - p1.1), d1) / len1_sq;

            let t_tol = tol / len1;
            let lo = t3.min(t4).max(-t_tol);
            let hi = t3.max(t4).min(1.0 + t_tol);

            if hi < lo - t_tol {
                return Intersection::None; // collinear but disjoint
            }

            let overlap = hi - lo;
            if overlap <= 2.0 * t_tol {
                // Segments touch at exactly one endpoint
                let t = ((lo + hi) * 0.5).clamp(0.0, 1.0);
                return Intersection::Points(vec![Vec3::xy(p1.0 + t * d1.0, p1.1 + t * d1.1)]);
            }

            return Intersection::Coincident;
        }

        // --- Non-parallel: compute parameters t (on self) and s (on other) ---
        // self:  P(t) = p1 + t·d1,   t ∈ [0,1]
        // other: Q(s) = p3 + s·d2,   s ∈ [0,1]
        // Solving: t = cross(w, d2) / denom,  s = cross(w, d1) / denom
        // where w = p3 - p1.
        let w = (p3.0 - p1.0, p3.1 - p1.1);
        let t = cross2(w, d2) / denom;
        let s = cross2(w, d1) / denom;

        let t_tol = tol / len1;
        let s_tol = tol / len2;

        if t >= -t_tol && t <= 1.0 + t_tol && s >= -s_tol && s <= 1.0 + s_tol {
            let tc = t.clamp(0.0, 1.0);
            Intersection::Points(vec![Vec3::xy(p1.0 + tc * d1.0, p1.1 + tc * d1.1)])
        } else {
            Intersection::None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::SQRT_2;

    const TOL: f64 = 1e-9;

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::xy(x, y)
    }
    fn line(x0: f64, y0: f64, x1: f64, y1: f64) -> Line {
        Line::new(v(x0, y0), v(x1, y1))
    }
    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    #[test]
    fn perpendicular_cross() {
        // Horizontal and vertical lines crossing at (0.5, 0.5)
        let l1 = line(0.0, 0.5, 1.0, 0.5);
        let l2 = line(0.5, 0.0, 0.5, 1.0);
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(0.5, 0.5)));
    }

    #[test]
    fn diagonal_cross() {
        // y = x and y = -x, crossing at (0,0)
        let l1 = line(-1.0, -1.0, 1.0, 1.0);
        let l2 = line(-1.0, 1.0, 1.0, -1.0);
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(0.0, 0.0)));
    }

    #[test]
    fn t_intersection_at_endpoint() {
        // L2 ends exactly on L1 (at its midpoint)
        let l1 = line(0.0, 0.0, 2.0, 0.0);
        let l2 = line(1.0, -1.0, 1.0, 0.0); // end touches l1
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn coincident_shared_endpoint() {
        // Two collinear segments touching at one endpoint
        let l1 = line(0.0, 0.0, 1.0, 0.0);
        let l2 = line(1.0, 0.0, 2.0, 0.0);
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1, "touching endpoint is a single point");
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn coincident_overlap() {
        // Overlapping collinear segments
        let l1 = line(0.0, 0.0, 2.0, 0.0);
        let l2 = line(1.0, 0.0, 3.0, 0.0);
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r, Intersection::Coincident);
    }

    #[test]
    fn coincident_one_inside_other() {
        let l1 = line(0.0, 0.0, 4.0, 0.0);
        let l2 = line(1.0, 0.0, 3.0, 0.0); // fully inside l1
        assert_eq!(l1.intersect(&l2, TOL), Intersection::Coincident);
    }

    #[test]
    fn parallel_no_intersection() {
        let l1 = line(0.0, 0.0, 1.0, 0.0);
        let l2 = line(0.0, 1.0, 1.0, 1.0);
        assert_eq!(l1.intersect(&l2, TOL), Intersection::None);
    }

    #[test]
    fn collinear_but_disjoint() {
        let l1 = line(0.0, 0.0, 1.0, 0.0);
        let l2 = line(2.0, 0.0, 3.0, 0.0);
        assert_eq!(l1.intersect(&l2, TOL), Intersection::None);
    }

    #[test]
    fn lines_dont_reach_each_other() {
        // Would intersect if extended, but segments don't overlap
        let l1 = line(0.0, 0.0, 0.4, 0.0);
        let l2 = line(0.5, -1.0, 0.5, -0.1); // stays below x-axis
        assert_eq!(l1.intersect(&l2, TOL), Intersection::None);
    }

    #[test]
    fn shared_start_endpoints() {
        let l1 = line(0.0, 0.0, 1.0, 0.0);
        let l2 = line(0.0, 0.0, 0.0, 1.0);
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(0.0, 0.0)));
    }

    #[test]
    fn diagonal_segments_no_overlap() {
        // Same angle lines separated diagonally
        let l1 = line(0.0, 0.0, 1.0, 1.0);
        let l2 = line(2.0, 0.0, 3.0, 1.0);
        assert_eq!(l1.intersect(&l2, TOL), Intersection::None);
    }

    #[test]
    fn intersection_at_45_degrees() {
        // l1: (0,0)→(2,2), l2: (0,2)→(2,0), cross at (1,1)
        let l1 = line(0.0, 0.0, 2.0, 2.0);
        let l2 = line(0.0, 2.0, 2.0, 0.0);
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(1.0, 1.0)));
    }

    #[test]
    fn degenerate_zero_length_segment() {
        let pt = line(1.0, 1.0, 1.0, 1.0); // zero length
        let l = line(0.0, 1.0, 2.0, 1.0);
        assert_eq!(pt.intersect(&l, TOL), Intersection::None);
        assert_eq!(l.intersect(&pt, TOL), Intersection::None);
    }

    #[test]
    fn perpendicular_length_check() {
        // sqrt(2) length diagonal, crossing at midpoint
        let l1 = line(0.0, 0.0, 1.0, 1.0); // length sqrt(2)
        let l2 = line(0.0, 1.0, 1.0, 0.0); // length sqrt(2)
        let r = l1.intersect(&l2, TOL);
        assert_eq!(r.point_count(), 1);
        let _ = SQRT_2; // used just to confirm import works
        assert!(approx_eq(r.points()[0], v(0.5, 0.5)));
    }
}
