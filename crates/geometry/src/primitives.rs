//! Geometric primitive types for 2D intersection calculations.

use cadkit_types::Vec3;

// ---------------------------------------------------------------------------
// Line segment
// ---------------------------------------------------------------------------

/// A finite line segment defined by two endpoints in the XY plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub start: Vec3,
    pub end: Vec3,
}

impl Line {
    pub fn new(start: Vec3, end: Vec3) -> Self {
        Self { start, end }
    }

    /// Euclidean length of the segment.
    pub fn length(&self) -> f64 {
        self.start.distance_to(&self.end)
    }

    /// Midpoint of the segment.
    pub fn midpoint(&self) -> Vec3 {
        Vec3::xy(
            (self.start.x + self.end.x) * 0.5,
            (self.start.y + self.end.y) * 0.5,
        )
    }

    /// Direction vector (not normalised).
    pub fn direction(&self) -> (f64, f64) {
        (self.end.x - self.start.x, self.end.y - self.start.y)
    }

    /// Returns `true` if the segment has zero (within `tol`) length.
    pub fn is_degenerate(&self, tol: f64) -> bool {
        self.length() < tol
    }

    /// Point at parameter `t` along the segment (`t = 0` → start, `t = 1` → end).
    pub fn point_at(&self, t: f64) -> Vec3 {
        let (dx, dy) = self.direction();
        Vec3::xy(self.start.x + t * dx, self.start.y + t * dy)
    }
}

// ---------------------------------------------------------------------------
// Arc
// ---------------------------------------------------------------------------

/// A circular arc in the XY plane, spanning CCW from `start_angle` to `end_angle`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub center: Vec3,
    pub radius: f64,
    /// Start angle in radians (CCW from +X).
    pub start_angle: f64,
    /// End angle in radians (CCW from +X).
    pub end_angle: f64,
}

impl Arc {
    pub fn new(center: Vec3, radius: f64, start_angle: f64, end_angle: f64) -> Self {
        Self {
            center,
            radius,
            start_angle,
            end_angle,
        }
    }

    /// CCW angular span of the arc, in the range (0, 2π].
    /// Returns 0 if `start_angle ≈ end_angle` (degenerate).
    pub fn span(&self) -> f64 {
        crate::utils::ccw_span(self.start_angle, self.end_angle)
    }

    /// Point on the arc at the given angle.
    pub fn point_at_angle(&self, angle: f64) -> Vec3 {
        Vec3::xy(
            self.center.x + self.radius * angle.cos(),
            self.center.y + self.radius * angle.sin(),
        )
    }

    /// Start point (at `start_angle`).
    pub fn start_point(&self) -> Vec3 {
        self.point_at_angle(self.start_angle)
    }

    /// End point (at `end_angle`).
    pub fn end_point(&self) -> Vec3 {
        self.point_at_angle(self.end_angle)
    }

    /// Returns `true` if `angle` lies within the arc's CCW span (± `tol` radians).
    pub fn contains_angle(&self, angle: f64, tol: f64) -> bool {
        crate::utils::angle_in_arc(angle, self.start_angle, self.end_angle, tol)
    }

    /// Returns `true` if the arc is degenerate (radius or span too small).
    pub fn is_degenerate(&self, tol: f64) -> bool {
        self.radius < tol || self.span() < tol
    }

    /// The underlying full circle.
    pub fn as_circle(&self) -> Circle {
        Circle {
            center: self.center,
            radius: self.radius,
        }
    }
}

// ---------------------------------------------------------------------------
// Circle
// ---------------------------------------------------------------------------

/// A full circle in the XY plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    pub center: Vec3,
    pub radius: f64,
}

impl Circle {
    pub fn new(center: Vec3, radius: f64) -> Self {
        Self { center, radius }
    }

    pub fn is_degenerate(&self, tol: f64) -> bool {
        self.radius < tol
    }

    /// Angle from the circle's centre to a point.
    pub fn angle_to(&self, point: Vec3) -> f64 {
        (point.y - self.center.y).atan2(point.x - self.center.x)
    }
}

// ---------------------------------------------------------------------------
// Polyline
// ---------------------------------------------------------------------------

/// An open or closed polyline in the XY plane.
#[derive(Clone, Debug, PartialEq)]
pub struct Polyline {
    pub vertices: Vec<Vec3>,
    pub closed: bool,
}

impl Polyline {
    pub fn new(vertices: Vec<Vec3>, closed: bool) -> Self {
        Self { vertices, closed }
    }

    /// Returns all constituent [`Line`] segments.
    pub fn segments(&self) -> Vec<Line> {
        let n = self.vertices.len();
        if n < 2 {
            return vec![];
        }
        let mut segs = Vec::with_capacity(if self.closed { n } else { n - 1 });
        for i in 0..n - 1 {
            segs.push(Line::new(self.vertices[i], self.vertices[i + 1]));
        }
        if self.closed {
            segs.push(Line::new(self.vertices[n - 1], self.vertices[0]));
        }
        segs
    }

    /// Returns `true` if the polyline has fewer than 2 vertices.
    pub fn is_degenerate(&self) -> bool {
        self.vertices.len() < 2
    }
}
