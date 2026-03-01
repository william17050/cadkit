//! Core data types for CadKit
//!
//! This crate defines the fundamental types used throughout the application:
//! - Geometric types (Vec2, Vec3)
//! - Identifiers (Guid)
//! - Tolerances and units
//! - Common errors

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// =============================================================================
// Geometric Types
// =============================================================================

/// 2D vector for planar coordinates
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
    pub const X_AXIS: Self = Self { x: 1.0, y: 0.0 };
    pub const Y_AXIS: Self = Self { x: 0.0, y: 1.0 };

    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    pub fn distance_to(&self, other: &Vec2) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn normalize(&self) -> Option<Self> {
        let len = self.length();
        if len > f64::EPSILON {
            Some(Self {
                x: self.x / len,
                y: self.y / len,
            })
        } else {
            None
        }
    }
}

/// 3D vector for spatial coordinates
/// Z-axis is UP, XY plane is ground
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, z: 0.0 };
    pub const X_AXIS: Self = Self { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y_AXIS: Self = Self { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z_AXIS: Self = Self { x: 0.0, y: 0.0, z: 1.0 };

    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Create 3D point on XY plane (z=0)
    pub fn xy(x: f64, y: f64) -> Self {
        Self { x, y, z: 0.0 }
    }

    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    pub fn distance_to(&self, other: &Vec3) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn normalize(&self) -> Option<Self> {
        let len = self.length();
        if len > f64::EPSILON {
            Some(Self {
                x: self.x / len,
                y: self.y / len,
                z: self.z / len,
            })
        } else {
            None
        }
    }
}

/// Convert 2D to 3D on XY plane
impl From<Vec2> for Vec3 {
    fn from(v: Vec2) -> Self {
        Self::xy(v.x, v.y)
    }
}

/// Project 3D to 2D by dropping Z
impl From<Vec3> for Vec2 {
    fn from(v: Vec3) -> Self {
        Self::new(v.x, v.y)
    }
}

// =============================================================================
// Identifiers
// =============================================================================

/// Globally unique identifier for entities
/// Stable across save/load operations
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Guid(Uuid);

impl Guid {
    /// Create a new random GUID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Create a nil (all zeros) GUID
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Check if this is a nil GUID
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for Guid {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Guid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// =============================================================================
// Tolerances and Units
// =============================================================================

/// Geometric tolerance values
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Tolerance {
    /// Distance tolerance (mm or inches)
    pub distance: f64,
    /// Angle tolerance (radians)
    pub angle: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            distance: 0.001, // 1 micron or 0.001mm
            angle: 0.001,    // ~0.057 degrees
        }
    }
}

/// Unit system for drawings
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Millimeters,
    Inches,
    Feet,
}

impl Unit {
    /// Convert value from this unit to millimeters
    pub fn to_mm(&self, value: f64) -> f64 {
        match self {
            Unit::Millimeters => value,
            Unit::Inches => value * 25.4,
            Unit::Feet => value * 304.8,
        }
    }

    /// Convert value from millimeters to this unit
    pub fn from_mm(&self, value_mm: f64) -> f64 {
        match self {
            Unit::Millimeters => value_mm,
            Unit::Inches => value_mm / 25.4,
            Unit::Feet => value_mm / 304.8,
        }
    }
}

/// Drawing unit configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawingUnits {
    pub base: Unit,
    pub display_precision: u8, // decimal places
}

impl Default for DrawingUnits {
    fn default() -> Self {
        Self {
            base: Unit::Millimeters,
            display_precision: 3,
        }
    }
}

// =============================================================================
// Errors
// =============================================================================

#[derive(thiserror::Error, Debug)]
pub enum CadError {
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Entity not found: {0}")]
    NotFound(Guid),

    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),

    #[error("File I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CadError>;

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vec2_basics() {
        let v = Vec2::new(3.0, 4.0);
        assert_eq!(v.length(), 5.0);
        
        let n = v.normalize().unwrap();
        assert!((n.length() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_vec3_basics() {
        let v = Vec3::new(1.0, 2.0, 2.0);
        assert_eq!(v.length(), 3.0);
        
        let xy = Vec3::xy(10.0, 20.0);
        assert_eq!(xy.z, 0.0);
    }

    #[test]
    fn test_vec2_to_vec3() {
        let v2 = Vec2::new(5.0, 10.0);
        let v3: Vec3 = v2.into();
        assert_eq!(v3.x, 5.0);
        assert_eq!(v3.y, 10.0);
        assert_eq!(v3.z, 0.0);
    }

    #[test]
    fn test_guid_uniqueness() {
        let g1 = Guid::new();
        let g2 = Guid::new();
        assert_ne!(g1, g2);
        
        let nil = Guid::nil();
        assert!(nil.is_nil());
    }

    #[test]
    fn test_unit_conversion() {
        let mm = Unit::Millimeters;
        let inches = Unit::Inches;
        
        assert_eq!(inches.to_mm(1.0), 25.4);
        assert_eq!(mm.from_mm(25.4), 25.4);
        assert!((inches.from_mm(25.4) - 1.0).abs() < 1e-10);
    }
}
