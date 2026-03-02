use cadkit_2d_core::Entity;
use cadkit_geometry::{Arc as GeomArc, Circle as GeomCircle, Line as GeomLine, Polyline as GeomPolyline};
use cadkit_types::{Guid, Vec2};

#[derive(Debug, Clone)]
pub enum ActiveTool {
    None,
    Line { start: Option<Vec2> },
    Circle { center: Option<Vec2> },
    Arc { start: Option<Vec2>, mid: Option<Vec2> },
    Polyline { points: Vec<Vec2> },
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum TrimPhase {
    Idle,
    SelectingEdges,
    Trimming,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OffsetPhase {
    Idle,
    EnteringDistance,
    SelectingEntity,
    SelectingSide,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MovePhase {
    Idle,
    SelectingEntities,
    BasePoint,
    Destination,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExtendPhase {
    Idle,
    SelectingBoundaries,
    Extending,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CopyPhase {
    Idle,
    SelectingEntities,
    BasePoint,
    Destination,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RotatePhase {
    Idle,
    SelectingEntities,
    BasePoint,
    Rotation,
}

/// FROM tracking: lets the user pick a base snap point then type an offset from it.
/// Triggered by typing "from" or "fr" while any point-pick is expected.
#[derive(Debug, Clone, PartialEq)]
pub enum FromPhase {
    Idle,
    WaitingBase,
    WaitingOffset,
}

/// DimLinear placement workflow phases.
#[derive(Debug, Clone, PartialEq)]
pub enum DimPhase {
    Idle,
    FirstPoint,
    SecondPoint { first: Vec2 },
    Placing { first: Vec2, second: Vec2 },
}

/// Result of a read-only trim computation; mutations are applied by the caller.
pub enum TrimResult {
    /// Operation failed; the string is the log message.
    Fail(String),
    /// Apply: remove `target_id`, add `new_entities`.
    Apply {
        target_id: Guid,
        new_entities: Vec<Entity>,
    },
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub entity: Guid,
    pub world: Vec2,
}

/// Geometry-crate primitive, used for intersection dispatch.
pub enum GeomPrim {
    Line(GeomLine),
    Circle(GeomCircle),
    Arc(GeomArc),
    Polyline(GeomPolyline),
}
