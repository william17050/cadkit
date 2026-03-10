//! DXF import / export for CadKit drawings.
//!
//! * Export: CadKit entities → DXF entities, layers with ACI colours
//! * Import: DXF entities → CadKit entities, unsupported types skipped with warnings

use crate::{Drawing, Entity, EntityKind, Linetype};
use cadkit_types::{CadError, Guid, Result, Vec3};
use dxf::entities::{EntityType, LwPolyline};
use dxf::enums::DimensionType;
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
        0 => [0, 0, 0],
        1 => [255, 0, 0],
        2 => [255, 255, 0],
        3 => [0, 255, 0],
        4 => [0, 255, 255],
        5 => [0, 0, 255],
        6 => [255, 0, 255],
        7 => [255, 255, 255],
        8 => [65, 65, 65],
        9 => [128, 128, 128],
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
        250 => [26, 26, 26],
        251 => [51, 51, 51],
        252 => [77, 77, 77],
        253 => [102, 102, 102],
        254 => [153, 153, 153],
        _ => [204, 204, 204],
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
            dl.line_type_name = layer.linetype.to_dxf_name().to_string();
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
                None => Color::by_layer(),
            };

            let opt: Option<DxfEntity> = match &entity.kind {
                EntityKind::Line { start, end } => {
                    use dxf::entities::Line as DxfLine;
                    let mut e = DxfEntity::new(EntityType::Line(DxfLine::new(
                        Point::new(start.x, start.y, start.z),
                        Point::new(end.x, end.y, end.z),
                    )));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    e.common.line_type_name = if entity.linetype_by_layer {
                        "BYLAYER".to_string()
                    } else {
                        entity.linetype.to_dxf_name().to_string()
                    };
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
                    e.common.line_type_name = if entity.linetype_by_layer {
                        "BYLAYER".to_string()
                    } else {
                        entity.linetype.to_dxf_name().to_string()
                    };
                    Some(e)
                }
                EntityKind::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    use dxf::entities::Arc as DxfArc;
                    let mut e = DxfEntity::new(EntityType::Arc(DxfArc::new(
                        Point::new(center.x, center.y, center.z),
                        *radius,
                        start_angle.to_degrees(),
                        end_angle.to_degrees(),
                    )));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    e.common.line_type_name = if entity.linetype_by_layer {
                        "BYLAYER".to_string()
                    } else {
                        entity.linetype.to_dxf_name().to_string()
                    };
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
                        e.common.line_type_name = if entity.linetype_by_layer {
                            "BYLAYER".to_string()
                        } else {
                            entity.linetype.to_dxf_name().to_string()
                        };
                        Some(e)
                    }
                }
                EntityKind::Text {
                    position,
                    content,
                    height,
                    rotation,
                    font_name,
                } => {
                    if content.contains('\n') {
                        use dxf::entities::MText as DxfMText;
                        let mut mt = DxfMText::default();
                        mt.insertion_point = Point::new(position.x, position.y, 0.0);
                        mt.initial_text_height = (*height).max(0.01);
                        // DXF MTEXT uses \P paragraph breaks for new lines.
                        mt.text = content.replace('\n', "\\P");
                        mt.rotation_angle = rotation.to_degrees();
                        mt.text_style_name = if font_name.trim().is_empty() {
                            "STANDARD".to_string()
                        } else {
                            font_name.clone()
                        };
                        let max_line = content
                            .lines()
                            .map(|l| l.chars().count())
                            .max()
                            .unwrap_or(1) as f64;
                        mt.reference_rectangle_width =
                            (max_line * mt.initial_text_height * 0.6).max(mt.initial_text_height);
                        let mut e = dxf::entities::Entity::new(EntityType::MText(mt));
                        e.common.layer = layer_name;
                        e.common.color = color;
                        e.common.line_type_name = if entity.linetype_by_layer {
                            "BYLAYER".to_string()
                        } else {
                            entity.linetype.to_dxf_name().to_string()
                        };
                        Some(e)
                    } else {
                        use dxf::entities::Text as DxfText;
                        let mut t = DxfText::default();
                        t.location = Point::new(position.x, position.y, 0.0);
                        t.text_height = *height;
                        t.value = content.clone();
                        t.rotation = rotation.to_degrees();
                        t.text_style_name = if font_name.trim().is_empty() {
                            "STANDARD".to_string()
                        } else {
                            font_name.clone()
                        };
                        let mut e = dxf::entities::Entity::new(EntityType::Text(t));
                        e.common.layer = layer_name;
                        e.common.color = color;
                        e.common.line_type_name = if entity.linetype_by_layer {
                            "BYLAYER".to_string()
                        } else {
                            entity.linetype.to_dxf_name().to_string()
                        };
                        Some(e)
                    }
                }
                EntityKind::DimAligned {
                    start,
                    end,
                    offset,
                    text_override,
                    text_pos,
                    ..
                } => {
                    use dxf::entities::{DimensionBase, RotatedDimension};
                    let sx = start.x;
                    let sy = start.y;
                    let ex = end.x;
                    let ey = end.y;
                    let dx = ex - sx;
                    let dy = ey - sy;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= 1e-9 {
                        None
                    } else {
                        let ux = dx / len;
                        let uy = dy / len;
                        let px = -uy;
                        let py = ux;
                        let ins = Point::new(sx + px * *offset, sy + py * *offset, 0.0);
                        let mut base = DimensionBase::default();
                        base.dimension_type = DimensionType::Aligned;
                        base.definition_point_1 = ins.clone();
                        base.text_mid_point = Point::new(text_pos.x, text_pos.y, 0.0);
                        base.text = text_override.clone().unwrap_or_else(|| "<>".to_string());
                        base.actual_measurement = len;
                        let mut dim = RotatedDimension::default();
                        dim.dimension_base = base;
                        dim.insertion_point = ins;
                        dim.definition_point_2 = Point::new(sx, sy, start.z);
                        dim.definition_point_3 = Point::new(ex, ey, end.z);
                        dim.rotation_angle = dy.atan2(dx).to_degrees();
                        dim.extension_line_angle = 0.0;
                        let mut e = DxfEntity::new(EntityType::RotatedDimension(dim));
                        e.common.layer = layer_name;
                        e.common.color = color;
                        e.common.line_type_name = if entity.linetype_by_layer {
                            "BYLAYER".to_string()
                        } else {
                            entity.linetype.to_dxf_name().to_string()
                        };
                        Some(e)
                    }
                }
                EntityKind::DimLinear {
                    start,
                    end,
                    offset,
                    text_override,
                    text_pos,
                    horizontal,
                    ..
                } => {
                    use dxf::entities::{DimensionBase, RotatedDimension};
                    let sx = start.x;
                    let sy = start.y;
                    let ex = end.x;
                    let ey = end.y;
                    let mid_x = (sx + ex) * 0.5;
                    let mid_y = (sy + ey) * 0.5;
                    let ins = if *horizontal {
                        Point::new(mid_x, mid_y + *offset, 0.0)
                    } else {
                        Point::new(mid_x + *offset, mid_y, 0.0)
                    };
                    let mut base = DimensionBase::default();
                    base.dimension_type = DimensionType::RotatedHorizontalOrVertical;
                    base.definition_point_1 = ins.clone();
                    base.text_mid_point = Point::new(text_pos.x, text_pos.y, 0.0);
                    base.text = text_override.clone().unwrap_or_else(|| "<>".to_string());
                    base.actual_measurement = if *horizontal {
                        (ex - sx).abs()
                    } else {
                        (ey - sy).abs()
                    };
                    let mut dim = RotatedDimension::default();
                    dim.dimension_base = base;
                    dim.insertion_point = ins;
                    dim.definition_point_2 = Point::new(sx, sy, start.z);
                    dim.definition_point_3 = Point::new(ex, ey, end.z);
                    dim.rotation_angle = if *horizontal { 0.0 } else { 90.0 };
                    dim.extension_line_angle = 0.0;
                    let mut e = DxfEntity::new(EntityType::RotatedDimension(dim));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    e.common.line_type_name = if entity.linetype_by_layer {
                        "BYLAYER".to_string()
                    } else {
                        entity.linetype.to_dxf_name().to_string()
                    };
                    Some(e)
                }
                EntityKind::DimAngular {
                    vertex,
                    line1_pt,
                    line2_pt,
                    radius,
                    text_override,
                    text_pos,
                    ..
                } => {
                    use dxf::entities::{AngularThreePointDimension, DimensionBase};
                    let a1 = (line1_pt.y - vertex.y).atan2(line1_pt.x - vertex.x);
                    let mut a2 = (line2_pt.y - vertex.y).atan2(line2_pt.x - vertex.x);
                    if a2 <= a1 {
                        a2 += std::f64::consts::TAU;
                    }
                    let am = (a1 + a2) * 0.5;
                    let p_arc = Point::new(
                        vertex.x + *radius * am.cos(),
                        vertex.y + *radius * am.sin(),
                        0.0,
                    );
                    let mut base = DimensionBase::default();
                    base.dimension_type = DimensionType::AngularThreePoint;
                    base.definition_point_1 = p_arc.clone();
                    base.text_mid_point = Point::new(text_pos.x, text_pos.y, 0.0);
                    base.text = text_override.clone().unwrap_or_else(|| "<>".to_string());
                    base.actual_measurement = a2 - a1;
                    let mut dim = AngularThreePointDimension::default();
                    dim.dimension_base = base;
                    dim.definition_point_2 = Point::new(line1_pt.x, line1_pt.y, line1_pt.z);
                    dim.definition_point_3 = Point::new(line2_pt.x, line2_pt.y, line2_pt.z);
                    dim.definition_point_4 = Point::new(vertex.x, vertex.y, vertex.z);
                    dim.definition_point_5 = p_arc;
                    let mut e = DxfEntity::new(EntityType::AngularThreePointDimension(dim));
                    e.common.layer = layer_name;
                    e.common.color = color;
                    e.common.line_type_name = if entity.linetype_by_layer {
                        "BYLAYER".to_string()
                    } else {
                        entity.linetype.to_dxf_name().to_string()
                    };
                    Some(e)
                }
                EntityKind::DimRadial {
                    center,
                    radius,
                    leader_pt,
                    is_diameter,
                    text_override,
                    text_pos,
                    ..
                } => {
                    use dxf::entities::{DiameterDimension, DimensionBase, RadialDimension};
                    let mut base = DimensionBase::default();
                    base.dimension_type = if *is_diameter {
                        DimensionType::Diameter
                    } else {
                        DimensionType::Radius
                    };
                    base.definition_point_1 = Point::new(center.x, center.y, center.z);
                    base.text_mid_point = Point::new(text_pos.x, text_pos.y, 0.0);
                    base.text = text_override.clone().unwrap_or_else(|| "<>".to_string());
                    base.actual_measurement = *radius;
                    let leader_length = ((leader_pt.x - center.x).powi(2)
                        + (leader_pt.y - center.y).powi(2))
                    .sqrt()
                    .max(0.0);
                    let mut e = if *is_diameter {
                        let mut dim = DiameterDimension::default();
                        dim.dimension_base = base;
                        dim.definition_point_2 = Point::new(leader_pt.x, leader_pt.y, leader_pt.z);
                        dim.leader_length = leader_length;
                        DxfEntity::new(EntityType::DiameterDimension(dim))
                    } else {
                        let mut dim = RadialDimension::default();
                        dim.dimension_base = base;
                        dim.definition_point_2 = Point::new(leader_pt.x, leader_pt.y, leader_pt.z);
                        dim.leader_length = leader_length;
                        DxfEntity::new(EntityType::RadialDimension(dim))
                    };
                    e.common.layer = layer_name;
                    e.common.color = color;
                    e.common.line_type_name = if entity.linetype_by_layer {
                        "BYLAYER".to_string()
                    } else {
                        entity.linetype.to_dxf_name().to_string()
                    };
                    Some(e)
                }
                EntityKind::Insert { .. } => {
                    // First-pass: native Insert export is not wired yet.
                    None
                }
            };

            if let Some(e) = opt {
                dxf.add_entity(e);
                count += 1;
            }
        }

        dxf.save_file(path)
            .map_err(|e| CadError::DxfError(e.to_string()))?;
        Ok(count)
    }

    /// Import a DXF file, returning the drawing and import statistics.
    /// Unsupported entity types are skipped and reported in the result.
    pub fn load_from_dxf(path: &str) -> Result<DxfImportResult> {
        let dxf = dxf::Drawing::load_file(path).map_err(|e| CadError::DxfError(e.to_string()))?;

        let mut drawing = Drawing::new("Imported".to_string());

        // layer name → CadKit layer id
        let mut name_to_id: HashMap<String, u32> = HashMap::new();
        name_to_id.insert("0".to_string(), 0);
        let blocks_by_name: HashMap<String, dxf::Block> =
            dxf.blocks().map(|b| (b.name.clone(), b.clone())).collect();

        let mut layer_count = 1usize; // layer "0" always exists

        // Import layers from DXF layer table.
        for dl in dxf.layers() {
            let aci = dl.color.index().unwrap_or(7);
            let rgb = aci_to_rgb(aci);

            if dl.name == "0" {
                // Update the existing default layer.
                if let Some(l) = drawing.get_layer_mut(0) {
                    l.color = rgb;
                    l.visible = dl.is_layer_on;
                    l.linetype = Linetype::from_dxf_name(&dl.line_type_name);
                }
                continue;
            }

            let new_id = drawing.add_layer_with_color(dl.name.clone(), rgb);
            if let Some(l) = drawing.get_layer_mut(new_id) {
                l.visible = dl.is_layer_on;
                l.linetype = Linetype::from_dxf_name(&dl.line_type_name);
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
            let linetype = Linetype::from_dxf_name(&de.common.line_type_name);

            let mut insert_count = 0usize;
            let cadkit: Option<Entity> = match &de.specific {
                EntityType::Line(line) => Some(Entity {
                    id: Guid::new(),
                    kind: EntityKind::Line {
                        start: Vec3::new(line.p1.x, line.p1.y, line.p1.z),
                        end: Vec3::new(line.p2.x, line.p2.y, line.p2.z),
                    },
                    layer: layer_id,
                    color,
                    linetype,
                    linetype_by_layer: de
                        .common
                        .line_type_name
                        .trim()
                        .eq_ignore_ascii_case("BYLAYER"),
                    linetype_scale: None,
                    block_params: crate::BlockParamValues::default(),
                    insert_dynamic_param_overrides: std::collections::HashMap::new(),
                }),

                EntityType::Circle(circle) => Some(Entity {
                    id: Guid::new(),
                    kind: EntityKind::Circle {
                        center: Vec3::new(circle.center.x, circle.center.y, circle.center.z),
                        radius: circle.radius,
                    },
                    layer: layer_id,
                    color,
                    linetype,
                    linetype_by_layer: de
                        .common
                        .line_type_name
                        .trim()
                        .eq_ignore_ascii_case("BYLAYER"),
                    linetype_scale: None,
                    block_params: crate::BlockParamValues::default(),
                    insert_dynamic_param_overrides: std::collections::HashMap::new(),
                }),

                EntityType::Arc(arc) => Some(Entity {
                    id: Guid::new(),
                    kind: {
                        let (start_angle, end_angle) =
                            dxf_arc_angles_ccw_radians(arc.start_angle, arc.end_angle);
                        EntityKind::Arc {
                            center: Vec3::new(arc.center.x, arc.center.y, arc.center.z),
                            radius: arc.radius,
                            start_angle,
                            end_angle,
                        }
                    },
                    layer: layer_id,
                    color,
                    linetype,
                    linetype_by_layer: de
                        .common
                        .line_type_name
                        .trim()
                        .eq_ignore_ascii_case("BYLAYER"),
                    linetype_scale: None,
                    block_params: crate::BlockParamValues::default(),
                    insert_dynamic_param_overrides: std::collections::HashMap::new(),
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
                            id: Guid::new(),
                            kind: EntityKind::Polyline {
                                vertices: verts,
                                closed,
                            },
                            layer: layer_id,
                            color,
                            linetype,
                            linetype_by_layer: de
                                .common
                                .line_type_name
                                .trim()
                                .eq_ignore_ascii_case("BYLAYER"),
                            linetype_scale: None,
                            block_params: crate::BlockParamValues::default(),
                            insert_dynamic_param_overrides: std::collections::HashMap::new(),
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
                            id: Guid::new(),
                            kind: EntityKind::Polyline {
                                vertices: verts,
                                closed,
                            },
                            layer: layer_id,
                            color,
                            linetype,
                            linetype_by_layer: de
                                .common
                                .line_type_name
                                .trim()
                                .eq_ignore_ascii_case("BYLAYER"),
                            linetype_scale: None,
                            block_params: crate::BlockParamValues::default(),
                            insert_dynamic_param_overrides: std::collections::HashMap::new(),
                        })
                    } else {
                        None
                    }
                }

                EntityType::Text(t) => Some(Entity {
                    id: Guid::new(),
                    kind: EntityKind::Text {
                        position: Vec3::xy(t.location.x, t.location.y),
                        content: t.value.clone(),
                        height: t.text_height.max(0.01),
                        rotation: t.rotation.to_radians(),
                        font_name: if t.text_style_name.trim().is_empty() {
                            "STANDARD".to_string()
                        } else {
                            t.text_style_name.clone()
                        },
                    },
                    layer: layer_id,
                    color,
                    linetype,
                    linetype_by_layer: de
                        .common
                        .line_type_name
                        .trim()
                        .eq_ignore_ascii_case("BYLAYER"),
                    linetype_scale: None,
                    block_params: crate::BlockParamValues::default(),
                    insert_dynamic_param_overrides: std::collections::HashMap::new(),
                }),

                EntityType::MText(t) => {
                    let raw = if t.text.is_empty() {
                        t.extended_text.join("")
                    } else if t.extended_text.is_empty() {
                        t.text.clone()
                    } else {
                        let mut s = t.extended_text.join("");
                        s.push_str(&t.text);
                        s
                    };
                    // Convert common DXF MTEXT paragraph delimiters back to plain newlines.
                    let content = raw.replace("\\P", "\n").replace("\\p", "\n");
                    Some(Entity {
                        id: Guid::new(),
                        kind: EntityKind::Text {
                            position: Vec3::xy(t.insertion_point.x, t.insertion_point.y),
                            content,
                            height: t.initial_text_height.max(0.01),
                            rotation: t.rotation_angle.to_radians(),
                            font_name: if t.text_style_name.trim().is_empty() {
                                "STANDARD".to_string()
                            } else {
                                t.text_style_name.clone()
                            },
                        },
                        layer: layer_id,
                        color,
                        linetype,
                        linetype_by_layer: de
                            .common
                            .line_type_name
                            .trim()
                            .eq_ignore_ascii_case("BYLAYER"),
                        linetype_scale: None,
                        block_params: crate::BlockParamValues::default(),
                        insert_dynamic_param_overrides: std::collections::HashMap::new(),
                    })
                }

                EntityType::RotatedDimension(d) => {
                    let text_override = parse_dimension_text_override(&d.dimension_base.text);
                    let text_pos = Vec3::xy(
                        d.dimension_base.text_mid_point.x,
                        d.dimension_base.text_mid_point.y,
                    );
                    let sx = d.definition_point_2.x;
                    let sy = d.definition_point_2.y;
                    let ex = d.definition_point_3.x;
                    let ey = d.definition_point_3.y;
                    let dx = ex - sx;
                    let dy = ey - sy;
                    let len = (dx * dx + dy * dy).sqrt();
                    if d.dimension_base.dimension_type == DimensionType::Aligned {
                        if len <= 1e-9 {
                            None
                        } else {
                            let px = -dy / len;
                            let py = dx / len;
                            let off =
                                (d.insertion_point.x - sx) * px + (d.insertion_point.y - sy) * py;
                            Some(Entity {
                                id: Guid::new(),
                                kind: EntityKind::DimAligned {
                                    start: Vec3::xy(sx, sy),
                                    end: Vec3::xy(ex, ey),
                                    offset: off,
                                    text_override,
                                    text_pos,
                                    arrow_length: 3.0,
                                    arrow_half_width: 0.75,
                                },
                                layer: layer_id,
                                color,
                                linetype,
                                linetype_by_layer: de
                                    .common
                                    .line_type_name
                                    .trim()
                                    .eq_ignore_ascii_case("BYLAYER"),
                                linetype_scale: None,
                                block_params: crate::BlockParamValues::default(),
                                insert_dynamic_param_overrides: std::collections::HashMap::new(),
                            })
                        }
                    } else {
                        let rot = d.rotation_angle.rem_euclid(360.0);
                        let horizontal =
                            !(45.0..135.0).contains(&rot) && !(225.0..315.0).contains(&rot);
                        let mid_x = (sx + ex) * 0.5;
                        let mid_y = (sy + ey) * 0.5;
                        let off = if horizontal {
                            d.insertion_point.y - mid_y
                        } else {
                            d.insertion_point.x - mid_x
                        };
                        Some(Entity {
                            id: Guid::new(),
                            kind: EntityKind::DimLinear {
                                start: Vec3::xy(sx, sy),
                                end: Vec3::xy(ex, ey),
                                offset: off,
                                text_override,
                                text_pos,
                                horizontal,
                                arrow_length: 3.0,
                                arrow_half_width: 0.75,
                            },
                            layer: layer_id,
                            color,
                            linetype,
                            linetype_by_layer: de
                                .common
                                .line_type_name
                                .trim()
                                .eq_ignore_ascii_case("BYLAYER"),
                            linetype_scale: None,
                            block_params: crate::BlockParamValues::default(),
                            insert_dynamic_param_overrides: std::collections::HashMap::new(),
                        })
                    }
                }

                EntityType::AngularThreePointDimension(d) => {
                    let text_override = parse_dimension_text_override(&d.dimension_base.text);
                    let vertex = Vec3::xy(d.definition_point_4.x, d.definition_point_4.y);
                    let line1_pt = Vec3::xy(d.definition_point_2.x, d.definition_point_2.y);
                    let line2_pt = Vec3::xy(d.definition_point_3.x, d.definition_point_3.y);
                    let r1 = ((d.definition_point_5.x - d.definition_point_4.x).powi(2)
                        + (d.definition_point_5.y - d.definition_point_4.y).powi(2))
                    .sqrt();
                    let r2 = ((d.dimension_base.text_mid_point.x - d.definition_point_4.x).powi(2)
                        + (d.dimension_base.text_mid_point.y - d.definition_point_4.y).powi(2))
                    .sqrt();
                    let radius = if r1 > 1e-9 { r1 } else { r2.max(1.0) };
                    Some(Entity {
                        id: Guid::new(),
                        kind: EntityKind::DimAngular {
                            vertex,
                            line1_pt,
                            line2_pt,
                            radius,
                            text_override,
                            text_pos: Vec3::xy(
                                d.dimension_base.text_mid_point.x,
                                d.dimension_base.text_mid_point.y,
                            ),
                            arrow_length: 3.0,
                            arrow_half_width: 0.75,
                        },
                        layer: layer_id,
                        color,
                        linetype,
                        linetype_by_layer: de
                            .common
                            .line_type_name
                            .trim()
                            .eq_ignore_ascii_case("BYLAYER"),
                        linetype_scale: None,
                        block_params: crate::BlockParamValues::default(),
                        insert_dynamic_param_overrides: std::collections::HashMap::new(),
                    })
                }

                EntityType::RadialDimension(d) => {
                    let text_override = parse_dimension_text_override(&d.dimension_base.text);
                    let center = Vec3::xy(
                        d.dimension_base.definition_point_1.x,
                        d.dimension_base.definition_point_1.y,
                    );
                    let leader_pt = Vec3::xy(d.definition_point_2.x, d.definition_point_2.y);
                    let geom_r = ((leader_pt.x - center.x).powi(2)
                        + (leader_pt.y - center.y).powi(2))
                    .sqrt();
                    let radius = if d.dimension_base.actual_measurement > 1e-9 {
                        d.dimension_base.actual_measurement
                    } else {
                        geom_r.max(1e-6)
                    };
                    Some(Entity {
                        id: Guid::new(),
                        kind: EntityKind::DimRadial {
                            center,
                            radius,
                            leader_pt,
                            is_diameter: false,
                            text_override,
                            text_pos: Vec3::xy(
                                d.dimension_base.text_mid_point.x,
                                d.dimension_base.text_mid_point.y,
                            ),
                            arrow_length: 3.0,
                            arrow_half_width: 0.75,
                        },
                        layer: layer_id,
                        color,
                        linetype,
                        linetype_by_layer: de
                            .common
                            .line_type_name
                            .trim()
                            .eq_ignore_ascii_case("BYLAYER"),
                        linetype_scale: None,
                        block_params: crate::BlockParamValues::default(),
                        insert_dynamic_param_overrides: std::collections::HashMap::new(),
                    })
                }

                EntityType::DiameterDimension(d) => {
                    let text_override = parse_dimension_text_override(&d.dimension_base.text);
                    let center = Vec3::xy(
                        d.dimension_base.definition_point_1.x,
                        d.dimension_base.definition_point_1.y,
                    );
                    let leader_pt = Vec3::xy(d.definition_point_2.x, d.definition_point_2.y);
                    let geom_r = ((leader_pt.x - center.x).powi(2)
                        + (leader_pt.y - center.y).powi(2))
                    .sqrt();
                    let radius = if d.dimension_base.actual_measurement > 1e-9 {
                        d.dimension_base.actual_measurement
                    } else {
                        geom_r.max(1e-6)
                    };
                    Some(Entity {
                        id: Guid::new(),
                        kind: EntityKind::DimRadial {
                            center,
                            radius,
                            leader_pt,
                            is_diameter: true,
                            text_override,
                            text_pos: Vec3::xy(
                                d.dimension_base.text_mid_point.x,
                                d.dimension_base.text_mid_point.y,
                            ),
                            arrow_length: 3.0,
                            arrow_half_width: 0.75,
                        },
                        layer: layer_id,
                        color,
                        linetype,
                        linetype_by_layer: de
                            .common
                            .line_type_name
                            .trim()
                            .eq_ignore_ascii_case("BYLAYER"),
                        linetype_scale: None,
                        block_params: crate::BlockParamValues::default(),
                        insert_dynamic_param_overrides: std::collections::HashMap::new(),
                    })
                }

                EntityType::Insert(ins) => {
                    insert_count = expand_insert_flattened(
                        ins,
                        layer_id,
                        color,
                        linetype,
                        &blocks_by_name,
                        &mut drawing,
                        &mut name_to_id,
                        &mut layer_count,
                        &mut skipped,
                        Aff2::identity(),
                        0,
                    );
                    None
                }

                other => {
                    skipped.insert(dxf_type_name(other).to_string());
                    None
                }
            };

            if let Some(e) = cadkit {
                drawing.add_entity(e);
                entity_count += 1;
            }
            entity_count += insert_count;
        }

        // Best-effort HATCH stub import for ASCII DXF:
        // current dxf crate version does not expose HATCH entities, so we parse
        // raw code pairs and convert boundary points into closed polylines.
        let hatch_stub_count =
            import_ascii_hatch_boundaries(path, &mut drawing, &mut name_to_id, &mut layer_count);
        entity_count += hatch_stub_count;

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
        EntityType::Insert(_) => "INSERT",
        EntityType::RotatedDimension(_)
        | EntityType::RadialDimension(_)
        | EntityType::DiameterDimension(_)
        | EntityType::AngularThreePointDimension(_)
        | EntityType::OrdinateDimension(_) => "DIMENSION",
        EntityType::Spline(_) => "SPLINE",
        EntityType::Ellipse(_) => "ELLIPSE",
        EntityType::Image(_) => "IMAGE",
        EntityType::Leader(_) => "LEADER",
        EntityType::Solid(_) => "SOLID",
        EntityType::Trace(_) => "TRACE",
        EntityType::Face3D(_) => "3DFACE",
        EntityType::Attribute(_) => "ATTRIB",
        EntityType::AttributeDefinition(_) => "ATTDEF",
        EntityType::Ray(_) => "RAY",
        EntityType::XLine(_) => "XLINE",
        EntityType::Body(_) => "BODY",
        EntityType::Region(_) => "REGION",
        _ => "UNSUPPORTED",
    }
}

fn parse_dimension_text_override(text: &str) -> Option<String> {
    let t = text.trim();
    if t.is_empty() || t == "<>" {
        None
    } else {
        Some(t.to_string())
    }
}

fn dxf_arc_angles_ccw_radians(start_deg: f64, end_deg: f64) -> (f64, f64) {
    let twopi = std::f64::consts::TAU;
    let sa = start_deg.to_radians().rem_euclid(twopi);
    let mut ea = end_deg.to_radians().rem_euclid(twopi);
    if ea <= sa {
        ea += twopi;
    }
    if (ea - sa).abs() < 1e-12 {
        // Guard degenerate imports where start==end by promoting to full-span CCW.
        ea = sa + twopi;
    }
    (sa, ea)
}

#[derive(Clone, Copy, Debug)]
struct Aff2 {
    m11: f64,
    m12: f64,
    m21: f64,
    m22: f64,
    tx: f64,
    ty: f64,
}

impl Aff2 {
    fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
    fn translate(dx: f64, dy: f64) -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: dx,
            ty: dy,
        }
    }
    fn scale(sx: f64, sy: f64) -> Self {
        Self {
            m11: sx,
            m12: 0.0,
            m21: 0.0,
            m22: sy,
            tx: 0.0,
            ty: 0.0,
        }
    }
    fn rotate(theta: f64) -> Self {
        let (s, c) = theta.sin_cos();
        Self {
            m11: c,
            m12: -s,
            m21: s,
            m22: c,
            tx: 0.0,
            ty: 0.0,
        }
    }
    fn compose(self, other: Self) -> Self {
        // self ∘ other
        Self {
            m11: self.m11 * other.m11 + self.m12 * other.m21,
            m12: self.m11 * other.m12 + self.m12 * other.m22,
            m21: self.m21 * other.m11 + self.m22 * other.m21,
            m22: self.m21 * other.m12 + self.m22 * other.m22,
            tx: self.m11 * other.tx + self.m12 * other.ty + self.tx,
            ty: self.m21 * other.tx + self.m22 * other.ty + self.ty,
        }
    }
    fn apply(&self, x: f64, y: f64) -> Vec3 {
        Vec3::xy(
            self.m11 * x + self.m12 * y + self.tx,
            self.m21 * x + self.m22 * y + self.ty,
        )
    }
    fn angle_of_x_axis(&self) -> f64 {
        self.m21.atan2(self.m11)
    }
}

fn resolve_layer_id_by_name(
    layer_name: &str,
    drawing: &mut Drawing,
    name_to_id: &mut HashMap<String, u32>,
    layer_count: &mut usize,
) -> u32 {
    if let Some(id) = name_to_id.get(layer_name).copied() {
        id
    } else {
        let new_id = drawing.add_layer_with_color(layer_name.to_string(), [255, 255, 255]);
        name_to_id.insert(layer_name.to_string(), new_id);
        *layer_count += 1;
        new_id
    }
}

fn sampled_circle_poly(xf: Aff2, center: &dxf::Point, radius: f64, steps: usize) -> Vec<Vec3> {
    let steps = steps.max(12);
    (0..steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let a = t * std::f64::consts::TAU;
            xf.apply(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect()
}

fn sampled_arc_poly(
    xf: Aff2,
    center: &dxf::Point,
    radius: f64,
    start_deg: f64,
    end_deg: f64,
    steps: usize,
) -> Vec<Vec3> {
    let (sa, ea) = dxf_arc_angles_ccw_radians(start_deg, end_deg);
    let steps = steps.max(8);
    (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let a = sa + (ea - sa) * t;
            xf.apply(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn expand_insert_flattened(
    ins: &dxf::entities::Insert,
    insert_layer_id: u32,
    _insert_color: Option<[u8; 3]>,
    insert_linetype: Linetype,
    blocks_by_name: &HashMap<String, dxf::Block>,
    drawing: &mut Drawing,
    name_to_id: &mut HashMap<String, u32>,
    layer_count: &mut usize,
    skipped: &mut BTreeSet<String>,
    parent_xf: Aff2,
    depth: u8,
) -> usize {
    if depth > 8 {
        skipped.insert("INSERT(depth)".to_string());
        return 0;
    }
    let Some(block) = blocks_by_name.get(&ins.name) else {
        skipped.insert("INSERT(missing-block)".to_string());
        return 0;
    };

    let cols = ins.column_count.max(1) as usize;
    let rows = ins.row_count.max(1) as usize;
    let sx = if ins.x_scale_factor.abs() < 1e-12 {
        1.0
    } else {
        ins.x_scale_factor
    };
    let sy = if ins.y_scale_factor.abs() < 1e-12 {
        1.0
    } else {
        ins.y_scale_factor
    };
    let rot = ins.rotation.to_radians();

    let mut added = 0usize;
    for r in 0..rows {
        for c in 0..cols {
            let local_offset_x = c as f64 * ins.column_spacing;
            let local_offset_y = r as f64 * ins.row_spacing;

            let local_to_insert = Aff2::translate(ins.location.x, ins.location.y)
                .compose(Aff2::rotate(rot))
                .compose(Aff2::scale(sx, sy))
                .compose(Aff2::translate(
                    -block.base_point.x + local_offset_x,
                    -block.base_point.y + local_offset_y,
                ));
            let xf = parent_xf.compose(local_to_insert);

            for be in &block.entities {
                let layer_id = if be.common.layer.trim() == "0" || be.common.layer.trim().is_empty()
                {
                    insert_layer_id
                } else {
                    resolve_layer_id_by_name(&be.common.layer, drawing, name_to_id, layer_count)
                };
                let color = if be.common.color.is_by_layer() {
                    None
                } else {
                    be.common.color.index().map(aci_to_rgb)
                };

                let kind_opt: Option<EntityKind> = match &be.specific {
                    EntityType::Line(line) => Some(EntityKind::Line {
                        start: xf.apply(line.p1.x, line.p1.y),
                        end: xf.apply(line.p2.x, line.p2.y),
                    }),
                    EntityType::LwPolyline(poly) => {
                        let verts: Vec<Vec3> =
                            poly.vertices.iter().map(|v| xf.apply(v.x, v.y)).collect();
                        if verts.len() >= 2 {
                            Some(EntityKind::Polyline {
                                vertices: verts,
                                closed: (poly.flags & 1) != 0,
                            })
                        } else {
                            None
                        }
                    }
                    EntityType::Polyline(poly) => {
                        let verts: Vec<Vec3> = poly
                            .vertices()
                            .map(|v| xf.apply(v.location.x, v.location.y))
                            .collect();
                        if verts.len() >= 2 {
                            Some(EntityKind::Polyline {
                                vertices: verts,
                                closed: (poly.flags & 1) != 0,
                            })
                        } else {
                            None
                        }
                    }
                    EntityType::Circle(circle) => {
                        let verts = sampled_circle_poly(xf, &circle.center, circle.radius, 96);
                        Some(EntityKind::Polyline {
                            vertices: verts,
                            closed: true,
                        })
                    }
                    EntityType::Arc(arc) => {
                        let verts = sampled_arc_poly(
                            xf,
                            &arc.center,
                            arc.radius,
                            arc.start_angle,
                            arc.end_angle,
                            64,
                        );
                        if verts.len() >= 2 {
                            Some(EntityKind::Polyline {
                                vertices: verts,
                                closed: false,
                            })
                        } else {
                            None
                        }
                    }
                    EntityType::Text(t) => {
                        let p = xf.apply(t.location.x, t.location.y);
                        let sx_len = (xf.m11 * xf.m11 + xf.m21 * xf.m21).sqrt();
                        let sy_len = (xf.m12 * xf.m12 + xf.m22 * xf.m22).sqrt();
                        let h_scale = ((sx_len + sy_len) * 0.5).max(1e-9);
                        Some(EntityKind::Text {
                            position: p,
                            content: t.value.clone(),
                            height: (t.text_height * h_scale).max(0.01),
                            rotation: t.rotation.to_radians() + xf.angle_of_x_axis(),
                            font_name: if t.text_style_name.trim().is_empty() {
                                "STANDARD".to_string()
                            } else {
                                t.text_style_name.clone()
                            },
                        })
                    }
                    EntityType::MText(t) => {
                        let raw = if t.text.is_empty() {
                            t.extended_text.join("")
                        } else if t.extended_text.is_empty() {
                            t.text.clone()
                        } else {
                            let mut s = t.extended_text.join("");
                            s.push_str(&t.text);
                            s
                        };
                        let content = raw.replace("\\P", "\n").replace("\\p", "\n");
                        let p = xf.apply(t.insertion_point.x, t.insertion_point.y);
                        let sx_len = (xf.m11 * xf.m11 + xf.m21 * xf.m21).sqrt();
                        let sy_len = (xf.m12 * xf.m12 + xf.m22 * xf.m22).sqrt();
                        let h_scale = ((sx_len + sy_len) * 0.5).max(1e-9);
                        Some(EntityKind::Text {
                            position: p,
                            content,
                            height: (t.initial_text_height * h_scale).max(0.01),
                            rotation: t.rotation_angle.to_radians() + xf.angle_of_x_axis(),
                            font_name: if t.text_style_name.trim().is_empty() {
                                "STANDARD".to_string()
                            } else {
                                t.text_style_name.clone()
                            },
                        })
                    }
                    EntityType::Insert(child_insert) => {
                        added += expand_insert_flattened(
                            child_insert,
                            layer_id,
                            color,
                            insert_linetype,
                            blocks_by_name,
                            drawing,
                            name_to_id,
                            layer_count,
                            skipped,
                            xf,
                            depth + 1,
                        );
                        None
                    }
                    other => {
                        skipped.insert(format!("INSERT child {}", dxf_type_name(other)));
                        None
                    }
                };

                if let Some(kind) = kind_opt {
                    drawing.add_entity(Entity {
                        id: Guid::new(),
                        kind,
                        layer: layer_id,
                        color,
                        linetype: insert_linetype,
                        linetype_by_layer: false,
                        linetype_scale: None,
                        block_params: crate::BlockParamValues::default(),
                        insert_dynamic_param_overrides: std::collections::HashMap::new(),
                    });
                    added += 1;
                }
            }
        }
    }
    added
}

fn import_ascii_hatch_boundaries(
    path: &str,
    drawing: &mut Drawing,
    name_to_id: &mut HashMap<String, u32>,
    layer_count: &mut usize,
) -> usize {
    let Ok(text) = std::fs::read_to_string(path) else {
        return 0;
    };
    if !text.contains("\nHATCH") && !text.contains("\r\nHATCH") {
        return 0;
    }

    let pairs = parse_ascii_dxf_code_pairs(&text);
    if pairs.is_empty() {
        return 0;
    }

    let mut imported = 0usize;
    let mut in_entities = false;
    let mut i = 0usize;
    while i < pairs.len() {
        let (code, val) = &pairs[i];
        if *code == 0 && val == "SECTION" {
            if i + 1 < pairs.len() && pairs[i + 1].0 == 2 && pairs[i + 1].1 == "ENTITIES" {
                in_entities = true;
            }
            i += 1;
            continue;
        }
        if in_entities && *code == 0 && val == "ENDSEC" {
            in_entities = false;
            i += 1;
            continue;
        }

        if in_entities && *code == 0 && val == "HATCH" {
            let start = i + 1;
            let mut end = start;
            while end < pairs.len() {
                if pairs[end].0 == 0 {
                    break;
                }
                end += 1;
            }
            if let Some(entity) =
                hatch_pairs_to_polyline(&pairs[start..end], drawing, name_to_id, layer_count)
            {
                drawing.add_entity(entity);
                imported += 1;
            }
            i = end;
            continue;
        }
        i += 1;
    }

    imported
}

fn parse_ascii_dxf_code_pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::with_capacity(lines.len() / 2);
    let mut i = 0usize;
    while i + 1 < lines.len() {
        let code_s = lines[i].trim();
        let value = lines[i + 1].trim().to_string();
        if let Ok(code) = code_s.parse::<i32>() {
            out.push((code, value));
        }
        i += 2;
    }
    out
}

fn hatch_pairs_to_polyline(
    pairs: &[(i32, String)],
    drawing: &mut Drawing,
    name_to_id: &mut HashMap<String, u32>,
    layer_count: &mut usize,
) -> Option<Entity> {
    let mut layer_name = "0".to_string();
    let mut color: Option<[u8; 3]> = None;
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut pending_x: Option<f64> = None;

    for (code, val) in pairs {
        match *code {
            8 => {
                if !val.is_empty() {
                    layer_name = val.clone();
                }
            }
            62 => {
                if let Ok(raw) = val.parse::<i16>() {
                    let idx = raw.unsigned_abs() as u16;
                    if idx != 0 && idx != 256 && idx <= 255 {
                        color = Some(aci_to_rgb(idx as u8));
                    }
                }
            }
            10 => {
                pending_x = val.parse::<f64>().ok();
            }
            20 => {
                if let (Some(x), Ok(y)) = (pending_x.take(), val.parse::<f64>()) {
                    pts.push((x, y));
                }
            }
            _ => {}
        }
    }

    // Coalesce consecutive duplicates from noisy pair streams.
    let mut verts_xy: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for p in pts {
        let is_dup = verts_xy
            .last()
            .map(|q| (q.0 - p.0).abs() <= 1e-9 && (q.1 - p.1).abs() <= 1e-9)
            .unwrap_or(false);
        if !is_dup {
            verts_xy.push(p);
        }
    }
    if verts_xy.len() < 3 {
        return None;
    }

    let layer_id = if let Some(id) = name_to_id.get(&layer_name).copied() {
        id
    } else {
        let new_id = drawing.add_layer_with_color(layer_name.clone(), [255, 255, 255]);
        name_to_id.insert(layer_name, new_id);
        *layer_count += 1;
        new_id
    };

    Some(Entity {
        id: Guid::new(),
        kind: EntityKind::Polyline {
            vertices: verts_xy.into_iter().map(|(x, y)| Vec3::xy(x, y)).collect(),
            closed: true,
        },
        layer: layer_id,
        color,
        linetype: Linetype::Continuous,
        linetype_by_layer: false,
        linetype_scale: None,
        block_params: crate::BlockParamValues::default(),
        insert_dynamic_param_overrides: std::collections::HashMap::new(),
    })
}
