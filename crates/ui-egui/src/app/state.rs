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

/// Result returned by `compute_extend`.
pub enum ExtendResult {
    /// Move a line endpoint to `new_pt`.
    Line { id: Guid, is_start: bool, new_pt: Vec2 },
    /// Rotate an arc endpoint to `new_angle` (radians, world CCW from +X).
    Arc  { id: Guid, is_start: bool, new_angle: f64 },
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

/// TEXT placement workflow phases.
#[derive(Debug, Clone, PartialEq)]
pub enum TextPhase {
    Idle,
    PlacingPosition,
    EnteringHeight   { position: Vec2 },
    EnteringRotation { position: Vec2, height: f64 },
    TypingContent    { position: Vec2, height: f64, rotation: f64 },
}

impl Default for TextPhase {
    fn default() -> Self { TextPhase::Idle }
}

/// EDITTEXT workflow: select a text entity then edit it via dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum EditTextPhase {
    Idle,
    SelectingEntity,
}

impl Default for EditTextPhase {
    fn default() -> Self { EditTextPhase::Idle }
}

/// State held while the Edit Text dialog is open.
#[derive(Debug, Clone)]
pub struct TextEditDialog {
    pub id:              Guid,
    pub content:         String,
    pub height_str:      String,
    pub rotation_str:    String,
    /// Set to true after the first frame so we only steal focus once.
    pub focus_requested: bool,
}

/// EDITDIM workflow: select a dimension entity to edit its text override.
#[derive(Debug, Clone, PartialEq)]
pub enum EditDimPhase {
    Idle,
    SelectingEntity,
}

impl Default for EditDimPhase {
    fn default() -> Self { EditDimPhase::Idle }
}

/// State held while the Edit Dim dialog is open.
#[derive(Debug, Clone)]
pub struct DimEditDialog {
    pub id:              Guid,
    /// Empty string = use the measured distance (auto).
    pub override_str:    String,
    pub focus_requested: bool,
}

/// DimAligned placement workflow phases.
#[derive(Debug, Clone, PartialEq)]
pub enum DimPhase {
    Idle,
    FirstPoint,
    SecondPoint { first: Vec2 },
    Placing { first: Vec2, second: Vec2 },
}

/// DimLinear (H/V locked) placement workflow phases.
#[derive(Debug, Clone, PartialEq)]
pub enum DimLinearPhase {
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

/// Identifies what kind of geometric snap point was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,       // end/start of line, arc, polyline vertex
    Midpoint,       // midpoint of a segment
    Center,         // circle or arc center
    Quadrant,       // circle cardinal points (N/S/E/W)
    Intersection,   // intersection of two entities
    Nearest,        // closest point on entity curve to cursor
    Perpendicular,  // foot of perpendicular from previous drawn point
    Tangent,        // tangent point on circle/arc from previous drawn point
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
