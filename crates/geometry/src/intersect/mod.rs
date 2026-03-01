//! Geometric intersection algorithms.
//!
//! The [`Intersects`] trait provides the unified API:
//! ```rust,ignore
//! let result: Intersection = a.intersect(&b, tol);
//! ```
//! where `tol` is a distance tolerance in drawing units (e.g. `1e-9` for mm).

use cadkit_types::Vec3;

mod arc_arc;
mod arc_circle;
mod circle_circle;
mod line_arc;
mod line_circle;
mod line_line;
mod polyline;


// ---------------------------------------------------------------------------
// Intersection result
// ---------------------------------------------------------------------------

/// The result of a 2D intersection test.
#[derive(Clone, Debug, PartialEq)]
pub enum Intersection {
    /// The two entities do not intersect.
    None,
    /// The entities are tangent at exactly one point.
    Tangent(Vec3),
    /// One or two proper crossing intersection points.
    Points(Vec<Vec3>),
    /// The entities are geometrically coincident (overlap in 1-D or fully).
    Coincident,
}

impl Intersection {
    /// Returns `true` for any non-`None` variant.
    pub fn is_some(&self) -> bool {
        !matches!(self, Intersection::None)
    }

    /// Number of discrete intersection points (0 for None/Coincident).
    pub fn point_count(&self) -> usize {
        match self {
            Intersection::Points(pts) => pts.len(),
            Intersection::Tangent(_) => 1,
            _ => 0,
        }
    }

    /// Collect all discrete points (empty for None/Coincident).
    pub fn points(&self) -> Vec<Vec3> {
        match self {
            Intersection::Points(pts) => pts.clone(),
            Intersection::Tangent(p) => vec![*p],
            _ => vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Compute the intersection of `self` with `other`.
///
/// `tol` is a distance tolerance in drawing units.  A value around `1e-9`
/// works for millimetre-precision geometry.
pub trait Intersects<Rhs = Self> {
    fn intersect(&self, other: &Rhs, tol: f64) -> Intersection;
}
