//! Polyline intersection with Line, Circle, Arc, and another Polyline.
//!
//! A polyline is decomposed into its constituent line segments; each segment
//! is intersected individually, and all results are merged.

use cadkit_types::Vec3;

use crate::primitives::{Arc, Circle, Line, Polyline};

use super::{Intersection, Intersects};

// ---------------------------------------------------------------------------
// Helper: merge per-segment results into a single Intersection
// ---------------------------------------------------------------------------

/// Collect intersection points/tangents from many `Intersection` results,
/// deduplicating points that are closer than `tol`.
fn merge(results: Vec<Intersection>, tol: f64) -> Intersection {
    let mut any_coincident = false;
    let mut pts: Vec<Vec3> = Vec::new();

    for r in results {
        match r {
            Intersection::None => {}
            Intersection::Coincident => {
                any_coincident = true;
            }
            Intersection::Tangent(p) => {
                if !pts.iter().any(|e: &Vec3| e.distance_to(&p) < tol) {
                    pts.push(p);
                }
            }
            Intersection::Points(new_pts) => {
                for p in new_pts {
                    if !pts.iter().any(|e: &Vec3| e.distance_to(&p) < tol) {
                        pts.push(p);
                    }
                }
            }
        }
    }

    match (any_coincident, pts.len()) {
        (true, 0) => Intersection::Coincident,
        (_, 0) => Intersection::None,
        _ => Intersection::Points(pts),
    }
}

// ---------------------------------------------------------------------------
// Polyline ∩ Line
// ---------------------------------------------------------------------------

impl Intersects<Line> for Polyline {
    fn intersect(&self, line: &Line, tol: f64) -> Intersection {
        if self.is_degenerate() {
            return Intersection::None;
        }
        let results: Vec<_> = self
            .segments()
            .iter()
            .map(|seg| seg.intersect(line, tol))
            .collect();
        merge(results, tol)
    }
}

impl Intersects<Polyline> for Line {
    fn intersect(&self, poly: &Polyline, tol: f64) -> Intersection {
        poly.intersect(self, tol)
    }
}

// ---------------------------------------------------------------------------
// Polyline ∩ Circle
// ---------------------------------------------------------------------------

impl Intersects<Circle> for Polyline {
    fn intersect(&self, circle: &Circle, tol: f64) -> Intersection {
        if self.is_degenerate() {
            return Intersection::None;
        }
        let results: Vec<_> = self
            .segments()
            .iter()
            .map(|seg| seg.intersect(circle, tol))
            .collect();
        merge(results, tol)
    }
}

impl Intersects<Polyline> for Circle {
    fn intersect(&self, poly: &Polyline, tol: f64) -> Intersection {
        poly.intersect(self, tol)
    }
}

// ---------------------------------------------------------------------------
// Polyline ∩ Arc
// ---------------------------------------------------------------------------

impl Intersects<Arc> for Polyline {
    fn intersect(&self, arc: &Arc, tol: f64) -> Intersection {
        if self.is_degenerate() {
            return Intersection::None;
        }
        let results: Vec<_> = self
            .segments()
            .iter()
            .map(|seg| seg.intersect(arc, tol))
            .collect();
        merge(results, tol)
    }
}

impl Intersects<Polyline> for Arc {
    fn intersect(&self, poly: &Polyline, tol: f64) -> Intersection {
        poly.intersect(self, tol)
    }
}

// ---------------------------------------------------------------------------
// Polyline ∩ Polyline
// ---------------------------------------------------------------------------

impl Intersects<Polyline> for Polyline {
    fn intersect(&self, other: &Polyline, tol: f64) -> Intersection {
        if self.is_degenerate() || other.is_degenerate() {
            return Intersection::None;
        }
        let results: Vec<_> = self
            .segments()
            .iter()
            .flat_map(|seg_self| {
                other
                    .segments()
                    .iter()
                    .map(|seg_other| seg_self.intersect(seg_other, tol))
                    .collect::<Vec<_>>()
            })
            .collect();
        merge(results, tol)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const TOL: f64 = 1e-9;

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::xy(x, y)
    }
    fn poly(pts: &[(f64, f64)], closed: bool) -> Polyline {
        Polyline::new(pts.iter().map(|&(x, y)| v(x, y)).collect(), closed)
    }
    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    // ----- Polyline ∩ Line --------------------------------------------------

    #[test]
    fn triangle_line_one_crossing() {
        // Triangle: (0,0),(2,0),(1,2)
        let p = poly(&[(0.0, 0.0), (2.0, 0.0), (1.0, 2.0)], true);
        // Vertical line at x=1 cuts the triangle
        let l = Line::new(v(1.0, -1.0), v(1.0, 3.0));
        let r = p.intersect(&l, TOL);
        assert!(r.point_count() >= 1);
    }

    #[test]
    fn open_polyline_line_two_crossings() {
        // Open Z-shape: (0,1)→(2,1)→(0,-1)→(2,-1)
        let p = poly(&[(0.0, 1.0), (2.0, 1.0), (0.0, -1.0), (2.0, -1.0)], false);
        // Vertical line at x=1 crosses all three segments
        let l = Line::new(v(1.0, -2.0), v(1.0, 2.0));
        let r = p.intersect(&l, TOL);
        assert!(r.point_count() >= 2);
    }

    #[test]
    fn polyline_line_no_intersection() {
        let p = poly(&[(0.0, 2.0), (1.0, 2.0), (2.0, 2.0)], false);
        let l = Line::new(v(0.0, 0.0), v(2.0, 0.0));
        assert_eq!(p.intersect(&l, TOL), Intersection::None);
    }

    #[test]
    fn symmetric_line_poly() {
        // line.intersect(poly) same as poly.intersect(line) point count
        let p = poly(&[(0.0, 0.0), (2.0, 0.0), (1.0, 2.0)], true);
        let l = Line::new(v(1.0, -1.0), v(1.0, 3.0));
        assert_eq!(p.intersect(&l, TOL).point_count(), l.intersect(&p, TOL).point_count());
    }

    // ----- Polyline ∩ Circle ------------------------------------------------

    #[test]
    fn square_circle_eight_crossings() {
        // Unit square (0,0)→(1,0)→(1,1)→(0,1), closed.
        // Circle centred at (0.5, 0.5) radius 0.6 is entirely inside the square
        // and produces a 2-point chord on each of the four sides → 8 total.
        // Per side: bottom y=0: (x−0.5)²+(0−0.5)²=0.36 → x≈0.168 and x≈0.832
        let p = poly(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], true);
        let c = Circle::new(v(0.5, 0.5), 0.6);
        let r = p.intersect(&c, TOL);
        assert_eq!(r.point_count(), 8, "circle inside square: 2 crossings per side = 8 total");
    }

    #[test]
    fn polyline_circle_no_intersection() {
        let p = poly(&[(5.0, 0.0), (6.0, 0.0)], false);
        let c = Circle::new(v(0.0, 0.0), 1.0);
        assert_eq!(p.intersect(&c, TOL), Intersection::None);
    }

    // ----- Polyline ∩ Arc ---------------------------------------------------

    #[test]
    fn polyline_arc_hit() {
        // Open poly along x-axis, arc in upper semicircle of unit circle
        let p = poly(&[(-2.0, 0.0), (2.0, 0.0)], false);
        let a = Arc::new(v(0.0, 0.0), 1.0, 0.0, PI);
        // Line y=0 hits the arc's boundary angles (0 and π)
        let r = p.intersect(&a, TOL);
        assert!(r.is_some());
    }

    // ----- Polyline ∩ Polyline ----------------------------------------------

    #[test]
    fn two_polylines_crossing() {
        // '+' shape
        let h = poly(&[(-1.0, 0.0), (1.0, 0.0)], false); // horizontal
        let v_line = poly(&[(0.0, -1.0), (0.0, 1.0)], false); // vertical
        let r = h.intersect(&v_line, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(0.0, 0.0)));
    }

    #[test]
    fn two_polylines_no_intersection() {
        let p1 = poly(&[(0.0, 0.0), (1.0, 0.0)], false);
        let p2 = poly(&[(0.0, 1.0), (1.0, 1.0)], false);
        assert_eq!(p1.intersect(&p2, TOL), Intersection::None);
    }

    #[test]
    fn two_polylines_coincident_segment() {
        let p1 = poly(&[(0.0, 0.0), (2.0, 0.0)], false);
        let p2 = poly(&[(1.0, 0.0), (3.0, 0.0)], false);
        assert_eq!(p1.intersect(&p2, TOL), Intersection::Coincident);
    }

    #[test]
    fn degenerate_polyline_returns_none() {
        let p1 = Polyline::new(vec![v(0.0, 0.0)], false); // 1 vertex = degenerate
        let p2 = poly(&[(0.0, 0.0), (1.0, 0.0)], false);
        assert_eq!(p1.intersect(&p2, TOL), Intersection::None);
        assert_eq!(p2.intersect(&p1, TOL), Intersection::None);
    }

    #[test]
    fn closed_triangle_deduplicates_shared_vertex() {
        // Two closed triangles sharing one vertex (1,0)
        let t1 = poly(&[(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)], true);
        let t2 = poly(&[(1.0, 0.0), (2.0, 0.0), (1.5, 1.0)], true);
        let r = t1.intersect(&t2, TOL);
        // They share only (1,0); deduplicated to one point
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }
}
