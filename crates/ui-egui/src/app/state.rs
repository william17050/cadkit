use cadkit_2d_core::Entity;
use cadkit_geometry::{
    Arc as GeomArc, Circle as GeomCircle, Line as GeomLine, Polyline as GeomPolyline,
};
use cadkit_types::{Guid, Vec2};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum ActiveTool {
    None,
    Line {
        start: Option<Vec2>,
    },
    Circle {
        center: Option<Vec2>,
    },
    Arc {
        start: Option<Vec2>,
        mid: Option<Vec2>,
    },
    Polyline {
        points: Vec<Vec2>,
    },
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
pub enum StretchPhase {
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
    Line {
        id: Guid,
        is_start: bool,
        new_pt: Vec2,
    },
    /// Rotate an arc endpoint to `new_angle` (radians, world CCW from +X).
    Arc {
        id: Guid,
        is_start: bool,
        new_angle: f64,
    },
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

#[derive(Debug, Clone, PartialEq)]
pub enum ScalePhase {
    Idle,
    SelectingEntities,
    BasePoint,
    ReferencePoint,
    Factor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirrorPhase {
    Idle,
    SelectingEntities,
    FirstAxisPoint,
    SecondAxisPoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilletPick {
    pub entity: Guid,
    pub click: Vec2,
    pub seg_start: Vec2,
    pub seg_end: Vec2,
    /// For polyline picks, indices of segment endpoints in the polyline vertex list.
    /// `None` means the picked entity is a Line.
    pub seg_start_index: Option<usize>,
    pub seg_end_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilletPhase {
    Idle,
    EnteringRadius,
    FirstEntity,
    SecondEntity { first: FilletPick },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChamferPhase {
    Idle,
    EnteringDistance,
    FirstEntity,
    SecondEntity { first: FilletPick },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PolygonPhase {
    Idle,
    EnteringSides,
    Center,
    Radius { center: Vec2 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EllipsePhase {
    Idle,
    Center,
    RadiusX { center: Vec2 },
    RadiusY { center: Vec2, rx: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RectanglePhase {
    Idle,
    FirstCorner,
    SecondCorner {
        first: Vec2,
    },
    EnteringDimensions {
        first: Vec2,
    },
    Direction {
        first: Vec2,
        width: f64,
        height: f64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayMode {
    Rectangular,
    Polar,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArrayPhase {
    Idle,
    SelectingEntities,
    ChoosingType,
    RectEnteringCount,
    RectEnteringSpacing,
    RectBasePoint,
    RectGripIdle,
    RectXSpacingGrip,
    RectXCountGrip,
    RectYSpacingGrip,
    RectYCountGrip,
    PolarEnteringCount,
    PolarEnteringAngle,
    PolarCenter,
    PolarBasePoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PeditPhase {
    Idle,
    SelectingPolyline,
    Joining { base: Guid },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BoundaryPhase {
    Idle,
    PickingPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HatchPhase {
    Idle,
    PickingPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IsoExtrudePhase {
    Idle,
    SelectingEntities,
    EnteringDepth,
    PickingElevationOrigin,
    PickingIsoOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DwIsoSidePhase {
    Idle,
    SelectingFrontEntities,
    PickingFrontOrigin,
    SelectingSideEntities,
    PickingSideOrigin,
    PickingIsoOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IsocirclePhase {
    Idle,
    Center,
    Radius { center: Vec2 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockPhase {
    Idle,
    PickBase { ids: Vec<Guid> },
    EnterName { ids: Vec<Guid>, base: Vec2 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertPhase {
    Idle,
    PickPoint { name: String },
}

/// Which isometric plane is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IsoPlane {
    #[default]
    Right,
    Left,
    Top,
}

impl IsoPlane {
    pub fn cycle(self) -> Self {
        match self {
            IsoPlane::Right => IsoPlane::Left,
            IsoPlane::Left => IsoPlane::Top,
            IsoPlane::Top => IsoPlane::Right,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            IsoPlane::Right => "Right",
            IsoPlane::Left => "Left",
            IsoPlane::Top => "Top",
        }
    }
    /// The two axis unit-vectors (world space, y-up) for this iso plane.
    pub fn axes(self) -> [Vec2; 2] {
        let a30  = Vec2::new( 0.8660254037844386,  0.5);
        let a150 = Vec2::new(-0.8660254037844386,  0.5);
        let a90  = Vec2::new( 0.0,                 1.0);
        match self {
            IsoPlane::Right => [a30,  a90],
            IsoPlane::Left  => [a150, a90],
            IsoPlane::Top   => [a30,  a150],
        }
    }
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
    EnteringHeight {
        position: Vec2,
    },
    EnteringRotation {
        position: Vec2,
        height: f64,
    },
    TypingContent {
        position: Vec2,
        height: f64,
        rotation: f64,
    },
}

impl Default for TextPhase {
    fn default() -> Self {
        TextPhase::Idle
    }
}

/// EDITTEXT workflow: select a text entity then edit it via dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum EditTextPhase {
    Idle,
    SelectingEntity,
}

impl Default for EditTextPhase {
    fn default() -> Self {
        EditTextPhase::Idle
    }
}

/// State held while the Edit Text dialog is open.
#[derive(Debug, Clone)]
pub struct TextEditDialog {
    pub id: Guid,
    pub content: String,
    pub height_str: String,
    pub rotation_str: String,
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
    fn default() -> Self {
        EditDimPhase::Idle
    }
}

/// State held while the Edit Dim dialog is open.
#[derive(Debug, Clone)]
pub struct DimEditDialog {
    pub id: Guid,
    /// Empty string = use the measured distance (auto).
    pub override_str: String,
    pub focus_requested: bool,
}

/// Global dimension style values used for new dimensions and text rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DimStyle {
    pub text_height: f64,
    pub precision: usize,
    pub color: [u8; 3],
    pub arrow_length: f64,
    pub arrow_half_width: f64,
}

impl Default for DimStyle {
    fn default() -> Self {
        Self {
            text_height: 2.5,
            precision: 3,
            color: [0, 180, 220],
            arrow_length: 3.0,
            arrow_half_width: 0.75,
        }
    }
}

/// State held while the DimStyle dialog is open.
#[derive(Debug, Clone)]
pub struct DimStyleDialog {
    pub text_height_str: String,
    pub precision_str: String,
    pub color: [u8; 3],
    pub arrow_length_str: String,
    pub arrow_half_width_str: String,
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

/// DimRadial (radius/diameter) placement workflow phases.
/// Workflow: click a circle or arc → drag leader out → click to place.
#[derive(Debug, Clone, PartialEq)]
pub enum DimRadialPhase {
    Idle,
    /// Waiting for the user to click a circle or arc entity.
    SelectingEntity {
        is_diameter: bool,
    },
    /// Entity picked; drag the leader endpoint, click to place.
    Placing {
        center: Vec2,
        radius: f64,
        is_diameter: bool,
    },
}

impl Default for DimRadialPhase {
    fn default() -> Self {
        DimRadialPhase::Idle
    }
}

/// DimAngular placement workflow phases.
/// Workflow: click first line/segment → click second line/segment →
///           auto-compute vertex from line intersection → drag arc radius → click to place.
#[derive(Debug, Clone, PartialEq)]
pub enum DimAngularPhase {
    Idle,
    /// Waiting for the user to click the first line or polyline segment.
    FirstEntity,
    /// First segment selected; waiting for the second.
    SecondEntity {
        first_click: Vec2, // world pos where user clicked (ray direction indicator)
        first_start: Vec2, // segment start in world space
        first_end: Vec2,   // segment end in world space
    },
    /// Both segments selected; vertex computed. Drag to set arc radius, click to place.
    Placing {
        vertex: Vec2,
        line1_pt: Vec2,
        line2_pt: Vec2,
    },
}

/// Result of a read-only trim computation; mutations are applied by the caller.
pub enum TrimResult {
    /// Operation failed; the string is the log message.
    Fail(String),
    /// Apply: remove `remove_ids`, add `new_entities`.
    Apply {
        remove_ids: Vec<Guid>,
        new_entities: Vec<Entity>,
    },
}

/// Identifies what kind of geometric snap point was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapKind {
    Endpoint,      // end/start of line, arc, polyline vertex
    Midpoint,      // midpoint of a segment
    Center,        // circle or arc center
    Quadrant,      // circle cardinal points (N/S/E/W)
    Intersection,  // intersection of two entities
    Parallel,      // tracked point from previous point, parallel to nearby segment
    Nearest,       // closest point on entity curve to cursor
    Perpendicular, // foot of perpendicular from previous drawn point
    Tangent,       // tangent point on circle/arc from previous drawn point
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub entity: Guid,
    pub world: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DimGripKind {
    Start,
    End,
    Offset,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimGripHandle {
    pub entity: Guid,
    pub kind: DimGripKind,
}

/// Geometry-crate primitive, used for intersection dispatch.
pub enum GeomPrim {
    Line(GeomLine),
    Circle(GeomCircle),
    Arc(GeomArc),
    Polyline(GeomPolyline),
}
