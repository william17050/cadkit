//! 2D CAD core - entities, layers, and drawing management
//!
//! This crate provides:
//! - 2D geometric entities (Line, Arc, Circle, Polyline)
//! - Layer management
//! - Drawing document structure
//! - Entity storage and queries

use cadkit_types::{Guid, Result, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// AutoCAD-style default layer colour palette (index 0–7).
pub const LAYER_COLORS: &[[u8; 3]] = &[
    [255, 255, 255], // 0 white
    [255,   0,   0], // 1 red
    [255, 255,   0], // 2 yellow
    [  0, 255,   0], // 3 green
    [  0, 255, 255], // 4 cyan
    [  0,   0, 255], // 5 blue
    [255,   0, 255], // 6 magenta
    [128, 128, 128], // 7 gray
];

fn default_layer_color() -> [u8; 3] {
    LAYER_COLORS[0]
}

// =============================================================================
// Entities
// =============================================================================

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
    Circle {
        center: Vec3,
        radius: f64,
    },
    
    /// Connected line/arc segments
    Polyline {
        vertices: Vec<Vec3>,
        closed: bool,
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
        }
    }
}

/// Complete entity with ID and metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    pub id: Guid,
    pub kind: EntityKind,
    pub layer: u32,
    // TODO: Add color, linetype, etc
}

impl Entity {
    pub fn new(kind: EntityKind, layer: u32) -> Self {
        Self {
            id: Guid::new(),
            kind,
            layer,
        }
    }
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
    /// RGB colour used when rendering entities on this layer.
    /// Defaults to white so that old `.cadkit` files without this field load cleanly.
    #[serde(default = "default_layer_color")]
    pub color: [u8; 3],
}

impl Layer {
    pub fn new(id: u32, name: String, color: [u8; 3]) -> Self {
        Self {
            id,
            name,
            visible: true,
            locked: false,
            color,
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
    entities: HashMap<Guid, Entity>,
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
            entities: HashMap::new(),
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
                .map(|l| l.visible)
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
