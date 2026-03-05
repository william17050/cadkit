//! DXF import / export for CadKit drawings.
//!
//! * Export: CadKit entities → DXF entities, layers with ACI colours
//! * Import: DXF entities → CadKit entities, unsupported types skipped with warnings

use crate::{Drawing, Entity, EntityKind};
use cadkit_types::{CadError, Guid, Result, Vec3};
use dxf::entities::{EntityType, LwPolyline};
use dxf::tables::Layer as DxfLayer;
use dxf::{Color, LwPolylineVertex, Point};
use std::collections::{BTreeSet, HashMap};

// ============================================================================
// ACI colour palette helpers
// ============================================================================

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    [
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    ]
}

/// Convert AutoCAD Color Index (ACI) 0-255 to RGB.
pub fn aci_to_rgb(idx: u8) -> [u8; 3] {
    match idx {
        0   => [  0,   0,   0],
        1   => [255,   0,   0],
        2   => [255, 255,   0],
        3   => [  0, 255,   0],
        4   => [  0, 255, 255],
        5   => [  0,   0, 255],
        6   => [255,   0, 255],
        7   => [255, 255, 255],
        8   => [ 65,  65,  65],
        9   => [128, 128, 128],
        10..=249 => {
            let group = (idx - 10) / 10;
            let shade = (idx - 10) % 10;
            let hue = group as f32 * 15.0;
            let (s, v): (f32, f32) = match shade {
                0 => (1.000, 1.000),
                1 => (0.500, 1.000),
                2 => (1.000, 0.500),
                3 => (0.500, 0.500),
                4 => (1.000, 0.250),
                5 => (0.250, 1.000),
                6 => (0.250, 0.500),
                7 => (0.500, 0.250),
                8 => (1.000, 0.125),
                _ => (0.125, 1.000),
            };
            hsv_to_rgb(hue, s, v)
        }
        250 => [ 26,  26,  26],
        251 => [ 51,  51,  51],
        252 => [ 77,  77,  77],
        253 => [102, 102, 102],
        254 => [153, 153, 153],
        _   => [204, 204, 204],
    }
}

/// Find the nearest ACI index (1-255) for the given RGB using minimum
/// Euclidean distance in RGB space.
pub fn rgb_to_aci(rgb: [u8; 3]) -> u8 {
    let [r, g, b] = rgb;
    let mut best_idx = 1u8;
    let mut best_dist = u32::MAX;
    for idx in 1u8..=255 {
        let [pr, pg, pb] = aci_to_rgb(idx);
        let dist = (r as i32 - pr as i32).pow(2) as u32
            + (g as i32 - pg as i32).pow(2) as u32
            + (b as i32 - pb as i32).pow(2) as u32;
        if dist < best_dist {
            best_dist = dist;
            best_idx = idx;
            if dist == 0 {
                break;
            }
        }
    }
    best_idx
}

// ============================================================================
// Import result
// ============================================================================

/// Result of a successful DXF import.
pub struct DxfImportResult {
    pub drawing: Drawing,
    pub entity_count: usize,
    pub layer_count: usize,
    /// Deduplicated list of DXF entity types that were skipped.
    pub skipped_entity_types: Vec<String>,
}

// ============================================================================
// Drawing impl — export + import
// ============================================================================

impl Drawing {
    /// Export this drawing to a DXF file.
    /// Returns the number of entities written.
    pub fn save_to_dxf(&self, path: &str) -> Result<usize> {
        use dxf::entities::Entity as DxfEntity;

        let mut dxf = dxf::Drawing::new();

        // Export layers.
        for layer in self.layers() {
            let aci = rgb_to_aci(layer.color);
            let mut dl = DxfLayer::default();
            dl.name = layer.name.clone();
            dl.color = Color::from_index(aci);
            dl.is_layer_on = layer.visible;
            dxf.add_layer(dl);
        }

        let mut count = 0usize;

        for entity in self.entities() {
            let layer_name = self
                .get_layer(entity.layer)
                .map(|l| l.name.clone())
                .unwrap_or_else(|| "0".to_string());

            let color = match entity.color {
                Some(rgb) => Color::from_index(rgb_to_aci(rgb)),
                None      => Color::by_layer(),
            };

            let opt: Option<DxfEntity> = match &entity.kind {
                EntityKind::Line { start, end } => {
                    use dxf::entities::Line as DxfLine;
                    let mut e = DxfEntity::new(EntityType::Line(DxfLine::new(
                        Point::new(start.x, start.y, start.z),
                        Point::new(end.x,   end.y,   end.z),
                    )));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    Some(e)
                }
                EntityKind::Circle { center, radius } => {
                    use dxf::entities::Circle as DxfCircle;
                    let mut e = DxfEntity::new(EntityType::Circle(DxfCircle::new(
                        Point::new(center.x, center.y, center.z),
                        *radius,
                    )));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    Some(e)
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    use dxf::entities::Arc as DxfArc;
                    let mut e = DxfEntity::new(EntityType::Arc(DxfArc::new(
                        Point::new(center.x, center.y, center.z),
                        *radius,
                        start_angle.to_degrees(),
                        end_angle.to_degrees(),
                    )));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    Some(e)
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 {
                        log::warn!(
                            "DXF export: skipping polyline with {} vertices (need ≥2)",
                            vertices.len()
                        );
                        None
                    } else {
                        let mut poly = LwPolyline::default();
                        if *closed {
                            poly.flags |= 1; // bit 0 = closed
                        }
                        for v in vertices {
                            poly.vertices.push(LwPolylineVertex {
                                x: v.x,
                                y: v.y,
                                ..Default::default()
                            });
                        }
                        let mut e = DxfEntity::new(EntityType::LwPolyline(poly));
                        e.common.layer = layer_name;
                        e.common.color = color;
                        Some(e)
                    }
                }
                EntityKind::Text { position, content, height, rotation } => {
                    use dxf::entities::Text as DxfText;
                    let mut t = DxfText::default();
                    t.location   = Point::new(position.x, position.y, 0.0);
                    t.text_height = *height;
                    t.value      = content.clone();
                    t.rotation   = rotation.to_degrees();
                    let mut e = dxf::entities::Entity::new(EntityType::Text(t));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    Some(e)
                }
                EntityKind::DimAligned { .. } => {
                    // TODO: export as DXF DIMENSION entity (AlignedDimension)
                    log::warn!("DXF export: DimAligned not yet exported");
                    None
                }
                EntityKind::DimLinear { .. } => {
                    // TODO: export as DXF DIMENSION entity (RotatedDimension)
                    log::warn!("DXF export: DimLinear not yet exported");
                    None
                }
            };

            if let Some(e) = opt {
                dxf.add_entity(e);
                count += 1;
            }
        }

        dxf.save_file(path).map_err(|e| CadError::DxfError(e.to_string()))?;
        Ok(count)
    }

    /// Import a DXF file, returning the drawing and import statistics.
    /// Unsupported entity types are skipped and reported in the result.
    pub fn load_from_dxf(path: &str) -> Result<DxfImportResult> {
        let dxf = dxf::Drawing::load_file(path)
            .map_err(|e| CadError::DxfError(e.to_string()))?;

        let mut drawing = Drawing::new("Imported".to_string());

        // layer name → CadKit layer id
        let mut name_to_id: HashMap<String, u32> = HashMap::new();
        name_to_id.insert("0".to_string(), 0);

        let mut layer_count = 1usize; // layer "0" always exists

        // Import layers from DXF layer table.
        for dl in dxf.layers() {
            let aci = dl.color.index().unwrap_or(7);
            let rgb = aci_to_rgb(aci);

            if dl.name == "0" {
                // Update the existing default layer.
                if let Some(l) = drawing.get_layer_mut(0) {
                    l.color   = rgb;
                    l.visible = dl.is_layer_on;
                }
                continue;
            }

            let new_id = drawing.add_layer_with_color(dl.name.clone(), rgb);
            if let Some(l) = drawing.get_layer_mut(new_id) {
                l.visible = dl.is_layer_on;
            }
            name_to_id.insert(dl.name.clone(), new_id);
            layer_count += 1;
        }

        let mut entity_count = 0usize;
        let mut skipped: BTreeSet<String> = BTreeSet::new();

        for de in dxf.entities() {
            let layer_name = de.common.layer.clone();

            // Resolve layer; auto-create if not in table.
            let layer_id = if let Some(&id) = name_to_id.get(&layer_name) {
                id
            } else {
                let new_id = drawing.add_layer_with_color(layer_name.clone(), [255, 255, 255]);
                name_to_id.insert(layer_name.clone(), new_id);
                layer_count += 1;
                new_id
            };

            // Entity colour: None = ByLayer.
            let color: Option<[u8; 3]> = if de.common.color.is_by_layer() {
                None
            } else {
                de.common.color.index().map(aci_to_rgb)
            };

            let cadkit: Option<Entity> = match &de.specific {
                EntityType::Line(line) => Some(Entity {
                    id:    Guid::new(),
                    kind:  EntityKind::Line {
                        start: Vec3::new(line.p1.x, line.p1.y, line.p1.z),
                        end:   Vec3::new(line.p2.x, line.p2.y, line.p2.z),
                    },
                    layer: layer_id,
                    color,
                }),

                EntityType::Circle(circle) => Some(Entity {
                    id:    Guid::new(),
                    kind:  EntityKind::Circle {
                        center: Vec3::new(circle.center.x, circle.center.y, circle.center.z),
                        radius: circle.radius,
                    },
                    layer: layer_id,
                    color,
                }),

                EntityType::Arc(arc) => Some(Entity {
                    id:    Guid::new(),
                    kind:  EntityKind::Arc {
                        center:      Vec3::new(arc.center.x, arc.center.y, arc.center.z),
                        radius:      arc.radius,
                        start_angle: arc.start_angle.to_radians(),
                        end_angle:   arc.end_angle.to_radians(),
                    },
                    layer: layer_id,
                    color,
                }),

                EntityType::LwPolyline(poly) => {
                    let verts: Vec<Vec3> = poly
                        .vertices
                        .iter()
                        .map(|v| Vec3::new(v.x, v.y, 0.0))
                        .collect();
                    if verts.len() >= 2 {
                        let closed = (poly.flags & 1) != 0;
                        Some(Entity {
                            id:    Guid::new(),
                            kind:  EntityKind::Polyline { vertices: verts, closed },
                            layer: layer_id,
                            color,
                        })
                    } else {
                        None
                    }
                }

                // Polyline (old-style) — convert vertices to CadKit Polyline.
                EntityType::Polyline(poly) => {
                    let verts: Vec<Vec3> = poly
                        .vertices()
                        .map(|v| Vec3::new(v.location.x, v.location.y, v.location.z))
                        .collect();
                    if verts.len() >= 2 {
                        let closed = (poly.flags & 1) != 0;
                        Some(Entity {
                            id:    Guid::new(),
                            kind:  EntityKind::Polyline { vertices: verts, closed },
                            layer: layer_id,
                            color,
                        })
                    } else {
                        None
                    }
                }

                EntityType::Text(t) => Some(Entity {
                    id:    Guid::new(),
                    kind:  EntityKind::Text {
                        position: Vec3::xy(t.location.x, t.location.y),
                        content:  t.value.clone(),
                        height:   t.text_height.max(0.01),
                        rotation: t.rotation.to_radians(),
                    },
                    layer: layer_id,
                    color,
                }),

                other => {
                    skipped.insert(dxf_type_name(other).to_string());
                    None
                }
            };

            if let Some(e) = cadkit {
                drawing.add_entity(e);
                entity_count += 1;
            }
        }

        Ok(DxfImportResult {
            drawing,
            entity_count,
            layer_count,
            skipped_entity_types: skipped.into_iter().collect(),
        })
    }
}

/// Return a human-readable DXF entity type name for warning messages.
fn dxf_type_name(et: &EntityType) -> &'static str {
    match et {
        EntityType::MText(_)                       => "MTEXT",
        EntityType::Insert(_)                      => "INSERT",
        EntityType::RotatedDimension(_)
        | EntityType::RadialDimension(_)
        | EntityType::DiameterDimension(_)
        | EntityType::AngularThreePointDimension(_)
        | EntityType::OrdinateDimension(_)         => "DIMENSION",
        EntityType::Spline(_)                      => "SPLINE",
        EntityType::Ellipse(_)                     => "ELLIPSE",
        EntityType::Image(_)                       => "IMAGE",
        EntityType::Leader(_)                      => "LEADER",
        EntityType::Solid(_)                       => "SOLID",
        EntityType::Trace(_)                       => "TRACE",
        EntityType::Face3D(_)                      => "3DFACE",
        EntityType::Attribute(_)                   => "ATTRIB",
        EntityType::AttributeDefinition(_)         => "ATTDEF",
        EntityType::Ray(_)                         => "RAY",
        EntityType::XLine(_)                       => "XLINE",
        EntityType::Body(_)                        => "BODY",
        EntityType::Region(_)                      => "REGION",
        _                                          => "UNSUPPORTED",
    }
}
