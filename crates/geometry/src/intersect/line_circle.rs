//! Line segment – Circle intersection.

use cadkit_types::Vec3;

use crate::{
    primitives::{Circle, Line},
    utils::dot2,
};

use super::{Intersection, Intersects};

impl Intersects<Circle> for Line {
    /// Intersect a finite line segment with a circle.
    ///
    /// Uses the perpendicular-distance form to avoid numerical issues with the
    /// standard quadratic discriminant when the line is far from the circle.
    ///
    /// Returns:
    /// - `None` – no intersection within the segment bounds.
    /// - `Tangent(p)` – the segment is tangent to the circle.
    /// - `Points([p1, p2])` or `Points([p])` – one or two crossing points
    ///   (one point if only one end of the chord lies on the segment).
    fn intersect(&self, circle: &Circle, tol: f64) -> Intersection {
        let d = (
            self.end.x - self.start.x,
            self.end.y - self.start.y,
        );
        let len_sq = dot2(d, d);
        if len_sq < tol * tol {
            return Intersection::None; // degenerate segment
        }
        let len = len_sq.sqrt();

        if circle.is_degenerate(tol) {
            return Intersection::None;
        }

        let r = circle.radius;

        // Vector from segment start to circle centre
        let m = (
            circle.center.x - self.start.x,
            circle.center.y - self.start.y,
        );

        // Parameter of the perpendicular foot on the infinite line
        let t_foot = dot2(m, d) / len_sq;

        // Distance² from circle centre to the infinite line
        let m_len_sq = dot2(m, m);
        let dist_sq = (m_len_sq - t_foot * t_foot * len_sq).max(0.0);
        let dist = dist_sq.sqrt();

        // No intersection: centre is farther from the line than the radius
        if dist > r + tol {
            return Intersection::None;
        }

        let t_tol = tol / len;

        // Tangent case: |dist − r| < tol
        if (dist - r).abs() <= tol {
            if t_foot >= -t_tol && t_foot <= 1.0 + t_tol {
                let tc = t_foot.clamp(0.0, 1.0);
                return Intersection::Tangent(Vec3::xy(
                    self.start.x + tc * d.0,
                    self.start.y + tc * d.1,
                ));
            }
            return Intersection::None;
        }

        // Two chord endpoints on the infinite line
        let half_chord = (r * r - dist_sq).max(0.0).sqrt();
        let dt = half_chord / len;
        let t1 = t_foot - dt;
        let t2 = t_foot + dt;

        let mut pts = Vec::new();
        for &t in &[t1, t2] {
            if t >= -t_tol && t <= 1.0 + t_tol {
                let tc = t.clamp(0.0, 1.0);
                pts.push(Vec3::xy(
                    self.start.x + tc * d.0,
                    self.start.y + tc * d.1,
                ));
            }
        }

        match pts.len() {
            0 => Intersection::None,
            _ => Intersection::Points(pts),
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
    fn circle(cx: f64, cy: f64, r: f64) -> Circle {
        Circle::new(v(cx, cy), r)
    }
    fn approx_eq(a: Vec3, b: Vec3) -> bool {
        a.distance_to(&b) < 1e-7
    }

    #[test]
    fn chord_through_centre() {
        // Line passes through the centre → two symmetric intersection points
        let l = line(-2.0, 0.0, 2.0, 0.0);
        let c = circle(0.0, 0.0, 1.0);
        let r = l.intersect(&c, TOL);
        assert_eq!(r.point_count(), 2);
        let pts = r.points();
        assert!(approx_eq(pts[0], v(-1.0, 0.0)));
        assert!(approx_eq(pts[1], v(1.0, 0.0)));
    }

    #[test]
    fn tangent_line_touches_top() {
        // Horizontal line y = 1 is tangent to unit circle centred at origin
        let l = line(-2.0, 1.0, 2.0, 1.0);
        let c = circle(0.0, 0.0, 1.0);
        let r = l.intersect(&c, TOL);
        assert!(
            matches!(r, Intersection::Tangent(_)),
            "expected Tangent, got {:?}",
            r
        );
        assert!(approx_eq(r.points()[0], v(0.0, 1.0)));
    }

    #[test]
    fn line_misses_circle() {
        let l = line(0.0, 2.0, 1.0, 2.0);
        let c = circle(0.0, 0.0, 1.0);
        assert_eq!(l.intersect(&c, TOL), Intersection::None);
    }

    #[test]
    fn segment_too_short_to_reach_circle() {
        // Infinite line would intersect but segment stops before it
        let l = line(-0.5, 2.0, 0.5, 2.0); // y = 2, circle radius 1
        let c = circle(0.0, 0.0, 1.0);
        assert_eq!(l.intersect(&c, TOL), Intersection::None);
    }

    #[test]
    fn segment_starts_inside_circle() {
        // One endpoint inside, the other outside → one crossing
        let l = line(0.0, 0.0, 2.0, 0.0);
        let c = circle(0.0, 0.0, 1.0);
        let r = l.intersect(&c, TOL);
        assert_eq!(r.point_count(), 1);
        assert!(approx_eq(r.points()[0], v(1.0, 0.0)));
    }

    #[test]
    fn segment_entirely_inside_circle() {
        let l = line(-0.5, 0.0, 0.5, 0.0);
        let c = circle(0.0, 0.0, 2.0);
        assert_eq!(l.intersect(&c, TOL), Intersection::None);
    }

    #[test]
    fn diagonal_chord() {
        // Diagonal line through unit circle at 45°
        // y = x, circle at origin r=1 → intersection at (±1/√2, ±1/√2)
        let s = 1.0 / SQRT_2;
        let l = line(-2.0, -2.0, 2.0, 2.0);
        let c = circle(0.0, 0.0, 1.0);
        let r = l.intersect(&c, TOL);
        assert_eq!(r.point_count(), 2);
        let pts = r.points();
        // Points should be (±s, ±s)
        let p_neg = v(-s, -s);
        let p_pos = v(s, s);
        assert!(approx_eq(pts[0], p_neg) || approx_eq(pts[1], p_neg));
        assert!(approx_eq(pts[0], p_pos) || approx_eq(pts[1], p_pos));
    }

    #[test]
    fn off_centre_circle() {
        // Circle not at origin
        let l = line(0.0, 3.0, 4.0, 3.0);
        let c = circle(2.0, 3.0, 1.5);
        let r = l.intersect(&c, TOL);
        assert_eq!(r.point_count(), 2);
        let pts = r.points();
        assert!(approx_eq(pts[0], v(0.5, 3.0)));
        assert!(approx_eq(pts[1], v(3.5, 3.0)));
    }

    #[test]
    fn tangent_at_endpoint_of_segment() {
        // Line from (0,0) to (0,1): tangent to circle centred at (1,0) radius 1
        // The tangent point is at (0, 0) which is the segment start
        let l = line(0.0, 0.0, 0.0, 2.0);
        let c = circle(1.0, 0.0, 1.0);
        let r = l.intersect(&c, TOL);
        // (0,0) is on the circle AND on the segment → tangent
        assert!(r.is_some());
    }
}
