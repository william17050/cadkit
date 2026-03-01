//! Internal geometric helper functions.

use std::f64::consts::TAU; // 2π

// ---------------------------------------------------------------------------
// Angle utilities
// ---------------------------------------------------------------------------

/// Normalise an angle to [0, 2π).
pub fn normalize_angle(a: f64) -> f64 {
    ((a % TAU) + TAU) % TAU
}

/// CCW angular span from `start` to `end` in [0, 2π).
/// Returns 0.0 when start ≈ end (callers treat this as a degenerate arc).
pub fn ccw_span(start: f64, end: f64) -> f64 {
    normalize_angle(end - start)
}

/// Returns `true` if `angle` lies within the CCW arc from `start` to `end`
/// (inclusive, using `tol` radians of tolerance at both boundary angles).
pub fn angle_in_arc(angle: f64, start: f64, end: f64, tol: f64) -> bool {
    let span = ccw_span(start, end);
    if span < tol {
        return false; // degenerate arc
    }
    let delta = normalize_angle(angle - start);
    delta <= span + tol
}

// ---------------------------------------------------------------------------
// 2D vector helpers  (tuples to avoid pulling in nalgebra / Vec3 for internals)
// ---------------------------------------------------------------------------

/// Dot product of two 2D vectors.
#[inline]
pub fn dot2(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

/// 2D cross product: a.x*b.y − a.y*b.x
#[inline]
pub fn cross2(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// Rotate a 2D vector 90° CCW: (x, y) → (−y, x).
#[inline]
pub fn rot90(v: (f64, f64)) -> (f64, f64) {
    (-v.1, v.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI};

    #[test]
    fn normalize_wraps_correctly() {
        assert!((normalize_angle(0.0)).abs() < 1e-12);
        assert!((normalize_angle(TAU) - 0.0).abs() < 1e-12);
        assert!((normalize_angle(-FRAC_PI_2) - (TAU - FRAC_PI_2)).abs() < 1e-12);
        assert!((normalize_angle(3.0 * PI) - PI).abs() < 1e-12);
    }

    #[test]
    fn ccw_span_basic() {
        assert!((ccw_span(0.0, FRAC_PI_2) - FRAC_PI_2).abs() < 1e-12);
        // Arc from 270° back to 90° (wrapping)
        assert!(
            (ccw_span(3.0 * FRAC_PI_2, FRAC_PI_2) - PI).abs() < 1e-12,
            "wrapping arc span should be π"
        );
    }

    #[test]
    fn angle_in_arc_non_wrapping() {
        let tol = 1e-9;
        // Arc from 0 to π
        assert!(angle_in_arc(FRAC_PI_2, 0.0, PI, tol));
        assert!(angle_in_arc(0.0, 0.0, PI, tol)); // start boundary
        assert!(angle_in_arc(PI, 0.0, PI, tol)); // end boundary
        assert!(!angle_in_arc(PI + 0.1, 0.0, PI, tol));
        assert!(!angle_in_arc(-0.1, 0.0, PI, tol));
    }

    #[test]
    fn angle_in_arc_wrapping() {
        let tol = 1e-9;
        // Arc from 270° (3π/2) to 90° (π/2), wrapping through 0°
        let start = 3.0 * FRAC_PI_2;
        let end = FRAC_PI_2;
        assert!(angle_in_arc(0.0, start, end, tol)); // at 0°, inside wrapping arc
        assert!(angle_in_arc(start, start, end, tol)); // start boundary
        assert!(angle_in_arc(end, start, end, tol)); // end boundary
        assert!(!angle_in_arc(PI, start, end, tol)); // at 180°, outside
    }

    #[test]
    fn rot90_is_ccw() {
        let v = (1.0_f64, 0.0_f64);
        let r = rot90(v);
        assert!((r.0 - 0.0).abs() < 1e-12);
        assert!((r.1 - (-1.0_f64).abs()).abs() < 1e-12 || (r.1 + 1.0).abs() < 1e-12); // (0, -1) wait
        // rot90((1,0)) = (0, 1) — CCW rotation: (x,y) -> (-y, x)
        // (-0, 1) = (0, 1) ✓  wait: rot90((1,0)) = (-0, 1) = (0, 1) ✓
        assert!((r.0).abs() < 1e-12);
        assert!((r.1 - 1.0).abs() < 1e-12); // actually wait: rot90(v) = (-v.1, v.0) = (-0, 1) = (0,1)
    }
}
