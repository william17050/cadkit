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

/// Named reusable block definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockDefinition {
    pub name: String,
    pub base_point: Vec3,
    pub entities: Vec<BlockEntity>,
    #[serde(default)]
    pub dynamic: Option<BlockDynamic>,
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
