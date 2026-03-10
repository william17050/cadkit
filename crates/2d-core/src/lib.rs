//! 2D CAD core - entities, layers, and drawing management
//!
//! This crate provides:
//! - 2D geometric entities (Line, Arc, Circle, Polyline)
//! - Layer management
//! - Drawing document structure
//! - Entity storage and queries

pub mod dxf_io;
pub use dxf_io::{aci_to_rgb, rgb_to_aci, DxfImportResult};

use cadkit_types::{Guid, Result, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AutoCAD-style default layer colour palette (index 0–7).
pub const LAYER_COLORS: &[[u8; 3]] = &[
    [255, 255, 255], // 0 white
    [255, 0, 0],     // 1 red
    [255, 255, 0],   // 2 yellow
    [0, 255, 0],     // 3 green
    [0, 255, 255],   // 4 cyan
    [0, 0, 255],     // 5 blue
    [255, 0, 255],   // 6 magenta
    [128, 128, 128], // 7 gray
];

fn default_layer_color() -> [u8; 3] {
    LAYER_COLORS[0]
}
fn default_layer_frozen() -> bool {
    false
}

fn default_dim_arrow_length() -> f64 {
    3.0
}
fn default_dim_arrow_half_width() -> f64 {
    0.75
}
fn default_text_font_name() -> String {
    "STANDARD".to_string()
}
fn default_linetype() -> Linetype {
    Linetype::Continuous
}
fn default_entity_linetype_by_layer() -> bool {
    false
}
fn default_layer_linetype() -> Linetype {
    Linetype::Continuous
}
fn default_linetype_scale() -> f64 {
    1.0
}
fn default_blocks() -> HashMap<String, BlockDefinition> {
    HashMap::new()
}
fn default_block_params() -> BlockParamValues {
    BlockParamValues::default()
}
fn default_insert_dynamic_param_overrides() -> HashMap<Guid, f64> {
    HashMap::new()
}
fn default_axis_mask_true() -> bool {
    true
}

// =============================================================================
// Entities
// =============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Linetype {
    Continuous,
    Hidden,
    Center,
}

impl Linetype {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Continuous => "Continuous",
            Self::Hidden => "Hidden",
            Self::Center => "Center",
        }
    }

    pub fn to_dxf_name(self) -> &'static str {
        match self {
            Self::Continuous => "CONTINUOUS",
            Self::Hidden => "HIDDEN",
            Self::Center => "CENTER",
        }
    }

    pub fn from_dxf_name(name: &str) -> Self {
        let n = name.trim();
        if n.eq_ignore_ascii_case("HIDDEN") {
            Self::Hidden
        } else if n.eq_ignore_ascii_case("CENTER") {
            Self::Center
        } else {
            Self::Continuous
        }
    }
}

/// Core 2D entity types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EntityKind {
    /// Straight line segment
    Line {
        start: Vec3, // z=0 for 2D phase
        end: Vec3,
    },

    /// Circular arc
    Arc {
        center: Vec3,
        radius: f64,
        start_angle: f64, // radians, 0=+X axis, CCW positive
        end_angle: f64,
    },

    /// Full circle
    Circle { center: Vec3, radius: f64 },

    /// Connected line/arc segments
    Polyline { vertices: Vec<Vec3>, closed: bool },

    /// Aligned dimension between two points (line parallel to measured entity)
    DimAligned {
        start: Vec3,                   // first extension line origin
        end: Vec3,                     // second extension line origin
        offset: f64,                   // signed perpendicular distance to dimension line
        text_override: Option<String>, // None = auto-format measured distance
        text_pos: Vec3,                // world-space centre of dimension text
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Horizontal or vertical (linear) dimension
    DimLinear {
        start: Vec3, // first extension line origin
        end: Vec3,   // second extension line origin
        offset: f64, // signed displacement from mid-Y (horiz) or mid-X (vert) to dim line
        text_override: Option<String>,
        text_pos: Vec3,
        horizontal: bool, // true = measures X distance; false = measures Y distance
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Angular dimension between two rays from a common vertex.
    /// The arc spans CCW from angle(line1_pt) to angle(line2_pt) relative to the vertex.
    DimAngular {
        vertex: Vec3,   // angle apex
        line1_pt: Vec3, // point on first ray from vertex
        line2_pt: Vec3, // point on second ray from vertex
        radius: f64,    // dimension arc radius (set during Placing)
        text_override: Option<String>,
        text_pos: Vec3, // world-space centre of dimension text
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Radius or diameter dimension on a circle or arc.
    /// `is_diameter = false` → "R…" label with one arrowhead;
    /// `is_diameter = true`  → "Ø…" label with chord line + two arrowheads.
    DimRadial {
        center: Vec3,    // circle/arc centre
        radius: f64,     // actual radius
        leader_pt: Vec3, // user's click point (leader endpoint + text anchor)
        is_diameter: bool,
        text_override: Option<String>,
        text_pos: Vec3,
        #[serde(default = "default_dim_arrow_length")]
        arrow_length: f64,
        #[serde(default = "default_dim_arrow_half_width")]
        arrow_half_width: f64,
    },

    /// Free-standing text label
    Text {
        position: Vec3, // insertion point (baseline-left), z=0
        content: String,
        height: f64,   // glyph height in world units
        rotation: f64, // CCW angle in radians from +X axis
        #[serde(default = "default_text_font_name")]
        font_name: String,
    },

    /// Block reference instance (true INSERT-style entity).
    Insert {
        name: String,
        position: Vec3,
        #[serde(default)]
        rotation: f64,
        #[serde(default = "default_linetype_scale")]
        scale_x: f64,
        #[serde(default = "default_linetype_scale")]
        scale_y: f64,
    },
}

impl EntityKind {
    /// Check if entity lies entirely on XY plane (z=0)
    pub fn is_planar(&self) -> bool {
        match self {
            EntityKind::Line { start, end } => {
                start.z.abs() < f64::EPSILON && end.z.abs() < f64::EPSILON
            }
            EntityKind::Arc { center, .. } | EntityKind::Circle { center, .. } => {
                center.z.abs() < f64::EPSILON
            }
            EntityKind::Polyline { vertices, .. } => {
                vertices.iter().all(|v| v.z.abs() < f64::EPSILON)
            }
            EntityKind::DimAligned {
                start,
                end,
                text_pos,
                ..
            }
            | EntityKind::DimLinear {
                start,
                end,
                text_pos,
                ..
            } => {
                start.z.abs() < f64::EPSILON
                    && end.z.abs() < f64::EPSILON
                    && text_pos.z.abs() < f64::EPSILON
            }
            EntityKind::DimAngular {
                vertex, text_pos, ..
            } => vertex.z.abs() < f64::EPSILON && text_pos.z.abs() < f64::EPSILON,
            EntityKind::DimRadial {
                center, text_pos, ..
            } => center.z.abs() < f64::EPSILON && text_pos.z.abs() < f64::EPSILON,
            EntityKind::Text { position, .. } => position.z.abs() < f64::EPSILON,
            EntityKind::Insert { position, .. } => position.z.abs() < f64::EPSILON,
        }
    }
}

/// Complete entity with ID and metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: Guid,
    pub kind: EntityKind,
    pub layer: u32,
    /// Per-entity colour override. `None` means "ByLayer" (inherit from the layer).
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    /// Simple built-in linetype. Defaults to `Continuous` for compatibility.
    #[serde(default = "default_linetype")]
    pub linetype: Linetype,
    /// When true, linetype is inherited from the layer.
    #[serde(default = "default_entity_linetype_by_layer")]
    pub linetype_by_layer: bool,
    /// Per-entity linetype scale override. `None` means "ByLayer".
    #[serde(default)]
    pub linetype_scale: Option<f64>,
    /// First-pass dynamic block parameter overrides for INSERT entities.
    #[serde(default = "default_block_params")]
    pub block_params: BlockParamValues,
    /// V1 dynamic parameter overrides for INSERT entities, keyed by parameter id.
    /// Empty means "use block parameter defaults".
    #[serde(default = "default_insert_dynamic_param_overrides")]
    pub insert_dynamic_param_overrides: HashMap<Guid, f64>,
}

impl Entity {
    pub fn new(kind: EntityKind, layer: u32) -> Self {
        Self {
            id: Guid::new(),
            kind,
            layer,
            color: None,
            linetype: Linetype::Continuous,
            linetype_by_layer: false,
            linetype_scale: None,
            block_params: BlockParamValues::default(),
            insert_dynamic_param_overrides: HashMap::new(),
        }
    }
}

/// Stored entity payload inside a block definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockEntity {
    pub kind: EntityKind,
    pub layer: u32,
    #[serde(default)]
    pub color: Option<[u8; 3]>,
    #[serde(default = "default_linetype")]
    pub linetype: Linetype,
    #[serde(default = "default_entity_linetype_by_layer")]
    pub linetype_by_layer: bool,
    #[serde(default)]
    pub linetype_scale: Option<f64>,
}

/// Simple first-pass dynamic block metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDynamic {
    #[serde(default)]
    pub enable_width: bool,
    #[serde(default)]
    pub enable_height: bool,
    #[serde(default = "default_linetype_scale")]
    pub base_width: f64,
    #[serde(default = "default_linetype_scale")]
    pub base_height: f64,
}

/// Per-insert dynamic parameter values.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct BlockParamValues {
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
}

/// Authored block-local entity payload used as the regeneration source.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockAuthoredEntity {
    pub local_entity_id: Guid,
    pub kind: EntityKind,
    pub layer: u32,
}

/// Axis for a dynamic parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParameterAxis {
    X,
    Y,
}

/// User-defined parameter metadata exposed by the block editor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub id: Guid,
    pub name: String,
    pub axis: ParameterAxis,
    pub default_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub step: f64,
}

/// Editor category for an action binding (runtime behavior is per-target).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionKind {
    Move,
    Stretch,
    Anchor,
    Visibility,
}

/// Runtime behavior for one bound action target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityBehavior {
    MoveRigid,
    KeepCentered,
    AnchorToCenter,
    AnchorToEdge,
    StretchFromLeft,
    StretchFromRight,
    StretchFromCenter,
    Ignore,
}

/// Block-local frame used to resolve offsets and placement rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReferenceFrame {
    BlockOrigin,
    BoundsCenter,
    LeftEdge,
    RightEdge,
    TopEdge,
    BottomEdge,
}

/// How a target should be positioned relative to a reference frame.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlacementRule {
    KeepDefault,
    Offset(f64),
    Proportional(f64),
}

/// Axis participation mask for target updates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AxisMask {
    #[serde(default = "default_axis_mask_true")]
    pub x: bool,
    #[serde(default = "default_axis_mask_true")]
    pub y: bool,
}

impl Default for AxisMask {
    fn default() -> Self {
        Self { x: true, y: true }
    }
}

/// Selection target for an action: full entity, rigid group, or sub-entity handle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TargetRef {
    Entity(Guid),
    Group(Guid),
    SubEntity { entity_id: Guid, handle: u32 },
}

/// One target entry bound to an action with target-specific behavior and references.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionTarget {
    pub target: TargetRef,
    pub behavior: EntityBehavior,
    pub reference_frame: ReferenceFrame,
    pub placement_rule: PlacementRule,
    #[serde(default)]
    pub axis_mask: AxisMask,
    #[serde(default = "default_linetype_scale")]
    pub weight: f64,
}

/// Parameter-driven action binding that owns an editor category and target set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionBinding {
    pub id: Guid,
    pub parameter_id: Guid,
    pub action_kind: ActionKind,
    #[serde(default)]
    pub targets: Vec<ActionTarget>,
    #[serde(default)]
    pub order: i32,
}

/// Authoring-time rigid group definition for move-style targets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidGroupDefinition {
    pub id: Guid,
    pub name: String,
    #[serde(default)]
    pub members: Vec<Guid>,
}

/// Bounds of the authored block geometry in local coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockBounds {
    pub min: Vec2,
    pub max: Vec2,
}

/// Full dynamic metadata for a block definition (authoring model).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DynamicBlockDefinition {
    pub block_name: String,
    #[serde(default)]
    pub base_entities: Vec<BlockAuthoredEntity>,
    pub base_bounds: BlockBounds,
    #[serde(default)]
    pub parameters: Vec<ParameterDefinition>,
    #[serde(default)]
    pub actions: Vec<ActionBinding>,
    #[serde(default)]
    pub groups: Vec<RigidGroupDefinition>,
}

/// Per-instance parameter value storage for an INSERT.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct BlockInstanceDynamicState {
    pub insert_entity_id: Guid,
    #[serde(default)]
    pub param_values: HashMap<Guid, f64>,
}

/// Named reusable block definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDefinition {
    pub name: String,
    pub base_point: Vec3,
    pub entities: Vec<BlockEntity>,
    #[serde(default)]
    pub dynamic: Option<BlockDynamic>,
    /// V1 dynamic authoring/action model (Phase 1 data-only).
    #[serde(default)]
    pub dynamic_v1: Option<DynamicBlockDefinition>,
}

// =============================================================================
// Layers
// =============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Layer {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    #[serde(default = "default_layer_frozen")]
    pub frozen: bool,
    /// RGB colour used when rendering entities on this layer.
    /// Defaults to white so that old `.cadkit` files without this field load cleanly.
    #[serde(default = "default_layer_color")]
    pub color: [u8; 3],
    /// Layer linetype for entities set to "ByLayer".
    #[serde(default = "default_layer_linetype")]
    pub linetype: Linetype,
    /// Layer linetype scale for entities set to "ByLayer" scale.
    #[serde(default = "default_linetype_scale")]
    pub linetype_scale: f64,
}

impl Layer {
    pub fn new(id: u32, name: String, color: [u8; 3]) -> Self {
        Self {
            id,
            name,
            visible: true,
            locked: false,
            frozen: false,
            color,
            linetype: Linetype::Continuous,
            linetype_scale: 1.0,
        }
    }
}

// =============================================================================
// Drawing Document
// =============================================================================

/// Main drawing document containing all entities and metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drawing {
    pub id: Guid,
    pub name: String,
    #[serde(default = "default_linetype_scale")]
    pub linetype_scale: f64,
    entities: HashMap<Guid, Entity>,
    #[serde(default = "default_blocks")]
    blocks: HashMap<String, BlockDefinition>,
    layers: HashMap<u32, Layer>,
    next_layer_id: u32,
    // TODO: Add units, limits, view settings
}

impl Drawing {
    pub fn new(name: String) -> Self {
        let mut layers = HashMap::new();
        let default_layer = Layer::new(0, "0".to_string(), LAYER_COLORS[0]);
        layers.insert(0, default_layer);

        Self {
            id: Guid::new(),
            name,
            linetype_scale: 1.0,
            entities: HashMap::new(),
            blocks: HashMap::new(),
            layers,
            next_layer_id: 1,
        }
    }

    // -------------------------------------------------------------------------
    // Entity Management
    // -------------------------------------------------------------------------

    pub fn add_entity(&mut self, entity: Entity) -> Guid {
        let id = entity.id;
        self.entities.insert(id, entity);
        id
    }

    pub fn remove_entity(&mut self, id: &Guid) -> Option<Entity> {
        self.entities.remove(id)
    }

    pub fn get_entity(&self, id: &Guid) -> Option<&Entity> {
        self.entities.get(id)
    }

    pub fn get_entity_mut(&mut self, id: &Guid) -> Option<&mut Entity> {
        self.entities.get_mut(id)
    }

    pub fn entities(&self) -> impl Iterator<Item = &Entity> {
        self.entities.values()
    }

    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    pub fn define_block(
        &mut self,
        name: String,
        base_point: Vec3,
        entities: Vec<BlockEntity>,
        dynamic: Option<BlockDynamic>,
    ) -> bool {
        if name.trim().is_empty() || entities.is_empty() {
            return false;
        }
        let key = name.trim().to_ascii_lowercase();
        self.blocks.insert(
            key,
            BlockDefinition {
                name: name.trim().to_string(),
                base_point,
                entities,
                dynamic,
                dynamic_v1: None,
            },
        );
        true
    }

    pub fn get_block(&self, name: &str) -> Option<&BlockDefinition> {
        self.blocks.get(&name.trim().to_ascii_lowercase())
    }

    pub fn block_names(&self) -> Vec<String> {
        let mut out: Vec<String> = self.blocks.values().map(|b| b.name.clone()).collect();
        out.sort();
        out
    }

    /// Fetch V1 dynamic block metadata for a block name.
    pub fn get_block_dynamic_v1(&self, name: &str) -> Option<&DynamicBlockDefinition> {
        self.get_block(name).and_then(|b| b.dynamic_v1.as_ref())
    }

    /// Set or clear V1 dynamic block metadata for a block name.
    pub fn set_block_dynamic_v1(
        &mut self,
        name: &str,
        dynamic_v1: Option<DynamicBlockDefinition>,
    ) -> bool {
        let key = name.trim().to_ascii_lowercase();
        let Some(block) = self.blocks.get_mut(&key) else {
            return false;
        };
        block.dynamic_v1 = dynamic_v1;
        true
    }

    /// Return effective dynamic parameter values for an INSERT entity id.
    /// Defaults come from block `dynamic_v1.parameters`; entity overrides replace defaults.
    pub fn get_insert_effective_dynamic_params(
        &self,
        insert_id: &Guid,
    ) -> Option<HashMap<Guid, f64>> {
        let entity = self.get_entity(insert_id)?;
        let EntityKind::Insert { name, .. } = &entity.kind else {
            return None;
        };

        let mut values: HashMap<Guid, f64> = HashMap::new();
        if let Some(block_dyn) = self.get_block_dynamic_v1(name) {
            for p in &block_dyn.parameters {
                values.insert(p.id, p.default_value);
            }
        }
        for (pid, val) in &entity.insert_dynamic_param_overrides {
            values.insert(*pid, *val);
        }
        Some(values)
    }

    /// Return raw override map for an INSERT entity id.
    pub fn get_insert_dynamic_param_overrides(
        &self,
        insert_id: &Guid,
    ) -> Option<&HashMap<Guid, f64>> {
        let entity = self.get_entity(insert_id)?;
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return None;
        }
        Some(&entity.insert_dynamic_param_overrides)
    }

    /// Set/update one dynamic parameter override for an INSERT entity.
    pub fn set_insert_dynamic_param_override(
        &mut self,
        insert_id: &Guid,
        parameter_id: Guid,
        value: f64,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity
            .insert_dynamic_param_overrides
            .insert(parameter_id, value);
        true
    }

    /// Remove one dynamic parameter override for an INSERT entity.
    pub fn remove_insert_dynamic_param_override(
        &mut self,
        insert_id: &Guid,
        parameter_id: &Guid,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity
            .insert_dynamic_param_overrides
            .remove(parameter_id)
            .is_some()
    }

    /// Clear all dynamic parameter overrides for an INSERT entity.
    pub fn clear_insert_dynamic_param_overrides(&mut self, insert_id: &Guid) -> bool {
        let Some(entity) = self.get_entity_mut(insert_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity.insert_dynamic_param_overrides.clear();
        true
    }

    /// Snapshot helper for serialization/interchange of per-instance dynamic values.
    pub fn get_block_instance_dynamic_state(
        &self,
        insert_id: &Guid,
    ) -> Option<BlockInstanceDynamicState> {
        let entity = self.get_entity(insert_id)?;
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return None;
        }
        Some(BlockInstanceDynamicState {
            insert_entity_id: *insert_id,
            param_values: entity.insert_dynamic_param_overrides.clone(),
        })
    }

    /// Apply a serialized dynamic state payload to an INSERT entity.
    pub fn apply_block_instance_dynamic_state(
        &mut self,
        state: &BlockInstanceDynamicState,
    ) -> bool {
        let Some(entity) = self.get_entity_mut(&state.insert_entity_id) else {
            return false;
        };
        if !matches!(entity.kind, EntityKind::Insert { .. }) {
            return false;
        }
        entity.insert_dynamic_param_overrides = state.param_values.clone();
        true
    }

    /// Evaluate INSERT-local entities, regenerating from `dynamic_v1` authored geometry
    /// when available. Falls back to stored block entities for non-dynamic blocks.
    pub fn evaluate_insert_local_entities(&self, insert: &Entity) -> Option<Vec<BlockEntity>> {
        let EntityKind::Insert { name, .. } = &insert.kind else {
            return None;
        };
        let block = self.get_block(name)?;
        let Some(dynv1) = &block.dynamic_v1 else {
            return Some(block.entities.clone());
        };
        if dynv1.base_entities.is_empty() {
            return Some(block.entities.clone());
        }

        // Always regenerate from authored base entities to avoid cumulative distortion.
        let mut working: Vec<BlockEntity> = dynv1
            .base_entities
            .iter()
            .map(|be| BlockEntity {
                kind: be.kind.clone(),
                layer: be.layer,
                color: None,
                linetype: Linetype::Continuous,
                linetype_by_layer: true,
                linetype_scale: None,
            })
            .collect();

        let authored_bounds_by_id: HashMap<Guid, (f64, f64, f64, f64)> = dynv1
            .base_entities
            .iter()
            .filter_map(|e| Self::kind_bounds_local(&e.kind).map(|b| (e.local_entity_id, b)))
            .collect();
        let index_by_local_id: HashMap<Guid, usize> = dynv1
            .base_entities
            .iter()
            .enumerate()
            .map(|(i, e)| (e.local_entity_id, i))
            .collect();

        let mut effective = self
            .get_insert_effective_dynamic_params(&insert.id)
            .unwrap_or_default();
        for p in &dynv1.parameters {
            effective.entry(p.id).or_insert(p.default_value);
        }

        let base_min_x = dynv1.base_bounds.min.x;
        let base_min_y = dynv1.base_bounds.min.y;
        let base_w = (dynv1.base_bounds.max.x - dynv1.base_bounds.min.x).max(1e-9);
        let base_h = (dynv1.base_bounds.max.y - dynv1.base_bounds.min.y).max(1e-9);
        let mut cur_w = base_w;
        let mut cur_h = base_h;
        for p in &dynv1.parameters {
            let val = *effective.get(&p.id).unwrap_or(&p.default_value);
            match p.axis {
                ParameterAxis::X => cur_w = val.max(1e-9),
                ParameterAxis::Y => cur_h = val.max(1e-9),
            }
        }

        let resolve_frame_value = |frame: ReferenceFrame, axis: ParameterAxis| -> f64 {
            match (frame, axis) {
                (ReferenceFrame::BlockOrigin, ParameterAxis::X) => 0.0,
                (ReferenceFrame::BlockOrigin, ParameterAxis::Y) => 0.0,
                (ReferenceFrame::BoundsCenter, ParameterAxis::X) => base_min_x + cur_w * 0.5,
                (ReferenceFrame::BoundsCenter, ParameterAxis::Y) => base_min_y + cur_h * 0.5,
                (ReferenceFrame::LeftEdge, ParameterAxis::X) => base_min_x,
                (ReferenceFrame::RightEdge, ParameterAxis::X) => base_min_x + cur_w,
                (ReferenceFrame::BottomEdge, ParameterAxis::Y) => base_min_y,
                (ReferenceFrame::TopEdge, ParameterAxis::Y) => base_min_y + cur_h,
                // If the frame does not map cleanly to this axis, fallback to center.
                (_, ParameterAxis::X) => base_min_x + cur_w * 0.5,
                (_, ParameterAxis::Y) => base_min_y + cur_h * 0.5,
            }
        };

        let mut actions = dynv1.actions.clone();
        actions.sort_by_key(|a| a.order);
        for action in actions {
            let Some(param) = dynv1
                .parameters
                .iter()
                .find(|p| p.id == action.parameter_id)
            else {
                continue;
            };
            let cur = *effective.get(&param.id).unwrap_or(&param.default_value);
            let delta = cur - param.default_value;
            if delta.abs() <= 1e-12 && !matches!(action.action_kind, ActionKind::Anchor) {
                continue;
            }

            for target in &action.targets {
                let mut idxs: Vec<usize> = Vec::new();
                match &target.target {
                    TargetRef::Entity(id) => {
                        if let Some(&idx) = index_by_local_id.get(id) {
                            idxs.push(idx);
                        }
                    }
                    TargetRef::Group(group_id) => {
                        if let Some(group) = dynv1.groups.iter().find(|g| g.id == *group_id) {
                            for member in &group.members {
                                if let Some(&idx) = index_by_local_id.get(member) {
                                    idxs.push(idx);
                                }
                            }
                        }
                    }
                    TargetRef::SubEntity { .. } => {
                        // TODO(dynamic-v1): Support sub-entity target deformation.
                        continue;
                    }
                }
                if idxs.is_empty() {
                    continue;
                }

                for idx in idxs {
                    let Some((x0, y0, x1, y1)) = Self::kind_bounds_local(&working[idx].kind) else {
                        continue;
                    };
                    let cx = (x0 + x1) * 0.5;
                    let cy = (y0 + y1) * 0.5;

                    let local_id = dynv1.base_entities[idx].local_entity_id;
                    let (default_off_x, default_off_y) = authored_bounds_by_id
                        .get(&local_id)
                        .map(|(ax0, ay0, ax1, ay1)| {
                            let acx = (ax0 + ax1) * 0.5;
                            let acy = (ay0 + ay1) * 0.5;
                            (
                                acx - resolve_frame_value(target.reference_frame, ParameterAxis::X),
                                acy - resolve_frame_value(target.reference_frame, ParameterAxis::Y),
                            )
                        })
                        .unwrap_or((0.0, 0.0));

                    let weight = if target.weight.is_finite() {
                        target.weight
                    } else {
                        1.0
                    };
                    let mut dx = 0.0;
                    let mut dy = 0.0;

                    match target.behavior {
                        EntityBehavior::MoveRigid => match param.axis {
                            ParameterAxis::X => dx = delta * weight,
                            ParameterAxis::Y => dy = delta * weight,
                        },
                        EntityBehavior::KeepCentered | EntityBehavior::AnchorToCenter => {
                            if target.axis_mask.x {
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => 0.0,
                                    PlacementRule::Proportional(_) => 0.0,
                                };
                                let center_x = resolve_frame_value(
                                    ReferenceFrame::BoundsCenter,
                                    ParameterAxis::X,
                                );
                                dx = (center_x + off) - cx;
                            }
                            if target.axis_mask.y {
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => 0.0,
                                    PlacementRule::Proportional(_) => 0.0,
                                };
                                let center_y = resolve_frame_value(
                                    ReferenceFrame::BoundsCenter,
                                    ParameterAxis::Y,
                                );
                                dy = (center_y + off) - cy;
                            }
                        }
                        EntityBehavior::AnchorToEdge => {
                            if target.axis_mask.x {
                                let ref_x =
                                    resolve_frame_value(target.reference_frame, ParameterAxis::X);
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => default_off_x,
                                    PlacementRule::Proportional(_) => default_off_x,
                                };
                                let edge_x = match target.reference_frame {
                                    ReferenceFrame::LeftEdge => x0,
                                    ReferenceFrame::RightEdge => x1,
                                    _ => cx,
                                };
                                dx = (ref_x + off) - edge_x;
                            }
                            if target.axis_mask.y {
                                let ref_y =
                                    resolve_frame_value(target.reference_frame, ParameterAxis::Y);
                                let off = match target.placement_rule {
                                    PlacementRule::Offset(v) => v,
                                    PlacementRule::KeepDefault => default_off_y,
                                    PlacementRule::Proportional(_) => default_off_y,
                                };
                                let edge_y = match target.reference_frame {
                                    ReferenceFrame::BottomEdge => y0,
                                    ReferenceFrame::TopEdge => y1,
                                    _ => cy,
                                };
                                dy = (ref_y + off) - edge_y;
                            }
                        }
                        EntityBehavior::StretchFromLeft
                        | EntityBehavior::StretchFromRight
                        | EntityBehavior::StretchFromCenter => {
                            // TODO(dynamic-v1): Stretch behavior plugs in next phase.
                            continue;
                        }
                        EntityBehavior::Ignore => continue,
                    }

                    if !target.axis_mask.x {
                        dx = 0.0;
                    }
                    if !target.axis_mask.y {
                        dy = 0.0;
                    }
                    if dx.abs() <= 1e-12 && dy.abs() <= 1e-12 {
                        continue;
                    }
                    working[idx].kind = Self::translate_kind_local(&working[idx].kind, dx, dy);
                }
            }
        }

        Some(working)
    }

    fn kind_bounds_local(kind: &EntityKind) -> Option<(f64, f64, f64, f64)> {
        match kind {
            EntityKind::Line { start, end } => Some((
                start.x.min(end.x),
                start.y.min(end.y),
                start.x.max(end.x),
                start.y.max(end.y),
            )),
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let mut pts: Vec<Vec2> = Vec::with_capacity(6);
                pts.push(Vec2::new(
                    center.x + radius * start_angle.cos(),
                    center.y + radius * start_angle.sin(),
                ));
                pts.push(Vec2::new(
                    center.x + radius * end_angle.cos(),
                    center.y + radius * end_angle.sin(),
                ));
                for &a in &[
                    0.0_f64,
                    std::f64::consts::FRAC_PI_2,
                    std::f64::consts::PI,
                    3.0 * std::f64::consts::FRAC_PI_2,
                ] {
                    if Self::angle_in_arc_ccw(a, *start_angle, *end_angle) {
                        pts.push(Vec2::new(
                            center.x + radius * a.cos(),
                            center.y + radius * a.sin(),
                        ));
                    }
                }
                Self::bounds_from_points(&pts)
            }
            EntityKind::Circle { center, radius } => Some((
                center.x - radius,
                center.y - radius,
                center.x + radius,
                center.y + radius,
            )),
            EntityKind::Polyline { vertices, .. } => {
                if vertices.is_empty() {
                    return None;
                }
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                for v in vertices {
                    min_x = min_x.min(v.x);
                    min_y = min_y.min(v.y);
                    max_x = max_x.max(v.x);
                    max_y = max_y.max(v.y);
                }
                Some((min_x, min_y, max_x, max_y))
            }
            EntityKind::DimAligned {
                start,
                end,
                text_pos,
                ..
            }
            | EntityKind::DimLinear {
                start,
                end,
                text_pos,
                ..
            } => Some((
                start.x.min(end.x).min(text_pos.x),
                start.y.min(end.y).min(text_pos.y),
                start.x.max(end.x).max(text_pos.x),
                start.y.max(end.y).max(text_pos.y),
            )),
            EntityKind::DimAngular {
                vertex, text_pos, ..
            } => Some((
                vertex.x.min(text_pos.x),
                vertex.y.min(text_pos.y),
                vertex.x.max(text_pos.x),
                vertex.y.max(text_pos.y),
            )),
            EntityKind::DimRadial {
                center,
                radius,
                leader_pt,
                ..
            } => Some((
                (center.x - radius).min(leader_pt.x),
                (center.y - radius).min(leader_pt.y),
                (center.x + radius).max(leader_pt.x),
                (center.y + radius).max(leader_pt.y),
            )),
            EntityKind::Text { position, .. } => {
                Some((position.x, position.y, position.x, position.y))
            }
            EntityKind::Insert { position, .. } => {
                Some((position.x, position.y, position.x, position.y))
            }
        }
    }

    fn bounds_from_points(points: &[Vec2]) -> Option<(f64, f64, f64, f64)> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        Some((min_x, min_y, max_x, max_y))
    }

    fn angle_in_arc_ccw(a: f64, start: f64, end: f64) -> bool {
        use std::f64::consts::TAU;
        let norm = |mut v: f64| {
            while v < 0.0 {
                v += TAU;
            }
            while v >= TAU {
                v -= TAU;
            }
            v
        };
        let a = norm(a);
        let s = norm(start);
        let mut e = norm(end);
        if e < s {
            e += TAU;
        }
        let mut aa = a;
        if aa < s {
            aa += TAU;
        }
        aa >= s - 1e-12 && aa <= e + 1e-12
    }

    fn translate_kind_local(kind: &EntityKind, dx: f64, dy: f64) -> EntityKind {
        let tp = |p: Vec3| Vec3::xy(p.x + dx, p.y + dy);
        match kind {
            EntityKind::Line { start, end } => EntityKind::Line {
                start: tp(*start),
                end: tp(*end),
            },
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => EntityKind::Arc {
                center: tp(*center),
                radius: *radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
            },
            EntityKind::Circle { center, radius } => EntityKind::Circle {
                center: tp(*center),
                radius: *radius,
            },
            EntityKind::Polyline { vertices, closed } => EntityKind::Polyline {
                vertices: vertices.iter().map(|v| tp(*v)).collect(),
                closed: *closed,
            },
            EntityKind::DimAligned {
                start,
                end,
                offset,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimAligned {
                start: tp(*start),
                end: tp(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimLinear {
                start,
                end,
                offset,
                text_override,
                text_pos,
                horizontal,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimLinear {
                start: tp(*start),
                end: tp(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                horizontal: *horizontal,
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimAngular {
                vertex,
                line1_pt,
                line2_pt,
                radius,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimAngular {
                vertex: tp(*vertex),
                line1_pt: tp(*line1_pt),
                line2_pt: tp(*line2_pt),
                radius: *radius,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimRadial {
                center,
                radius,
                leader_pt,
                is_diameter,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimRadial {
                center: tp(*center),
                radius: *radius,
                leader_pt: tp(*leader_pt),
                is_diameter: *is_diameter,
                text_override: text_override.clone(),
                text_pos: tp(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::Text {
                position,
                content,
                height,
                rotation,
                font_name,
            } => EntityKind::Text {
                position: tp(*position),
                content: content.clone(),
                height: *height,
                rotation: *rotation,
                font_name: font_name.clone(),
            },
            EntityKind::Insert {
                name,
                position,
                rotation,
                scale_x,
                scale_y,
            } => EntityKind::Insert {
                name: name.clone(),
                position: tp(*position),
                rotation: *rotation,
                scale_x: *scale_x,
                scale_y: *scale_y,
            },
        }
    }

    // -------------------------------------------------------------------------
    // Layer Management
    // -------------------------------------------------------------------------

    /// Add a layer, automatically selecting the next palette colour.
    pub fn add_layer(&mut self, name: String) -> u32 {
        let color = LAYER_COLORS[self.next_layer_id as usize % LAYER_COLORS.len()];
        self.add_layer_with_color(name, color)
    }

    /// Add a layer with an explicit RGB colour.
    pub fn add_layer_with_color(&mut self, name: String, color: [u8; 3]) -> u32 {
        let id = self.next_layer_id;
        self.next_layer_id += 1;
        let layer = Layer::new(id, name, color);
        self.layers.insert(id, layer);
        id
    }

    /// Remove a layer by id. Returns false if the layer does not exist or has entities on it.
    /// Layer 0 cannot be removed.
    pub fn remove_layer(&mut self, id: u32) -> bool {
        if id == 0 {
            return false;
        }
        if self.entities.values().any(|e| e.layer == id) {
            return false;
        }
        self.layers.remove(&id).is_some()
    }

    pub fn get_layer(&self, id: u32) -> Option<&Layer> {
        self.layers.get(&id)
    }

    pub fn get_layer_mut(&mut self, id: u32) -> Option<&mut Layer> {
        self.layers.get_mut(&id)
    }

    pub fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.values()
    }

    // -------------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------------

    pub fn entities_on_layer(&self, layer_id: u32) -> impl Iterator<Item = &Entity> {
        self.entities.values().filter(move |e| e.layer == layer_id)
    }

    pub fn visible_entities(&self) -> impl Iterator<Item = &Entity> + '_ {
        self.entities.values().filter(|e| {
            self.layers
                .get(&e.layer)
                .map(|l| l.visible && !l.frozen)
                .unwrap_or(false)
        })
    }

    // -------------------------------------------------------------------------
    // File I/O
    // -------------------------------------------------------------------------

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &str) -> Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let drawing = serde_json::from_str(&json)?;
        Ok(drawing)
    }
}

impl Default for Drawing {
    fn default() -> Self {
        Self::new("Untitled".to_string())
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Create a line entity on default layer
pub fn create_line(start: Vec2, end: Vec2) -> Entity {
    Entity::new(
        EntityKind::Line {
            start: start.into(),
            end: end.into(),
        },
        0,
    )
}

/// Create a circle entity on default layer
pub fn create_circle(center: Vec2, radius: f64) -> Entity {
    Entity::new(
        EntityKind::Circle {
            center: center.into(),
            radius,
        },
        0,
    )
}

/// Create an arc entity on default layer
pub fn create_arc(center: Vec2, radius: f64, start_angle: f64, end_angle: f64) -> Entity {
    Entity::new(
        EntityKind::Arc {
            center: center.into(),
            radius,
            start_angle,
            end_angle,
        },
        0,
    )
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_drawing() {
        let drawing = Drawing::new("Test".to_string());
        assert_eq!(drawing.name, "Test");
        assert_eq!(drawing.entity_count(), 0);
        assert_eq!(drawing.layers().count(), 1); // default layer
    }

    #[test]
    fn test_add_entity() {
        let mut drawing = Drawing::default();
        let line = create_line(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let id = drawing.add_entity(line);

        assert_eq!(drawing.entity_count(), 1);
        assert!(drawing.get_entity(&id).is_some());
    }

    #[test]
    fn test_remove_entity() {
        let mut drawing = Drawing::default();
        let line = create_line(Vec2::ZERO, Vec2::new(10.0, 10.0));
        let id = drawing.add_entity(line);

        let removed = drawing.remove_entity(&id);
        assert!(removed.is_some());
        assert_eq!(drawing.entity_count(), 0);
    }

    #[test]
    fn test_layers() {
        let mut drawing = Drawing::default();
        let layer_id = drawing.add_layer("Walls".to_string());

        assert!(drawing.get_layer(layer_id).is_some());
        assert_eq!(drawing.get_layer(layer_id).unwrap().name, "Walls");
    }

    #[test]
    fn test_entity_is_planar() {
        let line = EntityKind::Line {
            start: Vec3::xy(0.0, 0.0),
            end: Vec3::xy(10.0, 10.0),
        };
        assert!(line.is_planar());

        let line_3d = EntityKind::Line {
            start: Vec3::new(0.0, 0.0, 5.0),
            end: Vec3::new(10.0, 10.0, 0.0),
        };
        assert!(!line_3d.is_planar());
    }

    #[test]
    fn test_save_load() {
        let mut drawing = Drawing::new("SaveTest".to_string());
        let line = create_line(Vec2::ZERO, Vec2::new(100.0, 50.0));
        drawing.add_entity(line);

        let temp_path = "/tmp/test_drawing.json";
        drawing.save_to_file(temp_path).unwrap();

        let loaded = Drawing::load_from_file(temp_path).unwrap();
        assert_eq!(loaded.name, "SaveTest");
        assert_eq!(loaded.entity_count(), 1);

        std::fs::remove_file(temp_path).ok();
    }
}
