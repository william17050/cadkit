//! cadkit-geometry: 2D geometric intersection calculations for CadKit.
//!
//! Provides intersection algorithms for [`Line`], [`Arc`], [`Circle`], and
//! [`Polyline`] primitives. All coordinates use [`cadkit_types::Vec3`] with
//! z = 0. Angles are in radians, CCW positive from the +X axis.
//!
//! # Quick start
//! ```rust
//! use cadkit_geometry::{Line, Circle, Intersects};
//! use cadkit_types::Vec3;
//!
//! let line = Line::new(Vec3::xy(-2.0, 0.0), Vec3::xy(2.0, 0.0));
//! let circle = Circle::new(Vec3::xy(0.0, 0.0), 1.0);
//! let result = line.intersect(&circle, 1e-9);
//! assert_eq!(result.point_count(), 2);
//! ```

pub mod intersect;
pub mod primitives;
pub mod region;
pub(crate) mod utils;

pub use intersect::{Intersection, Intersects};
pub use primitives::{Arc, Circle, Line, Polyline};
pub use region::{
    detect_closed_boundaries, detect_closed_boundaries_from_polylines,
    detect_closed_boundaries_with_gap,
};
