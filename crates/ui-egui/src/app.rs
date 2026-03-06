//! CadKit - Main application entry point

use cadkit_2d_core::{create_arc, create_circle, create_line, Drawing, Entity, EntityKind};
// create_arc_from_three_points helper lives below in this file (UI layer-specific).
use cadkit_render_wgpu::{screen_to_world, world_to_screen, Viewport};
use cadkit_types::{Guid, Vec2, Vec3};
use cadkit_geometry::{Circle as GeomCircle, Line as GeomLine};
use eframe::egui;
use egui_wgpu::wgpu;
use std::collections::{HashMap, HashSet};

mod io;
mod ui_panels;
mod overlays;
mod commands;
mod state;
use state::*;

// ── Angular dimension geometry helpers ──────────────────────────────────────

/// Normalise two ray angles so the CCW sweep from a1 to a2 is in (0, 2π].
fn angular_arc_angles(vertex: cadkit_types::Vec2, p1: cadkit_types::Vec2, p2: cadkit_types::Vec2) -> (f64, f64) {
    use std::f64::consts::TAU;
    let a1 = (p1.y - vertex.y).atan2(p1.x - vertex.x);
    let mut a2 = (p2.y - vertex.y).atan2(p2.x - vertex.x);
    if a2 <= a1 { a2 += TAU; }
    (a1, a2)
}

/// Build world-space arc points for a DimAngular arc.
fn angular_arc_pts(vertex: cadkit_types::Vec2, a1: f64, a2: f64, radius: f64) -> Vec<cadkit_types::Vec2> {
    let sweep = a2 - a1; // always positive
    let steps = ((sweep * radius).abs().max(6.0) as usize).clamp(12, 96);
    (0..=steps).map(|i| {
        let t = i as f64 / steps as f64;
        let a = a1 + sweep * t;
        cadkit_types::Vec2::new(vertex.x + radius * a.cos(), vertex.y + radius * a.sin())
    }).collect()
}

/// Compute the intersection of two infinite lines defined by segments a→b and c→d.
/// Returns `None` when the lines are parallel (cross-product ≈ 0).
fn line_line_intersect(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let dax = b.x - a.x;  let day = b.y - a.y;
    let dcx = d.x - c.x;  let dcy = d.y - c.y;
    let denom = dax * dcy - day * dcx;
    if denom.abs() < 1e-10 { return None; }
    let t = ((c.x - a.x) * dcy - (c.y - a.y) * dcx) / denom;
    Some(Vec2::new(a.x + t * dax, a.y + t * day))
}

/// Given a line segment start→end, a click position, and the computed vertex, return
/// a direction-indicator point on the correct ray from vertex (the side the user clicked on).
fn ray_dir_from_vertex(seg_start: Vec2, seg_end: Vec2, click: Vec2, vertex: Vec2) -> Vec2 {
    let dx = seg_end.x - seg_start.x;
    let dy = seg_end.y - seg_start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-10 { return click; }
    let dir_x = dx / len;
    let dir_y = dy / len;
    // Choose the direction from vertex that faces toward the click.
    let to_cx = click.x - vertex.x;
    let to_cy = click.y - vertex.y;
    let dot = to_cx * dir_x + to_cy * dir_y;
    let (fx, fy) = if dot >= 0.0 { (dir_x, dir_y) } else { (-dir_x, -dir_y) };
    let dist = (to_cx * to_cx + to_cy * to_cy).sqrt().max(1.0);
    Vec2::new(vertex.x + fx * dist, vertex.y + fy * dist)
}

/// Squared distance from point `p` to segment `a`→`b`.
fn point_seg_dist2(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let dx = b.x - a.x;  let dy = b.y - a.y;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-20 {
        let ex = p.x - a.x;  let ey = p.y - a.y;
        return ex * ex + ey * ey;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    let cx = a.x + t * dx;  let cy = a.y + t * dy;
    let ex = p.x - cx;  let ey = p.y - cy;
    ex * ex + ey * ey
}

/// Given an entity kind and a click world position, return the nearest line segment (start, end).
/// Supports Line and Polyline entities only; returns `None` otherwise.
fn dim_angular_pick_segment(kind: &EntityKind, click: Vec2) -> Option<(Vec2, Vec2)> {
    match kind {
        EntityKind::Line { start, end } => {
            Some((Vec2::new(start.x, start.y), Vec2::new(end.x, end.y)))
        }
        EntityKind::Polyline { vertices, .. } if vertices.len() >= 2 => {
            let mut best_d2 = f64::INFINITY;
            let mut best = None;
            for seg in vertices.windows(2) {
                let a = Vec2::new(seg[0].x, seg[0].y);
                let b = Vec2::new(seg[1].x, seg[1].y);
                let d2 = point_seg_dist2(click, a, b);
                if d2 < best_d2 {
                    best_d2 = d2;
                    best = Some((a, b));
                }
            }
            best
        }
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct AppPrefs {
    pub snap_enabled: bool,
    pub ortho_enabled: bool,
    pub grid_visible: bool,
    pub grid_spacing: f64,
    pub current_file: Option<String>,
    pub recent_files: Vec<String>,
    pub dim_style: DimStyle,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            snap_enabled: true,
            ortho_enabled: true,
            grid_visible: true,
            grid_spacing: 12.0,
            current_file: None,
            recent_files: Vec::new(),
            dim_style: DimStyle::default(),
        }
    }
}

#[derive(Clone, Debug)]
struct AssocArraySource {
    kind: EntityKind,
    layer: u32,
    color: Option<[u8; 3]>,
}

#[derive(Clone, Debug)]
struct AssocRectArray {
    id: Guid,
    members: Vec<Guid>,
    source: Vec<AssocArraySource>,
    base: Vec2,
    direction: Vec2,
    cols: usize,
    rows: usize,
    dx: f64,
    dy: f64,
}

pub struct CadKitApp {
    drawing: Drawing,
    command_input: String,
    viewport: Option<Viewport>,
    viewport_texture_id: Option<egui::TextureId>,
    viewport_init_error: Option<String>,
    hover_world_pos: Option<cadkit_types::Vec2>,
    last_hover_world_pos: Option<cadkit_types::Vec2>,
    snap_enabled: bool,
    grid_visible: bool,
    grid_spacing: f64,
    active_tool: ActiveTool,
    selection: Option<Selection>,
    selected_entities: HashSet<Guid>,
    selection_drag_start: Option<egui::Pos2>,
    selection_drag_current: Option<egui::Pos2>,
    dim_grip_drag: Option<DimGripHandle>,
    dim_grip_is_dragging: bool,
    hover_dim_grip: Option<DimGripHandle>,
    ortho_enabled: bool,
    ortho_increment_deg: f64,
    distance_input: String,
    circle_use_diameter: bool,
    command_log: Vec<String>,
    snap_intersection_point: Option<Vec2>,
    hover_snap_kind: Option<SnapKind>,
    trim_cutting_edges: Vec<Guid>,
    trim_phase: TrimPhase,
    offset_distance: Option<f64>,
    offset_phase: OffsetPhase,
    offset_selected_entity: Option<Guid>,
    move_phase: MovePhase,
    move_base_point: Option<Vec2>,
    move_entities: Vec<Guid>,
    extend_phase: ExtendPhase,
    extend_boundary_edges: Vec<Guid>,
    copy_phase: CopyPhase,
    copy_base_point: Option<Vec2>,
    copy_entities: Vec<Guid>,
    rotate_phase: RotatePhase,
    rotate_base_point: Option<Vec2>,
    rotate_entities: Vec<Guid>,
    scale_phase: ScalePhase,
    scale_base_point: Option<Vec2>,
    scale_ref_point: Option<Vec2>,
    scale_entities: Vec<Guid>,
    mirror_phase: MirrorPhase,
    mirror_axis_p1: Option<Vec2>,
    mirror_entities: Vec<Guid>,
    fillet_phase: FilletPhase,
    fillet_radius: f64,
    chamfer_phase: ChamferPhase,
    chamfer_distance1: f64,
    chamfer_distance2: f64,
    polygon_phase: PolygonPhase,
    polygon_sides: usize,
    ellipse_phase: EllipsePhase,
    rectangle_phase: RectanglePhase,
    rectangle_width: f64,
    rectangle_height: f64,
    array_phase: ArrayPhase,
    array_mode: ArrayMode,
    array_entities: Vec<Guid>,
    array_rect_columns: usize,
    array_rect_rows: usize,
    array_rect_dx: f64,
    array_rect_dy: f64,
    array_rect_dir_point: Option<Vec2>,
    array_rect_y_sign: f64,
    array_polar_count: usize,
    array_polar_angle_deg: f64,
    array_center: Option<Vec2>,
    assoc_rect_arrays: HashMap<Guid, AssocRectArray>,
    assoc_member_to_array: HashMap<Guid, Guid>,
    array_edit_assoc: Option<Guid>,
    pedit_phase: PeditPhase,
    from_phase: FromPhase,
    from_base: Option<Vec2>,
    dim_phase: DimPhase,
    dim_linear_phase: DimLinearPhase,
    dim_angular_phase: DimAngularPhase,
    dim_radial_phase: DimRadialPhase,
    text_phase: TextPhase,
    last_text_height: f64,
    last_text_rotation: f64,
    edit_text_phase: EditTextPhase,
    text_edit_dialog: Option<TextEditDialog>,
    edit_dim_phase: EditDimPhase,
    dim_edit_dialog: Option<DimEditDialog>,
    dim_style: DimStyle,
    dim_style_dialog: Option<DimStyleDialog>,
    current_file: Option<String>,
    recent_files: Vec<String>,
    // Layer management
    current_layer: u32,
    next_layer_number: u32,
    layer_color_picking: Option<u32>,
    layer_editing_id: Option<u32>,
    layer_editing_text: String,
    layer_editing_original: String,
    // Properties panel
    properties_split: f32,      // fraction of right-panel height given to layers list
    entity_color_picker_open: bool,
    // Deferred DXF import (needs ctx, triggered by command alias)
    pending_dxf_import: bool,
    undo_stack: Vec<Drawing>,
    redo_stack: Vec<Drawing>,
    help_open: bool,
    bgcolor_picker_open: bool,
    last_saved_prefs: Option<AppPrefs>,
}

impl Default for CadKitApp {
    fn default() -> Self {
        let drawing = Drawing::new("New Drawing".to_string());

        let mut app = Self {
            drawing,
            command_input: String::new(),
            viewport: None,
            viewport_texture_id: None,
            viewport_init_error: None,
            hover_world_pos: None,
            last_hover_world_pos: None,
            snap_enabled: true,
            grid_visible: true,
            grid_spacing: 12.0,
            active_tool: ActiveTool::None,
            selection: None,
            selected_entities: HashSet::new(),
            selection_drag_start: None,
            selection_drag_current: None,
            dim_grip_drag: None,
            dim_grip_is_dragging: false,
            hover_dim_grip: None,
            ortho_enabled: true,
            ortho_increment_deg: 90.0,
            distance_input: String::new(),
            circle_use_diameter: false,
            command_log: Vec::new(),
            snap_intersection_point: None,
            hover_snap_kind: None,
            trim_cutting_edges: Vec::new(),
            trim_phase: TrimPhase::Idle,
            offset_distance: None,
            offset_phase: OffsetPhase::Idle,
            offset_selected_entity: None,
            move_phase: MovePhase::Idle,
            move_base_point: None,
            move_entities: Vec::new(),
            extend_phase: ExtendPhase::Idle,
            extend_boundary_edges: Vec::new(),
            copy_phase: CopyPhase::Idle,
            copy_base_point: None,
            copy_entities: Vec::new(),
            rotate_phase: RotatePhase::Idle,
            rotate_base_point: None,
            rotate_entities: Vec::new(),
            scale_phase: ScalePhase::Idle,
            scale_base_point: None,
            scale_ref_point: None,
            scale_entities: Vec::new(),
            mirror_phase: MirrorPhase::Idle,
            mirror_axis_p1: None,
            mirror_entities: Vec::new(),
            fillet_phase: FilletPhase::Idle,
            fillet_radius: 5.0,
            chamfer_phase: ChamferPhase::Idle,
            chamfer_distance1: 5.0,
            chamfer_distance2: 5.0,
            polygon_phase: PolygonPhase::Idle,
            polygon_sides: 6,
            ellipse_phase: EllipsePhase::Idle,
            rectangle_phase: RectanglePhase::Idle,
            rectangle_width: 10.0,
            rectangle_height: 10.0,
            array_phase: ArrayPhase::Idle,
            array_mode: ArrayMode::Rectangular,
            array_entities: Vec::new(),
            array_rect_columns: 4,
            array_rect_rows: 3,
            array_rect_dx: 10.0,
            array_rect_dy: 10.0,
            array_rect_dir_point: None,
            array_rect_y_sign: 1.0,
            array_polar_count: 6,
            array_polar_angle_deg: 360.0,
            array_center: None,
            assoc_rect_arrays: HashMap::new(),
            assoc_member_to_array: HashMap::new(),
            array_edit_assoc: None,
            pedit_phase: PeditPhase::Idle,
            from_phase: FromPhase::Idle,
            from_base: None,
            dim_phase: DimPhase::Idle,
            dim_linear_phase: DimLinearPhase::Idle,
            dim_angular_phase: DimAngularPhase::Idle,
            dim_radial_phase: DimRadialPhase::Idle,
            text_phase: TextPhase::Idle,
            last_text_height: 2.5,
            last_text_rotation: 0.0,
            edit_text_phase: EditTextPhase::Idle,
            text_edit_dialog: None,
            edit_dim_phase: EditDimPhase::Idle,
            dim_edit_dialog: None,
            dim_style: DimStyle::default(),
            dim_style_dialog: None,
            current_file: None,
            recent_files: Vec::new(),
            current_layer: 0,
            next_layer_number: 1,
            layer_color_picking: None,
            layer_editing_id: None,
            layer_editing_text: String::new(),
            layer_editing_original: String::new(),
            properties_split: 0.55,
            entity_color_picker_open: false,
            pending_dxf_import: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            help_open: false,
            bgcolor_picker_open: false,
            last_saved_prefs: None,
        };
        app.load_preferences();
        app
    }
}

impl CadKitApp {
    const UNDO_LIMIT: usize = 50;
    const PAN_SENSITIVITY: f32 = 0.3;
    const GRID_MAX_POINTS: usize = 20_000;
    const PICK_RADIUS: f32 = 16.0; // screen-space pixels
    pub(crate) const DIM_GRIP_RADIUS: f32 = 7.0;
    const GEOM_TOL: f64 = 1e-9;

    pub(crate) fn dim_grip_points(kind: &EntityKind) -> Option<Vec<(DimGripKind, Vec2)>> {
        match kind {
            EntityKind::DimAligned { start, end, offset, text_pos, .. } => {
                let sx = start.x;
                let sy = start.y;
                let ex = end.x;
                let ey = end.y;
                let ddx = ex - sx;
                let ddy = ey - sy;
                let len = (ddx * ddx + ddy * ddy).sqrt();
                if len < 1e-9 {
                    return None;
                }
                let perp = Vec2::new(-ddy / len, ddx / len);
                let mid = Vec2::new((sx + ex) * 0.5, (sy + ey) * 0.5);
                let offset_grip = Vec2::new(mid.x + perp.x * *offset, mid.y + perp.y * *offset);
                let text_grip = Vec2::new(text_pos.x, text_pos.y);
                Some(vec![
                    (DimGripKind::Start, Vec2::new(sx, sy)),
                    (DimGripKind::End, Vec2::new(ex, ey)),
                    (DimGripKind::Offset, offset_grip),
                    (DimGripKind::Text, text_grip),
                ])
            }
            EntityKind::DimLinear { start, end, offset, text_pos, horizontal, .. } => {
                let mid_x = (start.x + end.x) * 0.5;
                let mid_y = (start.y + end.y) * 0.5;
                let offset_grip = if *horizontal {
                    Vec2::new(mid_x, mid_y + *offset)
                } else {
                    Vec2::new(mid_x + *offset, mid_y)
                };
                let text_grip = Vec2::new(text_pos.x, text_pos.y);
                Some(vec![
                    (DimGripKind::Start, Vec2::new(start.x, start.y)),
                    (DimGripKind::End, Vec2::new(end.x, end.y)),
                    (DimGripKind::Offset, offset_grip),
                    (DimGripKind::Text, text_grip),
                ])
            }
            EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, text_pos, .. } => {
                let v = Vec2::new(vertex.x, vertex.y);
                let p1 = Vec2::new(line1_pt.x, line1_pt.y);
                let p2 = Vec2::new(line2_pt.x, line2_pt.y);
                let d1x = p1.x - v.x; let d1y = p1.y - v.y;
                let d2x = p2.x - v.x; let d2y = p2.y - v.y;
                let l1 = (d1x * d1x + d1y * d1y).sqrt();
                let l2 = (d2x * d2x + d2y * d2y).sqrt();
                if l1 < 1e-9 || l2 < 1e-9 { return None; }
                let arc_pt1 = Vec2::new(v.x + d1x / l1 * radius, v.y + d1y / l1 * radius);
                let arc_pt2 = Vec2::new(v.x + d2x / l2 * radius, v.y + d2y / l2 * radius);
                let (a1, a2) = angular_arc_angles(v, p1, p2);
                let mid_angle = (a1 + a2) * 0.5;
                let offset_grip = Vec2::new(v.x + radius * mid_angle.cos(), v.y + radius * mid_angle.sin());
                let text_grip = Vec2::new(text_pos.x, text_pos.y);
                Some(vec![
                    (DimGripKind::Start, arc_pt1),
                    (DimGripKind::End, arc_pt2),
                    (DimGripKind::Offset, offset_grip),
                    (DimGripKind::Text, text_grip),
                ])
            }
            EntityKind::DimRadial { center, radius, leader_pt, text_pos, .. } => {
                let cx = center.x; let cy = center.y;
                let dx = leader_pt.x - cx; let dy = leader_pt.y - cy;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-9 { return None; }
                let ux = dx / len; let uy = dy / len;
                // Offset grip = arrowhead position (on circle edge in leader direction)
                let offset_grip = Vec2::new(cx + ux * radius, cy + uy * radius);
                let text_grip = Vec2::new(text_pos.x, text_pos.y);
                Some(vec![
                    (DimGripKind::Offset, offset_grip),
                    (DimGripKind::Text, text_grip),
                ])
            }
            _ => None,
        }
    }

    pub(crate) fn dim_grip_display_points(
        kind: &EntityKind,
        viewport: &Viewport,
        rect: egui::Rect,
    ) -> Option<Vec<(DimGripKind, egui::Pos2)>> {
        let grips = Self::dim_grip_points(kind)?;
        let mut points: Vec<(DimGripKind, egui::Pos2)> = grips
            .into_iter()
            .map(|(kind, world)| {
                let (sx, sy) = world_to_screen(world.x as f32, world.y as f32, viewport);
                (kind, rect.min + egui::vec2(sx, sy))
            })
            .collect();

        let off_idx = points.iter().position(|(k, _)| *k == DimGripKind::Offset);
        let txt_idx = points.iter().position(|(k, _)| *k == DimGripKind::Text);
        if let (Some(oi), Some(ti)) = (off_idx, txt_idx) {
            let mut offset_pos = points[oi].1;
            let mut text_pos = points[ti].1;
            if offset_pos.distance(text_pos) < 8.0 {
            let mut n = egui::vec2(0.0, -1.0);
            match kind {
                EntityKind::DimAligned { start, end, .. } => {
                    let (sx, sy) = world_to_screen(start.x as f32, start.y as f32, viewport);
                    let (ex, ey) = world_to_screen(end.x as f32, end.y as f32, viewport);
                    let dir = egui::vec2(ex - sx, ey - sy);
                    let len = dir.length();
                    if len > f32::EPSILON {
                        n = egui::vec2(-dir.y / len, dir.x / len);
                    }
                }
                EntityKind::DimLinear { horizontal, .. } => {
                    n = if *horizontal {
                        egui::vec2(0.0, -1.0)
                    } else {
                        egui::vec2(1.0, 0.0)
                    };
                }
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, .. } => {
                    let v = Vec2::new(vertex.x, vertex.y);
                    let p1 = Vec2::new(line1_pt.x, line1_pt.y);
                    let p2 = Vec2::new(line2_pt.x, line2_pt.y);
                    let (a1, a2) = angular_arc_angles(v, p1, p2);
                    let mid_angle = ((a1 + a2) * 0.5) as f32;
                    // Perpendicular to the radial direction for separation
                    n = egui::vec2(-mid_angle.sin(), mid_angle.cos());
                }
                EntityKind::DimRadial { center, leader_pt, .. } => {
                    let dx = (leader_pt.x - center.x) as f32;
                    let dy = (leader_pt.y - center.y) as f32;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > f32::EPSILON {
                        // Perpendicular to leader direction
                        n = egui::vec2(-dy / len, dx / len);
                    }
                }
                _ => {}
            }
            let center = offset_pos + (text_pos - offset_pos) * 0.5;
            let sep = 8.0;
            offset_pos = center + n * sep;
            text_pos = center - n * sep;
            points[oi].1 = offset_pos;
            points[ti].1 = text_pos;
            }
        }

        Some(points)
    }

    fn pick_dim_grip_handle(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<DimGripHandle> {
        let mut best: Option<(f32, DimGripHandle)> = None;
        for entity in self.drawing.visible_entities() {
            if !self.selected_entities.contains(&entity.id) {
                continue;
            }
            let Some(points) = Self::dim_grip_display_points(&entity.kind, viewport, rect) else {
                continue;
            };
            for (kind, pos) in points {
                let dist = pos.distance(screen_pos);
                if dist <= Self::DIM_GRIP_RADIUS + 3.0 {
                    match best {
                        Some((best_dist, _)) if dist >= best_dist => {}
                        _ => {
                            best = Some((dist, DimGripHandle { entity: entity.id, kind }));
                        }
                    }
                }
            }
        }
        best.map(|(_, handle)| handle)
    }

    fn apply_dim_grip_drag(&mut self, handle: DimGripHandle, world: Vec2) {
        let Some(entity) = self.drawing.get_entity_mut(&handle.entity) else {
            return;
        };
        match &mut entity.kind {
            EntityKind::DimAligned { start, end, offset, text_pos, .. } => {
                let sx = start.x;
                let sy = start.y;
                let ex = end.x;
                let ey = end.y;
                let ddx = ex - sx;
                let ddy = ey - sy;
                let len = (ddx * ddx + ddy * ddy).sqrt();
                if len < 1e-9 {
                    return;
                }
                let perp = Vec2::new(-ddy / len, ddx / len);
                let mid = Vec2::new((sx + ex) * 0.5, (sy + ey) * 0.5);
                match handle.kind {
                    DimGripKind::Start => {
                        let old_mid = mid;
                        start.x = world.x;
                        start.y = world.y;
                        let new_mid = Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
                        text_pos.x += new_mid.x - old_mid.x;
                        text_pos.y += new_mid.y - old_mid.y;
                    }
                    DimGripKind::End => {
                        let old_mid = mid;
                        end.x = world.x;
                        end.y = world.y;
                        let new_mid = Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
                        text_pos.x += new_mid.x - old_mid.x;
                        text_pos.y += new_mid.y - old_mid.y;
                    }
                    DimGripKind::Offset => {
                        let new_offset = (world.x - mid.x) * perp.x + (world.y - mid.y) * perp.y;
                        let delta = new_offset - *offset;
                        *offset = new_offset;
                        text_pos.x += perp.x * delta;
                        text_pos.y += perp.y * delta;
                    }
                    DimGripKind::Text => {
                        text_pos.x = world.x;
                        text_pos.y = world.y;
                    }
                }
            }
            EntityKind::DimLinear { start, end, offset, text_pos, horizontal, .. } => {
                let mid_x = (start.x + end.x) * 0.5;
                let mid_y = (start.y + end.y) * 0.5;
                match handle.kind {
                    DimGripKind::Start => {
                        let old_mid = Vec2::new(mid_x, mid_y);
                        start.x = world.x;
                        start.y = world.y;
                        let new_mid = Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
                        text_pos.x += new_mid.x - old_mid.x;
                        text_pos.y += new_mid.y - old_mid.y;
                    }
                    DimGripKind::End => {
                        let old_mid = Vec2::new(mid_x, mid_y);
                        end.x = world.x;
                        end.y = world.y;
                        let new_mid = Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
                        text_pos.x += new_mid.x - old_mid.x;
                        text_pos.y += new_mid.y - old_mid.y;
                    }
                    DimGripKind::Offset => {
                        if *horizontal {
                            let new_offset = world.y - mid_y;
                            let delta = new_offset - *offset;
                            *offset = new_offset;
                            text_pos.y += delta;
                        } else {
                            let new_offset = world.x - mid_x;
                            let delta = new_offset - *offset;
                            *offset = new_offset;
                            text_pos.x += delta;
                        }
                    }
                    DimGripKind::Text => {
                        text_pos.x = world.x;
                        text_pos.y = world.y;
                    }
                }
            }
            EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, text_pos, .. } => {
                let v = Vec2::new(vertex.x, vertex.y);
                let p1_cur = Vec2::new(line1_pt.x, line1_pt.y);
                let p2_cur = Vec2::new(line2_pt.x, line2_pt.y);
                let (a1, a2) = angular_arc_angles(v, p1_cur, p2_cur);
                let mid_angle = (a1 + a2) * 0.5;
                let mx = mid_angle.cos();
                let my = mid_angle.sin();
                match handle.kind {
                    DimGripKind::Start => {
                        line1_pt.x = world.x;
                        line1_pt.y = world.y;
                    }
                    DimGripKind::End => {
                        line2_pt.x = world.x;
                        line2_pt.y = world.y;
                    }
                    DimGripKind::Offset => {
                        let dx = world.x - v.x;
                        let dy = world.y - v.y;
                        let new_radius = (dx * mx + dy * my).max(1.0);
                        let delta = new_radius - *radius;
                        *radius = new_radius;
                        text_pos.x += mx * delta;
                        text_pos.y += my * delta;
                    }
                    DimGripKind::Text => {
                        text_pos.x = world.x;
                        text_pos.y = world.y;
                    }
                }
            }
            EntityKind::DimRadial { center, leader_pt, text_pos, .. } => {
                match handle.kind {
                    DimGripKind::Offset => {
                        // Drag arrowhead = move leader direction
                        let old_lx = leader_pt.x; let old_ly = leader_pt.y;
                        leader_pt.x = world.x; leader_pt.y = world.y;
                        text_pos.x += leader_pt.x - old_lx;
                        text_pos.y += leader_pt.y - old_ly;
                        let _ = center;
                    }
                    DimGripKind::Text => {
                        text_pos.x = world.x;
                        text_pos.y = world.y;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn constrained_dim_grip_world(&self, handle: DimGripHandle, world: Vec2) -> Vec2 {
        if handle.kind != DimGripKind::Offset {
            return world;
        }
        let Some(entity) = self.drawing.get_entity(&handle.entity) else {
            return world;
        };
        match &entity.kind {
            EntityKind::DimAligned { start, end, .. } => {
                let sx = start.x;
                let sy = start.y;
                let ex = end.x;
                let ey = end.y;
                let ddx = ex - sx;
                let ddy = ey - sy;
                let len = (ddx * ddx + ddy * ddy).sqrt();
                if len < 1e-9 {
                    return world;
                }
                let perp = Vec2::new(-ddy / len, ddx / len);
                let mid = Vec2::new((sx + ex) * 0.5, (sy + ey) * 0.5);
                let d = Vec2::new(world.x - mid.x, world.y - mid.y);
                let t = d.x * perp.x + d.y * perp.y;
                Vec2::new(mid.x + perp.x * t, mid.y + perp.y * t)
            }
            EntityKind::DimLinear { start, end, horizontal, .. } => {
                let mid_x = (start.x + end.x) * 0.5;
                let mid_y = (start.y + end.y) * 0.5;
                if *horizontal {
                    Vec2::new(mid_x, world.y)
                } else {
                    Vec2::new(world.x, mid_y)
                }
            }
            EntityKind::DimAngular { vertex, line1_pt, line2_pt, .. } => {
                let v = Vec2::new(vertex.x, vertex.y);
                let p1 = Vec2::new(line1_pt.x, line1_pt.y);
                let p2 = Vec2::new(line2_pt.x, line2_pt.y);
                let (a1, a2) = angular_arc_angles(v, p1, p2);
                let mid_angle = (a1 + a2) * 0.5;
                let mx = mid_angle.cos();
                let my = mid_angle.sin();
                let dx = world.x - v.x;
                let dy = world.y - v.y;
                let t = (dx * mx + dy * my).max(1.0);
                Vec2::new(v.x + mx * t, v.y + my * t)
            }
            _ => world,
        }
    }

    fn snapped_world_for_grip_drag(
        &self,
        handle: DimGripHandle,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> (Vec2, Option<SnapKind>) {
        let local = screen_pos - rect.min;
        let raw_world = screen_to_world(local.x, local.y, viewport);
        // Object-snap only: explicit snap points first, excluding the dragged dimension itself.
        let pick = self.pick_entity_point_excluding(viewport, rect, screen_pos, Some(handle.entity));
        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or(raw_world);
        let mut kind = pick.as_ref().map(|(_, k)| *k);

        // Intersection snap as fallback (still object-based).
        if pick.is_none() && self.snap_enabled {
            if let Some(pt) =
                self.find_intersection_snap_excluding(viewport, rect, screen_pos, Some(handle.entity))
            {
                world = pt;
                kind = Some(SnapKind::Intersection);
            }
        }
        (world, kind)
    }

    fn is_layer_locked(&self, layer_id: u32) -> bool {
        self.drawing.get_layer(layer_id).map(|l| l.locked).unwrap_or(false)
    }

    fn is_entity_on_locked_layer(&self, id: &Guid) -> bool {
        self.drawing
            .get_entity(id)
            .map(|e| self.is_layer_locked(e.layer))
            .unwrap_or(false)
    }

    fn filter_editable_entity_ids(&mut self, ids: &[Guid], op: &str) -> Vec<Guid> {
        let editable: Vec<Guid> = ids
            .iter()
            .copied()
            .filter(|id| !self.is_entity_on_locked_layer(id))
            .collect();
        let skipped = ids.len().saturating_sub(editable.len());
        if skipped > 0 {
            self.command_log.push(format!(
                "{op}: {} entit{} on locked layer{} skipped",
                skipped,
                if skipped == 1 { "y" } else { "ies" },
                if skipped == 1 { "" } else { "s" }
            ));
        }
        editable
    }

    fn format_dim_measurement(&self, value: f64) -> String {
        format!("{:.*}", self.dim_style.precision, value)
    }

    fn dim_label_text(&self, text_override: &Option<String>, value: f64) -> String {
        let measurement = self.format_dim_measurement(value);
        match text_override {
            None => measurement,
            Some(s) if s.trim().is_empty() || s.trim() == "<>" => measurement,
            Some(s) => s.replace("<>", &measurement),
        }
    }

    fn ensure_dim_layer(&mut self) -> u32 {
        let existing = self
            .drawing
            .layers()
            .find(|l| l.name == "Dim")
            .map(|l| l.id);
        let dim_layer = if let Some(id) = existing {
            id
        } else {
            self.drawing
                .add_layer_with_color("Dim".to_string(), self.dim_style.color)
        };
        if let Some(layer) = self.drawing.get_layer_mut(dim_layer) {
            layer.color = self.dim_style.color;
        }
        dim_layer
    }

    pub(crate) fn open_dim_style_dialog(&mut self) {
        self.dim_style_dialog = Some(DimStyleDialog {
            text_height_str: format!("{:.4}", self.dim_style.text_height),
            precision_str: self.dim_style.precision.to_string(),
            color: self.dim_style.color,
            arrow_length_str: format!("{:.4}", self.dim_style.arrow_length),
            arrow_half_width_str: format!("{:.4}", self.dim_style.arrow_half_width),
        });
    }

    fn cancel_active_tool(&mut self) {
        self.active_tool = ActiveTool::None;
        self.exit_scale();
        self.exit_mirror();
        self.exit_fillet();
        self.exit_chamfer();
        self.exit_polygon();
        self.exit_ellipse();
        self.exit_rectangle();
        self.exit_array();
        self.exit_pedit();
        self.selection = None;
        // Reset tool-specific buffers
        if let ActiveTool::Polyline { .. } = self.active_tool {
            // already set to None above
        }
    }

    fn exit_from(&mut self) {
        self.from_phase = FromPhase::Idle;
        self.from_base = None;
    }

    fn exit_pedit(&mut self) {
        self.pedit_phase = PeditPhase::Idle;
    }

    fn exit_chamfer(&mut self) {
        self.chamfer_phase = ChamferPhase::Idle;
    }

    fn exit_polygon(&mut self) {
        self.polygon_phase = PolygonPhase::Idle;
    }

    fn exit_ellipse(&mut self) {
        self.ellipse_phase = EllipsePhase::Idle;
    }

    fn exit_rectangle(&mut self) {
        self.rectangle_phase = RectanglePhase::Idle;
    }

    fn exit_array(&mut self) {
        self.array_phase = ArrayPhase::Idle;
        self.array_entities.clear();
        self.array_center = None;
        self.array_rect_dir_point = None;
        self.array_rect_y_sign = 1.0;
        self.array_edit_assoc = None;
    }

    fn exit_dim(&mut self) {
        self.dim_phase = DimPhase::Idle;
        self.dim_linear_phase = DimLinearPhase::Idle;
        self.dim_angular_phase = DimAngularPhase::Idle;
        self.dim_radial_phase = DimRadialPhase::Idle;
    }

    fn has_active_dimension_command(&self) -> bool {
        !matches!(self.dim_phase, DimPhase::Idle)
            || !matches!(self.dim_linear_phase, DimLinearPhase::Idle)
            || !matches!(self.dim_angular_phase, DimAngularPhase::Idle)
            || !matches!(self.dim_radial_phase, DimRadialPhase::Idle)
    }

    fn exit_text(&mut self) {
        self.text_phase = TextPhase::Idle;
        self.command_input.clear();
    }

    fn exit_edit_text(&mut self) {
        self.edit_text_phase = EditTextPhase::Idle;
        self.text_edit_dialog = None;
    }

    fn exit_edit_dim(&mut self) {
        self.edit_dim_phase = EditDimPhase::Idle;
        self.dim_edit_dialog = None;
    }

    /// True when the current state expects the next user action to be picking a world point.
    fn is_picking_point(&self) -> bool {
        match &self.active_tool {
            ActiveTool::Line { .. }
            | ActiveTool::Circle { .. }
            | ActiveTool::Arc { .. }
            | ActiveTool::Polyline { .. } => true,
            ActiveTool::None => {
                matches!(self.move_phase, MovePhase::BasePoint | MovePhase::Destination)
                    || matches!(self.copy_phase, CopyPhase::BasePoint | CopyPhase::Destination)
                    || matches!(self.rotate_phase, RotatePhase::BasePoint | RotatePhase::Rotation)
                    || matches!(
                        self.scale_phase,
                        ScalePhase::BasePoint | ScalePhase::ReferencePoint | ScalePhase::Factor
                    )
                    || matches!(
                        self.mirror_phase,
                        MirrorPhase::FirstAxisPoint | MirrorPhase::SecondAxisPoint
                    )
                    || !matches!(self.dim_phase, DimPhase::Idle)
                    || !matches!(self.dim_linear_phase, DimLinearPhase::Idle)
                    || matches!(self.dim_angular_phase, DimAngularPhase::Placing { .. })
                    || matches!(self.dim_radial_phase, DimRadialPhase::Placing { .. })
                    || self.text_phase == TextPhase::PlacingPosition
                    || matches!(self.polygon_phase, PolygonPhase::Center | PolygonPhase::Radius { .. })
                    || matches!(
                        self.ellipse_phase,
                        EllipsePhase::Center | EllipsePhase::RadiusX { .. } | EllipsePhase::RadiusY { .. }
                    )
                    || matches!(
                        self.rectangle_phase,
                        RectanglePhase::FirstCorner
                            | RectanglePhase::SecondCorner { .. }
                            | RectanglePhase::Direction { .. }
                    )
                    || matches!(
                        self.array_phase,
                        ArrayPhase::RectBasePoint
                            | ArrayPhase::RectGripIdle
                            | ArrayPhase::RectXSpacingGrip
                            | ArrayPhase::RectXCountGrip
                            | ArrayPhase::RectYSpacingGrip
                            | ArrayPhase::RectYCountGrip
                            | ArrayPhase::PolarCenter
                            | ArrayPhase::PolarBasePoint
                    )
            }
        }
    }

    /// Deliver a resolved world point to whichever command/tool is currently waiting for one.
    /// Mirrors the click-handler logic for each state.
    fn deliver_point(&mut self, world: Vec2) {
        let layer = self.current_layer;
        match &mut self.active_tool {
            ActiveTool::Line { start } => {
                if start.is_none() {
                    *start = Some(world);
                    self.distance_input.clear();
                    self.command_log.push(format!("  Start: {:.4}, {:.4}", world.x, world.y));
                } else if let Some(s) = start.take() {
                    let end_pt = world;
                    self.push_undo();
                    let mut line = create_line(s, end_pt);
                    line.layer = layer;
                    self.drawing.add_entity(line);
                    if let ActiveTool::Line { start } = &mut self.active_tool {
                        *start = Some(end_pt);
                    }
                    self.distance_input.clear();
                    self.command_log.push(format!("  End: {:.4}, {:.4}", end_pt.x, end_pt.y));
                }
                return;
            }
            ActiveTool::Circle { center } => {
                if center.is_none() {
                    *center = Some(world);
                    self.distance_input.clear();
                    self.command_log.push(format!("  Center: {:.4}, {:.4}", world.x, world.y));
                } else if let Some(c) = center.take() {
                    let radius = c.distance_to(&world);
                    if radius > f64::EPSILON {
                        self.push_undo();
                        let mut circle = create_circle(c, radius);
                        circle.layer = layer;
                        self.drawing.add_entity(circle);
                        self.command_log.push(format!("  Radius: {:.4}", radius));
                    }
                    self.distance_input.clear();
                }
                return;
            }
            ActiveTool::Arc { start, mid } => {
                if start.is_none() {
                    *start = Some(world);
                    self.command_log.push(format!("  Start: {:.4}, {:.4}", world.x, world.y));
                } else if mid.is_none() {
                    *mid = Some(world);
                    self.command_log.push(format!("  Mid: {:.4}, {:.4}", world.x, world.y));
                } else if let (Some(s), Some(m)) = (start.take(), mid.take()) {
                    if let Some(mut a) = create_arc_from_three_points(s, m, world) {
                        self.push_undo();
                        a.layer = layer;
                        self.drawing.add_entity(a);
                        self.command_log.push(format!("  End: {:.4}, {:.4}", world.x, world.y));
                    } else {
                        self.command_log.push("  *Invalid arc (collinear)*".to_string());
                    }
                }
                return;
            }
            ActiveTool::Polyline { points } => {
                points.push(world);
                self.distance_input.clear();
                self.command_log.push(format!("  Pt {}: {:.4}, {:.4}", points.len(), world.x, world.y));
                return;
            }
            ActiveTool::None => {}
        }
        // Idle-mode commands.
        if self.move_phase == MovePhase::BasePoint {
            self.move_base_point = Some(world);
            self.move_phase = MovePhase::Destination;
            self.command_log.push("MOVE: Pick destination point".to_string());
        } else if self.move_phase == MovePhase::Destination {
            self.apply_move(world);
        } else if self.copy_phase == CopyPhase::BasePoint {
            self.copy_base_point = Some(world);
            self.copy_phase = CopyPhase::Destination;
            self.command_log.push("COPY: Pick destination (RClick/Enter=done)".to_string());
        } else if self.copy_phase == CopyPhase::Destination {
            self.apply_copy(world);
        } else if self.rotate_phase == RotatePhase::BasePoint {
            self.rotate_base_point = Some(world);
            self.rotate_phase = RotatePhase::Rotation;
            self.command_log.push("ROTATE: Specify angle (degrees) or click".to_string());
        } else if self.rotate_phase == RotatePhase::Rotation {
            if let Some(base) = self.rotate_base_point {
                let angle = (world.y - base.y).atan2(world.x - base.x);
                self.apply_rotate(angle);
            }
        } else if self.scale_phase == ScalePhase::BasePoint {
            self.scale_base_point = Some(world);
            self.scale_phase = ScalePhase::ReferencePoint;
            self.command_log.push("SCALE: Pick reference point".to_string());
        } else if self.scale_phase == ScalePhase::ReferencePoint {
            if let Some(base) = self.scale_base_point {
                let d = base.distance_to(&world);
                if d > 1e-9 {
                    self.scale_ref_point = Some(world);
                    self.scale_phase = ScalePhase::Factor;
                    self.command_log.push("SCALE: Specify factor or pick point".to_string());
                } else {
                    self.command_log.push("SCALE: Reference point too close to base".to_string());
                }
            }
        } else if self.scale_phase == ScalePhase::Factor {
            self.apply_scale_from_point(world);
        } else if self.mirror_phase == MirrorPhase::FirstAxisPoint {
            self.mirror_axis_p1 = Some(world);
            self.mirror_phase = MirrorPhase::SecondAxisPoint;
            self.command_log.push("MIRROR: Pick second axis point".to_string());
        } else if self.mirror_phase == MirrorPhase::SecondAxisPoint {
            if let Some(p1) = self.mirror_axis_p1 {
                let axis_p2 = if self.ortho_enabled {
                    Self::snap_angle(p1, world, self.ortho_increment_deg)
                } else {
                    world
                };
                self.apply_mirror(p1, axis_p2);
            }
        } else if self.polygon_phase == PolygonPhase::Center {
            self.polygon_phase = PolygonPhase::Radius { center: world };
            self.command_log.push("POLYGON: Specify radius point".to_string());
        } else if let PolygonPhase::Radius { center } = self.polygon_phase {
            if self.apply_polygon(center, world) {
                self.polygon_phase = PolygonPhase::Center;
            }
        } else if self.ellipse_phase == EllipsePhase::Center {
            self.ellipse_phase = EllipsePhase::RadiusX { center: world };
            self.command_log.push("ELLIPSE: Specify radius from center".to_string());
        } else if let EllipsePhase::RadiusX { center } = self.ellipse_phase {
            let p = if self.ortho_enabled {
                Self::snap_angle(center, world, self.ortho_increment_deg)
            } else {
                world
            };
            let rx = center.distance_to(&p);
            if rx > 1e-9 {
                self.ellipse_phase = EllipsePhase::RadiusY { center, rx };
                self.command_log.push("ELLIPSE: Specify height from center".to_string());
            } else {
                self.command_log.push("ELLIPSE: Radius too small".to_string());
            }
        } else if let EllipsePhase::RadiusY { center, rx } = self.ellipse_phase {
            let p = if self.ortho_enabled {
                Self::snap_angle(center, world, self.ortho_increment_deg)
            } else {
                world
            };
            let ry = center.distance_to(&p);
            if self.apply_ellipse(center, rx, ry) {
                self.ellipse_phase = EllipsePhase::Center;
            }
        } else if self.rectangle_phase == RectanglePhase::FirstCorner {
            self.rectangle_phase = RectanglePhase::SecondCorner { first: world };
            self.command_log
                .push("RECTANGLE: Specify opposite corner or [D=Dimensions]".to_string());
        } else if let RectanglePhase::SecondCorner { first } = self.rectangle_phase {
            if self.apply_rectangle_diagonal(first, world) {
                self.rectangle_phase = RectanglePhase::FirstCorner;
            }
        } else if let RectanglePhase::Direction { first, width, height } = self.rectangle_phase {
            if self.apply_rectangle_dimensions(first, width, height, world) {
                self.rectangle_phase = RectanglePhase::FirstCorner;
            }
        } else if self.array_phase == ArrayPhase::RectBasePoint {
            self.array_center = Some(world);
            self.array_rect_dir_point = Some(Vec2::new(world.x + self.array_rect_dx.abs().max(1.0), world.y));
            self.array_phase = ArrayPhase::RectGripIdle;
            self.command_log.push(
                "ARRAY: Grips visible. Click any grip to activate/edit. Press Enter to apply".to_string(),
            );
        } else if matches!(
            self.array_phase,
            ArrayPhase::RectXSpacingGrip
                | ArrayPhase::RectXCountGrip
                | ArrayPhase::RectYSpacingGrip
                | ArrayPhase::RectYCountGrip
        ) {
            if self.update_array_rect_from_world(world) {
                self.array_phase = ArrayPhase::RectGripIdle;
                self.command_log
                    .push("ARRAY: Grip released. Click another grip or Enter to apply".to_string());
            }
        } else if self.array_phase == ArrayPhase::PolarCenter {
            self.array_center = Some(world);
            self.array_phase = ArrayPhase::PolarBasePoint;
            self.command_log
                .push("ARRAY: Specify base/reference point".to_string());
        } else if self.array_phase == ArrayPhase::PolarBasePoint {
            if let Some(center) = self.array_center {
                if self.apply_array_polar(center, world) {
                    self.exit_array();
                }
            }
        } else if matches!(self.dim_phase, DimPhase::FirstPoint) {
            self.dim_phase = DimPhase::SecondPoint { first: world };
            self.command_log.push(format!("DIMALIGNED: First point ({:.4}, {:.4})", world.x, world.y));
        } else if let DimPhase::SecondPoint { first } = self.dim_phase {
            self.dim_phase = DimPhase::Placing { first, second: world };
            self.command_log.push(format!("DIMALIGNED: Second point ({:.4}, {:.4})", world.x, world.y));
        } else if let DimPhase::Placing { first, second } = self.dim_phase {
            self.place_dim_aligned(first, second, world);
        } else if matches!(self.dim_linear_phase, DimLinearPhase::FirstPoint) {
            self.dim_linear_phase = DimLinearPhase::SecondPoint { first: world };
            self.command_log.push(format!("DIMLINEAR: First point ({:.4}, {:.4})", world.x, world.y));
        } else if let DimLinearPhase::SecondPoint { first } = self.dim_linear_phase {
            self.dim_linear_phase = DimLinearPhase::Placing { first, second: world };
            self.command_log.push(format!("DIMLINEAR: Second point ({:.4}, {:.4})", world.x, world.y));
        } else if let DimLinearPhase::Placing { first, second } = self.dim_linear_phase {
            self.place_dim_linear(first, second, world);
        } else if let DimAngularPhase::Placing { vertex, line1_pt, line2_pt } = self.dim_angular_phase {
            self.place_dim_angular(vertex, line1_pt, line2_pt, world);
        } else if let DimRadialPhase::Placing { center, radius, is_diameter } = self.dim_radial_phase {
            self.place_dim_radial(center, radius, is_diameter, world);
        } else if self.text_phase == TextPhase::PlacingPosition {
            self.text_phase = TextPhase::EnteringHeight { position: world };
            self.command_input.clear();
            self.command_log.push(format!(
                "TEXT  Text height <{:.4}>:",
                self.last_text_height
            ));
        }
    }

    fn apply_from_result_point(&mut self, world: Vec2) {
        self.exit_from();
        if let DimPhase::SecondPoint { first } = self.dim_phase.clone() {
            self.dim_phase = DimPhase::Placing { first, second: world };
            self.command_log.push(format!(
                "DIMALIGNED: Second point ({:.4}, {:.4})",
                world.x, world.y
            ));
            return;
        }
        if let DimLinearPhase::SecondPoint { first } = self.dim_linear_phase.clone() {
            self.dim_linear_phase = DimLinearPhase::Placing { first, second: world };
            self.command_log.push(format!(
                "DIMLINEAR: Second point ({:.4}, {:.4})",
                world.x, world.y
            ));
            return;
        }
        self.deliver_point(world);
    }

    /// Request focus on the command line input if nothing else currently has it.
    fn auto_focus_command_line(&self, ctx: &egui::Context) {
        if !ctx.wants_keyboard_input() {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("cmd_input")));
        }
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::default();
        let initial_w = 800;
        let initial_h = 600;

        if let Some(render_state) = &cc.wgpu_render_state {
            match Viewport::new_with_device(
                render_state.device.clone(),
                render_state.queue.clone(),
                initial_w,
                initial_h,
            ) {
                Ok(viewport) => app.viewport = Some(viewport),
                Err(err) => app.viewport_init_error = Some(err.to_string()),
            }
        } else {
            match pollster::block_on(Viewport::new(initial_w, initial_h)) {
                Ok(viewport) => app.viewport = Some(viewport),
                Err(err) => app.viewport_init_error = Some(err.to_string()),
            }
        }

        app
    }

    fn ensure_registered_texture(&mut self, frame: &eframe::Frame) {
        let Some(render_state) = frame.wgpu_render_state() else {
            return;
        };
        let Some(viewport) = self.viewport.as_ref() else {
            return;
        };

        let mut renderer = render_state.renderer.write();
        match self.viewport_texture_id {
            Some(texture_id) => {
                renderer.update_egui_texture_from_wgpu_texture(
                    render_state.device.as_ref(),
                    viewport.texture_view(),
                    wgpu::FilterMode::Linear,
                    texture_id,
                );
            }
            None => {
                let texture_id = renderer.register_native_texture(
                    render_state.device.as_ref(),
                    viewport.texture_view(),
                    wgpu::FilterMode::Linear,
                );
                self.viewport_texture_id = Some(texture_id);
            }
        }
    }

    fn push_undo(&mut self) {
        self.undo_stack.push(self.drawing.clone());
        if self.undo_stack.len() > Self::UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            let current = std::mem::replace(&mut self.drawing, prev);
            self.redo_stack.push(current);
            self.selected_entities.clear();
            self.selection = None;
            self.command_log.push("Undo".to_string());
        } else {
            self.command_log.push("Undo: nothing to undo".to_string());
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack.push(self.drawing.clone());
            self.drawing = next;
            self.selected_entities.clear();
            self.selection = None;
            self.command_log.push("Redo".to_string());
        } else {
            self.command_log.push("Redo: nothing to redo".to_string());
        }
    }

    fn snap_to_grid(&self, world: cadkit_types::Vec2) -> cadkit_types::Vec2 {
        let s = self.grid_spacing;
        let gx = (world.x / s).round() * s;
        let gy = (world.y / s).round() * s;
        cadkit_types::Vec2::new(gx, gy)
    }

    fn select_entity_id(&mut self, entity: Option<Guid>, additive: bool) {
        let expand_group = |this: &Self, id: Guid| -> Vec<Guid> {
            if let Some(aid) = this.assoc_member_to_array.get(&id).copied() {
                if let Some(arr) = this.assoc_rect_arrays.get(&aid) {
                    let ids: Vec<Guid> = arr
                        .members
                        .iter()
                        .copied()
                        .filter(|m| this.drawing.get_entity(m).is_some())
                        .collect();
                    if !ids.is_empty() {
                        return ids;
                    }
                }
            }
            vec![id]
        };

        match (entity, additive) {
            (Some(id), true) => {
                let group = expand_group(self, id);
                let any_selected = group.iter().any(|gid| self.selected_entities.contains(gid));
                if any_selected {
                    for gid in group {
                        self.selected_entities.remove(&gid);
                    }
                } else {
                    for gid in group {
                        self.selected_entities.insert(gid);
                    }
                }
            }
            (Some(id), false) => {
                let group = expand_group(self, id);
                self.selected_entities.clear();
                for gid in group {
                    self.selected_entities.insert(gid);
                }
            }
            (None, false) => self.selected_entities.clear(),
            (None, true) => {}
        }
    }

    fn current_prompt(&self) -> String {
        // FROM mode overrides all other prompts.
        if self.from_phase == FromPhase::WaitingBase {
            return "FROM  Base point (snap to geometry):".into();
        }
        if self.from_phase == FromPhase::WaitingOffset {
            return "FROM  Offset (@dx,dy  or  @dist<angle  or click):".into();
        }
        // TEXT placement phases.
        if self.edit_text_phase == EditTextPhase::SelectingEntity {
            return "EDITTEXT  Click a text entity to edit:".into();
        }
        if self.edit_dim_phase == EditDimPhase::SelectingEntity {
            return "EDITDIM  Click a dimension entity to edit:".into();
        }
        if let Some(handle) = self.dim_grip_drag {
            if !self.dim_grip_is_dragging {
                return match handle.kind {
                    DimGripKind::Start | DimGripKind::End => {
                        "DIM GRIP  Base fixed. Drag direction, type distance, or click target:".into()
                    }
                    DimGripKind::Offset => "DIM GRIP  Drag or click to set offset:".into(),
                    DimGripKind::Text => "DIM GRIP  Drag or click to place text:".into(),
                };
            }
        }
        match &self.text_phase {
            TextPhase::PlacingPosition => return "TEXT  Specify insertion point:".into(),
            TextPhase::EnteringHeight { .. } => return format!(
                "TEXT  Text height <{:.4}>:", self.last_text_height),
            TextPhase::EnteringRotation { .. } => return format!(
                "TEXT  Rotation angle <{:.1}>:", self.last_text_rotation.to_degrees()),
            TextPhase::TypingContent { .. } => return "TEXT  Enter text:".into(),
            TextPhase::Idle => {}
        }
        match self.polygon_phase {
            PolygonPhase::EnteringSides => {
                return format!("POLYGON  Enter number of sides <{}>:", self.polygon_sides);
            }
            PolygonPhase::Center => return "POLYGON  Specify center point:".into(),
            PolygonPhase::Radius { .. } => {
                return "POLYGON  Specify radius (click point on circumcircle):".into();
            }
            PolygonPhase::Idle => {}
        }
        match self.ellipse_phase {
            EllipsePhase::Center => return "ELLIPSE  Specify center point:".into(),
            EllipsePhase::RadiusX { .. } => return "ELLIPSE  Specify radius from center:".into(),
            EllipsePhase::RadiusY { .. } => return "ELLIPSE  Specify height from center:".into(),
            EllipsePhase::Idle => {}
        }
        match self.rectangle_phase {
            RectanglePhase::FirstCorner => return "RECTANGLE  Specify first corner point:".into(),
            RectanglePhase::SecondCorner { .. } => {
                return "RECTANGLE  Specify opposite corner or [D=Dimensions]:".into();
            }
            RectanglePhase::EnteringDimensions { .. } => {
                return format!(
                    "RECTANGLE  Enter dimensions w,h <{:.4},{:.4}>:",
                    self.rectangle_width, self.rectangle_height
                );
            }
            RectanglePhase::Direction { .. } => {
                return "RECTANGLE  Specify direction point:".into();
            }
            RectanglePhase::Idle => {}
        }
        match self.array_phase {
            ArrayPhase::SelectingEntities => {
                return "ARRAY  Select entities, press Enter to continue:".into();
            }
            ArrayPhase::ChoosingType => {
                return "ARRAY  Choose type [Rectangular/Polar] <Rectangular>:".into();
            }
            ArrayPhase::RectEnteringCount => {
                return "ARRAY  Legacy count entry disabled; specify base point:".into();
            }
            ArrayPhase::RectEnteringSpacing => {
                return "ARRAY  Legacy spacing entry disabled; specify base point:".into();
            }
            ArrayPhase::RectBasePoint => return "ARRAY  Specify base point:".into(),
            ArrayPhase::RectGripIdle => {
                return "ARRAY  Grips visible. Click grip to activate; Enter=apply; E=explode associative array:".into();
            }
            ArrayPhase::RectXSpacingGrip => {
                return "ARRAY  X spacing grip active: click/drag or type. Enter=apply, E=explode:".into();
            }
            ArrayPhase::RectXCountGrip => {
                return "ARRAY  X count grip active: click/drag or type quantity. Enter=apply, E=explode:".into();
            }
            ArrayPhase::RectYSpacingGrip => {
                return "ARRAY  Y spacing grip active: click/drag or type. Enter=apply, E=explode:".into();
            }
            ArrayPhase::RectYCountGrip => {
                return "ARRAY  Y count grip active: click/drag or type quantity. Enter=apply, E=explode:".into();
            }
            ArrayPhase::PolarEnteringCount => {
                return format!("ARRAY  Enter item count <{}>:", self.array_polar_count);
            }
            ArrayPhase::PolarEnteringAngle => {
                return format!(
                    "ARRAY  Enter fill angle degrees <{:.4}>:",
                    self.array_polar_angle_deg
                );
            }
            ArrayPhase::PolarCenter => return "ARRAY  Specify center point:".into(),
            ArrayPhase::PolarBasePoint => return "ARRAY  Specify base/reference point:".into(),
            ArrayPhase::Idle => {}
        }
        match self.pedit_phase {
            PeditPhase::SelectingPolyline => {
                return "PEDIT  Select an open polyline:".into();
            }
            PeditPhase::Joining { .. } => {
                return "PEDIT  Select line or arc to join (Enter to finish):".into();
            }
            PeditPhase::Idle => {}
        }
        match self.chamfer_phase {
            ChamferPhase::EnteringDistance => {
                return format!(
                    "CHAMFER  Enter distances <{:.4},{:.4}>:",
                    self.chamfer_distance1, self.chamfer_distance2
                );
            }
            ChamferPhase::FirstEntity => {
                return "CHAMFER  Select first line or polyline segment:".into();
            }
            ChamferPhase::SecondEntity { .. } => {
                return "CHAMFER  Select second line or polyline segment:".into();
            }
            ChamferPhase::Idle => {}
        }
        match &self.active_tool {
            ActiveTool::None => match self.trim_phase {
                TrimPhase::Idle => match self.offset_phase {
                    OffsetPhase::Idle => match self.move_phase {
                        MovePhase::Idle => match self.extend_phase {
                        ExtendPhase::Idle => match self.copy_phase {
                            CopyPhase::Idle => match self.rotate_phase {
                                RotatePhase::Idle => match self.scale_phase {
                                    ScalePhase::Idle => match self.mirror_phase {
                                        MirrorPhase::Idle => match self.fillet_phase {
                                        FilletPhase::Idle => match self.dim_phase {
                                        DimPhase::FirstPoint => "DIMALIGNED  Specify first extension line origin:".into(),
                                        DimPhase::SecondPoint { .. } => "DIMALIGNED  Specify second extension line origin:".into(),
                                        DimPhase::Placing { .. } => "DIMALIGNED  Specify dimension line location:".into(),
                                        DimPhase::Idle => match self.dim_linear_phase {
                                            DimLinearPhase::FirstPoint => "DIMLINEAR  Specify first extension line origin:".into(),
                                            DimLinearPhase::SecondPoint { .. } => "DIMLINEAR  Specify second extension line origin:".into(),
                                            DimLinearPhase::Placing { .. } => "DIMLINEAR  Drag to set H or V dimension line location:".into(),
                                            DimLinearPhase::Idle => match self.dim_angular_phase {
                                                DimAngularPhase::FirstEntity => "DIMANGULAR  Click the first line or polyline segment:".into(),
                                                DimAngularPhase::SecondEntity { .. } => "DIMANGULAR  Click the second line or polyline segment:".into(),
                                                DimAngularPhase::Placing { .. } => "DIMANGULAR  Drag to set arc radius, click to place:".into(),
                                                DimAngularPhase::Idle => match self.dim_radial_phase {
                                                    DimRadialPhase::SelectingEntity { is_diameter } => if is_diameter {
                                                        "DIMDIAMETER  Click a circle or arc:".into()
                                                    } else {
                                                        "DIMRADIUS  Click a circle or arc:".into()
                                                    },
                                                    DimRadialPhase::Placing { is_diameter, .. } => if is_diameter {
                                                        "DIMDIAMETER  Drag leader, click to place:".into()
                                                    } else {
                                                        "DIMRADIUS  Drag leader, click to place:".into()
                                                    },
                                                    DimRadialPhase::Idle => "Command:".into(),
                                                },
                                            },
                                        },
                                        },
                                        FilletPhase::EnteringRadius => format!("FILLET  Enter radius <{:.4}>:", self.fillet_radius),
                                        FilletPhase::FirstEntity => "FILLET  Select first line or polyline segment:".into(),
                                        FilletPhase::SecondEntity { .. } => "FILLET  Select second line or polyline segment:".into(),
                                        },
                                        MirrorPhase::SelectingEntities => "MIRROR  Select entities, press Enter to continue:".into(),
                                        MirrorPhase::FirstAxisPoint => "MIRROR  Pick first axis point:".into(),
                                        MirrorPhase::SecondAxisPoint => "MIRROR  Pick second axis point:".into(),
                                    },
                                    ScalePhase::SelectingEntities => "SCALE  Select entities, press Enter to continue:".into(),
                                    ScalePhase::BasePoint => "SCALE  Pick base point:".into(),
                                    ScalePhase::ReferencePoint => "SCALE  Pick reference point:".into(),
                                    ScalePhase::Factor => "SCALE  Specify factor or pick point:".into(),
                                }
                                RotatePhase::SelectingEntities => "ROTATE  Select entities, press Enter to continue:".into(),
                                RotatePhase::BasePoint => "ROTATE  Pick base point:".into(),
                                RotatePhase::Rotation => "ROTATE  Specify angle (degrees) or click:".into(),
                            },
                            CopyPhase::SelectingEntities => "COPY  Select entities, press Enter to continue:".into(),
                            CopyPhase::BasePoint => "COPY  Pick base point:".into(),
                            CopyPhase::Destination => "COPY  Pick destination (Enter to finish):".into(),
                        },
                        ExtendPhase::SelectingBoundaries => "EXTEND  Select boundary edges (Enter when done):".into(),
                        ExtendPhase::Extending => "EXTEND  Click near line or arc endpoint to extend:".into(),
                    },
                        MovePhase::SelectingEntities => "MOVE  Select entities, press Enter to continue:".into(),
                        MovePhase::BasePoint => "MOVE  Pick base point:".into(),
                        MovePhase::Destination => "MOVE  Pick destination point:".into(),
                    },
                    OffsetPhase::EnteringDistance => "OFFSET  Enter distance:".into(),
                    OffsetPhase::SelectingEntity => "OFFSET  Select entity to offset:".into(),
                    OffsetPhase::SelectingSide => "OFFSET  Click side to offset toward:".into(),
                },
                TrimPhase::SelectingEdges => "TRIM  Select cutting edges (Enter when done):".into(),
                TrimPhase::Trimming => "TRIM  Click entity side to trim (Esc/Enter to exit):".into(),
            },
            ActiveTool::Line { start: None } => "LINE  Specify first point:".into(),
            ActiveTool::Line { start: Some(_) } => "LINE  Specify next point (Esc to finish):".into(),
            ActiveTool::Circle { center: None } => "CIRCLE  Specify center point:".into(),
            ActiveTool::Circle { center: Some(_) } => "CIRCLE  Specify radius:".into(),
            ActiveTool::Arc { start: None, .. } => "ARC  Specify start point:".into(),
            ActiveTool::Arc { start: Some(_), mid: None } => "ARC  Specify second point:".into(),
            ActiveTool::Arc { start: Some(_), mid: Some(_) } => "ARC  Specify end point:".into(),
            ActiveTool::Polyline { points } => match points.len() {
                0 => "PLINE  Specify start point:".into(),
                _ => "PLINE  Specify next point  [C=Close  RClick/Enter=Done]:".into(),
            },
        }
    }

    fn dim_grip_anchor_point(&self, handle: DimGripHandle) -> Option<Vec2> {
        let entity = self.drawing.get_entity(&handle.entity)?;
        match (&entity.kind, handle.kind) {
            (EntityKind::DimAligned { start, .. }, DimGripKind::Start)
            | (EntityKind::DimLinear { start, .. }, DimGripKind::Start) => {
                Some((*start).into())
            }
            (EntityKind::DimAligned { end, .. }, DimGripKind::End)
            | (EntityKind::DimLinear { end, .. }, DimGripKind::End) => {
                Some((*end).into())
            }
            (EntityKind::DimAngular { vertex, .. }, DimGripKind::Start)
            | (EntityKind::DimAngular { vertex, .. }, DimGripKind::End) => {
                Some((*vertex).into())
            }
            _ => None,
        }
    }

    fn dim_grip_tracking_target_handle(&self, handle: DimGripHandle) -> DimGripHandle {
        let kind = match handle.kind {
            DimGripKind::Start => DimGripKind::End,
            DimGripKind::End => DimGripKind::Start,
            other => other,
        };
        DimGripHandle { entity: handle.entity, kind }
    }

    pub(crate) fn apply_typed_dim_grip_input(&mut self, text: &str) -> bool {
        let Some(handle) = self.dim_grip_drag else {
            return false;
        };
        if self.dim_grip_is_dragging {
            return false;
        }
        if !matches!(handle.kind, DimGripKind::Start | DimGripKind::End) {
            self.command_log
                .push("DIM GRIP: distance entry is only for Start/End grips".to_string());
            return true;
        }
        let dist = match text.trim().parse::<f64>() {
            Ok(v) if v > f64::EPSILON => v,
            _ => return false,
        };
        let Some(base) = self.dim_grip_anchor_point(handle) else {
            self.command_log
                .push("DIM GRIP: could not determine anchor point".to_string());
            return true;
        };
        let Some(mut hover) = self.hover_world_pos else {
            self.command_log
                .push("DIM GRIP: move cursor to set direction".to_string());
            return true;
        };
        if self.ortho_enabled {
            hover = Self::snap_angle(base, hover, self.ortho_increment_deg);
        }
        let dx = hover.x - base.x;
        let dy = hover.y - base.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= f64::EPSILON {
            self.command_log
                .push("DIM GRIP: need a non-zero direction".to_string());
            return true;
        }
        let mut world = Vec2::new(base.x + dx / len * dist, base.y + dy / len * dist);
        let target_handle = self.dim_grip_tracking_target_handle(handle);
        world = self.constrained_dim_grip_world(target_handle, world);
        self.push_undo();
        self.apply_dim_grip_drag(target_handle, world);
        self.dim_grip_drag = None;
        self.dim_grip_is_dragging = false;
        self.command_log.push(format!("DIM GRIP: distance {:.4} applied", dist));
        true
    }


    /// Exit trim mode, clearing all trim state.
    fn exit_trim(&mut self) {
        self.trim_phase = TrimPhase::Idle;
        self.trim_cutting_edges.clear();
    }

    /// Exit offset mode, clearing all offset state.
    fn exit_offset(&mut self) {
        self.offset_phase = OffsetPhase::Idle;
        self.offset_distance = None;
        self.offset_selected_entity = None;
    }

    /// Exit move mode, clearing all move state (but leaving selected_entities intact).
    fn exit_move(&mut self) {
        self.move_phase = MovePhase::Idle;
        self.move_base_point = None;
        self.move_entities.clear();
    }

    /// Translate all `move_entities` by `dest - move_base_point`.
    fn apply_move(&mut self, dest: Vec2) {
        let base = match self.move_base_point {
            Some(b) => b,
            None => return,
        };
        let dx = dest.x - base.x;
        let dy = dest.y - base.y;
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            self.command_log.push("MOVE: Zero distance, nothing moved".to_string());
            self.exit_move();
            return;
        }
        let requested: Vec<Guid> = self.move_entities.clone();
        let ids = self.filter_editable_entity_ids(&requested, "MOVE");
        if ids.is_empty() {
            self.command_log.push("MOVE: No editable entities selected".to_string());
            self.exit_move();
            return;
        }
        self.push_undo();
        for id in &ids {
            if let Some(entity) = self.drawing.get_entity_mut(id) {
                match &mut entity.kind {
                    EntityKind::Line { start, end } => {
                        start.x += dx;
                        start.y += dy;
                        end.x += dx;
                        end.y += dy;
                    }
                    EntityKind::Circle { center, .. } | EntityKind::Arc { center, .. } => {
                        center.x += dx;
                        center.y += dy;
                    }
                    EntityKind::Polyline { vertices, .. } => {
                        for v in vertices.iter_mut() {
                            v.x += dx;
                            v.y += dy;
                        }
                    }
                    EntityKind::DimAligned { start, end, text_pos, .. }
                    | EntityKind::DimLinear { start, end, text_pos, .. } => {
                        start.x += dx; start.y += dy;
                        end.x += dx;   end.y += dy;
                        text_pos.x += dx; text_pos.y += dy;
                    }
                    EntityKind::DimAngular { vertex, line1_pt, line2_pt, text_pos, .. } => {
                        vertex.x   += dx; vertex.y   += dy;
                        line1_pt.x += dx; line1_pt.y += dy;
                        line2_pt.x += dx; line2_pt.y += dy;
                        text_pos.x += dx; text_pos.y += dy;
                    }
                    EntityKind::DimRadial { center, leader_pt, text_pos, .. } => {
                        center.x += dx; center.y += dy;
                        leader_pt.x += dx; leader_pt.y += dy;
                        text_pos.x += dx; text_pos.y += dy;
                    }
                    EntityKind::Text { position, .. } => {
                        position.x += dx;
                        position.y += dy;
                    }
                }
            }
        }
        // Keep the moved entities selected so the user can chain another move.
        self.selected_entities = ids.into_iter().collect();
        self.command_log.push("MOVE: Complete".to_string());
        self.exit_move();
    }

    /// Draw the MOVE rubber-band line and ghost entities during Destination phase.
    fn draw_move_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        if self.move_phase != MovePhase::Destination {
            return;
        }
        let base = match self.move_base_point {
            Some(b) => b,
            None => return,
        };
        let dx = world_cursor.x - base.x;
        let dy = world_cursor.y - base.y;
        let painter = ui.painter_at(rect);

        // Rubber-band line from base to cursor.
        let (bx, by) = world_to_screen(base.x as f32, base.y as f32, viewport);
        let (cx, cy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
        painter.line_segment(
            [rect.min + egui::vec2(bx, by), rect.min + egui::vec2(cx, cy)],
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(180, 180, 180, 140)),
        );

        // Ghost entities at offset position (dimmed).
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(120, 160, 210, 140));
        for id in &self.move_entities {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, .. } => {
                    let gv  = cadkit_types::Vec2::new(vertex.x   + dx, vertex.y   + dy);
                    let gp1 = cadkit_types::Vec2::new(line1_pt.x + dx, line1_pt.y + dy);
                    let gp2 = cadkit_types::Vec2::new(line2_pt.x + dx, line2_pt.y + dy);
                    let (a1, a2) = angular_arc_angles(gv, gp1, gp2);
                    let pts = angular_arc_pts(gv, a1, a2, *radius);
                    if let (Some(arc_s), Some(arc_e)) = (pts.first(), pts.last()) {
                        let to_s = |p: cadkit_types::Vec2| { let (sx,sy) = world_to_screen(p.x as f32, p.y as f32, viewport); rect.min + egui::vec2(sx,sy) };
                        painter.line_segment([to_s(gp1), to_s(*arc_s)], ghost_stroke);
                        painter.line_segment([to_s(gp2), to_s(*arc_e)], ghost_stroke);
                    }
                    let spts: Vec<egui::Pos2> = pts.iter().map(|p| { let (sx,sy) = world_to_screen(p.x as f32, p.y as f32, viewport); rect.min + egui::vec2(sx,sy) }).collect();
                    for w in spts.windows(2) { painter.line_segment([w[0], w[1]], ghost_stroke); }
                }
                EntityKind::Line { start, end } => {
                    let (x1, y1) = world_to_screen((start.x + dx) as f32, (start.y + dy) as f32, viewport);
                    let (x2, y2) = world_to_screen((end.x + dx) as f32, (end.y + dy) as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        ghost_stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let gx = center.x + dx;
                    let gy = center.y + dy;
                    let (sx, sy) = world_to_screen(gx as f32, gy as f32, viewport);
                    let (rx, _) = world_to_screen((gx + radius) as f32, gy as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(sx, sy), (rx - sx).abs(), ghost_stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let gx = center.x + dx;
                    let gy = center.y + dy;
                    let sweep = *end_angle - *start_angle;
                    let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = *start_angle + sweep * t;
                        let px = gx + *radius * ang.cos();
                        let py = gy + *radius * ang.sin();
                        let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last {
                            painter.line_segment([prev, pos], ghost_stroke);
                        }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    let shifted: Vec<egui::Pos2> = vertices.iter().map(|v| {
                        let (sx, sy) = world_to_screen((v.x + dx) as f32, (v.y + dy) as f32, viewport);
                        rect.min + egui::vec2(sx, sy)
                    }).collect();
                    for w in shifted.windows(2) {
                        painter.line_segment([w[0], w[1]], ghost_stroke);
                    }
                    if *closed && shifted.len() >= 2 {
                        painter.line_segment([*shifted.last().unwrap(), shifted[0]], ghost_stroke);
                    }
                }
                EntityKind::DimAligned { start, end, offset, .. } => {
                    let gsx = (start.x + dx) as f32; let gsy = (start.y + dy) as f32;
                    let gex = (end.x   + dx) as f32; let gey = (end.y   + dy) as f32;
                    let ddx2 = gex - gsx; let ddy2 = gey - gsy;
                    let glen = (ddx2*ddx2 + ddy2*ddy2).sqrt();
                    if glen < 1e-6 { continue; }
                    let perp = [-ddy2/glen, ddx2/glen];
                    let off = *offset as f32;
                    let (p1x, p1y) = world_to_screen(gsx, gsy, viewport);
                    let (p2x, p2y) = world_to_screen(gex, gey, viewport);
                    let (dl1x, dl1y) = world_to_screen(gsx + perp[0]*off, gsy + perp[1]*off, viewport);
                    let (dl2x, dl2y) = world_to_screen(gex + perp[0]*off, gey + perp[1]*off, viewport);
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p1x, p1y), rect.min + egui::vec2(dl1x, dl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p2x, p2y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                }
                EntityKind::DimLinear { start, end, offset, horizontal, .. } => {
                    let gsx = start.x + dx; let gsy = start.y + dy;
                    let gex = end.x + dx;   let gey = end.y + dy;
                    let off = *offset;
                    let (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y) = if *horizontal {
                        let x1 = gsx.min(gex); let x2 = gsx.max(gex);
                        let (p1x, p1y) = world_to_screen(x1 as f32, gsy as f32, viewport);
                        let (p2x, p2y) = world_to_screen(x2 as f32, gey as f32, viewport);
                        let (dl1x, dl1y) = world_to_screen(x1 as f32, ((gsy+gey)*0.5 + off) as f32, viewport);
                        let (dl2x, dl2y) = world_to_screen(x2 as f32, ((gsy+gey)*0.5 + off) as f32, viewport);
                        (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y)
                    } else {
                        let y1 = gsy.min(gey); let y2 = gsy.max(gey);
                        let (p1x, p1y) = world_to_screen(gsx as f32, y1 as f32, viewport);
                        let (p2x, p2y) = world_to_screen(gex as f32, y2 as f32, viewport);
                        let (dl1x, dl1y) = world_to_screen(((gsx+gex)*0.5 + off) as f32, y1 as f32, viewport);
                        let (dl2x, dl2y) = world_to_screen(((gsx+gex)*0.5 + off) as f32, y2 as f32, viewport);
                        (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y)
                    };
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p1x, p1y), rect.min + egui::vec2(dl1x, dl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p2x, p2y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                }
                EntityKind::DimRadial { center, radius, leader_pt, .. } => {
                    let gcx = (center.x + dx) as f32; let gcy = (center.y + dy) as f32;
                    let glx = (leader_pt.x + dx) as f32; let gly = (leader_pt.y + dy) as f32;
                    let (csx, csy) = world_to_screen(gcx, gcy, viewport);
                    let (lsx, lsy) = world_to_screen(glx, gly, viewport);
                    let (rsx, _) = world_to_screen(gcx + *radius as f32, gcy, viewport);
                    let (bx2, _) = world_to_screen(gcx, gcy, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(csx, csy), (rsx - bx2).abs(), ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(csx, csy), rect.min + egui::vec2(lsx, lsy)], ghost_stroke);
                }
                EntityKind::Text { .. } => {} // text ghost not rendered (position marker only)
            }
        }
    }

    /// Exit copy mode.
    fn exit_copy(&mut self) {
        self.copy_phase = CopyPhase::Idle;
        self.copy_base_point = None;
        self.copy_entities.clear();
    }

    /// Create copies of `copy_entities` offset by `dest - copy_base_point`.
    /// Stays in Destination phase so the user can make additional copies.
    fn apply_copy(&mut self, dest: Vec2) {
        let base = match self.copy_base_point {
            Some(b) => b,
            None => return,
        };
        let dx = dest.x - base.x;
        let dy = dest.y - base.y;
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            self.command_log.push("COPY: Zero distance, skipped".to_string());
            return;
        }
        let requested: Vec<Guid> = self.copy_entities.clone();
        let ids = self.filter_editable_entity_ids(&requested, "COPY");
        if ids.is_empty() {
            self.command_log.push("COPY: No editable entities selected".to_string());
            return;
        }
        self.push_undo();
        let mut count = 0usize;
        for id in &ids {
            if let Some(entity) = self.drawing.get_entity(id) {
                let new_kind = match &entity.kind {
                    EntityKind::Line { start, end } => EntityKind::Line {
                        start: Vec3::xy(start.x + dx, start.y + dy),
                        end: Vec3::xy(end.x + dx, end.y + dy),
                    },
                    EntityKind::Circle { center, radius } => EntityKind::Circle {
                        center: Vec3::xy(center.x + dx, center.y + dy),
                        radius: *radius,
                    },
                    EntityKind::Arc { center, radius, start_angle, end_angle } => EntityKind::Arc {
                        center: Vec3::xy(center.x + dx, center.y + dy),
                        radius: *radius,
                        start_angle: *start_angle,
                        end_angle: *end_angle,
                    },
                    EntityKind::Polyline { vertices, closed } => EntityKind::Polyline {
                        vertices: vertices.iter().map(|v| Vec3::xy(v.x + dx, v.y + dy)).collect(),
                        closed: *closed,
                    },
                    EntityKind::DimAligned { start, end, offset, text_override, text_pos, arrow_length, arrow_half_width } => EntityKind::DimAligned {
                        start: Vec3::xy(start.x + dx, start.y + dy),
                        end:   Vec3::xy(end.x   + dx, end.y   + dy),
                        offset: *offset,
                        text_override: text_override.clone(),
                        text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
                        arrow_length: *arrow_length,
                        arrow_half_width: *arrow_half_width,
                    },
                    EntityKind::DimLinear { start, end, offset, text_override, text_pos, horizontal, arrow_length, arrow_half_width } => EntityKind::DimLinear {
                        start: Vec3::xy(start.x + dx, start.y + dy),
                        end:   Vec3::xy(end.x   + dx, end.y   + dy),
                        offset: *offset,
                        text_override: text_override.clone(),
                        text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
                        horizontal: *horizontal,
                        arrow_length: *arrow_length,
                        arrow_half_width: *arrow_half_width,
                    },
                    EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, text_override, text_pos, arrow_length, arrow_half_width } => EntityKind::DimAngular {
                        vertex:   Vec3::xy(vertex.x   + dx, vertex.y   + dy),
                        line1_pt: Vec3::xy(line1_pt.x + dx, line1_pt.y + dy),
                        line2_pt: Vec3::xy(line2_pt.x + dx, line2_pt.y + dy),
                        radius: *radius,
                        text_override: text_override.clone(),
                        text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
                        arrow_length: *arrow_length,
                        arrow_half_width: *arrow_half_width,
                    },
                    EntityKind::DimRadial { center, radius, leader_pt, is_diameter, text_override, text_pos, arrow_length, arrow_half_width } => EntityKind::DimRadial {
                        center: Vec3::xy(center.x + dx, center.y + dy),
                        radius: *radius,
                        leader_pt: Vec3::xy(leader_pt.x + dx, leader_pt.y + dy),
                        is_diameter: *is_diameter,
                        text_override: text_override.clone(),
                        text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
                        arrow_length: *arrow_length,
                        arrow_half_width: *arrow_half_width,
                    },
                    EntityKind::Text { position, content, height, rotation } => EntityKind::Text {
                        position: Vec3::xy(position.x + dx, position.y + dy),
                        content: content.clone(),
                        height: *height,
                        rotation: *rotation,
                    },
                };
                let layer = entity.layer;
                self.drawing.add_entity(Entity::new(new_kind, layer));
                count += 1;
            }
        }
        self.command_log.push(format!("COPY: {} entit{} copied (pick next point or Enter to finish)",
            count, if count == 1 { "y" } else { "ies" }));
        // Stay in Destination phase for additional copies.
    }

    fn clone_kind_translated(kind: &EntityKind, dx: f64, dy: f64) -> EntityKind {
        match kind {
            EntityKind::Line { start, end } => EntityKind::Line {
                start: Vec3::xy(start.x + dx, start.y + dy),
                end: Vec3::xy(end.x + dx, end.y + dy),
            },
            EntityKind::Circle { center, radius } => EntityKind::Circle {
                center: Vec3::xy(center.x + dx, center.y + dy),
                radius: *radius,
            },
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => EntityKind::Arc {
                center: Vec3::xy(center.x + dx, center.y + dy),
                radius: *radius,
                start_angle: *start_angle,
                end_angle: *end_angle,
            },
            EntityKind::Polyline { vertices, closed } => EntityKind::Polyline {
                vertices: vertices
                    .iter()
                    .map(|v| Vec3::xy(v.x + dx, v.y + dy))
                    .collect(),
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
                start: Vec3::xy(start.x + dx, start.y + dy),
                end: Vec3::xy(end.x + dx, end.y + dy),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
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
                start: Vec3::xy(start.x + dx, start.y + dy),
                end: Vec3::xy(end.x + dx, end.y + dy),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
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
                vertex: Vec3::xy(vertex.x + dx, vertex.y + dy),
                line1_pt: Vec3::xy(line1_pt.x + dx, line1_pt.y + dy),
                line2_pt: Vec3::xy(line2_pt.x + dx, line2_pt.y + dy),
                radius: *radius,
                text_override: text_override.clone(),
                text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
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
                center: Vec3::xy(center.x + dx, center.y + dy),
                radius: *radius,
                leader_pt: Vec3::xy(leader_pt.x + dx, leader_pt.y + dy),
                is_diameter: *is_diameter,
                text_override: text_override.clone(),
                text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::Text {
                position,
                content,
                height,
                rotation,
            } => EntityKind::Text {
                position: Vec3::xy(position.x + dx, position.y + dy),
                content: content.clone(),
                height: *height,
                rotation: *rotation,
            },
        }
    }

    fn clone_kind_rotated(kind: &EntityKind, center: Vec2, angle_rad: f64) -> EntityKind {
        let (cos_a, sin_a) = (angle_rad.cos(), angle_rad.sin());
        let rotate_pt = |p: Vec3| -> Vec3 {
            let dx = p.x - center.x;
            let dy = p.y - center.y;
            Vec3::xy(
                center.x + dx * cos_a - dy * sin_a,
                center.y + dx * sin_a + dy * cos_a,
            )
        };
        match kind {
            EntityKind::Line { start, end } => EntityKind::Line {
                start: rotate_pt(*start),
                end: rotate_pt(*end),
            },
            EntityKind::Circle { center, radius } => EntityKind::Circle {
                center: rotate_pt(*center),
                radius: *radius,
            },
            EntityKind::Arc {
                center: c,
                radius,
                start_angle,
                end_angle,
            } => EntityKind::Arc {
                center: rotate_pt(*c),
                radius: *radius,
                start_angle: *start_angle + angle_rad,
                end_angle: *end_angle + angle_rad,
            },
            EntityKind::Polyline { vertices, closed } => EntityKind::Polyline {
                vertices: vertices.iter().map(|v| rotate_pt(*v)).collect(),
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
                start: rotate_pt(*start),
                end: rotate_pt(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: rotate_pt(*text_pos),
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
                start: rotate_pt(*start),
                end: rotate_pt(*end),
                offset: *offset,
                text_override: text_override.clone(),
                text_pos: rotate_pt(*text_pos),
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
                vertex: rotate_pt(*vertex),
                line1_pt: rotate_pt(*line1_pt),
                line2_pt: rotate_pt(*line2_pt),
                radius: *radius,
                text_override: text_override.clone(),
                text_pos: rotate_pt(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::DimRadial {
                center: c,
                radius,
                leader_pt,
                is_diameter,
                text_override,
                text_pos,
                arrow_length,
                arrow_half_width,
            } => EntityKind::DimRadial {
                center: rotate_pt(*c),
                radius: *radius,
                leader_pt: rotate_pt(*leader_pt),
                is_diameter: *is_diameter,
                text_override: text_override.clone(),
                text_pos: rotate_pt(*text_pos),
                arrow_length: *arrow_length,
                arrow_half_width: *arrow_half_width,
            },
            EntityKind::Text {
                position,
                content,
                height,
                rotation,
            } => EntityKind::Text {
                position: rotate_pt(*position),
                content: content.clone(),
                height: *height,
                rotation: *rotation + angle_rad,
            },
        }
    }

    fn cleanup_assoc_rect_arrays(&mut self) {
        self.assoc_member_to_array.clear();
        let ids: Vec<Guid> = self.assoc_rect_arrays.keys().copied().collect();
        let mut dead = Vec::new();
        for aid in ids {
            let Some(arr) = self.assoc_rect_arrays.get_mut(&aid) else { continue };
            arr.members.retain(|id| self.drawing.get_entity(id).is_some());
            if arr.members.is_empty() {
                dead.push(aid);
                continue;
            }
            for m in &arr.members {
                self.assoc_member_to_array.insert(*m, aid);
            }
        }
        for aid in dead {
            self.assoc_rect_arrays.remove(&aid);
        }
    }

    fn explode_assoc_rect_array(&mut self, aid: Guid) -> bool {
        self.cleanup_assoc_rect_arrays();
        let Some(arr) = self.assoc_rect_arrays.remove(&aid) else {
            return false;
        };
        for m in arr.members {
            self.assoc_member_to_array.remove(&m);
        }
        true
    }

    fn try_start_array_edit_from_selection(&mut self, requested: &[Guid]) -> bool {
        self.cleanup_assoc_rect_arrays();
        let Some(first_member) = requested
            .iter()
            .find(|id| self.assoc_member_to_array.contains_key(id))
            .copied()
        else {
            return false;
        };
        let Some(aid) = self.assoc_member_to_array.get(&first_member).copied() else {
            return false;
        };
        let Some(arr) = self.assoc_rect_arrays.get(&aid).cloned() else {
            return false;
        };
        self.array_mode = ArrayMode::Rectangular;
        self.array_entities = arr.members.clone();
        self.array_rect_columns = arr.cols.max(1);
        self.array_rect_rows = arr.rows.max(1);
        self.array_rect_dx = arr.dx;
        self.array_rect_dy = arr.dy;
        self.array_center = Some(arr.base);
        self.array_rect_dir_point = Some(arr.direction);
        self.array_rect_y_sign = if arr.dy < 0.0 { -1.0 } else { 1.0 };
        self.array_edit_assoc = Some(arr.id);
        self.array_phase = ArrayPhase::RectGripIdle;
        self.command_log.push(
            "ARRAY: Editing associative array. Grip-edit, Enter=apply, E=explode".to_string(),
        );
        true
    }

    fn apply_array_rectangular(&mut self, base: Vec2, direction: Vec2) -> bool {
        let cols = self.array_rect_columns.max(1);
        let rows = self.array_rect_rows.max(1);
        if cols == 1 && rows == 1 {
            self.command_log
                .push("ARRAY: Need at least 2 items (columns/rows)".to_string());
            return false;
        }
        let dir = if self.ortho_enabled {
            Self::snap_angle(base, direction, self.ortho_increment_deg)
        } else {
            direction
        };
        let vx = dir.x - base.x;
        let vy = dir.y - base.y;
        let vlen = (vx * vx + vy * vy).sqrt();
        if vlen <= 1e-9 {
            self.command_log
                .push("ARRAY: Direction point too close to base".to_string());
            return false;
        }
        let ux = vx / vlen;
        let uy = vy / vlen;
        let px = -uy;
        let py = ux;
        let dx_step = self.array_rect_dx;
        let dy_step = self.array_rect_dy;
        if dx_step.abs() <= 1e-9 && dy_step.abs() <= 1e-9 {
            self.command_log.push("ARRAY: Spacing too small".to_string());
            return false;
        }

        self.push_undo();
        self.cleanup_assoc_rect_arrays();

        let (array_id, source_defs) = if let Some(aid) = self.array_edit_assoc {
            let Some(existing) = self.assoc_rect_arrays.get(&aid).cloned() else {
                self.command_log
                    .push("ARRAY: Associative array not found; select entities again".to_string());
                return false;
            };
            for m in &existing.members {
                let _ = self.drawing.remove_entity(m);
                self.assoc_member_to_array.remove(m);
            }
            (aid, existing.source)
        } else {
            let ids = self.filter_editable_entity_ids(&self.array_entities.clone(), "ARRAY");
            if ids.is_empty() {
                self.command_log
                    .push("ARRAY: No editable entities selected".to_string());
                return false;
            }

            // If selection is members of exactly one associative array, treat it as editing that
            // block-like source instead of arraying each member as independent sources.
            let maybe_existing_aid = self
                .assoc_member_to_array
                .get(&ids[0])
                .copied()
                .filter(|aid| ids.iter().all(|id| self.assoc_member_to_array.get(id) == Some(aid)));
            if let Some(aid) = maybe_existing_aid {
                if let Some(existing) = self.assoc_rect_arrays.get(&aid).cloned() {
                    for m in &existing.members {
                        let _ = self.drawing.remove_entity(m);
                        self.assoc_member_to_array.remove(m);
                    }
                    (aid, existing.source)
                } else {
                    self.command_log
                        .push("ARRAY: Associative array source missing".to_string());
                    return false;
                }
            } else {
            let mut defs = Vec::new();
            for id in &ids {
                if let Some(src) = self.drawing.get_entity(id) {
                    defs.push(AssocArraySource {
                        kind: src.kind.clone(),
                        layer: src.layer,
                        color: src.color,
                    });
                }
            }
            for id in &ids {
                let _ = self.drawing.remove_entity(id);
                if let Some(aid) = self.assoc_member_to_array.remove(id) {
                    let _ = self.explode_assoc_rect_array(aid);
                }
            }
            (Guid::new(), defs)
            }
        };

        if source_defs.is_empty() {
            self.command_log.push("ARRAY: No source geometry".to_string());
            return false;
        }

        let mut members = Vec::new();
        let mut created = 0usize;
        for r in 0..rows {
            for c in 0..cols {
                let ox = ux * dx_step * c as f64 + px * dy_step * r as f64;
                let oy = uy * dx_step * c as f64 + py * dy_step * r as f64;
                for src in &source_defs {
                    let kind = Self::clone_kind_translated(&src.kind, ox, oy);
                    let mut e = Entity::new(kind, src.layer);
                    e.color = src.color;
                    let id = self.drawing.add_entity(e);
                    members.push(id);
                    self.assoc_member_to_array.insert(id, array_id);
                    created += 1;
                }
            }
        }
        self.assoc_rect_arrays.insert(
            array_id,
            AssocRectArray {
                id: array_id,
                members: members.clone(),
                source: source_defs,
                base,
                direction: dir,
                cols,
                rows,
                dx: dx_step,
                dy: dy_step,
            },
        );
        self.array_edit_assoc = Some(array_id);
        self.array_entities = members.clone();
        self.selected_entities.clear();
        if let Some(first) = members.first() {
            self.selected_entities.insert(*first);
        }
        self.command_log.push(format!(
            "ARRAY RECTANGULAR (associative): {} entit{} in array",
            created,
            if created == 1 { "y" } else { "ies" }
        ));
        true
    }

    fn apply_array_polar(&mut self, center: Vec2, base: Vec2) -> bool {
        let base = if self.ortho_enabled {
            Self::snap_angle(center, base, self.ortho_increment_deg)
        } else {
            base
        };
        let dx = base.x - center.x;
        let dy = base.y - center.y;
        let dlen = (dx * dx + dy * dy).sqrt();
        if dlen <= 1e-9 {
            self.command_log
                .push("ARRAY: Base/reference point too close to center".to_string());
            return false;
        }
        let count = self.array_polar_count.max(2);
        let fill = self.array_polar_angle_deg.to_radians();
        let step = if (self.array_polar_angle_deg - 360.0).abs() <= 1e-6 {
            std::f64::consts::TAU / count as f64
        } else if count <= 1 {
            0.0
        } else {
            fill / (count - 1) as f64
        };
        if step.abs() <= 1e-12 {
            self.command_log.push("ARRAY: Invalid angular step".to_string());
            return false;
        }

        let ids = self.filter_editable_entity_ids(&self.array_entities.clone(), "ARRAY");
        if ids.is_empty() {
            self.command_log
                .push("ARRAY: No editable entities selected".to_string());
            return false;
        }

        self.push_undo();
        let mut created = 0usize;
        for i in 1..count {
            let ang = step * i as f64;
            for id in &ids {
                if let Some(src) = self.drawing.get_entity(id) {
                    let kind = Self::clone_kind_rotated(&src.kind, center, ang);
                    self.drawing.add_entity(Entity::new(kind, src.layer));
                    created += 1;
                }
            }
        }
        self.command_log.push(format!(
            "ARRAY POLAR: {} entit{} created",
            created,
            if created == 1 { "y" } else { "ies" }
        ));
        true
    }

    /// Draw the COPY rubber-band line and ghost entities during Destination phase.
    fn draw_copy_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        if self.copy_phase != CopyPhase::Destination {
            return;
        }
        let base = match self.copy_base_point {
            Some(b) => b,
            None => return,
        };
        let dx = world_cursor.x - base.x;
        let dy = world_cursor.y - base.y;
        let painter = ui.painter_at(rect);

        // Rubber-band line from base to cursor.
        let (bx, by) = world_to_screen(base.x as f32, base.y as f32, viewport);
        let (cx, cy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
        painter.line_segment(
            [rect.min + egui::vec2(bx, by), rect.min + egui::vec2(cx, cy)],
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(180, 180, 180, 140)),
        );

        // Ghost entities at copy position (green tint to distinguish from MOVE).
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(120, 210, 160, 140));
        for id in &self.copy_entities {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, .. } => {
                    let gv  = cadkit_types::Vec2::new(vertex.x   + dx, vertex.y   + dy);
                    let gp1 = cadkit_types::Vec2::new(line1_pt.x + dx, line1_pt.y + dy);
                    let gp2 = cadkit_types::Vec2::new(line2_pt.x + dx, line2_pt.y + dy);
                    let (a1, a2) = angular_arc_angles(gv, gp1, gp2);
                    let pts = angular_arc_pts(gv, a1, a2, *radius);
                    if let (Some(arc_s), Some(arc_e)) = (pts.first(), pts.last()) {
                        let to_s = |p: cadkit_types::Vec2| { let (sx,sy) = world_to_screen(p.x as f32, p.y as f32, viewport); rect.min + egui::vec2(sx,sy) };
                        painter.line_segment([to_s(gp1), to_s(*arc_s)], ghost_stroke);
                        painter.line_segment([to_s(gp2), to_s(*arc_e)], ghost_stroke);
                    }
                    let spts: Vec<egui::Pos2> = pts.iter().map(|p| { let (sx,sy) = world_to_screen(p.x as f32, p.y as f32, viewport); rect.min + egui::vec2(sx,sy) }).collect();
                    for w in spts.windows(2) { painter.line_segment([w[0], w[1]], ghost_stroke); }
                }
                EntityKind::Line { start, end } => {
                    let (x1, y1) = world_to_screen((start.x + dx) as f32, (start.y + dy) as f32, viewport);
                    let (x2, y2) = world_to_screen((end.x + dx) as f32, (end.y + dy) as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        ghost_stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let gx = center.x + dx;
                    let gy = center.y + dy;
                    let (sx, sy) = world_to_screen(gx as f32, gy as f32, viewport);
                    let (rx, _) = world_to_screen((gx + radius) as f32, gy as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(sx, sy), (rx - sx).abs(), ghost_stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let gx = center.x + dx;
                    let gy = center.y + dy;
                    let sweep = *end_angle - *start_angle;
                    let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = *start_angle + sweep * t;
                        let px = gx + *radius * ang.cos();
                        let py = gy + *radius * ang.sin();
                        let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last { painter.line_segment([prev, pos], ghost_stroke); }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    let shifted: Vec<egui::Pos2> = vertices.iter().map(|v| {
                        let (sx, sy) = world_to_screen((v.x + dx) as f32, (v.y + dy) as f32, viewport);
                        rect.min + egui::vec2(sx, sy)
                    }).collect();
                    for w in shifted.windows(2) {
                        painter.line_segment([w[0], w[1]], ghost_stroke);
                    }
                    if *closed && shifted.len() >= 2 {
                        painter.line_segment([*shifted.last().unwrap(), shifted[0]], ghost_stroke);
                    }
                }
                EntityKind::DimAligned { start, end, offset, .. } => {
                    let gsx = (start.x + dx) as f32; let gsy = (start.y + dy) as f32;
                    let gex = (end.x   + dx) as f32; let gey = (end.y   + dy) as f32;
                    let ddx2 = gex - gsx; let ddy2 = gey - gsy;
                    let glen = (ddx2*ddx2 + ddy2*ddy2).sqrt();
                    if glen < 1e-6 { continue; }
                    let perp = [-ddy2/glen, ddx2/glen];
                    let off = *offset as f32;
                    let (p1x, p1y) = world_to_screen(gsx, gsy, viewport);
                    let (p2x, p2y) = world_to_screen(gex, gey, viewport);
                    let (dl1x, dl1y) = world_to_screen(gsx + perp[0]*off, gsy + perp[1]*off, viewport);
                    let (dl2x, dl2y) = world_to_screen(gex + perp[0]*off, gey + perp[1]*off, viewport);
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p1x, p1y), rect.min + egui::vec2(dl1x, dl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p2x, p2y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                }
                EntityKind::DimLinear { start, end, offset, horizontal, .. } => {
                    let gsx = start.x + dx; let gsy = start.y + dy;
                    let gex = end.x + dx;   let gey = end.y + dy;
                    let off = *offset;
                    let (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y) = if *horizontal {
                        let x1 = gsx.min(gex); let x2 = gsx.max(gex);
                        let (p1x, p1y) = world_to_screen(x1 as f32, gsy as f32, viewport);
                        let (p2x, p2y) = world_to_screen(x2 as f32, gey as f32, viewport);
                        let (dl1x, dl1y) = world_to_screen(x1 as f32, ((gsy+gey)*0.5 + off) as f32, viewport);
                        let (dl2x, dl2y) = world_to_screen(x2 as f32, ((gsy+gey)*0.5 + off) as f32, viewport);
                        (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y)
                    } else {
                        let y1 = gsy.min(gey); let y2 = gsy.max(gey);
                        let (p1x, p1y) = world_to_screen(gsx as f32, y1 as f32, viewport);
                        let (p2x, p2y) = world_to_screen(gex as f32, y2 as f32, viewport);
                        let (dl1x, dl1y) = world_to_screen(((gsx+gex)*0.5 + off) as f32, y1 as f32, viewport);
                        let (dl2x, dl2y) = world_to_screen(((gsx+gex)*0.5 + off) as f32, y2 as f32, viewport);
                        (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y)
                    };
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p1x, p1y), rect.min + egui::vec2(dl1x, dl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p2x, p2y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                }
                EntityKind::DimRadial { center, radius, leader_pt, .. } => {
                    let gcx = (center.x + dx) as f32; let gcy = (center.y + dy) as f32;
                    let glx = (leader_pt.x + dx) as f32; let gly = (leader_pt.y + dy) as f32;
                    let (csx, csy) = world_to_screen(gcx, gcy, viewport);
                    let (lsx, lsy) = world_to_screen(glx, gly, viewport);
                    let (rsx, _) = world_to_screen(gcx + *radius as f32, gcy, viewport);
                    let (bx2, _) = world_to_screen(gcx, gcy, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(csx, csy), (rsx - bx2).abs(), ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(csx, csy), rect.min + egui::vec2(lsx, lsy)], ghost_stroke);
                }
                EntityKind::Text { .. } => {}
            }
        }
    }

    /// Exit rotate mode.
    fn exit_rotate(&mut self) {
        self.rotate_phase = RotatePhase::Idle;
        self.rotate_base_point = None;
        self.rotate_entities.clear();
    }

    /// Rotate all `rotate_entities` by `angle_rad` around `rotate_base_point`.
    fn apply_rotate(&mut self, angle_rad: f64) {
        let base = match self.rotate_base_point {
            Some(b) => b,
            None => return,
        };
        if angle_rad.abs() < 1e-9 {
            self.command_log.push("ROTATE: Zero angle, nothing rotated".to_string());
            self.exit_rotate();
            return;
        }
        let (cos_a, sin_a) = (angle_rad.cos(), angle_rad.sin());

        fn rotate_pt(p: Vec3, bx: f64, by: f64, cos_a: f64, sin_a: f64) -> Vec3 {
            let dx = p.x - bx;
            let dy = p.y - by;
            Vec3::xy(bx + dx * cos_a - dy * sin_a, by + dx * sin_a + dy * cos_a)
        }

        let requested: Vec<Guid> = self.rotate_entities.clone();
        let ids = self.filter_editable_entity_ids(&requested, "ROTATE");
        if ids.is_empty() {
            self.command_log.push("ROTATE: No editable entities selected".to_string());
            self.exit_rotate();
            return;
        }
        self.push_undo();
        for id in &ids {
            if let Some(entity) = self.drawing.get_entity_mut(id) {
                match &mut entity.kind {
                    EntityKind::Line { start, end } => {
                        *start = rotate_pt(*start, base.x, base.y, cos_a, sin_a);
                        *end   = rotate_pt(*end,   base.x, base.y, cos_a, sin_a);
                    }
                    EntityKind::Circle { center, .. } => {
                        *center = rotate_pt(*center, base.x, base.y, cos_a, sin_a);
                    }
                    EntityKind::Arc { center, start_angle, end_angle, .. } => {
                        *center = rotate_pt(*center, base.x, base.y, cos_a, sin_a);
                        *start_angle += angle_rad;
                        *end_angle   += angle_rad;
                    }
                    EntityKind::Polyline { vertices, .. } => {
                        for v in vertices.iter_mut() {
                            *v = rotate_pt(*v, base.x, base.y, cos_a, sin_a);
                        }
                    }
                    EntityKind::DimAligned { start, end, text_pos, .. }
                    | EntityKind::DimLinear { start, end, text_pos, .. } => {
                        *start    = rotate_pt(*start,    base.x, base.y, cos_a, sin_a);
                        *end      = rotate_pt(*end,      base.x, base.y, cos_a, sin_a);
                        *text_pos = rotate_pt(*text_pos, base.x, base.y, cos_a, sin_a);
                        // offset scalar is preserved by rotation
                    }
                    EntityKind::DimAngular { vertex, line1_pt, line2_pt, text_pos, .. } => {
                        *vertex   = rotate_pt(*vertex,   base.x, base.y, cos_a, sin_a);
                        *line1_pt = rotate_pt(*line1_pt, base.x, base.y, cos_a, sin_a);
                        *line2_pt = rotate_pt(*line2_pt, base.x, base.y, cos_a, sin_a);
                        *text_pos = rotate_pt(*text_pos, base.x, base.y, cos_a, sin_a);
                    }
                    EntityKind::DimRadial { center, leader_pt, text_pos, .. } => {
                        *center    = rotate_pt(*center,    base.x, base.y, cos_a, sin_a);
                        *leader_pt = rotate_pt(*leader_pt, base.x, base.y, cos_a, sin_a);
                        *text_pos  = rotate_pt(*text_pos,  base.x, base.y, cos_a, sin_a);
                    }
                    EntityKind::Text { position, rotation, .. } => {
                        *position = rotate_pt(*position, base.x, base.y, cos_a, sin_a);
                        *rotation += angle_rad;
                    }
                }
            }
        }
        self.selected_entities = ids.into_iter().collect();
        self.command_log.push(format!("ROTATE: {:.2}°", angle_rad.to_degrees()));
        self.exit_rotate();
    }

    /// Draw rubber-band line + rotated ghost during Rotation phase.
    fn draw_rotate_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        if self.rotate_phase != RotatePhase::Rotation {
            return;
        }
        let base = match self.rotate_base_point {
            Some(b) => b,
            None => return,
        };
        let angle_rad = (world_cursor.y - base.y).atan2(world_cursor.x - base.x);
        let (cos_a, sin_a) = (angle_rad.cos(), angle_rad.sin());
        let painter = ui.painter_at(rect);

        // Rubber-band line from base to cursor.
        let (bx, by) = world_to_screen(base.x as f32, base.y as f32, viewport);
        let (cx, cy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
        painter.line_segment(
            [rect.min + egui::vec2(bx, by), rect.min + egui::vec2(cx, cy)],
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(180, 180, 180, 140)),
        );

        // Ghost entities at rotated position (orange tint).
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(210, 160, 80, 160));

        let rot = |p: Vec3| -> (f32, f32) {
            let dx = p.x - base.x;
            let dy = p.y - base.y;
            let rx = base.x + dx * cos_a - dy * sin_a;
            let ry = base.y + dx * sin_a + dy * cos_a;
            world_to_screen(rx as f32, ry as f32, viewport)
        };

        for id in &self.rotate_entities {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let (x1, y1) = rot(*start);
                    let (x2, y2) = rot(*end);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        ghost_stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let (sx, sy) = rot(*center);
                    let (rx, _) = world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                    let (bx2, _) = world_to_screen(center.x as f32, center.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(sx, sy), (rx - bx2).abs(), ghost_stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let (gcx, gcy) = rot(*center);
                    let (rx, _) = world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                    let (bx2, _) = world_to_screen(center.x as f32, center.y as f32, viewport);
                    let screen_r = (rx - bx2).abs();
                    let sa = start_angle + angle_rad;
                    let ea = end_angle + angle_rad;
                    let sweep = ea - sa;
                    let steps = ((sweep.abs() * radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = sa + sweep * t;
                        let px = gcx + screen_r * ang.cos() as f32;
                        let py = gcy - screen_r * ang.sin() as f32; // screen Y is flipped
                        let pos = rect.min + egui::vec2(px, py);
                        if let Some(prev) = last { painter.line_segment([prev, pos], ghost_stroke); }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    let pts: Vec<egui::Pos2> = vertices.iter().map(|v| {
                        let (sx, sy) = rot(*v);
                        rect.min + egui::vec2(sx, sy)
                    }).collect();
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]], ghost_stroke);
                    }
                    if *closed && pts.len() >= 2 {
                        painter.line_segment([*pts.last().unwrap(), pts[0]], ghost_stroke);
                    }
                }
                EntityKind::DimAligned { start, end, offset, .. } => {
                    let (rs1x, rs1y) = rot(*start);
                    let (rs2x, rs2y) = rot(*end);
                    let ddx = end.x - start.x;
                    let ddy = end.y - start.y;
                    let glen = (ddx*ddx + ddy*ddy).sqrt();
                    if glen < 1e-9 { continue; }
                    let perp = Vec3::xy(-ddy/glen, ddx/glen);
                    let off = *offset;
                    let dl1 = Vec3::xy(start.x + perp.x * off, start.y + perp.y * off);
                    let dl2 = Vec3::xy(end.x   + perp.x * off, end.y   + perp.y * off);
                    let (rdl1x, rdl1y) = rot(dl1);
                    let (rdl2x, rdl2y) = rot(dl2);
                    painter.line_segment([rect.min + egui::vec2(rdl1x, rdl1y), rect.min + egui::vec2(rdl2x, rdl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(rs1x, rs1y), rect.min + egui::vec2(rdl1x, rdl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(rs2x, rs2y), rect.min + egui::vec2(rdl2x, rdl2y)], ghost_stroke);
                }
                EntityKind::DimLinear { start, end, offset, horizontal, .. } => {
                    // Rotate the start/end points and approximate the dim line.
                    let (rs1x, rs1y) = rot(*start);
                    let (rs2x, rs2y) = rot(*end);
                    let off = *offset;
                    // dim line endpoints in world space before rotation
                    let mid_x = (start.x + end.x) * 0.5;
                    let mid_y = (start.y + end.y) * 0.5;
                    let (dl1, dl2) = if *horizontal {
                        let x1 = start.x.min(end.x); let x2 = start.x.max(end.x);
                        (Vec3::xy(x1, mid_y + off), Vec3::xy(x2, mid_y + off))
                    } else {
                        let y1 = start.y.min(end.y); let y2 = start.y.max(end.y);
                        (Vec3::xy(mid_x + off, y1), Vec3::xy(mid_x + off, y2))
                    };
                    let (rdl1x, rdl1y) = rot(dl1);
                    let (rdl2x, rdl2y) = rot(dl2);
                    painter.line_segment([rect.min + egui::vec2(rdl1x, rdl1y), rect.min + egui::vec2(rdl2x, rdl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(rs1x, rs1y), rect.min + egui::vec2(rdl1x, rdl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(rs2x, rs2y), rect.min + egui::vec2(rdl2x, rdl2y)], ghost_stroke);
                }
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, .. } => {
                    use std::f64::consts::TAU;
                    let a1 = (line1_pt.y - vertex.y).atan2(line1_pt.x - vertex.x) + angle_rad;
                    let mut a2 = (line2_pt.y - vertex.y).atan2(line2_pt.x - vertex.x) + angle_rad;
                    if a2 <= a1 { a2 += TAU; }
                    let rad = *radius;
                    let sweep = a2 - a1;
                    let steps = ((sweep.abs() * rad).max(6.0) as usize).clamp(8, 48);
                    let (rvx, rvy) = rot(*vertex);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = a1 + sweep * t;
                        let wx = vertex.x + rad * ang.cos();
                        let wy = vertex.y + rad * ang.sin();
                        // rotate the arc point around base
                        let (rx, ry) = rot(Vec3::xy(wx, wy));
                        let pos = rect.min + egui::vec2(rx, ry);
                        if let Some(prev) = last { painter.line_segment([prev, pos], ghost_stroke); }
                        last = Some(pos);
                    }
                    // Extension lines
                    let (r1x, r1y) = rot(*line1_pt);
                    let (r2x, r2y) = rot(*line2_pt);
                    let _ = (rvx, rvy, r1x, r1y, r2x, r2y);
                }
                EntityKind::DimRadial { center, radius, leader_pt, .. } => {
                    let (rcx, rcy) = rot(*center);
                    let (rlx, rly) = rot(*leader_pt);
                    let (cx_px, _) = world_to_screen(center.x as f32, center.y as f32, viewport);
                    let (rx_px, _) = world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                    let screen_r = (rx_px - cx_px).abs();
                    painter.circle_stroke(rect.min + egui::vec2(rcx, rcy), screen_r, ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(rcx, rcy), rect.min + egui::vec2(rlx, rly)], ghost_stroke);
                }
                EntityKind::Text { .. } => {}
            }
        }
    }

    /// Exit scale mode.
    fn exit_scale(&mut self) {
        self.scale_phase = ScalePhase::Idle;
        self.scale_base_point = None;
        self.scale_ref_point = None;
        self.scale_entities.clear();
    }

    fn apply_scale_from_point(&mut self, world: Vec2) {
        let (Some(base), Some(reference)) = (self.scale_base_point, self.scale_ref_point) else {
            return;
        };
        let base_to_ref = base.distance_to(&reference);
        if base_to_ref <= 1e-9 {
            self.command_log.push("SCALE: Invalid reference length".to_string());
            self.exit_scale();
            return;
        }
        let factor = base.distance_to(&world) / base_to_ref;
        self.apply_scale_factor(factor);
    }

    fn apply_scale_factor(&mut self, factor: f64) {
        let base = match self.scale_base_point {
            Some(b) => b,
            None => return,
        };
        if !factor.is_finite() || factor <= 1e-9 {
            self.command_log.push("SCALE: Factor must be > 0".to_string());
            return;
        }
        if (factor - 1.0).abs() < 1e-9 {
            self.command_log.push("SCALE: Factor 1.0, nothing scaled".to_string());
            self.exit_scale();
            return;
        }

        let scale_pt = |p: Vec3| -> Vec3 {
            Vec3::xy(
                base.x + (p.x - base.x) * factor,
                base.y + (p.y - base.y) * factor,
            )
        };

        let requested: Vec<Guid> = self.scale_entities.clone();
        let ids = self.filter_editable_entity_ids(&requested, "SCALE");
        if ids.is_empty() {
            self.command_log.push("SCALE: No editable entities selected".to_string());
            self.exit_scale();
            return;
        }

        self.push_undo();
        for id in &ids {
            if let Some(entity) = self.drawing.get_entity_mut(id) {
                match &mut entity.kind {
                    EntityKind::Line { start, end } => {
                        *start = scale_pt(*start);
                        *end = scale_pt(*end);
                    }
                    EntityKind::Circle { center, radius } => {
                        *center = scale_pt(*center);
                        *radius *= factor;
                    }
                    EntityKind::Arc { center, radius, .. } => {
                        *center = scale_pt(*center);
                        *radius *= factor;
                    }
                    EntityKind::Polyline { vertices, .. } => {
                        for v in vertices.iter_mut() {
                            *v = scale_pt(*v);
                        }
                    }
                    EntityKind::DimAligned { start, end, offset, text_pos, .. } => {
                        *start = scale_pt(*start);
                        *end = scale_pt(*end);
                        *text_pos = scale_pt(*text_pos);
                        *offset *= factor;
                    }
                    EntityKind::DimLinear { start, end, offset, text_pos, .. } => {
                        *start = scale_pt(*start);
                        *end = scale_pt(*end);
                        *text_pos = scale_pt(*text_pos);
                        *offset *= factor;
                    }
                    EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, text_pos, .. } => {
                        *vertex = scale_pt(*vertex);
                        *line1_pt = scale_pt(*line1_pt);
                        *line2_pt = scale_pt(*line2_pt);
                        *text_pos = scale_pt(*text_pos);
                        *radius *= factor;
                    }
                    EntityKind::DimRadial { center, radius, leader_pt, text_pos, .. } => {
                        *center = scale_pt(*center);
                        *leader_pt = scale_pt(*leader_pt);
                        *text_pos = scale_pt(*text_pos);
                        *radius *= factor;
                    }
                    EntityKind::Text { position, height, .. } => {
                        *position = scale_pt(*position);
                        *height *= factor;
                    }
                }
            }
        }
        self.selected_entities = ids.into_iter().collect();
        self.command_log.push(format!("SCALE: factor {:.4}", factor));
        self.exit_scale();
    }

    /// Draw SCALE preview: base/ref/factor rubber-band and scaled ghost entities.
    fn draw_scale_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        if !matches!(self.scale_phase, ScalePhase::ReferencePoint | ScalePhase::Factor) {
            return;
        }
        let Some(base) = self.scale_base_point else { return };
        let painter = ui.painter_at(rect);
        let guide_stroke =
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(180, 180, 180, 140));
        let ghost_stroke =
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(180, 120, 220, 150));

        let (bx, by) = world_to_screen(base.x as f32, base.y as f32, viewport);
        let bp = rect.min + egui::vec2(bx, by);

        let ref_point = self.scale_ref_point.unwrap_or(world_cursor);
        let (rx, ry) = world_to_screen(ref_point.x as f32, ref_point.y as f32, viewport);
        let rp = rect.min + egui::vec2(rx, ry);
        painter.line_segment([bp, rp], guide_stroke);

        let mut factor = 1.0;
        if self.scale_phase == ScalePhase::Factor {
            let (cx, cy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
            let cp = rect.min + egui::vec2(cx, cy);
            painter.line_segment([bp, cp], guide_stroke);
            let ref_len = base.distance_to(&ref_point);
            if ref_len > 1e-9 {
                factor = base.distance_to(&world_cursor) / ref_len;
            }
        }
        if !factor.is_finite() || factor <= 1e-9 {
            return;
        }

        let scale_pt = |p: Vec3| -> Vec3 {
            Vec3::xy(
                base.x + (p.x - base.x) * factor,
                base.y + (p.y - base.y) * factor,
            )
        };

        for id in &self.scale_entities {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let s = scale_pt(*start);
                    let e = scale_pt(*end);
                    let (x1, y1) = world_to_screen(s.x as f32, s.y as f32, viewport);
                    let (x2, y2) = world_to_screen(e.x as f32, e.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        ghost_stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let c = scale_pt(*center);
                    let r = radius * factor;
                    let (sx, sy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                    let (rx, _) = world_to_screen((c.x + r) as f32, c.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(sx, sy), (rx - sx).abs(), ghost_stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let c = scale_pt(*center);
                    let r = radius * factor;
                    let sweep = *end_angle - *start_angle;
                    let steps = ((sweep.abs() * r).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = *start_angle + sweep * t;
                        let px = c.x + r * ang.cos();
                        let py = c.y + r * ang.sin();
                        let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last {
                            painter.line_segment([prev, pos], ghost_stroke);
                        }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    let pts: Vec<egui::Pos2> = vertices.iter().map(|v| {
                        let p = scale_pt(*v);
                        let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                        rect.min + egui::vec2(sx, sy)
                    }).collect();
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]], ghost_stroke);
                    }
                    if *closed && pts.len() >= 2 {
                        painter.line_segment([*pts.last().unwrap(), pts[0]], ghost_stroke);
                    }
                }
                EntityKind::DimAligned { start, end, offset, .. } => {
                    let s = scale_pt(*start);
                    let e = scale_pt(*end);
                    let off = *offset * factor;
                    let gsx = s.x as f32; let gsy = s.y as f32;
                    let gex = e.x as f32; let gey = e.y as f32;
                    let ddx = gex - gsx; let ddy = gey - gsy;
                    let len = (ddx * ddx + ddy * ddy).sqrt();
                    if len < 1e-6 { continue; }
                    let perp = [-ddy / len, ddx / len];
                    let (p1x, p1y) = world_to_screen(gsx, gsy, viewport);
                    let (p2x, p2y) = world_to_screen(gex, gey, viewport);
                    let (dl1x, dl1y) =
                        world_to_screen(gsx + perp[0] * off as f32, gsy + perp[1] * off as f32, viewport);
                    let (dl2x, dl2y) =
                        world_to_screen(gex + perp[0] * off as f32, gey + perp[1] * off as f32, viewport);
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p1x, p1y), rect.min + egui::vec2(dl1x, dl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p2x, p2y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                }
                EntityKind::DimLinear { start, end, offset, horizontal, .. } => {
                    let s = scale_pt(*start);
                    let e = scale_pt(*end);
                    let off = *offset * factor;
                    let gsx = s.x; let gsy = s.y;
                    let gex = e.x; let gey = e.y;
                    let (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y) = if *horizontal {
                        let x1 = gsx.min(gex); let x2 = gsx.max(gex);
                        let (p1x, p1y) = world_to_screen(x1 as f32, gsy as f32, viewport);
                        let (p2x, p2y) = world_to_screen(x2 as f32, gey as f32, viewport);
                        let (dl1x, dl1y) = world_to_screen(x1 as f32, ((gsy + gey) * 0.5 + off) as f32, viewport);
                        let (dl2x, dl2y) = world_to_screen(x2 as f32, ((gsy + gey) * 0.5 + off) as f32, viewport);
                        (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y)
                    } else {
                        let y1 = gsy.min(gey); let y2 = gsy.max(gey);
                        let (p1x, p1y) = world_to_screen(gsx as f32, y1 as f32, viewport);
                        let (p2x, p2y) = world_to_screen(gex as f32, y2 as f32, viewport);
                        let (dl1x, dl1y) = world_to_screen(((gsx + gex) * 0.5 + off) as f32, y1 as f32, viewport);
                        let (dl2x, dl2y) = world_to_screen(((gsx + gex) * 0.5 + off) as f32, y2 as f32, viewport);
                        (p1x, p1y, p2x, p2y, dl1x, dl1y, dl2x, dl2y)
                    };
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p1x, p1y), rect.min + egui::vec2(dl1x, dl1y)], ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(p2x, p2y), rect.min + egui::vec2(dl2x, dl2y)], ghost_stroke);
                }
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, .. } => {
                    let v = scale_pt(*vertex);
                    let p1 = scale_pt(*line1_pt);
                    let p2 = scale_pt(*line2_pt);
                    let (a1, a2) = angular_arc_angles(v.into(), p1.into(), p2.into());
                    let pts = angular_arc_pts(v.into(), a1, a2, *radius * factor);
                    if let (Some(arc_s), Some(arc_e)) = (pts.first(), pts.last()) {
                        let to_s = |p: cadkit_types::Vec2| {
                            let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                            rect.min + egui::vec2(sx, sy)
                        };
                        painter.line_segment([to_s(p1.into()), to_s(*arc_s)], ghost_stroke);
                        painter.line_segment([to_s(p2.into()), to_s(*arc_e)], ghost_stroke);
                    }
                    let spts: Vec<egui::Pos2> = pts
                        .iter()
                        .map(|p| {
                            let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                            rect.min + egui::vec2(sx, sy)
                        })
                        .collect();
                    for w in spts.windows(2) {
                        painter.line_segment([w[0], w[1]], ghost_stroke);
                    }
                }
                EntityKind::DimRadial { center, radius, leader_pt, .. } => {
                    let c = scale_pt(*center);
                    let l = scale_pt(*leader_pt);
                    let r = radius * factor;
                    let (csx, csy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                    let (lsx, lsy) = world_to_screen(l.x as f32, l.y as f32, viewport);
                    let (rsx, _) = world_to_screen((c.x + r) as f32, c.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(csx, csy), (rsx - csx).abs(), ghost_stroke);
                    painter.line_segment([rect.min + egui::vec2(csx, csy), rect.min + egui::vec2(lsx, lsy)], ghost_stroke);
                }
                EntityKind::Text { position, .. } => {
                    let p = scale_pt(*position);
                    let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                    let pos = rect.min + egui::vec2(sx, sy);
                    painter.rect_stroke(
                        egui::Rect::from_center_size(pos, egui::vec2(10.0, 10.0)),
                        2.0,
                        ghost_stroke,
                    );
                }
            }
        }
    }

    /// Exit mirror mode.
    fn exit_mirror(&mut self) {
        self.mirror_phase = MirrorPhase::Idle;
        self.mirror_axis_p1 = None;
        self.mirror_entities.clear();
    }

    /// Mirror selected entities about axis line p1->p2.
    fn apply_mirror(&mut self, p1: Vec2, p2: Vec2) {
        let ax = p2.x - p1.x;
        let ay = p2.y - p1.y;
        let len = (ax * ax + ay * ay).sqrt();
        if len <= 1e-9 {
            self.command_log.push("MIRROR: Axis points too close".to_string());
            self.exit_mirror();
            return;
        }
        let ux = ax / len;
        let uy = ay / len;

        let reflect_vec2 = |x: f64, y: f64| -> Vec2 {
            let px = x - p1.x;
            let py = y - p1.y;
            let dot = px * ux + py * uy;
            let projx = ux * dot;
            let projy = uy * dot;
            Vec2::new(
                p1.x + (2.0 * projx - px),
                p1.y + (2.0 * projy - py),
            )
        };
        let reflect_pt = |p: Vec3| -> Vec3 {
            let r = reflect_vec2(p.x, p.y);
            Vec3::xy(r.x, r.y)
        };

        let requested: Vec<Guid> = self.mirror_entities.clone();
        let ids = self.filter_editable_entity_ids(&requested, "MIRROR");
        if ids.is_empty() {
            self.command_log.push("MIRROR: No editable entities selected".to_string());
            self.exit_mirror();
            return;
        }

        self.push_undo();
        for id in &ids {
            if let Some(entity) = self.drawing.get_entity_mut(id) {
                match &mut entity.kind {
                    EntityKind::Line { start, end } => {
                        *start = reflect_pt(*start);
                        *end = reflect_pt(*end);
                    }
                    EntityKind::Circle { center, .. } => {
                        *center = reflect_pt(*center);
                    }
                    EntityKind::Arc { center, radius, start_angle, end_angle } => {
                        let old_start = Vec3::xy(
                            center.x + *radius * start_angle.cos(),
                            center.y + *radius * start_angle.sin(),
                        );
                        let old_end = Vec3::xy(
                            center.x + *radius * end_angle.cos(),
                            center.y + *radius * end_angle.sin(),
                        );
                        let new_center = reflect_pt(*center);
                        let new_start_ref = reflect_pt(old_start);
                        let new_end_ref = reflect_pt(old_end);

                        // Mirroring reverses orientation; swap endpoints to keep stored arc CCW.
                        let mut ns = (new_end_ref.y - new_center.y).atan2(new_end_ref.x - new_center.x);
                        let mut ne = (new_start_ref.y - new_center.y).atan2(new_start_ref.x - new_center.x);
                        if ne <= ns {
                            ne += std::f64::consts::TAU;
                        }
                        if ns < 0.0 {
                            ns += std::f64::consts::TAU;
                        }
                        *center = new_center;
                        *start_angle = ns;
                        *end_angle = ne;
                    }
                    EntityKind::Polyline { vertices, .. } => {
                        for v in vertices.iter_mut() {
                            *v = reflect_pt(*v);
                        }
                    }
                    EntityKind::DimAligned { start, end, offset, text_pos, .. } => {
                        let old_s = *start;
                        let old_e = *end;
                        let old_dx = old_e.x - old_s.x;
                        let old_dy = old_e.y - old_s.y;
                        let old_len = (old_dx * old_dx + old_dy * old_dy).sqrt();
                        let old_perp = if old_len > 1e-9 {
                            Vec2::new(-old_dy / old_len, old_dx / old_len)
                        } else {
                            Vec2::new(0.0, 0.0)
                        };
                        let old_mid = Vec2::new((old_s.x + old_e.x) * 0.5, (old_s.y + old_e.y) * 0.5);
                        let old_dl = Vec2::new(old_mid.x + old_perp.x * *offset, old_mid.y + old_perp.y * *offset);

                        *start = reflect_pt(*start);
                        *end = reflect_pt(*end);
                        *text_pos = reflect_pt(*text_pos);
                        let new_dl = reflect_vec2(old_dl.x, old_dl.y);

                        let new_dx = end.x - start.x;
                        let new_dy = end.y - start.y;
                        let new_len = (new_dx * new_dx + new_dy * new_dy).sqrt();
                        if new_len > 1e-9 {
                            let new_perp = Vec2::new(-new_dy / new_len, new_dx / new_len);
                            let new_mid = Vec2::new((start.x + end.x) * 0.5, (start.y + end.y) * 0.5);
                            *offset = (new_dl.x - new_mid.x) * new_perp.x + (new_dl.y - new_mid.y) * new_perp.y;
                        }
                    }
                    EntityKind::DimLinear { start, end, offset, text_pos, horizontal, .. } => {
                        let old_mid_x = (start.x + end.x) * 0.5;
                        let old_mid_y = (start.y + end.y) * 0.5;
                        let old_dl = if *horizontal {
                            Vec2::new(old_mid_x, old_mid_y + *offset)
                        } else {
                            Vec2::new(old_mid_x + *offset, old_mid_y)
                        };
                        *start = reflect_pt(*start);
                        *end = reflect_pt(*end);
                        *text_pos = reflect_pt(*text_pos);
                        let new_dl = reflect_vec2(old_dl.x, old_dl.y);
                        let new_mid_x = (start.x + end.x) * 0.5;
                        let new_mid_y = (start.y + end.y) * 0.5;
                        *offset = if *horizontal {
                            new_dl.y - new_mid_y
                        } else {
                            new_dl.x - new_mid_x
                        };
                    }
                    EntityKind::DimAngular { vertex, line1_pt, line2_pt, text_pos, .. } => {
                        *vertex = reflect_pt(*vertex);
                        *line1_pt = reflect_pt(*line1_pt);
                        *line2_pt = reflect_pt(*line2_pt);
                        *text_pos = reflect_pt(*text_pos);
                    }
                    EntityKind::DimRadial { center, leader_pt, text_pos, .. } => {
                        *center = reflect_pt(*center);
                        *leader_pt = reflect_pt(*leader_pt);
                        *text_pos = reflect_pt(*text_pos);
                    }
                    EntityKind::Text { position, rotation, .. } => {
                        *position = reflect_pt(*position);
                        let vx = rotation.cos();
                        let vy = rotation.sin();
                        let dot = vx * ux + vy * uy;
                        let rvx = 2.0 * dot * ux - vx;
                        let rvy = 2.0 * dot * uy - vy;
                        *rotation = rvy.atan2(rvx);
                    }
                }
            }
        }
        self.selected_entities = ids.into_iter().collect();
        self.command_log.push("MIRROR: Complete".to_string());
        self.exit_mirror();
    }

    fn draw_mirror_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        if self.mirror_phase != MirrorPhase::SecondAxisPoint {
            return;
        }
        let Some(p1) = self.mirror_axis_p1 else { return };
        let p2 = if self.ortho_enabled {
            Self::snap_angle(p1, world_cursor, self.ortho_increment_deg)
        } else {
            world_cursor
        };
        let ax = p2.x - p1.x;
        let ay = p2.y - p1.y;
        let len = (ax * ax + ay * ay).sqrt();
        if len <= 1e-9 {
            return;
        }
        let ux = ax / len;
        let uy = ay / len;

        let reflect_pt = |p: Vec3| -> Vec3 {
            let px = p.x - p1.x;
            let py = p.y - p1.y;
            let dot = px * ux + py * uy;
            let projx = ux * dot;
            let projy = uy * dot;
            Vec3::xy(p1.x + (2.0 * projx - px), p1.y + (2.0 * projy - py))
        };

        let painter = ui.painter_at(rect);
        let axis_stroke =
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(220, 180, 80, 180));
        let ghost_stroke =
            egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(150, 210, 255, 150));

        let (x1, y1) = world_to_screen(p1.x as f32, p1.y as f32, viewport);
        let (x2, y2) = world_to_screen(p2.x as f32, p2.y as f32, viewport);
        painter.line_segment(
            [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
            axis_stroke,
        );

        for id in &self.mirror_entities {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let s = reflect_pt(*start);
                    let e = reflect_pt(*end);
                    let (sx, sy) = world_to_screen(s.x as f32, s.y as f32, viewport);
                    let (ex, ey) = world_to_screen(e.x as f32, e.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(sx, sy), rect.min + egui::vec2(ex, ey)],
                        ghost_stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let c = reflect_pt(*center);
                    let (cx, cy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                    let (rx, _) = world_to_screen((c.x + radius) as f32, c.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(cx, cy), (rx - cx).abs(), ghost_stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let c = reflect_pt(*center);
                    let old_start = Vec3::xy(
                        center.x + *radius * start_angle.cos(),
                        center.y + *radius * start_angle.sin(),
                    );
                    let old_end = Vec3::xy(
                        center.x + *radius * end_angle.cos(),
                        center.y + *radius * end_angle.sin(),
                    );
                    let new_start_ref = reflect_pt(old_start);
                    let new_end_ref = reflect_pt(old_end);
                    let mut sa = (new_end_ref.y - c.y).atan2(new_end_ref.x - c.x);
                    let mut ea = (new_start_ref.y - c.y).atan2(new_start_ref.x - c.x);
                    if ea <= sa {
                        ea += std::f64::consts::TAU;
                    }
                    if sa < 0.0 {
                        sa += std::f64::consts::TAU;
                    }
                    let sweep = ea - sa;
                    let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = sa + sweep * t;
                        let px = c.x + *radius * ang.cos();
                        let py = c.y + *radius * ang.sin();
                        let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last {
                            painter.line_segment([prev, pos], ghost_stroke);
                        }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    let pts: Vec<egui::Pos2> = vertices
                        .iter()
                        .map(|v| {
                            let p = reflect_pt(*v);
                            let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                            rect.min + egui::vec2(sx, sy)
                        })
                        .collect();
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]], ghost_stroke);
                    }
                    if *closed && pts.len() >= 2 {
                        painter.line_segment([*pts.last().unwrap(), pts[0]], ghost_stroke);
                    }
                }
                EntityKind::DimAligned { start, end, .. }
                | EntityKind::DimLinear { start, end, .. } => {
                    let s = reflect_pt(*start);
                    let e = reflect_pt(*end);
                    let (sx, sy) = world_to_screen(s.x as f32, s.y as f32, viewport);
                    let (ex, ey) = world_to_screen(e.x as f32, e.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(sx, sy), rect.min + egui::vec2(ex, ey)],
                        ghost_stroke,
                    );
                }
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, .. } => {
                    let v = reflect_pt(*vertex);
                    let p1r = reflect_pt(*line1_pt);
                    let p2r = reflect_pt(*line2_pt);
                    let (vx, vy) = world_to_screen(v.x as f32, v.y as f32, viewport);
                    let (a1x, a1y) = world_to_screen(p1r.x as f32, p1r.y as f32, viewport);
                    let (a2x, a2y) = world_to_screen(p2r.x as f32, p2r.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(vx, vy), rect.min + egui::vec2(a1x, a1y)],
                        ghost_stroke,
                    );
                    painter.line_segment(
                        [rect.min + egui::vec2(vx, vy), rect.min + egui::vec2(a2x, a2y)],
                        ghost_stroke,
                    );
                }
                EntityKind::DimRadial { center, leader_pt, .. } => {
                    let c = reflect_pt(*center);
                    let l = reflect_pt(*leader_pt);
                    let (cx, cy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                    let (lx, ly) = world_to_screen(l.x as f32, l.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(cx, cy), rect.min + egui::vec2(lx, ly)],
                        ghost_stroke,
                    );
                }
                EntityKind::Text { position, .. } => {
                    let p = reflect_pt(*position);
                    let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                    let pos = rect.min + egui::vec2(sx, sy);
                    painter.rect_stroke(
                        egui::Rect::from_center_size(pos, egui::vec2(10.0, 10.0)),
                        2.0,
                        ghost_stroke,
                    );
                }
            }
        }
    }

    fn exit_fillet(&mut self) {
        self.fillet_phase = FilletPhase::Idle;
    }

    fn try_pick_fillet_edge(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<FilletPick> {
        // Use a fillet-specific hit test so non-edge entities don't steal the pick.
        let mut best: Option<(f32, Guid)> = None;
        for entity in self.drawing.visible_entities() {
            let eligible = matches!(
                entity.kind,
                EntityKind::Line { .. } | EntityKind::Polyline { .. }
            );
            if !eligible {
                continue;
            }
            let d = Self::screen_dist_to_entity(&entity.kind, viewport, rect, screen_pos);
            if d <= Self::PICK_RADIUS && best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                best = Some((d, entity.id));
            }
        }
        let id = best.map(|(_, id)| id)?;
        let entity = self.drawing.get_entity(&id)?;
        let local = screen_pos - rect.min;
        let click = screen_to_world(local.x, local.y, viewport);
        match &entity.kind {
            EntityKind::Line { start, end } => Some(FilletPick {
                entity: id,
                click,
                seg_start: Vec2::new(start.x, start.y),
                seg_end: Vec2::new(end.x, end.y),
                seg_start_index: None,
                seg_end_index: None,
            }),
            EntityKind::Polyline { vertices, closed } if vertices.len() >= 2 => {
                let mut best: Option<(f64, usize, usize, Vec2, Vec2)> = None;
                for i in 0..vertices.len() - 1 {
                    let a = Vec2::new(vertices[i].x, vertices[i].y);
                    let b = Vec2::new(vertices[i + 1].x, vertices[i + 1].y);
                    let d2 = point_seg_dist2(click, a, b);
                    if best.as_ref().map_or(true, |(bd2, ..)| d2 < *bd2) {
                        best = Some((d2, i, i + 1, a, b));
                    }
                }
                if *closed {
                    let i = vertices.len() - 1;
                    let j = 0usize;
                    let a = Vec2::new(vertices[i].x, vertices[i].y);
                    let b = Vec2::new(vertices[j].x, vertices[j].y);
                    let d2 = point_seg_dist2(click, a, b);
                    if best.as_ref().map_or(true, |(bd2, ..)| d2 < *bd2) {
                        best = Some((d2, i, j, a, b));
                    }
                }
                let (_, i, j, a, b) = best?;
                Some(FilletPick {
                    entity: id,
                    click,
                    seg_start: a,
                    seg_end: b,
                    seg_start_index: Some(i),
                    seg_end_index: Some(j),
                })
            }
            _ => None,
        }
    }

    fn apply_chamfer(&mut self, first: FilletPick, second: FilletPick) -> bool {
        if self.chamfer_distance1 < 0.0
            || self.chamfer_distance2 < 0.0
            || !self.chamfer_distance1.is_finite()
            || !self.chamfer_distance2.is_finite()
        {
            self.command_log
                .push("CHAMFER: Distances must be >= 0".to_string());
            return false;
        }
        if first.entity == second.entity
            && (first.seg_start_index.is_none() || second.seg_start_index.is_none())
        {
            self.command_log
                .push("CHAMFER: Pick two polyline segments for same-entity chamfer".to_string());
            return false;
        }
        if self.is_entity_on_locked_layer(&first.entity)
            || self.is_entity_on_locked_layer(&second.entity)
        {
            self.command_log
                .push("CHAMFER: Cannot edit entities on locked layers".to_string());
            return false;
        }

        let Some(e1) = self.drawing.get_entity(&first.entity).cloned() else {
            self.command_log.push("CHAMFER: First entity missing".to_string());
            return false;
        };
        let Some(e2) = self.drawing.get_entity(&second.entity).cloned() else {
            self.command_log.push("CHAMFER: Second entity missing".to_string());
            return false;
        };

        let (l1s, l1e) = (first.seg_start, first.seg_end);
        let (l2s, l2e) = (second.seg_start, second.seg_end);
        let Some(v) = line_line_intersect(l1s, l1e, l2s, l2e) else {
            self.command_log.push("CHAMFER: Lines are parallel".to_string());
            return false;
        };

        let dir_from_line = |a: Vec2, b: Vec2, pick: Vec2| -> Option<Vec2> {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= 1e-9 {
                return None;
            }
            let ex = dx / len;
            let ey = dy / len;
            let mut s = (pick.x - v.x) * ex + (pick.y - v.y) * ey;
            if s.abs() <= 1e-9 {
                let d_a = (a.x - pick.x).powi(2) + (a.y - pick.y).powi(2);
                let d_b = (b.x - pick.x).powi(2) + (b.y - pick.y).powi(2);
                s = if d_a <= d_b { -1.0 } else { 1.0 };
            }
            let sign = if s >= 0.0 { 1.0 } else { -1.0 };
            Some(Vec2::new(ex * sign, ey * sign))
        };

        let Some(u1) = dir_from_line(l1s, l1e, first.click) else {
            self.command_log
                .push("CHAMFER: First line is degenerate".to_string());
            return false;
        };
        let Some(u2) = dir_from_line(l2s, l2e, second.click) else {
            self.command_log
                .push("CHAMFER: Second line is degenerate".to_string());
            return false;
        };

        let d1 = self.chamfer_distance1;
        let d2 = self.chamfer_distance2;
        let t1 = Vec2::new(v.x + u1.x * d1, v.y + u1.y * d1);
        let t2 = Vec2::new(v.x + u2.x * d2, v.y + u2.y * d2);

        let on_seg = |p: Vec2, a: Vec2, b: Vec2| -> bool {
            let min_x = a.x.min(b.x) - 1e-6;
            let max_x = a.x.max(b.x) + 1e-6;
            let min_y = a.y.min(b.y) - 1e-6;
            let max_y = a.y.max(b.y) + 1e-6;
            let cross = ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)).abs();
            cross <= 1e-4 && p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
        };
        let first_is_line = matches!(e1.kind, EntityKind::Line { .. });
        let second_is_line = matches!(e2.kind, EntityKind::Line { .. });
        if (!on_seg(t1, l1s, l1e) && !first_is_line) || (!on_seg(t2, l2s, l2e) && !second_is_line) {
            self.command_log.push(
                "CHAMFER: Distance too large for picked polyline segments".to_string(),
            );
            return false;
        }

        if first.entity == second.entity {
            let Some(i0) = first.seg_start_index else {
                self.command_log
                    .push("CHAMFER: Invalid first polyline segment".to_string());
                return false;
            };
            let Some(i1) = first.seg_end_index else {
                self.command_log
                    .push("CHAMFER: Invalid first polyline segment".to_string());
                return false;
            };
            let Some(j0) = second.seg_start_index else {
                self.command_log
                    .push("CHAMFER: Invalid second polyline segment".to_string());
                return false;
            };
            let Some(j1) = second.seg_end_index else {
                self.command_log
                    .push("CHAMFER: Invalid second polyline segment".to_string());
                return false;
            };

            let (verts, closed, layer) = match &e1.kind {
                EntityKind::Polyline { vertices, closed } => (vertices.clone(), *closed, e1.layer),
                _ => {
                    self.command_log
                        .push("CHAMFER: Same-entity chamfer requires a polyline".to_string());
                    return false;
                }
            };
            let n = verts.len();
            if n < 3 {
                self.command_log
                    .push("CHAMFER: Polyline needs at least 3 vertices".to_string());
                return false;
            }

            let shared = if i0 == j0 || i0 == j1 {
                Some(i0)
            } else if i1 == j0 || i1 == j1 {
                Some(i1)
            } else {
                None
            };
            let Some(shared_idx) = shared else {
                self.command_log
                    .push("CHAMFER: Same-polyline chamfer requires adjacent segments".to_string());
                return false;
            };
            if !closed && (shared_idx == 0 || shared_idx + 1 >= n) {
                self.command_log
                    .push("CHAMFER: Same-polyline chamfer requires an interior corner".to_string());
                return false;
            }
            let prev_idx = if closed {
                (shared_idx + n - 1) % n
            } else {
                shared_idx - 1
            };
            let next_idx = if closed {
                (shared_idx + 1) % n
            } else {
                shared_idx + 1
            };
            let seg_is = |a: usize, b: usize, u: usize, v: usize| {
                (a == u && b == v) || (a == v && b == u)
            };
            let first_is_prev = seg_is(i0, i1, prev_idx, shared_idx);
            let first_is_next = seg_is(i0, i1, shared_idx, next_idx);
            let second_is_prev = seg_is(j0, j1, prev_idx, shared_idx);
            let second_is_next = seg_is(j0, j1, shared_idx, next_idx);
            let tp = if first_is_prev {
                Some(t1)
            } else if second_is_prev {
                Some(t2)
            } else {
                None
            };
            let tn = if first_is_next {
                Some(t1)
            } else if second_is_next {
                Some(t2)
            } else {
                None
            };
            let (Some(t_prev), Some(t_next)) = (tp, tn) else {
                self.command_log
                    .push("CHAMFER: Same-polyline chamfer requires adjacent corner segments".to_string());
                return false;
            };

            self.push_undo();
            let _ = self.drawing.remove_entity(&first.entity);
            if closed {
                let mut loop_pts: Vec<Vec3> = Vec::new();
                loop_pts.push(Vec3::xy(t_next.x, t_next.y));
                let mut idx = (shared_idx + 1) % n;
                while idx != shared_idx {
                    loop_pts.push(verts[idx]);
                    idx = (idx + 1) % n;
                }
                if t_prev.distance_to(&t_next) > 1e-8 {
                    loop_pts.push(Vec3::xy(t_prev.x, t_prev.y));
                }
                self.drawing.add_entity(Entity::new(
                    EntityKind::Polyline {
                        vertices: loop_pts,
                        closed: true,
                    },
                    layer,
                ));
            } else {
                let mut joined: Vec<Vec3> = Vec::new();
                let mut push_unique = |p: Vec3| {
                    if joined.last().map_or(true, |q| {
                        ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt() > 1e-8
                    }) {
                        joined.push(p);
                    }
                };
                for vtx in verts.iter().take(prev_idx + 1) {
                    push_unique(*vtx);
                }
                push_unique(Vec3::xy(t_prev.x, t_prev.y));
                push_unique(Vec3::xy(t_next.x, t_next.y));
                for vtx in verts.iter().skip(next_idx) {
                    push_unique(*vtx);
                }
                self.drawing.add_entity(Entity::new(
                    EntityKind::Polyline {
                        vertices: joined,
                        closed: false,
                    },
                    layer,
                ));
            }
            self.command_log
                .push(format!("CHAMFER: d1={:.4}, d2={:.4}", d1, d2));
            return true;
        }

        self.push_undo();
        let use_l1_start = l1s.distance_to(&t1) <= l1e.distance_to(&t1);
        let use_l2_start = l2s.distance_to(&t2) <= l2e.distance_to(&t2);

        if let Some(e) = self.drawing.get_entity_mut(&first.entity) {
            match &mut e.kind {
                EntityKind::Line { start, end } => {
                    if use_l1_start {
                        start.x = t1.x;
                        start.y = t1.y;
                    } else {
                        end.x = t1.x;
                        end.y = t1.y;
                    }
                }
                EntityKind::Polyline { vertices, .. } => {
                    if let (Some(i0), Some(i1)) = (first.seg_start_index, first.seg_end_index) {
                        let target = if use_l1_start { i0 } else { i1 };
                        if let Some(vtx) = vertices.get_mut(target) {
                            vtx.x = t1.x;
                            vtx.y = t1.y;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = self.drawing.get_entity_mut(&second.entity) {
            match &mut e.kind {
                EntityKind::Line { start, end } => {
                    if use_l2_start {
                        start.x = t2.x;
                        start.y = t2.y;
                    } else {
                        end.x = t2.x;
                        end.y = t2.y;
                    }
                }
                EntityKind::Polyline { vertices, .. } => {
                    if let (Some(i0), Some(i1)) = (second.seg_start_index, second.seg_end_index) {
                        let target = if use_l2_start { i0 } else { i1 };
                        if let Some(vtx) = vertices.get_mut(target) {
                            vtx.x = t2.x;
                            vtx.y = t2.y;
                        }
                    }
                }
                _ => {}
            }
        }

        if t1.distance_to(&t2) > 1e-8 {
            let mut line = create_line(t1, t2);
            line.layer = e1.layer;
            self.drawing.add_entity(line);
        }
        self.command_log
            .push(format!("CHAMFER: d1={:.4}, d2={:.4}", d1, d2));
        true
    }

    fn apply_fillet(&mut self, first: FilletPick, second: FilletPick) -> bool {
        if (self.fillet_radius <= 1e-9) || !self.fillet_radius.is_finite() {
            self.command_log.push("FILLET: Radius must be > 0".to_string());
            return false;
        }
        if first.entity == second.entity
            && (first.seg_start_index.is_none() || second.seg_start_index.is_none())
        {
            self.command_log
                .push("FILLET: Pick two polyline segments for same-entity fillet".to_string());
            return false;
        }
        if self.is_entity_on_locked_layer(&first.entity)
            || self.is_entity_on_locked_layer(&second.entity)
        {
            self.command_log
                .push("FILLET: Cannot edit entities on locked layers".to_string());
            return false;
        }

        let Some(e1) = self.drawing.get_entity(&first.entity).cloned() else {
            self.command_log.push("FILLET: First entity missing".to_string());
            return false;
        };
        let Some(_e2) = self.drawing.get_entity(&second.entity).cloned() else {
            self.command_log.push("FILLET: Second entity missing".to_string());
            return false;
        };
        let (l1s, l1e) = (first.seg_start, first.seg_end);
        let (l2s, l2e) = (second.seg_start, second.seg_end);

        let Some(v) = line_line_intersect(l1s, l1e, l2s, l2e) else {
            self.command_log.push("FILLET: Lines are parallel".to_string());
            return false;
        };

        let dir_from_line = |a: Vec2, b: Vec2, pick: Vec2| -> Option<Vec2> {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len <= 1e-9 {
                return None;
            }
            let ex = dx / len;
            let ey = dy / len;
            let mut s = (pick.x - v.x) * ex + (pick.y - v.y) * ey;
            if s.abs() <= 1e-9 {
                let d_a = (a.x - pick.x).powi(2) + (a.y - pick.y).powi(2);
                let d_b = (b.x - pick.x).powi(2) + (b.y - pick.y).powi(2);
                s = if d_a <= d_b { -1.0 } else { 1.0 };
            }
            let sign = if s >= 0.0 { 1.0 } else { -1.0 };
            Some(Vec2::new(ex * sign, ey * sign))
        };

        let Some(u1) = dir_from_line(l1s, l1e, first.click) else {
            self.command_log.push("FILLET: First line is degenerate".to_string());
            return false;
        };
        let Some(u2) = dir_from_line(l2s, l2e, second.click) else {
            self.command_log.push("FILLET: Second line is degenerate".to_string());
            return false;
        };

        let dot = (u1.x * u2.x + u1.y * u2.y).clamp(-1.0, 1.0);
        let theta = dot.acos();
        if theta <= 1e-6 || (std::f64::consts::PI - theta).abs() <= 1e-6 {
            self.command_log.push("FILLET: Invalid corner angle".to_string());
            return false;
        }

        let t = self.fillet_radius / (theta * 0.5).tan();
        let h = self.fillet_radius / (theta * 0.5).sin();
        if !t.is_finite() || !h.is_finite() {
            self.command_log.push("FILLET: Invalid geometry".to_string());
            return false;
        }

        let t1 = Vec2::new(v.x + u1.x * t, v.y + u1.y * t);
        let t2 = Vec2::new(v.x + u2.x * t, v.y + u2.y * t);
        let bis = Vec2::new(u1.x + u2.x, u1.y + u2.y);
        let bis_len = (bis.x * bis.x + bis.y * bis.y).sqrt();
        if bis_len <= 1e-9 {
            self.command_log.push("FILLET: Invalid angle bisector".to_string());
            return false;
        }
        let c = Vec2::new(v.x + bis.x / bis_len * h, v.y + bis.y / bis_len * h);

        let on_seg = |p: Vec2, a: Vec2, b: Vec2| -> bool {
            let min_x = a.x.min(b.x) - 1e-6;
            let max_x = a.x.max(b.x) + 1e-6;
            let min_y = a.y.min(b.y) - 1e-6;
            let max_y = a.y.max(b.y) + 1e-6;
            let cross = ((b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)).abs();
            cross <= 1e-4 && p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
        };
        let first_is_line = matches!(e1.kind, EntityKind::Line { .. });
        let second_is_line = matches!(_e2.kind, EntityKind::Line { .. });
        let first_ok = on_seg(t1, l1s, l1e) || first_is_line;
        let second_ok = on_seg(t2, l2s, l2e) || second_is_line;
        if !first_ok || !second_ok {
            self.command_log.push(
                "FILLET: Radius too large for picked polyline segments".to_string(),
            );
            return false;
        }

        // Trim the endpoint that belongs to the corner side (closest to the tangent point).
        let use_l1_start = l1s.distance_to(&t1) <= l1e.distance_to(&t1);
        let use_l2_start = l2s.distance_to(&t2) <= l2e.distance_to(&t2);
        let (mut a1, mut a2) = ((t1.y - c.y).atan2(t1.x - c.x), (t2.y - c.y).atan2(t2.x - c.x));
        let mut span = Self::ccw_from(a1, a2);
        if span > std::f64::consts::PI {
            std::mem::swap(&mut a1, &mut a2);
            span = Self::ccw_from(a1, a2);
        }
        // Keep stored arc convention: CCW with end_angle > start_angle.
        if a2 <= a1 {
            a2 += std::f64::consts::TAU;
        }
        if span <= 1e-6 {
            self.command_log.push("FILLET: Degenerate fillet arc".to_string());
            return false;
        }

        // Polyline + line join mode: rebuild into one open polyline that includes
        // sampled arc points between tangency points (vertex-only polyline model).
        if first.entity != second.entity {
            let first_is_poly = matches!(e1.kind, EntityKind::Polyline { .. });
            let second_is_poly = matches!(_e2.kind, EntityKind::Polyline { .. });
            let first_is_line = matches!(e1.kind, EntityKind::Line { .. });
            let second_is_line = matches!(_e2.kind, EntityKind::Line { .. });
            if (first_is_poly && second_is_line) || (second_is_poly && first_is_line) {
                let (poly_id, line_id, poly_pick, line_pick, t_poly, t_line, use_line_start, poly_layer) =
                    if first_is_poly {
                        (
                            first.entity,
                            second.entity,
                            first,
                            second,
                            t1,
                            t2,
                            use_l2_start,
                            e1.layer,
                        )
                    } else {
                        (
                            second.entity,
                            first.entity,
                            second,
                            first,
                            t2,
                            t1,
                            use_l1_start,
                            _e2.layer,
                        )
                    };

                let Some(poly_entity) = self.drawing.get_entity(&poly_id).cloned() else {
                    self.command_log.push("FILLET: Polyline entity missing".to_string());
                    return false;
                };
                let Some(line_entity) = self.drawing.get_entity(&line_id).cloned() else {
                    self.command_log.push("FILLET: Line entity missing".to_string());
                    return false;
                };

                let (mut verts, closed) = match poly_entity.kind {
                    EntityKind::Polyline { vertices, closed } => (vertices, closed),
                    _ => {
                        self.command_log.push("FILLET: Polyline join requires a polyline".to_string());
                        return false;
                    }
                };
                let (line_start, line_end) = match line_entity.kind {
                    EntityKind::Line { start, end } => (start, end),
                    _ => {
                        self.command_log.push("FILLET: Polyline join requires a line".to_string());
                        return false;
                    }
                };
                if closed || verts.len() < 2 {
                    // Closed polyline join is handled by same-entity branch, or not supported in mixed mode.
                    self.command_log.push("FILLET: Polyline+line join requires an open polyline".to_string());
                    return false;
                }

                let Some(i0) = poly_pick.seg_start_index else {
                    self.command_log.push("FILLET: Invalid polyline segment pick".to_string());
                    return false;
                };
                let Some(i1) = poly_pick.seg_end_index else {
                    self.command_log.push("FILLET: Invalid polyline segment pick".to_string());
                    return false;
                };
                let target_idx = if poly_pick.seg_start.distance_to(&t_poly)
                    <= poly_pick.seg_end.distance_to(&t_poly)
                {
                    i0
                } else {
                    i1
                };
                let at_start = target_idx == 0;
                let at_end = target_idx + 1 == verts.len();
                if !at_start && !at_end {
                    // Keep existing behavior for interior segment fillets.
                } else {
                    let line_far = if use_line_start { line_end } else { line_start };
                    let far_pt = Vec3::xy(line_far.x, line_far.y);

                    let arc_pts = {
                        let start = if at_end { t_poly } else { t_line };
                        let end = if at_end { t_line } else { t_poly };
                        let mut sa = (start.y - c.y).atan2(start.x - c.x);
                        let mut ea = (end.y - c.y).atan2(end.x - c.x);
                        if Self::ccw_from(sa, ea) > std::f64::consts::PI {
                            std::mem::swap(&mut sa, &mut ea);
                        }
                        if ea <= sa {
                            ea += std::f64::consts::TAU;
                        }
                        let mut pts = angular_arc_pts(c, sa, ea, self.fillet_radius);
                        if let (Some(first_pt), Some(last_pt)) = (pts.first(), pts.last()) {
                            let d_first = first_pt.distance_to(&start);
                            let d_last = last_pt.distance_to(&start);
                            if d_last < d_first {
                                pts.reverse();
                            }
                        }
                        pts
                    };

                    let mut out: Vec<Vec3> = Vec::new();
                    let mut push_unique = |p: Vec3| {
                        if out
                            .last()
                            .map_or(true, |q| ((q.x - p.x).powi(2) + (q.y - p.y).powi(2)).sqrt() > 1e-6)
                        {
                            out.push(p);
                        }
                    };

                    if at_end {
                        let last = verts.len() - 1;
                        verts[last] = Vec3::xy(t_poly.x, t_poly.y);
                        for v in verts.iter().take(verts.len() - 1) {
                            push_unique(*v);
                        }
                        for p in arc_pts {
                            push_unique(Vec3::xy(p.x, p.y));
                        }
                        push_unique(far_pt);
                    } else {
                        verts[0] = Vec3::xy(t_poly.x, t_poly.y);
                        push_unique(far_pt);
                        for p in arc_pts {
                            push_unique(Vec3::xy(p.x, p.y));
                        }
                        for v in verts.iter().skip(1) {
                            push_unique(*v);
                        }
                    }

                    if out.len() >= 2 {
                        self.push_undo();
                        let _ = self.drawing.remove_entity(&poly_id);
                        let _ = self.drawing.remove_entity(&line_id);
                        self.drawing.add_entity(Entity::new(
                            EntityKind::Polyline {
                                vertices: out,
                                closed: false,
                            },
                            poly_layer,
                        ));
                        self.command_log
                            .push(format!("FILLET: r={:.4} (joined polyline)", self.fillet_radius));
                        return true;
                    }
                }
                let _ = line_pick; // keeps pattern explicit for future mixed-entity extensions
            }
        }

        // Same polyline fillet (adjacent segments): split into two trimmed polylines + fillet arc.
        if first.entity == second.entity {
            let Some(i0) = first.seg_start_index else {
                self.command_log.push("FILLET: Invalid first polyline segment".to_string());
                return false;
            };
            let Some(i1) = first.seg_end_index else {
                self.command_log.push("FILLET: Invalid first polyline segment".to_string());
                return false;
            };
            let Some(j0) = second.seg_start_index else {
                self.command_log.push("FILLET: Invalid second polyline segment".to_string());
                return false;
            };
            let Some(j1) = second.seg_end_index else {
                self.command_log.push("FILLET: Invalid second polyline segment".to_string());
                return false;
            };

            let (verts, closed, layer) = match &e1.kind {
                EntityKind::Polyline { vertices, closed } => (vertices.clone(), *closed, e1.layer),
                _ => {
                    self.command_log.push("FILLET: Same-entity fillet requires a polyline".to_string());
                    return false;
                }
            };
            if verts.len() < 3 {
                self.command_log.push("FILLET: Polyline needs at least 3 vertices".to_string());
                return false;
            }

            let shared = if i0 == j0 || i0 == j1 {
                Some(i0)
            } else if i1 == j0 || i1 == j1 {
                Some(i1)
            } else {
                None
            };
            let Some(shared_idx) = shared else {
                self.command_log
                    .push("FILLET: Same-polyline fillet requires adjacent segments".to_string());
                return false;
            };
            let other_first = if i0 == shared_idx { i1 } else { i0 };
            let other_second = if j0 == shared_idx { j1 } else { j0 };
            if other_first == other_second {
                self.command_log.push("FILLET: Select two different segments".to_string());
                return false;
            }
            // Map tangent points to the two sides of the corner: prev->shared and shared->next.
            let n = verts.len();
            if !closed && (shared_idx == 0 || shared_idx + 1 >= n) {
                self.command_log
                    .push("FILLET: Same-polyline fillet requires an interior corner".to_string());
                return false;
            }
            let prev_idx = if closed {
                (shared_idx + n - 1) % n
            } else {
                shared_idx - 1
            };
            let next_idx = if closed {
                (shared_idx + 1) % n
            } else {
                shared_idx + 1
            };
            let seg_is = |a: usize, b: usize, u: usize, v: usize| {
                (a == u && b == v) || (a == v && b == u)
            };
            let first_is_prev = seg_is(i0, i1, prev_idx, shared_idx);
            let first_is_next = seg_is(i0, i1, shared_idx, next_idx);
            let second_is_prev = seg_is(j0, j1, prev_idx, shared_idx);
            let second_is_next = seg_is(j0, j1, shared_idx, next_idx);
            let tp = if first_is_prev {
                Some(t1)
            } else if second_is_prev {
                Some(t2)
            } else {
                None
            };
            let tn = if first_is_next {
                Some(t1)
            } else if second_is_next {
                Some(t2)
            } else {
                None
            };
            let (Some(t_prev), Some(t_next)) = (tp, tn) else {
                self.command_log
                    .push("FILLET: Same-polyline fillet requires adjacent corner segments".to_string());
                return false;
            };

            self.push_undo();
            let _ = self.drawing.remove_entity(&first.entity);
            if closed {
                // Build one stitched loop from t_next around the ring to t_prev,
                // then add sampled arc points back to t_next so the fillet is inside the polyline.
                let mut loop_pts: Vec<Vec3> = Vec::new();
                loop_pts.push(Vec3::xy(t_next.x, t_next.y));
                let mut idx = (shared_idx + 1) % verts.len();
                while idx != shared_idx {
                    loop_pts.push(verts[idx]);
                    idx = (idx + 1) % verts.len();
                }
                loop_pts.push(Vec3::xy(t_prev.x, t_prev.y));

                let mut sa = (t_prev.y - c.y).atan2(t_prev.x - c.x);
                let mut ea = (t_next.y - c.y).atan2(t_next.x - c.x);
                if Self::ccw_from(sa, ea) > std::f64::consts::PI {
                    std::mem::swap(&mut sa, &mut ea);
                }
                if ea <= sa {
                    ea += std::f64::consts::TAU;
                }
                let mut arc_pts = angular_arc_pts(c, sa, ea, self.fillet_radius);
                if let (Some(first_pt), Some(last_pt)) = (arc_pts.first(), arc_pts.last()) {
                    if last_pt.distance_to(&t_prev) < first_pt.distance_to(&t_prev) {
                        arc_pts.reverse();
                    }
                }
                for p in arc_pts.into_iter().skip(1) {
                    loop_pts.push(Vec3::xy(p.x, p.y));
                }

                if loop_pts.len() >= 3 {
                    self.drawing.add_entity(Entity::new(
                        EntityKind::Polyline {
                            vertices: loop_pts,
                            closed: true,
                        },
                        layer,
                    ));
                }
                self.command_log
                    .push(format!("FILLET: r={:.4} (added to closed polyline)", self.fillet_radius));
                return true;
            } else {
                // Open polyline: rebuild as a single polyline with the fillet arc sampled in.
                let mut sa = (t_prev.y - c.y).atan2(t_prev.x - c.x);
                let mut ea = (t_next.y - c.y).atan2(t_next.x - c.x);
                if Self::ccw_from(sa, ea) > std::f64::consts::PI {
                    std::mem::swap(&mut sa, &mut ea);
                }
                if ea <= sa {
                    ea += std::f64::consts::TAU;
                }
                let mut arc_pts = angular_arc_pts(c, sa, ea, self.fillet_radius);
                if let (Some(first_pt), Some(last_pt)) = (arc_pts.first(), arc_pts.last()) {
                    if last_pt.distance_to(&t_prev) < first_pt.distance_to(&t_prev) {
                        arc_pts.reverse();
                    }
                }

                let mut joined: Vec<Vec3> = Vec::new();
                for v in verts.iter().take(prev_idx + 1) {
                    joined.push(*v);
                }
                joined.push(Vec3::xy(t_prev.x, t_prev.y));
                for p in arc_pts.into_iter().skip(1).take_while(|p| p.distance_to(&t_next) > 1e-8) {
                    joined.push(Vec3::xy(p.x, p.y));
                }
                joined.push(Vec3::xy(t_next.x, t_next.y));
                for v in verts.iter().skip(next_idx) {
                    joined.push(*v);
                }

                if joined.len() >= 2 {
                    self.drawing.add_entity(Entity::new(
                        EntityKind::Polyline {
                            vertices: joined,
                            closed: false,
                        },
                        layer,
                    ));
                    self.command_log.push(format!(
                        "FILLET: r={:.4} (added to open polyline)",
                        self.fillet_radius
                    ));
                    return true;
                }
                self.command_log.push("FILLET: Failed to rebuild polyline".to_string());
                return false;
            }
        }

        self.push_undo();

        if let Some(e) = self.drawing.get_entity_mut(&first.entity) {
            match &mut e.kind {
                EntityKind::Line { start, end } => {
                    if use_l1_start {
                        start.x = t1.x;
                        start.y = t1.y;
                    } else {
                        end.x = t1.x;
                        end.y = t1.y;
                    }
                }
                EntityKind::Polyline { vertices, .. } => {
                    if let (Some(i0), Some(i1)) = (first.seg_start_index, first.seg_end_index) {
                        let target = if use_l1_start { i0 } else { i1 };
                        if let Some(v) = vertices.get_mut(target) {
                            v.x = t1.x;
                            v.y = t1.y;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = self.drawing.get_entity_mut(&second.entity) {
            match &mut e.kind {
                EntityKind::Line { start, end } => {
                    if use_l2_start {
                        start.x = t2.x;
                        start.y = t2.y;
                    } else {
                        end.x = t2.x;
                        end.y = t2.y;
                    }
                }
                EntityKind::Polyline { vertices, .. } => {
                    if let (Some(i0), Some(i1)) = (second.seg_start_index, second.seg_end_index) {
                        let target = if use_l2_start { i0 } else { i1 };
                        if let Some(v) = vertices.get_mut(target) {
                            v.x = t2.x;
                            v.y = t2.y;
                        }
                    }
                }
                _ => {}
            }
        }

        let mut arc = create_arc(c, self.fillet_radius, a1, a2);
        arc.layer = e1.layer;
        self.drawing.add_entity(arc);
        self.command_log.push(format!(
            "FILLET: r={:.4}",
            self.fillet_radius
        ));
        true
    }

    /// Place a DimAligned entity. Called when the user clicks the dimension line location.
    /// After placement, resets to FirstPoint so the user can continue dimensioning.
    fn place_dim_aligned(&mut self, first: Vec2, second: Vec2, offset_world: Vec2) {
        let dx = second.x - first.x;
        let dy = second.y - first.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            self.command_log.push("DIMALIGNED: Degenerate dimension, ignored".to_string());
            return;
        }
        let dir = (dx / len, dy / len);
        let perp = (-dir.1, dir.0);
        let mx = (first.x + second.x) * 0.5;
        let my = (first.y + second.y) * 0.5;
        let offset = (offset_world.x - mx) * perp.0 + (offset_world.y - my) * perp.1;
        // Ensure minimum visible offset.
        let offset = if offset.abs() < 5.0 { if offset >= 0.0 { 5.0 } else { -5.0 } } else { offset };
        let text_pos = Vec3::xy(mx + perp.0 * offset, my + perp.1 * offset);
        self.push_undo();
        // Keep dimensions on a dedicated layer managed by current DimStyle.
        let dim_layer = self.ensure_dim_layer();
        let entity = Entity::new(
            EntityKind::DimAligned {
                start: Vec3::xy(first.x, first.y),
                end: Vec3::xy(second.x, second.y),
                offset,
                text_override: None,
                text_pos,
                arrow_length: self.dim_style.arrow_length,
                arrow_half_width: self.dim_style.arrow_half_width,
            },
            dim_layer,
        );
        self.drawing.add_entity(entity);
        self.command_log
            .push(format!("DIMALIGNED: Distance = {}", self.format_dim_measurement(len)));
        // Stay in FirstPoint so user can chain dimensions.
        self.dim_phase = DimPhase::FirstPoint;
    }

    /// Place a DimLinear (H or V locked) entity.
    /// `offset_world` is the cursor position during the Placing phase;
    /// the axis lock is determined by which displacement component is larger.
    fn place_dim_linear(&mut self, first: Vec2, second: Vec2, offset_world: Vec2) {
        let dx = (second.x - first.x).abs();
        let dy = (second.y - first.y).abs();
        if dx < 1e-6 && dy < 1e-6 {
            self.command_log.push("DIMLINEAR: Degenerate dimension, ignored".to_string());
            return;
        }
        // Axis lock: if the user drags mostly vertically from the midpoint,
        // the dimension is horizontal (measures X distance) and vice versa.
        let mid_x = (first.x + second.x) * 0.5;
        let mid_y = (first.y + second.y) * 0.5;
        let horizontal = (offset_world.y - mid_y).abs() > (offset_world.x - mid_x).abs();
        let offset = if horizontal {
            let raw = offset_world.y - mid_y;
            if raw.abs() < 5.0 { if raw >= 0.0 { 5.0 } else { -5.0 } } else { raw }
        } else {
            let raw = offset_world.x - mid_x;
            if raw.abs() < 5.0 { if raw >= 0.0 { 5.0 } else { -5.0 } } else { raw }
        };
        let text_pos = if horizontal {
            Vec3::xy(mid_x, mid_y + offset)
        } else {
            Vec3::xy(mid_x + offset, mid_y)
        };
        let dist = if horizontal { dx } else { dy };
        self.push_undo();
        let dim_layer = self.ensure_dim_layer();
        let entity = Entity::new(
            EntityKind::DimLinear {
                start: Vec3::xy(first.x, first.y),
                end: Vec3::xy(second.x, second.y),
                offset,
                text_override: None,
                text_pos,
                horizontal,
                arrow_length: self.dim_style.arrow_length,
                arrow_half_width: self.dim_style.arrow_half_width,
            },
            dim_layer,
        );
        self.drawing.add_entity(entity);
        self.command_log.push(format!(
            "DIMLINEAR: {} = {}",
            if horizontal { "Width" } else { "Height" },
            self.format_dim_measurement(dist)
        ));
        // Stay in FirstPoint for chaining.
        self.dim_linear_phase = DimLinearPhase::FirstPoint;
    }

    /// Place a DimAngular entity. `cursor` is used to determine the arc radius.
    fn place_dim_angular(&mut self, vertex: Vec2, line1_pt: Vec2, line2_pt: Vec2, cursor: Vec2) {
        let (a1, a2) = angular_arc_angles(vertex, line1_pt, line2_pt);
        let sweep_deg = (a2 - a1).to_degrees();
        if sweep_deg < 0.001 {
            self.command_log.push("DIMANGULAR: Degenerate angle, ignored".to_string());
            return;
        }
        let radius = ((cursor.x - vertex.x).powi(2) + (cursor.y - vertex.y).powi(2)).sqrt();
        let radius = radius.max(5.0);
        let mid_angle = a1 + (a2 - a1) * 0.5;
        let text_pos = Vec3::xy(
            vertex.x + radius * mid_angle.cos(),
            vertex.y + radius * mid_angle.sin(),
        );
        self.push_undo();
        let dim_layer = self.ensure_dim_layer();
        let entity = Entity::new(
            EntityKind::DimAngular {
                vertex:   Vec3::xy(vertex.x, vertex.y),
                line1_pt: Vec3::xy(line1_pt.x, line1_pt.y),
                line2_pt: Vec3::xy(line2_pt.x, line2_pt.y),
                radius,
                text_override: None,
                text_pos,
                arrow_length: self.dim_style.arrow_length,
                arrow_half_width: self.dim_style.arrow_half_width,
            },
            dim_layer,
        );
        self.drawing.add_entity(entity);
        self.command_log.push(format!(
            "DIMANGULAR: Angle = {:.prec$}°",
            sweep_deg, prec = self.dim_style.precision
        ));
        // Stay in FirstEntity phase for chaining another angular dim.
        self.dim_angular_phase = DimAngularPhase::FirstEntity;
    }

    /// Draw the DIMANGULAR rubber-band preview.
    fn draw_dim_angular_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(220, 210, 80, 180));
        let painter = ui.painter_at(rect);

        let to_screen = |wx: f64, wy: f64| -> egui::Pos2 {
            let (sx, sy) = world_to_screen(wx as f32, wy as f32, viewport);
            rect.min + egui::vec2(sx, sy)
        };

        match &self.dim_angular_phase {
            DimAngularPhase::FirstEntity => {
                // Small crosshair at cursor — waiting for first entity pick.
                let p = to_screen(world_cursor.x, world_cursor.y);
                let r = 6.0_f32;
                painter.line_segment([p - egui::vec2(r, 0.0), p + egui::vec2(r, 0.0)], ghost_stroke);
                painter.line_segment([p - egui::vec2(0.0, r), p + egui::vec2(0.0, r)], ghost_stroke);
            }
            DimAngularPhase::SecondEntity { first_start, first_end, .. } => {
                // Draw the locked first segment, plus crosshair at cursor.
                painter.line_segment(
                    [to_screen(first_start.x, first_start.y), to_screen(first_end.x, first_end.y)],
                    egui::Stroke::new(2.0, egui::Color32::from_rgba_premultiplied(80, 220, 220, 200)),
                );
                let p = to_screen(world_cursor.x, world_cursor.y);
                let r = 6.0_f32;
                painter.line_segment([p - egui::vec2(r, 0.0), p + egui::vec2(r, 0.0)], ghost_stroke);
                painter.line_segment([p - egui::vec2(0.0, r), p + egui::vec2(0.0, r)], ghost_stroke);
            }
            DimAngularPhase::Placing { vertex, line1_pt, line2_pt } => {
                let radius = ((world_cursor.x - vertex.x).powi(2) + (world_cursor.y - vertex.y).powi(2)).sqrt().max(5.0);
                let (a1, a2) = angular_arc_angles(*vertex, *line1_pt, *line2_pt);
                let pts = angular_arc_pts(*vertex, a1, a2, radius);
                // Extension lines
                let arc_start = pts.first().copied().unwrap_or(*line1_pt);
                let arc_end   = pts.last().copied().unwrap_or(*line2_pt);
                painter.line_segment([to_screen(line1_pt.x, line1_pt.y), to_screen(arc_start.x, arc_start.y)], ghost_stroke);
                painter.line_segment([to_screen(line2_pt.x, line2_pt.y), to_screen(arc_end.x,   arc_end.y  )], ghost_stroke);
                // Arc
                let screen_pts: Vec<egui::Pos2> = pts.iter().map(|p| to_screen(p.x, p.y)).collect();
                for w in screen_pts.windows(2) {
                    painter.line_segment([w[0], w[1]], ghost_stroke);
                }
                // Text preview
                let sweep_deg = (a2 - a1).to_degrees();
                let mid_angle = a1 + (a2 - a1) * 0.5;
                let text_world = cadkit_types::Vec2::new(
                    vertex.x + radius * mid_angle.cos(),
                    vertex.y + radius * mid_angle.sin(),
                );
                let label = format!("{:.prec$}°", sweep_deg, prec = self.dim_style.precision);
                let ghost_color = egui::Color32::from_rgba_premultiplied(220, 210, 80, 180);
                let font_size = (self.dim_style.text_height * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
                let galley = painter.ctx().fonts(|f| {
                    f.layout_no_wrap(label, egui::FontId::proportional(font_size), ghost_color)
                });
                let w = galley.size().x;
                let h = galley.size().y;
                let tc = to_screen(text_world.x, text_world.y);
                let anchor = tc - egui::vec2(w * 0.5, h * 0.5);
                painter.add(egui::Shape::Text(egui::epaint::TextShape {
                    pos: anchor, galley,
                    underline: egui::epaint::Stroke::NONE,
                    fallback_color: ghost_color,
                    override_text_color: None,
                    opacity_factor: 1.0,
                    angle: 0.0,
                }));
            }
            DimAngularPhase::Idle => {}
        }
    }

    /// Place a DimRadial (radius or diameter) entity.
    fn place_dim_radial(&mut self, center: Vec2, radius: f64, is_diameter: bool, cursor: Vec2) {
        let dx = cursor.x - center.x;
        let dy = cursor.y - center.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            self.command_log.push("DIMRADIAL: Degenerate leader, ignored".to_string());
            return;
        }
        let dim_layer = self.ensure_dim_layer();
        let text_pos = Vec3::xy(cursor.x, cursor.y);
        let label = if is_diameter {
            format!("Ø{}", self.format_dim_measurement(radius * 2.0))
        } else {
            format!("R{}", self.format_dim_measurement(radius))
        };
        self.push_undo();
        let entity = Entity::new(
            EntityKind::DimRadial {
                center: Vec3::xy(center.x, center.y),
                radius,
                leader_pt: Vec3::xy(cursor.x, cursor.y),
                is_diameter,
                text_override: None,
                text_pos,
                arrow_length: self.dim_style.arrow_length,
                arrow_half_width: self.dim_style.arrow_half_width,
            },
            dim_layer,
        );
        self.drawing.add_entity(entity);
        self.command_log.push(format!("DIMRADIAL: {}", label));
        // Chain back to SelectingEntity.
        self.dim_radial_phase = DimRadialPhase::SelectingEntity { is_diameter };
    }

    /// Draw the DIMRADIUS / DIMDIAMETER rubber-band preview.
    fn draw_dim_radial_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(220, 210, 80, 180));
        let ghost_color  = egui::Color32::from_rgba_premultiplied(220, 210, 80, 180);
        let painter = ui.painter_at(rect);

        let to_screen = |wx: f64, wy: f64| -> egui::Pos2 {
            let (sx, sy) = world_to_screen(wx as f32, wy as f32, viewport);
            rect.min + egui::vec2(sx, sy)
        };

        match &self.dim_radial_phase {
            DimRadialPhase::SelectingEntity { .. } => {
                let p = to_screen(world_cursor.x, world_cursor.y);
                let r = 6.0_f32;
                painter.line_segment([p - egui::vec2(r, 0.0), p + egui::vec2(r, 0.0)], ghost_stroke);
                painter.line_segment([p - egui::vec2(0.0, r), p + egui::vec2(0.0, r)], ghost_stroke);
            }
            DimRadialPhase::Placing { center, radius, is_diameter } => {
                let (csx, csy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                let (rx, _) = world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                let screen_r = (rx - csx).abs();
                let center_s = rect.min + egui::vec2(csx, csy);
                // Draw the circle outline
                painter.circle_stroke(center_s, screen_r, ghost_stroke);
                // Leader line from center to cursor
                let cursor_s = to_screen(world_cursor.x, world_cursor.y);
                painter.line_segment([center_s, cursor_s], ghost_stroke);
                // Text preview
                let val = if *is_diameter { radius * 2.0 } else { *radius };
                let prefix = if *is_diameter { "Ø" } else { "R" };
                let label = format!("{}{:.prec$}", prefix, val, prec = self.dim_style.precision);
                let font_size = (self.dim_style.text_height * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
                let galley = painter.ctx().fonts(|f| {
                    f.layout_no_wrap(label, egui::FontId::proportional(font_size), ghost_color)
                });
                let w = galley.size().x;
                let h = galley.size().y;
                let anchor = cursor_s - egui::vec2(w * 0.5, h * 0.5);
                painter.add(egui::Shape::Text(egui::epaint::TextShape {
                    pos: anchor, galley,
                    underline: egui::epaint::Stroke::NONE,
                    fallback_color: ghost_color,
                    override_text_color: None,
                    opacity_factor: 1.0,
                    angle: 0.0,
                }));
            }
            DimRadialPhase::Idle => {}
        }
    }

    /// Draw the DIMALIGNED rubber-band preview during SecondPoint and Placing phases.
    fn draw_dim_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(220, 210, 80, 180));
        let painter = ui.painter_at(rect);

        match &self.dim_phase {
            DimPhase::SecondPoint { first } => {
                // Draw tick at first point and rubber-band line to cursor.
                let (x1, y1) = world_to_screen(first.x as f32, first.y as f32, viewport);
                let p1 = rect.min + egui::vec2(x1, y1);
                let r = 5.0_f32;
                painter.line_segment([p1 - egui::vec2(r, r), p1 + egui::vec2(r, r)], ghost_stroke);
                painter.line_segment([p1 - egui::vec2(r, -r), p1 + egui::vec2(r, -r)], ghost_stroke);
                let (x2, y2) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
                let p2 = rect.min + egui::vec2(x2, y2);
                painter.line_segment([p1, p2], ghost_stroke);
            }
            DimPhase::Placing { first, second } => {
                let dx = second.x - first.x;
                let dy = second.y - first.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-6 { return; }
                let dir = [dx / len, dy / len];
                let perp = [-dir[1], dir[0]];
                let mx = (first.x + second.x) * 0.5;
                let my = (first.y + second.y) * 0.5;
                let offset = (world_cursor.x - mx) * perp[0] + (world_cursor.y - my) * perp[1];
                let offset = if offset.abs() < 5.0 { if offset >= 0.0 { 5.0 } else { -5.0 } } else { offset };
                let dl1 = [first.x + perp[0] * offset, first.y + perp[1] * offset];
                let dl2 = [second.x + perp[0] * offset, second.y + perp[1] * offset];

                let (sx1, sy1) = world_to_screen(first.x as f32, first.y as f32, viewport);
                let (sx2, sy2) = world_to_screen(second.x as f32, second.y as f32, viewport);
                let (dl1x, dl1y) = world_to_screen(dl1[0] as f32, dl1[1] as f32, viewport);
                let (dl2x, dl2y) = world_to_screen(dl2[0] as f32, dl2[1] as f32, viewport);

                let p_s1 = rect.min + egui::vec2(sx1, sy1);
                let p_s2 = rect.min + egui::vec2(sx2, sy2);
                let p_d1 = rect.min + egui::vec2(dl1x, dl1y);
                let p_d2 = rect.min + egui::vec2(dl2x, dl2y);

                // Extension lines
                painter.line_segment([p_s1, p_d1], ghost_stroke);
                painter.line_segment([p_s2, p_d2], ghost_stroke);
                // Dim line
                painter.line_segment([p_d1, p_d2], ghost_stroke);

                // Dimension text via egui painter (matches draw_dim_entities style).
                let dist_text = format!("{:.3}", len);
                let tc_world = [(dl1[0] + dl2[0]) * 0.5, (dl1[1] + dl2[1]) * 0.5];
                let (tcsx, tcsy) = world_to_screen(tc_world[0] as f32, tc_world[1] as f32, viewport);
                let text_center = rect.min + egui::vec2(tcsx, tcsy);
                let dir_f = [dir[0] as f32, dir[1] as f32];
                let text_dir = if dir_f[0] < -1e-6 || (dir_f[0].abs() < 1e-6 && dir_f[1] < -1e-6) {
                    [-dir_f[0], -dir_f[1]]
                } else { dir_f };
                let screen_angle = -(text_dir[1].atan2(text_dir[0]));
                let font_size = (2.5 * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
                let ghost_color = egui::Color32::from_rgba_premultiplied(220, 210, 80, 180);
                let galley = painter.ctx().fonts(|f| {
                    f.layout_no_wrap(dist_text, egui::FontId::proportional(font_size), ghost_color)
                });
                let w = galley.size().x;
                let h = galley.size().y;
                let cos_a = screen_angle.cos();
                let sin_a = screen_angle.sin();
                let rot = |vx: f32, vy: f32| egui::vec2(vx * cos_a - vy * sin_a, vx * sin_a + vy * cos_a);
                let anchor = text_center - rot(w * 0.5, h * 0.5);
                painter.add(egui::Shape::Text(egui::epaint::TextShape {
                    pos:                 anchor,
                    galley,
                    underline:           egui::epaint::Stroke::NONE,
                    fallback_color:      ghost_color,
                    override_text_color: None,
                    opacity_factor:      1.0,
                    angle:               screen_angle,
                }));
            }
            _ => {}
        }
    }

    /// Draw the DIMLINEAR rubber-band preview during SecondPoint and Placing phases.
    fn draw_dim_linear_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let ghost_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(220, 210, 80, 180));
        let painter = ui.painter_at(rect);

        match &self.dim_linear_phase {
            DimLinearPhase::SecondPoint { first } => {
                let (x1, y1) = world_to_screen(first.x as f32, first.y as f32, viewport);
                let p1 = rect.min + egui::vec2(x1, y1);
                let r = 5.0_f32;
                painter.line_segment([p1 - egui::vec2(r, r), p1 + egui::vec2(r, r)], ghost_stroke);
                painter.line_segment([p1 - egui::vec2(r, -r), p1 + egui::vec2(r, -r)], ghost_stroke);
                let (x2, y2) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
                let p2 = rect.min + egui::vec2(x2, y2);
                painter.line_segment([p1, p2], ghost_stroke);
            }
            DimLinearPhase::Placing { first, second } => {
                let mid_x = (first.x + second.x) * 0.5;
                let mid_y = (first.y + second.y) * 0.5;
                let horizontal = (world_cursor.y - mid_y).abs() > (world_cursor.x - mid_x).abs();
                let (dist, dim_line_val) = if horizontal {
                    let raw = world_cursor.y - mid_y;
                    let offset = if raw.abs() < 5.0 { if raw >= 0.0 { 5.0 } else { -5.0 } } else { raw };
                    ((second.x - first.x).abs(), mid_y + offset)
                } else {
                    let raw = world_cursor.x - mid_x;
                    let offset = if raw.abs() < 5.0 { if raw >= 0.0 { 5.0 } else { -5.0 } } else { raw };
                    ((second.y - first.y).abs(), mid_x + offset)
                };
                // Draw H or V dim line + extension lines
                let (p_s1, p_s2, p_d1, p_d2, text_world) = if horizontal {
                    // Horizontal dim: measures X. Dim line is horizontal at Y = dim_line_val.
                    let x1 = first.x.min(second.x); let x2 = first.x.max(second.x);
                    let (ex1, ey1) = world_to_screen(x1 as f32, first.y.min(second.y) as f32, viewport);
                    let (ex2, ey2) = world_to_screen(x2 as f32, first.y.max(second.y) as f32, viewport);
                    let (dl1x, dl1y) = world_to_screen(x1 as f32, dim_line_val as f32, viewport);
                    let (dl2x, dl2y) = world_to_screen(x2 as f32, dim_line_val as f32, viewport);
                    let _ = (ey1, ey2);
                    (rect.min + egui::vec2(ex1, world_to_screen(x1 as f32, first.y as f32, viewport).1),
                     rect.min + egui::vec2(ex2, world_to_screen(x2 as f32, second.y as f32, viewport).1),
                     rect.min + egui::vec2(dl1x, dl1y),
                     rect.min + egui::vec2(dl2x, dl2y),
                     [mid_x, dim_line_val])
                } else {
                    let y1 = first.y.min(second.y); let y2 = first.y.max(second.y);
                    let (dl1x, dl1y) = world_to_screen(dim_line_val as f32, y1 as f32, viewport);
                    let (dl2x, dl2y) = world_to_screen(dim_line_val as f32, y2 as f32, viewport);
                    (rect.min + egui::vec2(world_to_screen(first.x as f32, y1 as f32, viewport).0, world_to_screen(first.x as f32, y1 as f32, viewport).1),
                     rect.min + egui::vec2(world_to_screen(second.x as f32, y2 as f32, viewport).0, world_to_screen(second.x as f32, y2 as f32, viewport).1),
                     rect.min + egui::vec2(dl1x, dl1y),
                     rect.min + egui::vec2(dl2x, dl2y),
                     [dim_line_val, mid_y])
                };
                painter.line_segment([p_d1, p_d2], ghost_stroke);
                painter.line_segment([p_s1, p_d1], ghost_stroke);
                painter.line_segment([p_s2, p_d2], ghost_stroke);
                // Text label
                let dist_text = format!("{:.3}", dist);
                let (tcsx, tcsy) = world_to_screen(text_world[0] as f32, text_world[1] as f32, viewport);
                let text_center = rect.min + egui::vec2(tcsx, tcsy);
                let font_size = (2.5 * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
                let ghost_color = egui::Color32::from_rgba_premultiplied(220, 210, 80, 180);
                let galley = painter.ctx().fonts(|f| {
                    f.layout_no_wrap(dist_text, egui::FontId::proportional(font_size), ghost_color)
                });
                let w = galley.size().x;
                let h = galley.size().y;
                let anchor = text_center - egui::vec2(w * 0.5, h * 0.5);
                painter.add(egui::Shape::Text(egui::epaint::TextShape {
                    pos: anchor, galley,
                    underline: egui::epaint::Stroke::NONE,
                    fallback_color: ghost_color,
                    override_text_color: None,
                    opacity_factor: 1.0,
                    angle: 0.0,
                }));
            }
            _ => {}
        }
    }

    /// Exit extend mode, clearing all extend state.
    fn exit_extend(&mut self) {
        self.extend_phase = ExtendPhase::Idle;
        self.extend_boundary_edges.clear();
    }

    /// Find the nearest line endpoint to `screen_pos` and extend it to the nearest
    /// boundary intersection that lies beyond it.
    /// Returns an `ExtendResult` or an error message.
    fn compute_extend(
        &self,
        screen_pos: egui::Pos2,
        viewport: &Viewport,
        rect: egui::Rect,
    ) -> Result<ExtendResult, String> {
        // Tag for what kind of endpoint was found.
        enum EpKind { Line { is_start: bool }, Arc { is_start: bool } }

        // 1. Find the nearest line OR arc endpoint within PICK_RADIUS.
        let mut best: Option<(f32, Guid, EpKind)> = None;
        for entity in self.drawing.visible_entities() {
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    for (pt, is_start) in [(*start, true), (*end, false)] {
                        let (sx, sy) = world_to_screen(pt.x as f32, pt.y as f32, viewport);
                        let sp = rect.min + egui::vec2(sx, sy);
                        let d = screen_pos.distance(sp);
                        if d <= Self::PICK_RADIUS && best.as_ref().map_or(true, |(bd, _, _)| d < *bd) {
                            best = Some((d, entity.id, EpKind::Line { is_start }));
                        }
                    }
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    for (pt, is_start) in [
                        (cadkit_types::Vec3::xy(
                            center.x + radius * start_angle.cos(),
                            center.y + radius * start_angle.sin(),
                        ), true),
                        (cadkit_types::Vec3::xy(
                            center.x + radius * end_angle.cos(),
                            center.y + radius * end_angle.sin(),
                        ), false),
                    ] {
                        let (sx, sy) = world_to_screen(pt.x as f32, pt.y as f32, viewport);
                        let sp = rect.min + egui::vec2(sx, sy);
                        let d = screen_pos.distance(sp);
                        if d <= Self::PICK_RADIUS && best.as_ref().map_or(true, |(bd, _, _)| d < *bd) {
                            best = Some((d, entity.id, EpKind::Arc { is_start }));
                        }
                    }
                }
                _ => {}
            }
        }
        let (_, eid, ep_kind) = best
            .ok_or_else(|| "EXTEND: Click near a line or arc endpoint".to_string())?;

        let entity = self.drawing.get_entity(&eid)
            .ok_or_else(|| "EXTEND: Entity not found".to_string())?;

        // Build the boundary prim: lines → treat as infinite; others → as-is.
        // Line boundaries are infinite so that extending to the LINE (not just its
        // segment extent) matches AutoCAD behaviour.
        let inf_boundary = |kind: &EntityKind| -> Option<GeomPrim> {
            match kind {
                EntityKind::Line { start, end } => {
                    let bdx = end.x - start.x;
                    let bdy = end.y - start.y;
                    let blen = (bdx * bdx + bdy * bdy).sqrt();
                    if blen < 1e-9 { return None; }
                    let bux = bdx / blen;
                    let buy = bdy / blen;
                    const INF: f64 = 1_000_000.0;
                    let is = cadkit_types::Vec3::xy(start.x - bux * INF, start.y - buy * INF);
                    let ie = cadkit_types::Vec3::xy(start.x + bux * INF, start.y + buy * INF);
                    Some(GeomPrim::Line(GeomLine::new(is, ie)))
                }
                other => CadKitApp::entity_to_geom_prim(other),
            }
        };

        match ep_kind {
            EpKind::Line { is_start } => {
                // clicked end of the line; extend in the line's direction.
                let (clicked_pt, other_pt) = match &entity.kind {
                    EntityKind::Line { start, end } => {
                        if is_start { (*start, *end) } else { (*end, *start) }
                    }
                    _ => return Err("EXTEND: Not a line".to_string()),
                };
                let dx = clicked_pt.x - other_pt.x;
                let dy = clicked_pt.y - other_pt.y;
                let seg_len = (dx * dx + dy * dy).sqrt();
                if seg_len < 1e-9 {
                    return Err("EXTEND: Degenerate line".to_string());
                }
                let dir_x = dx / seg_len;
                let dir_y = dy / seg_len;

                const FAR: f64 = 1_000_000.0;
                let ray = GeomLine::new(
                    other_pt,
                    cadkit_types::Vec3::xy(other_pt.x + dir_x * FAR, other_pt.y + dir_y * FAR),
                );

                let mut best_pt: Option<Vec2> = None;
                let mut best_dot = f64::INFINITY;
                for &bid in &self.extend_boundary_edges {
                    if bid == eid { continue; }
                    let Some(b) = self.drawing.get_entity(&bid) else { continue };
                    let Some(bprim) = inf_boundary(&b.kind) else { continue };
                    for pt in Self::intersect_geom_prims(&GeomPrim::Line(ray), &bprim, Self::GEOM_TOL).points() {
                        let dot = (pt.x - clicked_pt.x) * dir_x + (pt.y - clicked_pt.y) * dir_y;
                        if dot > 1e-6 && dot < best_dot {
                            best_dot = dot;
                            best_pt = Some(Vec2::new(pt.x, pt.y));
                        }
                    }
                }
                best_pt
                    .map(|p| ExtendResult::Line { id: eid, is_start, new_pt: p })
                    .ok_or_else(|| "EXTEND: No intersection found beyond endpoint".to_string())
            }

            EpKind::Arc { is_start } => {
                // clicked end of an arc; extend by rotating the angle to the boundary.
                let (center, radius, start_angle, end_angle) = match &entity.kind {
                    EntityKind::Arc { center, radius, start_angle, end_angle } => {
                        (*center, *radius, *start_angle, *end_angle)
                    }
                    _ => return Err("EXTEND: Not an arc".to_string()),
                };
                let arc_circle = GeomCircle::new(center, radius);
                let twopi = std::f64::consts::TAU;

                // Intersect the arc's full circle with every boundary edge.
                let mut candidates: Vec<f64> = Vec::new();
                for &bid in &self.extend_boundary_edges {
                    if bid == eid { continue; }
                    let Some(b) = self.drawing.get_entity(&bid) else { continue };
                    let Some(bprim) = inf_boundary(&b.kind) else { continue };
                    for pt in Self::intersect_geom_prims(
                        &GeomPrim::Circle(arc_circle),
                        &bprim,
                        Self::GEOM_TOL,
                    ).points() {
                        candidates.push((pt.y - center.y).atan2(pt.x - center.x));
                    }
                }
                if candidates.is_empty() {
                    return Err("EXTEND: No boundary intersection found for arc".to_string());
                }

                // Span of the existing arc (CCW, always > 0 after normalisation).
                let arc_span = Self::ccw_from(start_angle, end_angle);
                // Gap region is twopi - arc_span.  A valid extension target must lie
                // strictly inside that gap (not on the arc itself).
                let gap_span = twopi - arc_span;

                let new_angle = if is_start {
                    // Extend START: find the angle in the gap that is nearest (CW) to
                    // start_angle, i.e. smallest CCW distance from a → start_angle.
                    candidates.iter()
                        .filter_map(|&a| {
                            let gap = Self::ccw_from(a, start_angle);
                            if gap > 1e-6 && gap < gap_span - 1e-6 { Some((gap, a)) } else { None }
                        })
                        .min_by(|x, y| x.0.partial_cmp(&y.0).unwrap())
                        .map(|(_, a)| a)
                        .ok_or_else(|| "EXTEND: No intersection before arc start".to_string())?
                } else {
                    // Extend END: find the angle in the gap that is nearest (CCW) past
                    // end_angle, i.e. smallest CCW distance from end_angle → a.
                    candidates.iter()
                        .filter_map(|&a| {
                            let gap = Self::ccw_from(end_angle, a);
                            if gap > 1e-6 && gap < gap_span - 1e-6 { Some((gap, a)) } else { None }
                        })
                        .min_by(|x, y| x.0.partial_cmp(&y.0).unwrap())
                        .map(|(_, a)| a)
                        .ok_or_else(|| "EXTEND: No intersection beyond arc end".to_string())?
                };
                Ok(ExtendResult::Arc { id: eid, is_start, new_angle })
            }
        }
    }

    /// Render all Text entities via egui painter (crisp at any zoom).
    fn draw_text_entities(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        let painter = ui.painter_at(rect);
        for entity in self.drawing.visible_entities() {
            if let EntityKind::Text { position, content, height, rotation } = &entity.kind {
                let (sx, sy) = world_to_screen(position.x as f32, position.y as f32, viewport);
                let screen_pos = rect.min + egui::vec2(sx, sy);
                let height_px = (height * viewport.zoom as f64) as f32;
                if height_px < 1.0 { continue; }

                // Entity colour (ByLayer or per-entity override).
                let [r, g, b] = entity.color.unwrap_or_else(|| {
                    self.drawing.get_layer(entity.layer)
                        .map(|l| l.color)
                        .unwrap_or([255, 255, 255])
                });
                let color = if self.selected_entities.contains(&entity.id) {
                    egui::Color32::from_rgb(0, 200, 255)
                } else {
                    egui::Color32::from_rgb(r, g, b)
                };

                let font_id = egui::FontId::proportional(height_px);
                let galley = ui.ctx().fonts(|f| {
                    f.layout_no_wrap(content.clone(), font_id, color)
                });
                // World CCW from +X → screen CW (Y axis flipped), so negate.
                let screen_angle = -(*rotation as f32);
                let text_shape = egui::epaint::TextShape {
                    pos: screen_pos,
                    galley,
                    underline: egui::epaint::Stroke::NONE,
                    fallback_color: color,
                    override_text_color: None,
                    opacity_factor: 1.0,
                    angle: screen_angle,
                };
                painter.add(egui::Shape::Text(text_shape));
            }
        }
    }

    /// Render DimAligned text labels via egui (crisp, rotated, with background mask).
    fn draw_dim_entities(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        let [r, g, b] = viewport.bg_srgb();
        let bg = egui::Color32::from_rgb(r, g, b);
        let painter = ui.painter_at(rect);

        for entity in self.drawing.visible_entities() {
            let (text_pos, text_override, dist, screen_angle, dim_p1, dim_p2, mut dim_dir_screen) =
                match &entity.kind {
                EntityKind::DimAligned { start, end, offset, text_pos, text_override, .. } => {
                    let dist = start.distance_to(end);
                    let dx = (end.x - start.x) as f32;
                    let dy = (end.y - start.y) as f32;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1e-6 { continue; }
                    let dir = [dx / len, dy / len];
                    let text_dir = if dir[0] < -1e-6 || (dir[0].abs() < 1e-6 && dir[1] < -1e-6) {
                        [-dir[0], -dir[1]] } else { dir };
                    let screen_angle = -(text_dir[1].atan2(text_dir[0]));
                    let perp = [-dir[1], dir[0]];
                    let dl1 = Vec2::new(start.x + perp[0] as f64 * *offset, start.y + perp[1] as f64 * *offset);
                    let dl2 = Vec2::new(end.x + perp[0] as f64 * *offset, end.y + perp[1] as f64 * *offset);
                    let (p1x, p1y) = world_to_screen(dl1.x as f32, dl1.y as f32, viewport);
                    let (p2x, p2y) = world_to_screen(dl2.x as f32, dl2.y as f32, viewport);
                    let dim_p1 = rect.min + egui::vec2(p1x, p1y);
                    let dim_p2 = rect.min + egui::vec2(p2x, p2y);
                    let dim_dir_screen = egui::vec2(text_dir[0], -text_dir[1]);
                    (text_pos, text_override, dist, screen_angle, dim_p1, dim_p2, dim_dir_screen)
                }
                EntityKind::DimLinear { start, end, offset, text_pos, text_override, horizontal, .. } => {
                    let dist = if *horizontal {
                        (end.x - start.x).abs()
                    } else {
                        (end.y - start.y).abs()
                    };
                    let (dl1, dl2, dim_dir_world) = if *horizontal {
                        let x1 = start.x.min(end.x);
                        let x2 = start.x.max(end.x);
                        let y = (start.y + end.y) * 0.5 + *offset;
                        (Vec2::new(x1, y), Vec2::new(x2, y), egui::vec2(1.0, 0.0))
                    } else {
                        let y1 = start.y.min(end.y);
                        let y2 = start.y.max(end.y);
                        let x = (start.x + end.x) * 0.5 + *offset;
                        (Vec2::new(x, y1), Vec2::new(x, y2), egui::vec2(0.0, 1.0))
                    };
                    let (p1x, p1y) = world_to_screen(dl1.x as f32, dl1.y as f32, viewport);
                    let (p2x, p2y) = world_to_screen(dl2.x as f32, dl2.y as f32, viewport);
                    let dim_p1 = rect.min + egui::vec2(p1x, p1y);
                    let dim_p2 = rect.min + egui::vec2(p2x, p2y);
                    let dim_dir_screen = egui::vec2(dim_dir_world.x, -dim_dir_world.y);
                    (text_pos, text_override, dist, 0.0_f32, dim_p1, dim_p2, dim_dir_screen) // always horizontal text
                }
                _ => continue,
            };

            let label = self.dim_label_text(text_override, dist);

            // Text centre in screen space.
            let (tx, ty) = world_to_screen(text_pos.x as f32, text_pos.y as f32, viewport);
            let mut text_center = rect.min + egui::vec2(tx, ty);

            // Colour.
            let [r, g, b] = entity.color.unwrap_or_else(|| {
                self.drawing.get_layer(entity.layer)
                    .map(|l| l.color)
                    .unwrap_or([255, 255, 255])
            });
            let color = if self.selected_entities.contains(&entity.id) {
                egui::Color32::from_rgb(0, 200, 255)
            } else {
                egui::Color32::from_rgb(r, g, b)
            };

            let font_size = (self.dim_style.text_height * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
            let galley = ui.ctx().fonts(|f| {
                f.layout_no_wrap(label, egui::FontId::proportional(font_size), color)
            });
            let w = galley.size().x;
            let h = galley.size().y;
            let pad = 3.0_f32;

            let mut leader: Option<(egui::Pos2, egui::Pos2)> = None;
            let available = dim_p1.distance(dim_p2);
            let needed = w + pad * 2.0 + 8.0;
            if needed > available {
                let dir_len = dim_dir_screen.length();
                if dir_len > 1e-6 {
                    dim_dir_screen /= dir_len;
                } else {
                    dim_dir_screen = egui::vec2(1.0, 0.0);
                }
                let s1 = dim_p1.to_vec2().dot(dim_dir_screen);
                let s2 = dim_p2.to_vec2().dot(dim_dir_screen);
                let base = if s2 >= s1 { dim_p2 } else { dim_p1 };
                let gap = 10.0_f32;
                text_center = base + dim_dir_screen * (gap + w * 0.5 + pad);
                let end = text_center - dim_dir_screen * (w * 0.5 + pad);
                leader = Some((base, end));
            }

            // Compute the TextShape anchor so the galley is centred at text_center.
            // TextShape rotates the galley around `pos`; galley origin is top-left.
            // Centre offset in the unrotated glyph frame: (w/2, h/2).
            let cos_a = screen_angle.cos();
            let sin_a = screen_angle.sin();
            let rot = |vx: f32, vy: f32| egui::vec2(vx * cos_a - vy * sin_a, vx * sin_a + vy * cos_a);
            let anchor = text_center - rot(w * 0.5, h * 0.5);

            // Mask: rotated padded bounding rect drawn before the text.
            let corners = [
                rot(-pad,     -pad    ),
                rot(w + pad,  -pad    ),
                rot(w + pad,   h + pad),
                rot(-pad,      h + pad),
            ];
            let mask_pts: Vec<egui::Pos2> = corners.iter().map(|&v| anchor + v).collect();
            if let Some((from, to)) = leader {
                painter.line_segment([from, to], egui::Stroke::new(1.5, color));
            }
            painter.add(egui::Shape::convex_polygon(mask_pts, bg, egui::Stroke::NONE));

            // Rotated text.
            painter.add(egui::Shape::Text(egui::epaint::TextShape {
                pos:                  anchor,
                galley,
                underline:            egui::epaint::Stroke::NONE,
                fallback_color:       color,
                override_text_color:  None,
                opacity_factor:       1.0,
                angle:                screen_angle,
            }));
        }

        // Angular dimension text labels (separate loop — always horizontal, angle in °).
        for entity in self.drawing.visible_entities() {
            let EntityKind::DimAngular { vertex, line1_pt, line2_pt, text_override, text_pos, .. } =
                &entity.kind
            else {
                continue;
            };

            use std::f64::consts::TAU;
            let a1 = (line1_pt.y - vertex.y).atan2(line1_pt.x - vertex.x);
            let mut a2 = (line2_pt.y - vertex.y).atan2(line2_pt.x - vertex.x);
            if a2 <= a1 { a2 += TAU; }
            let angle_deg = (a2 - a1).to_degrees();

            let measurement = format!("{:.*}°", self.dim_style.precision, angle_deg);
            let label = match text_override {
                None => measurement.clone(),
                Some(s) if s.trim().is_empty() || s.trim() == "<>" => measurement.clone(),
                Some(s) => s.replace("<>", &measurement),
            };

            let [r, g, b] = entity.color.unwrap_or_else(|| {
                self.drawing.get_layer(entity.layer)
                    .map(|l| l.color)
                    .unwrap_or([255, 255, 255])
            });
            let color = if self.selected_entities.contains(&entity.id) {
                egui::Color32::from_rgb(0, 200, 255)
            } else {
                egui::Color32::from_rgb(r, g, b)
            };

            let font_size = (self.dim_style.text_height * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
            let galley = ui.ctx().fonts(|f| {
                f.layout_no_wrap(label, egui::FontId::proportional(font_size), color)
            });
            let w = galley.size().x;
            let h = galley.size().y;
            let pad = 3.0_f32;

            let (tx, ty) = world_to_screen(text_pos.x as f32, text_pos.y as f32, viewport);
            let text_center = rect.min + egui::vec2(tx, ty);
            let anchor = text_center - egui::vec2(w * 0.5, h * 0.5);

            let corners = [
                egui::vec2(-pad,     -pad    ),
                egui::vec2(w + pad,  -pad    ),
                egui::vec2(w + pad,   h + pad),
                egui::vec2(-pad,      h + pad),
            ];
            let mask_pts: Vec<egui::Pos2> = corners.iter().map(|&v| anchor + v).collect();
            painter.add(egui::Shape::convex_polygon(mask_pts, bg, egui::Stroke::NONE));
            painter.add(egui::Shape::Text(egui::epaint::TextShape {
                pos:                  anchor,
                galley,
                underline:            egui::epaint::Stroke::NONE,
                fallback_color:       color,
                override_text_color:  None,
                opacity_factor:       1.0,
                angle:                0.0,
            }));
        }

        // Radial dimension text labels (separate loop — always horizontal).
        for entity in self.drawing.visible_entities() {
            let EntityKind::DimRadial { radius, is_diameter, text_override, text_pos, .. } =
                &entity.kind
            else {
                continue;
            };

            let measurement = if *is_diameter {
                format!("Ø{}", self.format_dim_measurement(radius * 2.0))
            } else {
                format!("R{}", self.format_dim_measurement(*radius))
            };
            let label = match text_override {
                None => measurement.clone(),
                Some(s) if s.trim().is_empty() || s.trim() == "<>" => measurement.clone(),
                Some(s) => s.replace("<>", &measurement),
            };

            let [r, g, b] = entity.color.unwrap_or_else(|| {
                self.drawing.get_layer(entity.layer)
                    .map(|l| l.color)
                    .unwrap_or([255, 255, 255])
            });
            let color = if self.selected_entities.contains(&entity.id) {
                egui::Color32::from_rgb(0, 200, 255)
            } else {
                egui::Color32::from_rgb(r, g, b)
            };

            let font_size = (self.dim_style.text_height * viewport.zoom as f64).clamp(8.0, 48.0) as f32;
            let galley = ui.ctx().fonts(|f| {
                f.layout_no_wrap(label, egui::FontId::proportional(font_size), color)
            });
            let w = galley.size().x;
            let h = galley.size().y;
            let pad = 3.0_f32;

            let (tx, ty) = world_to_screen(text_pos.x as f32, text_pos.y as f32, viewport);
            let text_center = rect.min + egui::vec2(tx, ty);
            let anchor = text_center - egui::vec2(w * 0.5, h * 0.5);

            let corners = [
                egui::vec2(-pad,     -pad    ),
                egui::vec2(w + pad,  -pad    ),
                egui::vec2(w + pad,   h + pad),
                egui::vec2(-pad,      h + pad),
            ];
            let mask_pts: Vec<egui::Pos2> = corners.iter().map(|&v| anchor + v).collect();
            painter.add(egui::Shape::convex_polygon(mask_pts, bg, egui::Stroke::NONE));
            painter.add(egui::Shape::Text(egui::epaint::TextShape {
                pos:                  anchor,
                galley,
                underline:            egui::epaint::Stroke::NONE,
                fallback_color:       color,
                override_text_color:  None,
                opacity_factor:       1.0,
                angle:                0.0,
            }));
        }
    }

    /// Ghost text preview during the TEXT command phases.
    fn draw_text_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let ghost = egui::Color32::from_rgba_premultiplied(180, 180, 255, 160);
        let painter = ui.painter_at(rect);

        match &self.text_phase {
            TextPhase::PlacingPosition => {
                // Small "T" marker at cursor.
                let (sx, sy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
                let pos = rect.min + egui::vec2(sx, sy);
                painter.text(pos, egui::Align2::LEFT_BOTTOM, "T",
                    egui::FontId::proportional(18.0), ghost);
            }
            TextPhase::EnteringHeight { position } | TextPhase::EnteringRotation { position, .. } => {
                // Preview marker at the selected insertion point.
                let (sx, sy) = world_to_screen(position.x as f32, position.y as f32, viewport);
                let pos = rect.min + egui::vec2(sx, sy);
                painter.text(pos, egui::Align2::LEFT_BOTTOM, "T",
                    egui::FontId::proportional(18.0), ghost);
            }
            TextPhase::TypingContent { position, height, .. } => {
                let preview = if self.command_input.is_empty() { "…" } else { &self.command_input };
                let (sx, sy) = world_to_screen(position.x as f32, position.y as f32, viewport);
                let pos = rect.min + egui::vec2(sx, sy);
                let height_px = (height * viewport.zoom as f64).max(8.0) as f32;
                painter.text(pos, egui::Align2::LEFT_BOTTOM, preview,
                    egui::FontId::proportional(height_px), ghost);
            }
            TextPhase::Idle => {}
        }
    }

    /// Polygon rubber-band preview during POLYGON center/radius phases.
    fn draw_polygon_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let painter = ui.painter_at(rect);
        let ghost = egui::Stroke::new(1.8, egui::Color32::from_rgba_premultiplied(160, 230, 255, 180));

        match self.polygon_phase {
            PolygonPhase::Center => {
                let (sx, sy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
                let c = rect.min + egui::vec2(sx, sy);
                let r = 6.0_f32;
                painter.line_segment([c - egui::vec2(r, 0.0), c + egui::vec2(r, 0.0)], ghost);
                painter.line_segment([c - egui::vec2(0.0, r), c + egui::vec2(0.0, r)], ghost);
            }
            PolygonPhase::Radius { center } => {
                if self.polygon_sides < 3 {
                    return;
                }
                let cursor = if self.ortho_enabled {
                    Self::snap_angle(center, world_cursor, self.ortho_increment_deg)
                } else {
                    world_cursor
                };
                let radius = center.distance_to(&cursor);
                if radius <= 1e-9 {
                    return;
                }
                let base = (cursor.y - center.y).atan2(cursor.x - center.x);
                let step = std::f64::consts::TAU / self.polygon_sides as f64;
                let mut pts: Vec<egui::Pos2> = Vec::with_capacity(self.polygon_sides);
                for i in 0..self.polygon_sides {
                    let a = base + i as f64 * step;
                    let x = center.x + radius * a.cos();
                    let y = center.y + radius * a.sin();
                    let (sx, sy) = world_to_screen(x as f32, y as f32, viewport);
                    pts.push(rect.min + egui::vec2(sx, sy));
                }
                for w in pts.windows(2) {
                    painter.line_segment([w[0], w[1]], ghost);
                }
                if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
                    painter.line_segment([*last, *first], ghost);
                }
                let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                let c = rect.min + egui::vec2(cx, cy);
                let (rx, ry) = world_to_screen(cursor.x as f32, cursor.y as f32, viewport);
                let p = rect.min + egui::vec2(rx, ry);
                painter.line_segment([c, p], egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(160, 230, 255, 120)));
            }
            PolygonPhase::Idle | PolygonPhase::EnteringSides => {}
        }
    }

    /// Ellipse rubber-band preview during ELLIPSE phases.
    fn draw_ellipse_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let painter = ui.painter_at(rect);
        let ghost = egui::Stroke::new(1.8, egui::Color32::from_rgba_premultiplied(255, 220, 160, 180));

        match self.ellipse_phase {
            EllipsePhase::Center => {
                let (sx, sy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
                let c = rect.min + egui::vec2(sx, sy);
                let r = 6.0_f32;
                painter.line_segment([c - egui::vec2(r, 0.0), c + egui::vec2(r, 0.0)], ghost);
                painter.line_segment([c - egui::vec2(0.0, r), c + egui::vec2(0.0, r)], ghost);
            }
            EllipsePhase::RadiusX { center } => {
                let p = if self.ortho_enabled {
                    Self::snap_angle(center, world_cursor, self.ortho_increment_deg)
                } else {
                    world_cursor
                };
                let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                let (px, py) = world_to_screen(p.x as f32, p.y as f32, viewport);
                painter.line_segment(
                    [rect.min + egui::vec2(cx, cy), rect.min + egui::vec2(px, py)],
                    ghost,
                );
            }
            EllipsePhase::RadiusY { center, rx } => {
                let p = if self.ortho_enabled {
                    Self::snap_angle(center, world_cursor, self.ortho_increment_deg)
                } else {
                    world_cursor
                };
                let ry = center.distance_to(&p);
                if rx <= 1e-9 || ry <= 1e-9 {
                    return;
                }
                let steps = 96usize;
                let mut last: Option<egui::Pos2> = None;
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let a = t * std::f64::consts::TAU;
                    let x = center.x + rx * a.cos();
                    let y = center.y + ry * a.sin();
                    let (sx, sy) = world_to_screen(x as f32, y as f32, viewport);
                    let pos = rect.min + egui::vec2(sx, sy);
                    if let Some(prev) = last {
                        painter.line_segment([prev, pos], ghost);
                    }
                    last = Some(pos);
                }
            }
            EllipsePhase::Idle => {}
        }
    }

    /// Rectangle rubber-band preview during RECTANGLE phases.
    fn draw_rectangle_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let painter = ui.painter_at(rect);
        let ghost = egui::Stroke::new(1.8, egui::Color32::from_rgba_premultiplied(200, 255, 180, 180));

        let rect_pts: Option<[Vec2; 4]> = match self.rectangle_phase {
            RectanglePhase::FirstCorner => None,
            RectanglePhase::SecondCorner { first } => self.rectangle_points_from_diagonal(first, world_cursor),
            RectanglePhase::EnteringDimensions { .. } => None,
            RectanglePhase::Direction { first, width, height } => {
                self.rectangle_points_from_dimensions(first, width, height, world_cursor)
            }
            RectanglePhase::Idle => None,
        };

        if let Some([p0, p1, p2, p3]) = rect_pts {
            let to_screen = |p: Vec2| {
                let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                rect.min + egui::vec2(sx, sy)
            };
            let s0 = to_screen(p0);
            let s1 = to_screen(p1);
            let s2 = to_screen(p2);
            let s3 = to_screen(p3);
            painter.line_segment([s0, s1], ghost);
            painter.line_segment([s1, s2], ghost);
            painter.line_segment([s2, s3], ghost);
            painter.line_segment([s3, s0], ghost);
        } else if self.rectangle_phase == RectanglePhase::FirstCorner {
            let (sx, sy) = world_to_screen(world_cursor.x as f32, world_cursor.y as f32, viewport);
            let c = rect.min + egui::vec2(sx, sy);
            let r = 6.0_f32;
            painter.line_segment([c - egui::vec2(r, 0.0), c + egui::vec2(r, 0.0)], ghost);
            painter.line_segment([c - egui::vec2(0.0, r), c + egui::vec2(0.0, r)], ghost);
        }
    }

    fn array_rect_basis(&self) -> Option<(Vec2, f64, f64, f64, f64)> {
        let base = self.array_center?;
        let fallback_dx = self.array_rect_dx.abs().max(1.0);
        let dirp = self
            .array_rect_dir_point
            .unwrap_or_else(|| Vec2::new(base.x + fallback_dx, base.y));
        let vx = dirp.x - base.x;
        let vy = dirp.y - base.y;
        let vlen = (vx * vx + vy * vy).sqrt();
        if vlen <= 1e-9 {
            return Some((base, 1.0, 0.0, 0.0, 1.0));
        }
        let ux = vx / vlen;
        let uy = vy / vlen;
        let px = -uy;
        let py = ux;
        Some((base, ux, uy, px, py))
    }

    fn array_rect_grip_points(&self) -> Option<(Vec2, Vec2, Vec2, Vec2)> {
        let (base, ux, uy, px, py) = self.array_rect_basis()?;
        let cols = self.array_rect_columns.max(1);
        let rows = self.array_rect_rows.max(1);
        let dx_step = self.array_rect_dx.abs().max(1e-6);
        let dy_sign = if self.array_rect_dy < 0.0 { -1.0 } else { 1.0 };
        let dy_mag = self.array_rect_dy.abs().max(1e-6);
        let dy_step = dy_mag * dy_sign;

        let x_close = Vec2::new(base.x + ux * dx_step, base.y + uy * dx_step);
        let x_far = Vec2::new(
            base.x + ux * dx_step * (cols.saturating_sub(1) as f64),
            base.y + uy * dx_step * (cols.saturating_sub(1) as f64),
        );
        let y_close = Vec2::new(base.x + px * dy_step, base.y + py * dy_step);
        let y_far = Vec2::new(
            base.x + px * dy_step * (rows.saturating_sub(1) as f64),
            base.y + py * dy_step * (rows.saturating_sub(1) as f64),
        );
        Some((x_close, x_far, y_close, y_far))
    }

    fn pick_array_rect_grip(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<ArrayPhase> {
        let (x_close, x_far, y_close, y_far) = self.array_rect_grip_points()?;
        let to_screen = |p: Vec2| {
            let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
            rect.min + egui::vec2(sx, sy)
        };
        let candidates = [
            (ArrayPhase::RectXSpacingGrip, to_screen(x_close)),
            (ArrayPhase::RectXCountGrip, to_screen(x_far)),
            (ArrayPhase::RectYSpacingGrip, to_screen(y_close)),
            (ArrayPhase::RectYCountGrip, to_screen(y_far)),
        ];
        let mut best: Option<(f32, ArrayPhase)> = None;
        for (phase, p) in candidates {
            let d = p.distance(screen_pos);
            if d <= 12.0 && best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                best = Some((d, phase));
            }
        }
        best.map(|(_, phase)| phase)
    }

    fn set_active_array_grip(&mut self, phase: ArrayPhase) {
        self.array_phase = phase.clone();
        let msg = match phase {
            ArrayPhase::RectXSpacingGrip => {
                "ARRAY: Horizontal spacing grip active. Click/drag to edit, Enter to apply"
            }
            ArrayPhase::RectXCountGrip => {
                "ARRAY: Horizontal count grip active. Click/drag to edit, Enter to apply"
            }
            ArrayPhase::RectYSpacingGrip => {
                "ARRAY: Vertical spacing grip active. Click/drag to edit, Enter to apply"
            }
            ArrayPhase::RectYCountGrip => {
                "ARRAY: Vertical count grip active. Click/drag to edit, Enter to apply"
            }
            _ => "ARRAY: Grip active. Enter to apply",
        };
        self.command_log.push(msg.to_string());
    }

    fn update_array_rect_from_world(&mut self, world: Vec2) -> bool {
        let Some((base, ux, uy, px, py)) = self.array_rect_basis() else {
            return false;
        };
        match self.array_phase {
            ArrayPhase::RectXSpacingGrip => {
                let dir = if self.ortho_enabled {
                    Self::snap_angle(base, world, self.ortho_increment_deg)
                } else {
                    world
                };
                let d = base.distance_to(&dir);
                if d <= 1e-9 {
                    return false;
                }
                self.array_rect_dx = d;
                self.array_rect_dir_point = Some(dir);
                true
            }
            ArrayPhase::RectXCountGrip => {
                let dx = self.array_rect_dx.abs();
                if dx <= 1e-9 {
                    return false;
                }
                let proj = ((world.x - base.x) * ux + (world.y - base.y) * uy).abs();
                self.array_rect_columns = (proj / dx).floor() as usize + 1;
                true
            }
            ArrayPhase::RectYSpacingGrip => {
                let proj = (world.x - base.x) * px + (world.y - base.y) * py;
                let d = proj.abs();
                if d <= 1e-9 {
                    return false;
                }
                self.array_rect_y_sign = if proj >= 0.0 { 1.0 } else { -1.0 };
                self.array_rect_dy = d * self.array_rect_y_sign;
                true
            }
            ArrayPhase::RectYCountGrip => {
                let dy = self.array_rect_dy.abs();
                if dy <= 1e-9 {
                    return false;
                }
                let proj = ((world.x - base.x) * px + (world.y - base.y) * py).abs();
                self.array_rect_rows = (proj / dy).floor() as usize + 1;
                true
            }
            _ => false,
        }
    }

    fn draw_array_preview(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport, world_cursor: Vec2) {
        let painter = ui.painter_at(rect);
        let guide = egui::Stroke::new(1.5, egui::Color32::from_rgba_premultiplied(220, 220, 140, 180));
        let ghost_stroke = egui::Stroke::new(1.4, egui::Color32::from_rgba_premultiplied(140, 220, 255, 150));

        let preview_sources: Vec<AssocArraySource> = if let Some(aid) = self.array_edit_assoc {
            self.assoc_rect_arrays
                .get(&aid)
                .map(|arr| arr.source.clone())
                .unwrap_or_default()
        } else {
            self.array_entities
                .iter()
                .filter_map(|id| self.drawing.get_entity(id))
                .map(|entity| AssocArraySource {
                    kind: entity.kind.clone(),
                    layer: entity.layer,
                    color: entity.color,
                })
                .collect()
        };

        let draw_kind = |kind: &EntityKind, stroke: egui::Stroke| {
            match kind {
                EntityKind::Line { start, end } => {
                    let (x1, y1) = world_to_screen(start.x as f32, start.y as f32, viewport);
                    let (x2, y2) = world_to_screen(end.x as f32, end.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                    let (rx, _) = world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(cx, cy), (rx - cx).abs(), stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let sweep = *end_angle - *start_angle;
                    let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = *start_angle + sweep * t;
                        let x = center.x + *radius * ang.cos();
                        let y = center.y + *radius * ang.sin();
                        let (sx, sy) = world_to_screen(x as f32, y as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last {
                            painter.line_segment([prev, pos], stroke);
                        }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 {
                        return;
                    }
                    let pts: Vec<egui::Pos2> = vertices
                        .iter()
                        .map(|v| {
                            let (sx, sy) = world_to_screen(v.x as f32, v.y as f32, viewport);
                            rect.min + egui::vec2(sx, sy)
                        })
                        .collect();
                    for w in pts.windows(2) {
                        painter.line_segment([w[0], w[1]], stroke);
                    }
                    if *closed && pts.len() >= 2 {
                        painter.line_segment([*pts.last().unwrap(), pts[0]], stroke);
                    }
                }
                EntityKind::DimAligned { start, end, .. } | EntityKind::DimLinear { start, end, .. } => {
                    let (x1, y1) = world_to_screen(start.x as f32, start.y as f32, viewport);
                    let (x2, y2) = world_to_screen(end.x as f32, end.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, .. } => {
                    let (vx, vy) = world_to_screen(vertex.x as f32, vertex.y as f32, viewport);
                    let (x1, y1) = world_to_screen(line1_pt.x as f32, line1_pt.y as f32, viewport);
                    let (x2, y2) = world_to_screen(line2_pt.x as f32, line2_pt.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(vx, vy), rect.min + egui::vec2(x1, y1)],
                        stroke,
                    );
                    painter.line_segment(
                        [rect.min + egui::vec2(vx, vy), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
                EntityKind::DimRadial { center, leader_pt, radius, .. } => {
                    let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                    let (lx, ly) = world_to_screen(leader_pt.x as f32, leader_pt.y as f32, viewport);
                    let (rx, _) = world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(cx, cy), (rx - cx).abs(), stroke);
                    painter.line_segment(
                        [rect.min + egui::vec2(cx, cy), rect.min + egui::vec2(lx, ly)],
                        stroke,
                    );
                }
                EntityKind::Text { position, .. } => {
                    let (sx, sy) = world_to_screen(position.x as f32, position.y as f32, viewport);
                    let p = rect.min + egui::vec2(sx, sy);
                    let r = 3.0_f32;
                    painter.line_segment([p - egui::vec2(r, r), p + egui::vec2(r, r)], stroke);
                    painter.line_segment([p - egui::vec2(r, -r), p + egui::vec2(r, -r)], stroke);
                }
            }
        };

        match self.array_phase {
            ArrayPhase::RectGripIdle
            | ArrayPhase::RectXSpacingGrip
            | ArrayPhase::RectXCountGrip
            | ArrayPhase::RectYSpacingGrip
            | ArrayPhase::RectYCountGrip => {
                let Some(base) = self.array_center else { return };

                let mut cols = self.array_rect_columns.max(1);
                let mut rows = self.array_rect_rows.max(1);
                let mut dx_step = self.array_rect_dx.abs().max(1e-6);
                let mut dy_step = if self.array_rect_dy.abs() <= 1e-9 {
                    1e-6 * self.array_rect_y_sign.signum()
                } else {
                    self.array_rect_dy
                };

                let mut dir = self.array_rect_dir_point.unwrap_or_else(|| Vec2::new(base.x + dx_step, base.y));
                if self.array_phase == ArrayPhase::RectXSpacingGrip {
                    dir = if self.ortho_enabled {
                        Self::snap_angle(base, world_cursor, self.ortho_increment_deg)
                    } else {
                        world_cursor
                    };
                    let d = base.distance_to(&dir);
                    if d > 1e-9 {
                        dx_step = d;
                    } else {
                        dir = Vec2::new(base.x + dx_step, base.y);
                    }
                }

                let vx = dir.x - base.x;
                let vy = dir.y - base.y;
                let vlen = (vx * vx + vy * vy).sqrt();
                if vlen <= 1e-9 {
                    return;
                }
                let ux = vx / vlen;
                let uy = vy / vlen;
                let px = -uy;
                let py = ux;

                if self.array_phase == ArrayPhase::RectXCountGrip && dx_step > 1e-9 {
                    let proj = ((world_cursor.x - base.x) * ux + (world_cursor.y - base.y) * uy).abs();
                    cols = (proj / dx_step).floor() as usize + 1;
                }
                if self.array_phase == ArrayPhase::RectYSpacingGrip {
                    let proj = (world_cursor.x - base.x) * px + (world_cursor.y - base.y) * py;
                    let d = proj.abs();
                    if d > 1e-9 {
                        dy_step = d * if proj >= 0.0 { 1.0 } else { -1.0 };
                    }
                }
                if self.array_phase == ArrayPhase::RectYCountGrip {
                    let proj = ((world_cursor.x - base.x) * px + (world_cursor.y - base.y) * py).abs();
                    if dy_step.abs() > 1e-9 {
                        rows = (proj / dy_step.abs()).floor() as usize + 1;
                    }
                }

                for r in 0..rows {
                    for c in 0..cols {
                        if r == 0 && c == 0 {
                            continue;
                        }
                        let ox = ux * dx_step * c as f64 + px * dy_step * r as f64;
                        let oy = uy * dx_step * c as f64 + py * dy_step * r as f64;
                        for src in &preview_sources {
                            let shifted = Self::clone_kind_translated(&src.kind, ox, oy);
                            draw_kind(&shifted, ghost_stroke);
                        }
                    }
                }

                let to_screen = |p: Vec2| {
                    let (sx, sy) = world_to_screen(p.x as f32, p.y as f32, viewport);
                    rect.min + egui::vec2(sx, sy)
                };
                let center_screen = to_screen(base);
                let x_close = Vec2::new(base.x + ux * dx_step, base.y + uy * dx_step);
                let x_far =
                    Vec2::new(base.x + ux * dx_step * (cols.saturating_sub(1) as f64), base.y + uy * dx_step * (cols.saturating_sub(1) as f64));
                let y_close = Vec2::new(base.x + px * dy_step, base.y + py * dy_step);
                let y_far =
                    Vec2::new(base.x + px * dy_step * (rows.saturating_sub(1) as f64), base.y + py * dy_step * (rows.saturating_sub(1) as f64));

                painter.line_segment([center_screen, to_screen(x_close)], guide);
                painter.line_segment([center_screen, to_screen(x_far)], guide);
                painter.line_segment([center_screen, to_screen(y_close)], guide);
                painter.line_segment([center_screen, to_screen(y_far)], guide);

                let draw_grip = |p: Vec2, active: bool| {
                    let s = to_screen(p);
                    let side = if active { 9.0 } else { 7.0 };
                    let color = if active {
                        egui::Color32::from_rgb(80, 230, 255)
                    } else {
                        egui::Color32::from_rgb(220, 220, 220)
                    };
                    let r = egui::Rect::from_center_size(s, egui::vec2(side, side));
                    painter.rect_filled(r, 1.5, color);
                };
                draw_grip(x_close, self.array_phase == ArrayPhase::RectXSpacingGrip);
                draw_grip(x_far, self.array_phase == ArrayPhase::RectXCountGrip);
                draw_grip(y_close, self.array_phase == ArrayPhase::RectYSpacingGrip);
                draw_grip(y_far, self.array_phase == ArrayPhase::RectYCountGrip);

                let font = egui::FontId::proportional(12.0);
                let label_color = egui::Color32::from_rgb(220, 230, 240);
                painter.text(
                    to_screen(x_close) + egui::vec2(8.0, -6.0),
                    egui::Align2::LEFT_TOP,
                    format!("dx {:.3}", dx_step),
                    font.clone(),
                    label_color,
                );
                painter.text(
                    to_screen(x_far) + egui::vec2(8.0, -6.0),
                    egui::Align2::LEFT_TOP,
                    format!("cols {}", cols),
                    font.clone(),
                    label_color,
                );
                painter.text(
                    to_screen(y_close) + egui::vec2(8.0, -6.0),
                    egui::Align2::LEFT_TOP,
                    format!("dy {:.3}", dy_step.abs()),
                    font.clone(),
                    label_color,
                );
                painter.text(
                    to_screen(y_far) + egui::vec2(8.0, -6.0),
                    egui::Align2::LEFT_TOP,
                    format!("rows {}", rows),
                    font,
                    label_color,
                );
            }
            ArrayPhase::PolarBasePoint => {
                if let Some(center) = self.array_center {
                    let p = if self.ortho_enabled {
                        Self::snap_angle(center, world_cursor, self.ortho_increment_deg)
                    } else {
                        world_cursor
                    };
                    let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                    let (px, py) = world_to_screen(p.x as f32, p.y as f32, viewport);
                    let c = rect.min + egui::vec2(cx, cy);
                    let t = rect.min + egui::vec2(px, py);
                    painter.line_segment([c, t], guide);
                    let r = ((t.x - c.x).powi(2) + (t.y - c.y).powi(2)).sqrt();
                    if r > 1.0 {
                        painter.circle_stroke(c, r, guide);
                    }
                }
            }
            _ => {}
        }
    }

    /// Draw green highlight overlay for EXTEND boundary edges.
    fn draw_extend_overlay(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        if matches!(self.extend_phase, ExtendPhase::Idle) {
            return;
        }
        let stroke = egui::Stroke::new(2.5, egui::Color32::from_rgb(80, 220, 80));
        let painter = ui.painter_at(rect);
        for id in &self.extend_boundary_edges {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let (x1, y1) = world_to_screen(start.x as f32, start.y as f32, viewport);
                    let (x2, y2) = world_to_screen(end.x as f32, end.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let c: Vec2 = (*center).into();
                    let (cx, cy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                    let (rx, _) = world_to_screen((c.x + radius) as f32, c.y as f32, viewport);
                    painter.circle_stroke(rect.min + egui::vec2(cx, cy), (rx - cx).abs(), stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let c: Vec2 = (*center).into();
                    let sweep = *end_angle - *start_angle;
                    let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = *start_angle + sweep * t;
                        let px = c.x + *radius * ang.cos();
                        let py = c.y + *radius * ang.sin();
                        let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last { painter.line_segment([prev, pos], stroke); }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    for seg in vertices.windows(2) {
                        let a: Vec2 = seg[0].into();
                        let b: Vec2 = seg[1].into();
                        let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                        let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                        painter.line_segment(
                            [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                            stroke,
                        );
                    }
                    if *closed && vertices.len() >= 2 {
                        let a: Vec2 = vertices.last().unwrap().to_owned().into();
                        let b: Vec2 = vertices.first().unwrap().to_owned().into();
                        let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                        let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                        painter.line_segment(
                            [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                            stroke,
                        );
                    }
                }
                EntityKind::DimAligned { .. } | EntityKind::DimLinear { .. } | EntityKind::DimAngular { .. } | EntityKind::DimRadial { .. } | EntityKind::Text { .. } => {}
            }
        }
    }

    /// Compute an offset entity from the currently selected entity toward `world_click`.
    /// Returns the new entity on success, or an error message string on failure.
    fn apply_offset(&self, world_click: Vec2) -> Result<cadkit_2d_core::Entity, String> {
        let dist = match self.offset_distance {
            Some(d) => d,
            None => return Err("OFFSET: No distance set".to_string()),
        };
        let eid = match self.offset_selected_entity {
            Some(id) => id,
            None => return Err("OFFSET: Select an entity first".to_string()),
        };
        let entity = match self.drawing.get_entity(&eid) {
            Some(e) => e,
            None => return Err("OFFSET: Entity not found".to_string()),
        };
        if self.is_layer_locked(entity.layer) {
            return Err("OFFSET: Entity is on a locked layer".to_string());
        }
        let layer = entity.layer;

        match &entity.kind {
            EntityKind::Line { start, end } => {
                let dx = end.x - start.x;
                let dy = end.y - start.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-9 {
                    return Err("OFFSET: Line is degenerate".to_string());
                }
                // Left-normal of line direction (CCW 90°): (-dy/len, dx/len)
                let nx = -dy / len;
                let ny = dx / len;
                // Cross product determines which side of the line the click is on.
                let cp = dx * (world_click.y - start.y) - dy * (world_click.x - start.x);
                let sign = if cp >= 0.0 { 1.0 } else { -1.0 };
                let new_start = Vec2::new(start.x + sign * dist * nx, start.y + sign * dist * ny);
                let new_end = Vec2::new(end.x + sign * dist * nx, end.y + sign * dist * ny);
                let mut e = create_line(new_start, new_end);
                e.layer = layer;
                Ok(e)
            }
            EntityKind::Circle { center, radius } => {
                let dx = world_click.x - center.x;
                let dy = world_click.y - center.y;
                let d = (dx * dx + dy * dy).sqrt();
                let new_radius = if d > *radius { radius + dist } else { radius - dist };
                if new_radius <= 0.0 {
                    return Err("OFFSET: Result would be invalid".to_string());
                }
                let mut e = create_circle(Vec2::new(center.x, center.y), new_radius);
                e.layer = layer;
                Ok(e)
            }
            EntityKind::Arc { center, radius, start_angle, end_angle } => {
                let dx = world_click.x - center.x;
                let dy = world_click.y - center.y;
                let d = (dx * dx + dy * dy).sqrt();
                let new_radius = if d > *radius { radius + dist } else { radius - dist };
                if new_radius <= 0.0 {
                    return Err("OFFSET: Result would be invalid".to_string());
                }
                let mut e = create_arc(Vec2::new(center.x, center.y), new_radius, *start_angle, *end_angle);
                e.layer = layer;
                Ok(e)
            }
            EntityKind::Polyline { .. } => {
                Err("OFFSET: Polyline offset not yet supported".to_string())
            }
            EntityKind::DimAligned { .. } | EntityKind::DimLinear { .. } | EntityKind::DimAngular { .. } | EntityKind::DimRadial { .. } => {
                Err("OFFSET: Cannot offset dimension entities".to_string())
            }
            EntityKind::Text { .. } => {
                Err("OFFSET: Cannot offset text entities".to_string())
            }
        }
    }

    /// Draw yellow highlight overlay for TRIM cutting edges.
    fn draw_trim_overlay(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        if matches!(self.trim_phase, TrimPhase::Idle) {
            return;
        }
        let stroke = egui::Stroke::new(2.5, egui::Color32::from_rgb(255, 220, 40));
        let painter = ui.painter_at(rect);
        for id in &self.trim_cutting_edges {
            let Some(entity) = self.drawing.get_entity(id) else { continue };
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let (x1, y1) = world_to_screen(start.x as f32, start.y as f32, viewport);
                    let (x2, y2) = world_to_screen(end.x as f32, end.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let c: Vec2 = (*center).into();
                    let r = *radius;
                    let (cx, cy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                    let (rx, _) = world_to_screen((c.x + r) as f32, c.y as f32, viewport);
                    let screen_r = (rx - cx).abs();
                    painter.circle_stroke(rect.min + egui::vec2(cx, cy), screen_r, stroke);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let c: Vec2 = (*center).into();
                    let sweep = *end_angle - *start_angle;
                    let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                    let mut last: Option<egui::Pos2> = None;
                    for i in 0..=steps {
                        let t = i as f64 / steps as f64;
                        let ang = *start_angle + sweep * t;
                        let px = c.x + *radius * ang.cos();
                        let py = c.y + *radius * ang.sin();
                        let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                        let pos = rect.min + egui::vec2(sx, sy);
                        if let Some(prev) = last {
                            painter.line_segment([prev, pos], stroke);
                        }
                        last = Some(pos);
                    }
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.len() < 2 { continue; }
                    for seg in vertices.windows(2) {
                        let a: Vec2 = seg[0].into();
                        let b: Vec2 = seg[1].into();
                        let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                        let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                        painter.line_segment(
                            [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                            stroke,
                        );
                    }
                    if *closed && vertices.len() >= 2 {
                        let a: Vec2 = vertices.last().unwrap().to_owned().into();
                        let b: Vec2 = vertices.first().unwrap().to_owned().into();
                        let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                        let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                        painter.line_segment(
                            [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                            stroke,
                        );
                    }
                }
                EntityKind::DimAligned { .. } | EntityKind::DimLinear { .. } | EntityKind::DimAngular { .. } | EntityKind::DimRadial { .. } | EntityKind::Text { .. } => {}
            }
        }
    }

    /// Draw cyan highlight overlay for the OFFSET selected entity.
    fn draw_offset_overlay(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        let id = match self.offset_selected_entity {
            Some(id) if self.offset_phase == OffsetPhase::SelectingSide => id,
            _ => return,
        };
        let Some(entity) = self.drawing.get_entity(&id) else { return };
        let stroke = egui::Stroke::new(2.5, egui::Color32::from_rgb(40, 220, 255));
        let painter = ui.painter_at(rect);
        match &entity.kind {
            EntityKind::Line { start, end } => {
                let (x1, y1) = world_to_screen(start.x as f32, start.y as f32, viewport);
                let (x2, y2) = world_to_screen(end.x as f32, end.y as f32, viewport);
                painter.line_segment(
                    [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                    stroke,
                );
            }
            EntityKind::Circle { center, radius } => {
                let c: Vec2 = (*center).into();
                let (cx, cy) = world_to_screen(c.x as f32, c.y as f32, viewport);
                let (rx, _) = world_to_screen((c.x + radius) as f32, c.y as f32, viewport);
                painter.circle_stroke(rect.min + egui::vec2(cx, cy), (rx - cx).abs(), stroke);
            }
            EntityKind::Arc { center, radius, start_angle, end_angle } => {
                let c: Vec2 = (*center).into();
                let sweep = *end_angle - *start_angle;
                let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                let mut last: Option<egui::Pos2> = None;
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let ang = *start_angle + sweep * t;
                    let px = c.x + *radius * ang.cos();
                    let py = c.y + *radius * ang.sin();
                    let (sx, sy) = world_to_screen(px as f32, py as f32, viewport);
                    let pos = rect.min + egui::vec2(sx, sy);
                    if let Some(prev) = last {
                        painter.line_segment([prev, pos], stroke);
                    }
                    last = Some(pos);
                }
            }
            EntityKind::Polyline { vertices, closed } => {
                if vertices.len() < 2 { return; }
                for seg in vertices.windows(2) {
                    let a: Vec2 = seg[0].into();
                    let b: Vec2 = seg[1].into();
                    let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                    let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
                if *closed && vertices.len() >= 2 {
                    let a: Vec2 = vertices.last().unwrap().to_owned().into();
                    let b: Vec2 = vertices.first().unwrap().to_owned().into();
                    let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                    let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(x1, y1), rect.min + egui::vec2(x2, y2)],
                        stroke,
                    );
                }
            }
            EntityKind::DimAligned { .. } | EntityKind::DimLinear { .. } | EntityKind::DimAngular { .. } | EntityKind::DimRadial { .. } | EntityKind::Text { .. } => {}
        }
    }

    /// Find the entity whose geometry is nearest `screen_pos` within PICK_RADIUS.
    fn entity_at_screen_pos(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<Guid> {
        let mut best: Option<(f32, Guid)> = None;
        for entity in self.drawing.visible_entities() {
            let d = Self::screen_dist_to_entity(&entity.kind, viewport, rect, screen_pos);
            if d <= Self::PICK_RADIUS {
                if best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                    best = Some((d, entity.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    fn draw_arc_input_ticks(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        if let ActiveTool::Arc { start, mid } = &self.active_tool {
            if let Some(s) = start {
                Self::draw_tick_marker(ui, rect, viewport, *s, egui::Color32::from_rgb(255, 230, 80));
            }
            if let Some(m) = mid {
                Self::draw_tick_marker(ui, rect, viewport, *m, egui::Color32::from_rgb(120, 255, 200));
            }
        }
    }

}

impl eframe::App for CadKitApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // === Deferred operations that need ctx ===
        if self.pending_dxf_import {
            self.pending_dxf_import = false;
            self.import_dxf(ctx);
        }

        // === Global keyboard shortcuts (fire even while command line has focus) ===

        // Ctrl+S: save
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S)) {
            self.save(ctx);
        }
        // Ctrl+Z / Ctrl+Y: undo/redo
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            self.undo();
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
            || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z)) {
            self.redo();
        }
        // F3: snap toggle
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F3)) {
            self.snap_enabled = !self.snap_enabled;
            self.command_log.push(format!("Snap {}", if self.snap_enabled { "ON" } else { "OFF" }));
        }
        // F8: ortho toggle
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F8)) {
            self.ortho_enabled = !self.ortho_enabled;
            self.command_log.push(format!("Ortho {}", if self.ortho_enabled { "ON" } else { "OFF" }));
        }
        // ESC: clear command input if non-empty; else cancel FROM, then tool, trim, etc.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if !self.command_input.is_empty() {
                self.command_input.clear();
            } else if self.from_phase != FromPhase::Idle {
                self.exit_from();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.trim_phase, TrimPhase::Idle) {
                self.exit_trim();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.offset_phase, OffsetPhase::Idle) {
                self.exit_offset();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.move_phase, MovePhase::Idle) {
                self.exit_move();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.copy_phase, CopyPhase::Idle) {
                self.exit_copy();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.rotate_phase, RotatePhase::Idle) {
                self.exit_rotate();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.scale_phase, ScalePhase::Idle) {
                self.exit_scale();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.mirror_phase, MirrorPhase::Idle) {
                self.exit_mirror();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.fillet_phase, FilletPhase::Idle) {
                self.exit_fillet();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.chamfer_phase, ChamferPhase::Idle) {
                self.exit_chamfer();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.polygon_phase, PolygonPhase::Idle) {
                self.exit_polygon();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.ellipse_phase, EllipsePhase::Idle) {
                self.exit_ellipse();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.rectangle_phase, RectanglePhase::Idle) {
                self.exit_rectangle();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.array_phase, ArrayPhase::Idle) {
                self.exit_array();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.pedit_phase, PeditPhase::Idle) {
                self.exit_pedit();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.extend_phase, ExtendPhase::Idle) {
                self.exit_extend();
                self.command_log.push("*Cancel*".to_string());
            } else if self.has_active_dimension_command() {
                self.exit_dim();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.text_phase, TextPhase::Idle) {
                self.exit_text();
                self.command_log.push("*Cancel*".to_string());
            } else if self.text_edit_dialog.is_some() || !matches!(self.edit_text_phase, EditTextPhase::Idle) {
                self.exit_edit_text();
                self.command_log.push("*Cancel*".to_string());
            } else if self.dim_edit_dialog.is_some() || !matches!(self.edit_dim_phase, EditDimPhase::Idle) {
                self.exit_edit_dim();
                self.command_log.push("*Cancel*".to_string());
            } else if matches!(self.active_tool, ActiveTool::None) {
                self.selected_entities.clear();
                self.selection = None;
                self.selection_drag_start = None;
                self.selection_drag_current = None;
            } else {
                self.cancel_active_tool();
            }
        }
        // Delete: remove selected entities (only when command line is empty and no tool active)
        if self.command_input.is_empty()
            && matches!(self.active_tool, ActiveTool::None)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Delete))
        {
            let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
            let ids = self.filter_editable_entity_ids(&requested, "DELETE");
            if !ids.is_empty() {
                self.push_undo();
            }
            for id in &ids {
                let _ = self.drawing.remove_entity(id);
            }
            if !ids.is_empty() {
                self.command_log.push(format!(
                    "Deleted {} entit{}",
                    ids.len(),
                    if ids.len() == 1 { "y" } else { "ies" }
                ));
            }
            self.selected_entities.clear();
            self.selection = None;
        }

        // Mirror command_input → distance_input so rubber-band preview tracks typed value
        if self.tool_uses_distance_input() {
            self.distance_input = self.command_input.clone();
        } else {
            self.distance_input.clear();
        }

        // UI panels (menu, toolbars, properties, command line)
        self.draw_ui_panels(ctx);

        // === Edit Text dialog ===
        if self.text_edit_dialog.is_some() {
            let mut ok_clicked = false;
            let mut cancel_clicked = false;

            // Temporarily take the dialog out to allow &mut self in the closure.
            let mut dlg = self.text_edit_dialog.take().unwrap();

            egui::Window::new("Edit Text")
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Grid::new("edit_text_grid")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Content:");
                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut dlg.content)
                                    .desired_width(240.0)
                                    .hint_text("Enter text"),
                            );
                            // Only steal focus on the very first frame so the user
                            // can freely click Height / Rotation fields afterward.
                            if !dlg.focus_requested {
                                resp.request_focus();
                                dlg.focus_requested = true;
                            }
                            ui.end_row();

                            ui.label("Height (world units):");
                            ui.add(
                                egui::TextEdit::singleline(&mut dlg.height_str)
                                    .desired_width(80.0),
                            );
                            ui.end_row();

                            ui.label("Rotation (degrees):");
                            ui.add(
                                egui::TextEdit::singleline(&mut dlg.rotation_str)
                                    .desired_width(80.0),
                            );
                            ui.end_row();
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            ok_clicked = true;
                        }
                        if ui.button("Cancel").clicked() {
                            cancel_clicked = true;
                        }
                    });

                    // Enter = OK, Escape = Cancel
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ok_clicked = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel_clicked = true;
                    }
                });

            if ok_clicked {
                let height = dlg.height_str.trim().parse::<f64>().ok()
                    .filter(|&h| h > f64::EPSILON)
                    .unwrap_or(self.last_text_height);
                let rotation = dlg.rotation_str.trim().parse::<f64>()
                    .map(|d| d.to_radians())
                    .unwrap_or(self.last_text_rotation);
                let content = dlg.content.clone();

                if !content.is_empty() {
                    if self.is_entity_on_locked_layer(&dlg.id) {
                        self.command_log
                            .push("EDITTEXT: Entity is on a locked layer".to_string());
                    } else {
                        self.last_text_height = height;
                        self.last_text_rotation = rotation;
                        self.push_undo();
                        if let Some(entity) = self.drawing.get_entity_mut(&dlg.id) {
                            if let EntityKind::Text { content: c, height: h, rotation: r, .. } = &mut entity.kind {
                                *c = content;
                                *h = height;
                                *r = rotation;
                            }
                        }
                    }
                }
                // dialog consumed — leave text_edit_dialog = None
            } else if cancel_clicked {
                // dialog consumed — leave text_edit_dialog = None
            } else {
                // dialog still open — put it back
                self.text_edit_dialog = Some(dlg);
            }
        }

        // === Edit Dim dialog ===
        if self.dim_edit_dialog.is_some() {
            let mut ok_clicked = false;
            let mut cancel_clicked = false;
            let mut dlg = self.dim_edit_dialog.take().unwrap();

            egui::Window::new("Edit Dimension Text")
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Text override ('<>' = measured distance, e.g. '<> mm'):");
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut dlg.override_str)
                            .desired_width(240.0)
                            .hint_text("auto"),
                    );
                    if !dlg.focus_requested {
                        resp.request_focus();
                        dlg.focus_requested = true;
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() { ok_clicked = true; }
                        if ui.button("Cancel").clicked() { cancel_clicked = true; }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel_clicked = true;
                    }
                });

            if ok_clicked {
                if self.is_entity_on_locked_layer(&dlg.id) {
                    self.command_log
                        .push("EDITDIM: Entity is on a locked layer".to_string());
                } else {
                    self.push_undo();
                    if let Some(entity) = self.drawing.get_entity_mut(&dlg.id) {
                        match &mut entity.kind {
                            EntityKind::DimAligned { text_override, .. }
                            | EntityKind::DimLinear { text_override, .. }
                            | EntityKind::DimAngular { text_override, .. }
                            | EntityKind::DimRadial { text_override, .. } => {
                                let s = dlg.override_str.trim();
                                *text_override = if s.is_empty() || s == "<>" { None } else { Some(s.to_string()) };
                            }
                            _ => {}
                        }
                    }
                }
            } else if !cancel_clicked {
                self.dim_edit_dialog = Some(dlg);
            }
        }

        // === DimStyle dialog ===
        if self.dim_style_dialog.is_some() {
            let mut ok_clicked = false;
            let mut cancel_clicked = false;
            let mut dlg = self.dim_style_dialog.take().unwrap();

            egui::Window::new("DimStyle")
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Grid::new("dimstyle_grid")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Text height:");
                            ui.add(egui::TextEdit::singleline(&mut dlg.text_height_str).desired_width(90.0));
                            ui.end_row();

                            ui.label("Precision:");
                            ui.add(egui::TextEdit::singleline(&mut dlg.precision_str).desired_width(90.0));
                            ui.end_row();

                            ui.label("Layer color:");
                            ui.color_edit_button_srgb(&mut dlg.color);
                            ui.end_row();

                            ui.label("Arrow length:");
                            ui.add(egui::TextEdit::singleline(&mut dlg.arrow_length_str).desired_width(90.0));
                            ui.end_row();

                            ui.label("Arrow half-width:");
                            ui.add(egui::TextEdit::singleline(&mut dlg.arrow_half_width_str).desired_width(90.0));
                            ui.end_row();
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() { ok_clicked = true; }
                        if ui.button("Cancel").clicked() { cancel_clicked = true; }
                    });
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        ok_clicked = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        cancel_clicked = true;
                    }
                });

            if ok_clicked {
                let text_height = dlg
                    .text_height_str
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| *v > f64::EPSILON)
                    .unwrap_or(self.dim_style.text_height);
                let precision = dlg
                    .precision_str
                    .trim()
                    .parse::<usize>()
                    .ok()
                    .map(|p| p.min(8))
                    .unwrap_or(self.dim_style.precision);
                let arrow_length = dlg
                    .arrow_length_str
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| *v > f64::EPSILON)
                    .unwrap_or(self.dim_style.arrow_length);
                let arrow_half_width = dlg
                    .arrow_half_width_str
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|v| *v > f64::EPSILON)
                    .unwrap_or(self.dim_style.arrow_half_width);
                self.dim_style = DimStyle {
                    text_height,
                    precision,
                    color: dlg.color,
                    arrow_length,
                    arrow_half_width,
                };
                let _ = self.ensure_dim_layer();
                self.command_log.push("DIMSTYLE: Updated".to_string());
            } else if !cancel_clicked {
                self.dim_style_dialog = Some(dlg);
            }
        }

        if self.help_open {
            egui::Window::new("CadKit — Command Reference")
                .open(&mut self.help_open)
                .resizable(true)
                .default_width(480.0)
                .default_height(520.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace("DRAW");
                        ui.separator();
                        egui::Grid::new("help_draw").striped(true).show(ui, |ui| {
                            for (alias, full, desc) in [
                                ("L / LINE",        "",          "Draw lines (click-click or type coords)"),
                                ("C / CIRCLE",      "",          "Draw circle (center then radius/diameter)"),
                                ("A / ARC",         "",          "Draw arc through 3 points"),
                                ("PL / PLINE",      "POLYLINE",  "Draw polyline; C closes it"),
                                ("POL / POLYGON",   "",          "Regular polygon (sides, center, radius point)"),
                                ("EL / ELLIPSE",    "",          "Ellipse by center, radius, and height"),
                                ("REC / RECTANGLE", "",          "Rectangle by diagonal or dimensions"),
                                ("T / TEXT",        "",          "Place a text label"),
                            ] {
                                ui.label(egui::RichText::new(alias).strong());
                                ui.label(full);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                        ui.add_space(8.0);
                        ui.monospace("EDIT");
                        ui.separator();
                        egui::Grid::new("help_edit").striped(true).show(ui, |ui| {
                            for (alias, full, desc) in [
                                ("TR / TRIM",      "",          "Trim entity at cutting edges"),
                                ("EX / EXTEND",    "",          "Extend entity to boundary"),
                                ("O / OFFSET",     "",          "Offset entity by distance"),
                                ("M / MOVE",       "",          "Move entities"),
                                ("CO / COPY",      "",          "Copy entities"),
                                ("RO / ROTATE",    "",          "Rotate entities"),
                                ("SC / SCALE",     "",          "Scale entities from base/reference"),
                                ("MI / MIRROR",    "",          "Mirror entities about an axis"),
                                ("AR / ARRAY",     "",          "Create rectangular or polar arrays"),
                                ("FI / FILLET",    "",          "Round two lines with tangent arc"),
                                ("CHA / CHAMFER",  "",          "Bevel two lines/segments; use d or d1,d2 (0 allowed)"),
                                ("J / JOIN",       "",          "Join touching lines/polylines into selected polyline"),
                                ("PE / PEDIT",     "",          "Select polyline, then join touching line/arc"),
                                ("ET / EDITTEXT",  "",          "Edit a text entity via dialog"),
                                ("U / UNDO",       "",          "Undo last change"),
                                ("R / REDO",       "",          "Redo undone change"),
                            ] {
                                ui.label(egui::RichText::new(alias).strong());
                                ui.label(full);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                        ui.add_space(8.0);
                        ui.monospace("DIMENSION");
                        ui.separator();
                        egui::Grid::new("help_dim").striped(true).show(ui, |ui| {
                            for (alias, full, desc) in [
                                ("DAL",          "DIMALIGNED",  "Place an aligned dimension (true distance)"),
                                ("DLI",          "DIMLINEAR",   "Place a H or V linear dimension (drag to lock axis)"),
                                ("DANG",         "DIMANGULAR",  "Place an angular dimension (vertex → ray1 → ray2 → drag radius)"),
                                ("DRA",          "DIMRADIUS",   "Place a radial dimension on a circle or arc"),
                                ("DDI",          "DIMDIAMETER", "Place a diameter dimension on a circle or arc"),
                                ("ED / EDITDIM", "",            "Edit dimension text (<> = measured distance)"),
                                ("DIMSTYLE",     "",            "Edit dimension style defaults"),
                            ] {
                                ui.label(egui::RichText::new(alias).strong());
                                ui.label(full);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                        ui.add_space(8.0);
                        ui.monospace("FILE");
                        ui.separator();
                        egui::Grid::new("help_file").striped(true).show(ui, |ui| {
                            for (alias, full, desc) in [
                                ("DXFOUT", "",  "Export drawing to DXF"),
                                ("DXFIN",  "",  "Import a DXF file"),
                            ] {
                                ui.label(egui::RichText::new(alias).strong());
                                ui.label(full);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                        ui.add_space(8.0);
                        ui.monospace("VIEW / SETTINGS");
                        ui.separator();
                        egui::Grid::new("help_view").striped(true).show(ui, |ui| {
                            for (alias, full, desc) in [
                                ("BGCOLOR",     "",  "Open background colour picker"),
                                ("GR / GRID",   "",  "Toggle grid visibility (dots off, snap still works)"),
                                ("LA / LAYER",  "",  "Manage layers (see right panel)"),
                            ] {
                                ui.label(egui::RichText::new(alias).strong());
                                ui.label(full);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                        ui.add_space(8.0);
                        ui.monospace("INPUT MODIFIERS");
                        ui.separator();
                        egui::Grid::new("help_input").striped(true).show(ui, |ui| {
                            for (alias, full, desc) in [
                                ("FROM / FR",    "",  "Specify point relative to a snapped base"),
                                ("@x,y",         "",  "Relative Cartesian offset from last point"),
                                ("@dist<angle",  "",  "Relative polar offset (angle in degrees)"),
                                ("ESC / CANCEL", "",  "Cancel current command"),
                            ] {
                                ui.label(egui::RichText::new(alias).strong());
                                ui.label(full);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                    });
                });
        }

        if let Some(layer_id) = self.layer_color_picking {
            let mut still_open = true;
            let mut picked_color: Option<[u8; 3]> = None;
            let current_color = self.drawing.get_layer(layer_id)
                .map(|l| l.color)
                .unwrap_or([255, 255, 255]);

            egui::Window::new("Layer Colour")
                .open(&mut still_open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("Standard").small().color(egui::Color32::from_gray(150)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 1u8..=9 {
                            let rgb = aci_color(i);
                            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, c);
                            let (sw, sc) = if current_color == rgb {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (0.5, egui::Color32::from_gray(70))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(sw, sc));
                            if resp.on_hover_text(format!("ACI {i}")).clicked() {
                                picked_color = Some(rgb);
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Index colours").small().color(egui::Color32::from_gray(150)));
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for row in 0u8..10 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            for col in 0u8..24 {
                                let idx: u8 = 10 + col * 10 + row;
                                let rgb = aci_color(idx);
                                let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                                ui.painter().rect_filled(rect, 1.0, c);
                                if current_color == rgb {
                                    ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                                }
                                if resp.on_hover_text(format!("ACI {idx}")).clicked() {
                                    picked_color = Some(rgb);
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Grayscale").small().color(egui::Color32::from_gray(150)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 250u8..=255 {
                            let rgb = aci_color(i);
                            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, c);
                            let (sw, sc) = if current_color == rgb {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (0.5, egui::Color32::from_gray(70))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(sw, sc));
                            if resp.on_hover_text(format!("ACI {i}")).clicked() {
                                picked_color = Some(rgb);
                            }
                        }
                    });
                });

            if let Some(rgb) = picked_color {
                if let Some(l) = self.drawing.get_layer_mut(layer_id) {
                    l.color = rgb;
                }
                self.layer_color_picking = None;
            }
            if !still_open {
                self.layer_color_picking = None;
            }
        }

        if self.bgcolor_picker_open {
            let mut still_open = true;
            let mut picked_color: Option<[u8; 3]> = None;
            let current_color = self.viewport.as_ref().map(|vp| vp.bg_srgb()).unwrap_or([81, 81, 81]);

            egui::Window::new("Background Colour")
                .open(&mut still_open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(egui::RichText::new("Standard").small().color(egui::Color32::from_gray(150)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 1u8..=9 {
                            let rgb = aci_color(i);
                            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, c);
                            let (sw, sc) = if current_color == rgb {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (0.5, egui::Color32::from_gray(70))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(sw, sc));
                            if resp.on_hover_text(format!("ACI {i}")).clicked() {
                                picked_color = Some(rgb);
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Index colours").small().color(egui::Color32::from_gray(150)));
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for row in 0u8..10 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            for col in 0u8..24 {
                                let idx: u8 = 10 + col * 10 + row;
                                let rgb = aci_color(idx);
                                let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                                ui.painter().rect_filled(rect, 1.0, c);
                                if current_color == rgb {
                                    ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                                }
                                if resp.on_hover_text(format!("ACI {idx}")).clicked() {
                                    picked_color = Some(rgb);
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Grayscale").small().color(egui::Color32::from_gray(150)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 250u8..=255 {
                            let rgb = aci_color(i);
                            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, c);
                            let (sw, sc) = if current_color == rgb {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (0.5, egui::Color32::from_gray(70))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(sw, sc));
                            if resp.on_hover_text(format!("ACI {i}")).clicked() {
                                picked_color = Some(rgb);
                            }
                        }
                    });
                });

            if let Some([r, g, b]) = picked_color {
                let to_linear = |v: u8| (v as f32 / 255.0).powf(2.2);
                if let Some(vp) = &mut self.viewport {
                    vp.clear_color = [to_linear(r), to_linear(g), to_linear(b)];
                }
                self.bgcolor_picker_open = false;
            }
            if !still_open {
                self.bgcolor_picker_open = false;
            }
        }

        if self.entity_color_picker_open && !self.selected_entities.is_empty() {
            let mut still_open = true;
            let mut picked_color: Option<[u8; 3]> = None;

            // Determine current common entity colour for highlight.
            let mut cur_ec: Option<Option<[u8; 3]>> = None;
            let mut ec_mixed = false;
            for id in &self.selected_entities {
                if let Some(e) = self.drawing.get_entity(id) {
                    match cur_ec {
                        None                          => cur_ec = Some(e.color),
                        Some(c) if c == e.color       => {}
                        _                             => { ec_mixed = true; break; }
                    }
                }
            }
            let highlight: Option<[u8; 3]> = if ec_mixed { None } else { cur_ec.flatten() };

            egui::Window::new("Entity Colour")
                .open(&mut still_open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    let current_color = highlight.unwrap_or([255, 255, 255]);

                    // Standard colours 1-9
                    ui.label(egui::RichText::new("Standard").small().color(egui::Color32::from_gray(150)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 1u8..=9 {
                            let rgb = aci_color(i);
                            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, c);
                            let (sw, sc) = if current_color == rgb && !ec_mixed {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (0.5, egui::Color32::from_gray(70))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(sw, sc));
                            if resp.on_hover_text(format!("ACI {i}")).clicked() {
                                picked_color = Some(rgb);
                            }
                        }
                    });

                    ui.add_space(6.0);

                    // Index colour grid 10-249
                    ui.label(egui::RichText::new("Index colours").small().color(egui::Color32::from_gray(150)));
                    ui.spacing_mut().item_spacing.y = 1.0;
                    for row in 0u8..10 {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            for col in 0u8..24 {
                                let idx: u8 = 10 + col * 10 + row;
                                let rgb = aci_color(idx);
                                let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                                ui.painter().rect_filled(rect, 1.0, c);
                                if current_color == rgb && !ec_mixed {
                                    ui.painter().rect_stroke(rect, 1.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
                                }
                                if resp.on_hover_text(format!("ACI {idx}")).clicked() {
                                    picked_color = Some(rgb);
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);

                    // Grayscale 250-255
                    ui.label(egui::RichText::new("Grayscale").small().color(egui::Color32::from_gray(150)));
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        for i in 250u8..=255 {
                            let rgb = aci_color(i);
                            let c = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, c);
                            let (sw, sc) = if current_color == rgb && !ec_mixed {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (0.5, egui::Color32::from_gray(70))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(sw, sc));
                            if resp.on_hover_text(format!("ACI {i}")).clicked() {
                                picked_color = Some(rgb);
                            }
                        }
                    });
                });

            if let Some(rgb) = picked_color {
                let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                let ids = self.filter_editable_entity_ids(&requested, "PROPERTIES");
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) {
                        e.color = Some(rgb);
                    }
                }
                self.entity_color_picker_open = false;
            }
            if !still_open {
                self.entity_color_picker_open = false;
            }
        }

        // Bottom panel - command line (AutoCAD-style with history)
        
        // Central panel - viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let width = available.x.max(1.0) as u32;
            let height = available.y.max(1.0) as u32;
            self.hover_world_pos = None;
            self.snap_intersection_point = None;
            self.hover_dim_grip = None;

            // Auto-focus command line when nothing else has keyboard focus
            self.auto_focus_command_line(ctx);

            if self.viewport.is_some() {
                if let Some(viewport) = &mut self.viewport {
                    let (current_w, current_h) = viewport.size();
                    if current_w != width || current_h != height {
                        viewport.resize(width, height);
                    }
                    viewport.render(&self.drawing);
                }

                self.ensure_registered_texture(frame);
                if let Some(texture_id) = self.viewport_texture_id {
                    let response = ui.add(
                        egui::Image::new((texture_id, available))
                            .sense(egui::Sense::click_and_drag()),
                    );

                    if let Some(viewport) = self.viewport.as_ref() {
                        if self.grid_visible {
                            Self::draw_grid_overlay(ui, response.rect, viewport, self.grid_spacing);
                        }
                        self.draw_selected_entities_overlay(ui, response.rect, viewport);
                        self.draw_arc_input_ticks(ui, response.rect, viewport);
                        self.draw_trim_overlay(ui, response.rect, viewport);
                        self.draw_offset_overlay(ui, response.rect, viewport);
                        self.draw_extend_overlay(ui, response.rect, viewport);
                        self.draw_text_entities(ui, response.rect, viewport);
                        self.draw_dim_entities(ui, response.rect, viewport);
                        self.draw_dimension_grips(ui, response.rect, viewport);
                    }

                    // Draw current snap/pick marker (if any).
                    if let (Some(selection), Some(viewport)) =
                        (&self.selection, self.viewport.as_ref())
                    {
                        let (sx, sy) = world_to_screen(
                            selection.world.x as f32,
                            selection.world.y as f32,
                            viewport,
                        );
                        let pos = response.rect.min + egui::vec2(sx, sy);
                        let color = egui::Color32::from_rgb(0, 200, 255);
                        let painter = ui.painter_at(response.rect);
                        painter.rect_filled(
                            egui::Rect::from_center_size(pos, egui::vec2(10.0, 10.0)),
                            2.0,
                            color,
                        );
                        painter.rect_stroke(
                            egui::Rect::from_center_size(pos, egui::vec2(14.0, 14.0)),
                            3.0,
                            egui::Stroke::new(1.5, color),
                        );
                    }

                    // Selection drag box for window/crossing selection.
                    if let (Some(start), Some(current)) =
                        (self.selection_drag_start, self.selection_drag_current)
                    {
                        let r = egui::Rect::from_two_pos(start, current);
                        let window_mode = current.x >= start.x;
                        let stroke_color = if window_mode {
                            egui::Color32::from_rgb(120, 210, 255)
                        } else {
                            egui::Color32::from_rgb(120, 255, 180)
                        };
                        let fill_color = if window_mode {
                            egui::Color32::from_rgba_premultiplied(80, 180, 255, 30)
                        } else {
                            egui::Color32::from_rgba_premultiplied(80, 255, 180, 30)
                        };
                        let painter = ui.painter_at(response.rect);
                        painter.rect_filled(r, 0.0, fill_color);
                        painter.rect_stroke(r, 0.0, egui::Stroke::new(1.5, stroke_color));
                    }

                    // Entity selection in idle mode: click, shift-toggle, and window/crossing drag.
                    if matches!(self.active_tool, ActiveTool::None) {
                        // TRIM click handling overrides selection when trim is active.
                        if !matches!(self.trim_phase, TrimPhase::Idle) {
                            if response.clicked_by(egui::PointerButton::Primary) {
                                if let (Some(click_pos), Some(viewport)) = (
                                    response.interact_pointer_pos(),
                                    self.viewport.as_ref(),
                                ) {
                                    match self.trim_phase {
                                        TrimPhase::SelectingEdges => {
                                            // Toggle entity in/out of cutting edges.
                                            if let Some(id) = self.entity_at_screen_pos(
                                                viewport,
                                                response.rect,
                                                click_pos,
                                            ) {
                                                if let Some(pos) =
                                                    self.trim_cutting_edges.iter().position(|&x| x == id)
                                                {
                                                    self.trim_cutting_edges.remove(pos);
                                                    self.command_log
                                                        .push("TRIM: Removed cutting edge.".to_string());
                                                } else {
                                                    self.trim_cutting_edges.push(id);
                                                    self.command_log
                                                        .push("TRIM: Added cutting edge.".to_string());
                                                }
                                            }
                                        }
                                        TrimPhase::Trimming => {
                                            // compute_trim is &self: compatible with viewport: &self.viewport.
                                            // Mutations are applied via direct field access (borrow splitting).
                                            let rect = response.rect;
                                            let trim_result =
                                                self.compute_trim(click_pos, viewport, rect);
                                            match trim_result {
                                                TrimResult::Fail(msg) => {
                                                    self.command_log.push(msg);
                                                }
                                                TrimResult::Apply { target_id, new_entities } => {
                                                    if self.is_entity_on_locked_layer(&target_id) {
                                                        self.command_log.push(
                                                            "TRIM: Target entity is on a locked layer".to_string(),
                                                        );
                                                    } else {
                                                        self.push_undo();
                                                        let _ = self.drawing.remove_entity(&target_id);
                                                        self.trim_cutting_edges
                                                            .retain(|&id| id != target_id);
                                                        for entity in new_entities {
                                                            self.drawing.add_entity(entity);
                                                        }
                                                        log::info!("TRIM: entity trimmed");
                                                    }
                                                }
                                            }
                                        }
                                        TrimPhase::Idle => {}
                                    }
                                }
                            }
                        } else {
                            // Idle (no trim): EXTEND, MOVE, OFFSET, or regular selection.
                            if response.clicked_by(egui::PointerButton::Primary) {
                                if let (Some(click_pos), Some(viewport)) = (
                                    response.interact_pointer_pos(),
                                    self.viewport.as_ref(),
                                ) {
                                    if let Some(handle) = self.dim_grip_drag {
                                        if self.dim_grip_is_dragging {
                                            // Drag release click: ignore normal click routing.
                                        } else {
                                            let apply_handle = if matches!(handle.kind, DimGripKind::Start | DimGripKind::End) {
                                                self.dim_grip_tracking_target_handle(handle)
                                            } else {
                                                handle
                                            };
                                            let (world, _) = self.snapped_world_for_grip_drag(
                                                handle,
                                                viewport,
                                                response.rect,
                                                click_pos,
                                            );
                                            let world = self.constrained_dim_grip_world(apply_handle, world);
                                            self.push_undo();
                                            self.apply_dim_grip_drag(apply_handle, world);
                                            self.dim_grip_drag = None;
                                            self.dim_grip_is_dragging = false;
                                        }
                                    } else if self
                                        .pick_dim_grip_handle(viewport, response.rect, click_pos)
                                        .is_some()
                                    {
                                        if let Some(handle) =
                                            self.pick_dim_grip_handle(viewport, response.rect, click_pos)
                                        {
                                            if self.is_entity_on_locked_layer(&handle.entity) {
                                                self.command_log.push(
                                                    "DIM: Entity is on a locked layer".to_string(),
                                                );
                                            } else {
                                                self.dim_grip_drag = Some(handle);
                                                self.dim_grip_is_dragging = false;
                                                self.command_log.push(
                                                    "DIM GRIP: Base fixed. Drag direction, type distance, or click target"
                                                        .to_string(),
                                                );
                                            }
                                        }
                                    } else if self.from_phase == FromPhase::WaitingBase || self.from_phase == FromPhase::WaitingOffset {
                                        // FROM base/offset pick in idle mode — same snap as MOVE.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.from_phase == FromPhase::WaitingBase {
                                            self.from_base = Some(world);
                                            self.from_phase = FromPhase::WaitingOffset;
                                            self.command_log.push(format!("  Base: {:.4}, {:.4}", world.x, world.y));
                                            self.command_log.push("FROM  Offset (@dx,dy  or  @dist<angle):".to_string());
                                        } else {
                                            // WaitingOffset + click = use cursor position directly.
                                            let result = world;
                                            self.apply_from_result_point(result);
                                        }
                                    } else if self.extend_phase == ExtendPhase::SelectingBoundaries {
                                        // Toggle boundary edge.
                                        match self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                            Some(id) => {
                                                if let Some(pos) = self.extend_boundary_edges.iter().position(|&x| x == id) {
                                                    self.extend_boundary_edges.remove(pos);
                                                } else {
                                                    self.extend_boundary_edges.push(id);
                                                }
                                            }
                                            None => {
                                                self.command_log.push("EXTEND: Nothing found near click".to_string());
                                            }
                                        }
                                    } else if self.extend_phase == ExtendPhase::Extending {
                                        let rect = response.rect;
                                        match self.compute_extend(click_pos, viewport, rect) {
                                            Ok(result) => {
                                                let target_id = match &result {
                                                    ExtendResult::Line { id, .. } | ExtendResult::Arc { id, .. } => *id,
                                                };
                                                if self.is_entity_on_locked_layer(&target_id) {
                                                    self.command_log.push(
                                                        "EXTEND: Target entity is on a locked layer".to_string(),
                                                    );
                                                } else {
                                                    self.push_undo();
                                                    match result {
                                                        ExtendResult::Line { id: eid, is_start, new_pt } => {
                                                            if let Some(entity) = self.drawing.get_entity_mut(&eid) {
                                                                if let EntityKind::Line { start, end } = &mut entity.kind {
                                                                    if is_start {
                                                                        start.x = new_pt.x;
                                                                        start.y = new_pt.y;
                                                                    } else {
                                                                        end.x = new_pt.x;
                                                                        end.y = new_pt.y;
                                                                    }
                                                                }
                                                            }
                                                            self.command_log.push("EXTEND: Line extended".to_string());
                                                        }
                                                        ExtendResult::Arc { id: eid, is_start, new_angle } => {
                                                            if let Some(entity) = self.drawing.get_entity_mut(&eid) {
                                                                if let EntityKind::Arc { start_angle, end_angle, .. } = &mut entity.kind {
                                                                    if is_start {
                                                                        *start_angle = new_angle;
                                                                    } else {
                                                                        *end_angle = new_angle;
                                                                    }
                                                                }
                                                            }
                                                            self.command_log.push("EXTEND: Arc extended".to_string());
                                                        }
                                                    }
                                                }
                                            }
                                            Err(msg) => {
                                                self.command_log.push(msg);
                                            }
                                        }
                                    } else if matches!(self.move_phase, MovePhase::BasePoint | MovePhase::Destination) {
                                        // MOVE point pick: entity snap is always active (snap highlight
                                        // is always shown in idle mode), grid snap only when enabled.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        // Always attempt entity-point snap (matches hover highlight behaviour).
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.move_phase == MovePhase::BasePoint {
                                            self.move_base_point = Some(world);
                                            self.move_phase = MovePhase::Destination;
                                            self.command_log.push("MOVE: Pick destination point".to_string());
                                        } else {
                                            self.apply_move(world);
                                        }
                                    } else if matches!(self.copy_phase, CopyPhase::BasePoint | CopyPhase::Destination) {
                                        // COPY point pick — same snap logic as MOVE.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.copy_phase == CopyPhase::BasePoint {
                                            self.copy_base_point = Some(world);
                                            self.copy_phase = CopyPhase::Destination;
                                            self.command_log.push("COPY: Pick destination (Enter to finish)".to_string());
                                        } else {
                                            self.apply_copy(world);
                                        }
                                    } else if matches!(self.rotate_phase, RotatePhase::BasePoint | RotatePhase::Rotation) {
                                        // ROTATE point pick — same snap logic.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.rotate_phase == RotatePhase::BasePoint {
                                            self.rotate_base_point = Some(world);
                                            self.rotate_phase = RotatePhase::Rotation;
                                            self.command_log.push("ROTATE: Specify angle (degrees) or click".to_string());
                                        } else if let Some(base) = self.rotate_base_point {
                                            let angle = (world.y - base.y).atan2(world.x - base.x);
                                            self.apply_rotate(angle);
                                        }
                                    } else if matches!(
                                        self.scale_phase,
                                        ScalePhase::BasePoint | ScalePhase::ReferencePoint | ScalePhase::Factor
                                    ) {
                                        // SCALE point pick — same snap logic.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.scale_phase == ScalePhase::BasePoint {
                                            self.scale_base_point = Some(world);
                                            self.scale_phase = ScalePhase::ReferencePoint;
                                            self.command_log.push("SCALE: Pick reference point".to_string());
                                        } else if self.scale_phase == ScalePhase::ReferencePoint {
                                            if let Some(base) = self.scale_base_point {
                                                let dist = base.distance_to(&world);
                                                if dist > 1e-9 {
                                                    self.scale_ref_point = Some(world);
                                                    self.scale_phase = ScalePhase::Factor;
                                                    self.command_log.push("SCALE: Specify factor or pick point".to_string());
                                                } else {
                                                    self.command_log.push("SCALE: Reference point too close to base".to_string());
                                                }
                                            }
                                        } else {
                                            self.apply_scale_from_point(world);
                                        }
                                    } else if matches!(
                                        self.mirror_phase,
                                        MirrorPhase::FirstAxisPoint | MirrorPhase::SecondAxisPoint
                                    ) {
                                        // MIRROR axis point pick — same snap logic.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.mirror_phase == MirrorPhase::FirstAxisPoint {
                                            self.mirror_axis_p1 = Some(world);
                                            self.mirror_phase = MirrorPhase::SecondAxisPoint;
                                            self.command_log.push("MIRROR: Pick second axis point".to_string());
                                        } else if let Some(p1) = self.mirror_axis_p1 {
                                            let axis_p2 = if self.ortho_enabled {
                                                Self::snap_angle(p1, world, self.ortho_increment_deg)
                                            } else {
                                                world
                                            };
                                            self.apply_mirror(p1, axis_p2);
                                        }
                                    } else if matches!(
                                        self.array_phase,
                                        ArrayPhase::RectBasePoint
                                            | ArrayPhase::RectGripIdle
                                            | ArrayPhase::RectXSpacingGrip
                                            | ArrayPhase::RectXCountGrip
                                            | ArrayPhase::RectYSpacingGrip
                                            | ArrayPhase::RectYCountGrip
                                            | ArrayPhase::PolarCenter
                                            | ArrayPhase::PolarBasePoint
                                    ) {
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        match self.array_phase {
                                            ArrayPhase::RectBasePoint => {
                                                self.array_center = Some(world);
                                                self.array_rect_dir_point = Some(Vec2::new(
                                                    world.x + self.array_rect_dx.abs().max(1.0),
                                                    world.y,
                                                ));
                                                self.array_phase = ArrayPhase::RectGripIdle;
                                                self.command_log.push(
                                                    "ARRAY: Grips visible. Click any grip to activate/edit. Press Enter to apply"
                                                        .to_string(),
                                                );
                                            }
                                            ArrayPhase::RectXSpacingGrip
                                            | ArrayPhase::RectXCountGrip
                                            | ArrayPhase::RectYSpacingGrip
                                            | ArrayPhase::RectYCountGrip => {
                                                if let Some(grip_phase) =
                                                    self.pick_array_rect_grip(viewport, response.rect, click_pos)
                                                {
                                                    self.set_active_array_grip(grip_phase);
                                                } else {
                                                    if self.update_array_rect_from_world(world) {
                                                        self.array_phase = ArrayPhase::RectGripIdle;
                                                        self.command_log.push(
                                                            "ARRAY: Grip released. Click another grip or Enter to apply"
                                                                .to_string(),
                                                        );
                                                    }
                                                }
                                            }
                                            ArrayPhase::RectGripIdle => {
                                                if let Some(grip_phase) =
                                                    self.pick_array_rect_grip(viewport, response.rect, click_pos)
                                                {
                                                    self.set_active_array_grip(grip_phase);
                                                }
                                            }
                                            ArrayPhase::PolarCenter => {
                                                self.array_center = Some(world);
                                                self.array_phase = ArrayPhase::PolarBasePoint;
                                                self.command_log.push("ARRAY: Specify base/reference point".to_string());
                                            }
                                            ArrayPhase::PolarBasePoint => {
                                                if let Some(center) = self.array_center {
                                                    if self.apply_array_polar(center, world) {
                                                        self.exit_array();
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    } else if matches!(
                                        self.fillet_phase,
                                        FilletPhase::FirstEntity | FilletPhase::SecondEntity { .. }
                                    ) {
                                        if let Some(pick) = self.try_pick_fillet_edge(viewport, response.rect, click_pos) {
                                            if self.fillet_phase == FilletPhase::FirstEntity {
                                                self.fillet_phase = FilletPhase::SecondEntity { first: pick };
                                                self.command_log
                                                    .push("FILLET: Select second line or polyline segment".to_string());
                                            } else if let FilletPhase::SecondEntity { first } = self.fillet_phase {
                                                if self.apply_fillet(first, pick) {
                                                    self.fillet_phase = FilletPhase::FirstEntity;
                                                }
                                            }
                                        } else {
                                            self.command_log
                                                .push("FILLET: Pick a line or polyline segment".to_string());
                                        }
                                    } else if matches!(
                                        self.chamfer_phase,
                                        ChamferPhase::FirstEntity | ChamferPhase::SecondEntity { .. }
                                    ) {
                                        if let Some(pick) = self.try_pick_fillet_edge(viewport, response.rect, click_pos) {
                                            if self.chamfer_phase == ChamferPhase::FirstEntity {
                                                self.chamfer_phase = ChamferPhase::SecondEntity { first: pick };
                                                self.command_log.push(
                                                    "CHAMFER: Select second line or polyline segment".to_string(),
                                                );
                                            } else if let ChamferPhase::SecondEntity { first } = self.chamfer_phase {
                                                if self.apply_chamfer(first, pick) {
                                                    self.chamfer_phase = ChamferPhase::FirstEntity;
                                                }
                                            }
                                        } else {
                                            self.command_log
                                                .push("CHAMFER: Pick a line or polyline segment".to_string());
                                        }
                                    } else if matches!(
                                        self.polygon_phase,
                                        PolygonPhase::Center | PolygonPhase::Radius { .. }
                                    ) {
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.polygon_phase == PolygonPhase::Center {
                                            self.polygon_phase = PolygonPhase::Radius { center: world };
                                            self.command_log.push("POLYGON: Specify radius point".to_string());
                                        } else if let PolygonPhase::Radius { center } = self.polygon_phase {
                                            if self.apply_polygon(center, world) {
                                                self.polygon_phase = PolygonPhase::Center;
                                            }
                                        }
                                    } else if matches!(
                                        self.ellipse_phase,
                                        EllipsePhase::Center | EllipsePhase::RadiusX { .. } | EllipsePhase::RadiusY { .. }
                                    ) {
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.ellipse_phase == EllipsePhase::Center {
                                            self.ellipse_phase = EllipsePhase::RadiusX { center: world };
                                            self.command_log.push("ELLIPSE: Specify radius from center".to_string());
                                        } else if let EllipsePhase::RadiusX { center } = self.ellipse_phase {
                                            let p = if self.ortho_enabled {
                                                Self::snap_angle(center, world, self.ortho_increment_deg)
                                            } else {
                                                world
                                            };
                                            let rx = center.distance_to(&p);
                                            if rx > 1e-9 {
                                                self.ellipse_phase = EllipsePhase::RadiusY { center, rx };
                                                self.command_log.push("ELLIPSE: Specify height from center".to_string());
                                            } else {
                                                self.command_log.push("ELLIPSE: Radius too small".to_string());
                                            }
                                        } else if let EllipsePhase::RadiusY { center, rx } = self.ellipse_phase {
                                            let p = if self.ortho_enabled {
                                                Self::snap_angle(center, world, self.ortho_increment_deg)
                                            } else {
                                                world
                                            };
                                            let ry = center.distance_to(&p);
                                            if self.apply_ellipse(center, rx, ry) {
                                                self.ellipse_phase = EllipsePhase::Center;
                                            }
                                        }
                                    } else if matches!(
                                        self.rectangle_phase,
                                        RectanglePhase::FirstCorner
                                            | RectanglePhase::SecondCorner { .. }
                                            | RectanglePhase::Direction { .. }
                                    ) {
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if self.rectangle_phase == RectanglePhase::FirstCorner {
                                            self.rectangle_phase = RectanglePhase::SecondCorner { first: world };
                                            self.command_log.push(
                                                "RECTANGLE: Specify opposite corner or [D=Dimensions]".to_string(),
                                            );
                                        } else if let RectanglePhase::SecondCorner { first } = self.rectangle_phase {
                                            if self.apply_rectangle_diagonal(first, world) {
                                                self.rectangle_phase = RectanglePhase::FirstCorner;
                                            }
                                        } else if let RectanglePhase::Direction { first, width, height } = self.rectangle_phase {
                                            if self.apply_rectangle_dimensions(first, width, height, world) {
                                                self.rectangle_phase = RectanglePhase::FirstCorner;
                                            }
                                        }
                                    } else if matches!(
                                        self.pedit_phase,
                                        PeditPhase::SelectingPolyline | PeditPhase::Joining { .. }
                                    ) {
                                        if let Some(id) = self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                            if self.pedit_phase == PeditPhase::SelectingPolyline {
                                                if self.is_entity_on_locked_layer(&id) {
                                                    self.command_log
                                                        .push("PEDIT: Entity is on a locked layer".to_string());
                                                } else if let Some(e) = self.drawing.get_entity(&id) {
                                                    if matches!(
                                                        e.kind,
                                                        EntityKind::Polyline { closed: false, .. }
                                                    ) {
                                                        self.pedit_phase = PeditPhase::Joining { base: id };
                                                        self.selected_entities.clear();
                                                        self.selected_entities.insert(id);
                                                        self.command_log.push(
                                                            "PEDIT: Select line or arc to join".to_string(),
                                                        );
                                                    } else {
                                                        self.command_log.push(
                                                            "PEDIT: Select an open polyline".to_string(),
                                                        );
                                                    }
                                                }
                                            } else if let PeditPhase::Joining { base } = self.pedit_phase {
                                                if let Some(new_base) = self.pedit_join_entity_into_polyline(base, id) {
                                                    self.pedit_phase = PeditPhase::Joining { base: new_base };
                                                    self.selected_entities.clear();
                                                    self.selected_entities.insert(new_base);
                                                    self.command_log
                                                        .push("PEDIT: Select next line or arc to join".to_string());
                                                }
                                            }
                                        } else if self.pedit_phase == PeditPhase::SelectingPolyline {
                                            self.command_log
                                                .push("PEDIT: Select an open polyline".to_string());
                                        } else {
                                            self.command_log
                                                .push("PEDIT: Select line or arc to join".to_string());
                                        }
                                    } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                        // DIMALIGNED point pick — same snap logic as MOVE/COPY/ROTATE.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if matches!(self.dim_phase, DimPhase::FirstPoint) {
                                            self.dim_phase = DimPhase::SecondPoint { first: world };
                                            self.command_log.push(format!("DIMALIGNED: First point ({:.4}, {:.4})", world.x, world.y));
                                        } else if let DimPhase::SecondPoint { first } = self.dim_phase {
                                            self.dim_phase = DimPhase::Placing { first, second: world };
                                            self.command_log.push(format!("DIMALIGNED: Second point ({:.4}, {:.4})", world.x, world.y));
                                        } else if let DimPhase::Placing { first, second } = self.dim_phase {
                                            self.place_dim_aligned(first, second, world);
                                        }
                                    } else if !matches!(self.dim_linear_phase, DimLinearPhase::Idle) {
                                        // DIMLINEAR point pick.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        if matches!(self.dim_linear_phase, DimLinearPhase::FirstPoint) {
                                            self.dim_linear_phase = DimLinearPhase::SecondPoint { first: world };
                                            self.command_log.push(format!("DIMLINEAR: First point ({:.4}, {:.4})", world.x, world.y));
                                        } else if let DimLinearPhase::SecondPoint { first } = self.dim_linear_phase {
                                            self.dim_linear_phase = DimLinearPhase::Placing { first, second: world };
                                            self.command_log.push(format!("DIMLINEAR: Second point ({:.4}, {:.4})", world.x, world.y));
                                        } else if let DimLinearPhase::Placing { first, second } = self.dim_linear_phase {
                                            self.place_dim_linear(first, second, world);
                                        }
                                    } else if matches!(self.dim_angular_phase, DimAngularPhase::FirstEntity | DimAngularPhase::SecondEntity { .. }) {
                                        // DIMANGULAR entity pick: click on a line or polyline segment.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        if let Some(eid) = self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                            if let Some(entity) = self.drawing.get_entity(&eid) {
                                                if let Some((seg_start, seg_end)) = dim_angular_pick_segment(&entity.kind, raw_world) {
                                                    match self.dim_angular_phase {
                                                        DimAngularPhase::FirstEntity => {
                                                            self.dim_angular_phase = DimAngularPhase::SecondEntity {
                                                                first_click: raw_world,
                                                                first_start: seg_start,
                                                                first_end: seg_end,
                                                            };
                                                            self.command_log.push("DIMANGULAR: Click the second line or segment".to_string());
                                                        }
                                                        DimAngularPhase::SecondEntity { first_click, first_start, first_end } => {
                                                            match line_line_intersect(first_start, first_end, seg_start, seg_end) {
                                                                Some(vertex) => {
                                                                    let line1_pt = ray_dir_from_vertex(first_start, first_end, first_click, vertex);
                                                                    let line2_pt = ray_dir_from_vertex(seg_start, seg_end, raw_world, vertex);
                                                                    self.dim_angular_phase = DimAngularPhase::Placing { vertex, line1_pt, line2_pt };
                                                                    self.command_log.push("DIMANGULAR: Drag arc radius, click to place".to_string());
                                                                }
                                                                None => {
                                                                    self.command_log.push("DIMANGULAR: Lines are parallel — pick different segments".to_string());
                                                                }
                                                            }
                                                        }
                                                        _ => {}
                                                    }
                                                } else {
                                                    self.command_log.push("DIMANGULAR: Only Line and Polyline entities supported".to_string());
                                                }
                                            }
                                        } else {
                                            self.command_log.push("DIMANGULAR: No line found — click on a line or polyline".to_string());
                                        }
                                    } else if let DimAngularPhase::Placing { vertex, line1_pt, line2_pt } = self.dim_angular_phase {
                                        // DIMANGULAR arc-radius pick — uses full snap chain.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        self.place_dim_angular(vertex, line1_pt, line2_pt, world);
                                    } else if matches!(self.dim_radial_phase, DimRadialPhase::SelectingEntity { .. }) {
                                        // DIMRADIUS / DIMDIAMETER entity pick: click on a circle or arc.
                                        let is_diameter = matches!(self.dim_radial_phase, DimRadialPhase::SelectingEntity { is_diameter: true });
                                        if let Some(eid) = self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                            if let Some(entity) = self.drawing.get_entity(&eid) {
                                                let picked = match &entity.kind {
                                                    EntityKind::Circle { center, radius } => {
                                                        Some((Vec2::new(center.x, center.y), *radius))
                                                    }
                                                    EntityKind::Arc { center, radius, .. } => {
                                                        Some((Vec2::new(center.x, center.y), *radius))
                                                    }
                                                    _ => None,
                                                };
                                                if let Some((center, radius)) = picked {
                                                    self.dim_radial_phase = DimRadialPhase::Placing { center, radius, is_diameter };
                                                    let cmd = if is_diameter { "DIMDIAMETER" } else { "DIMRADIUS" };
                                                    self.command_log.push(format!("{cmd}: Drag leader, click to place"));
                                                } else {
                                                    let cmd = if is_diameter { "DIMDIAMETER" } else { "DIMRADIUS" };
                                                    self.command_log.push(format!("{cmd}: Click on a circle or arc"));
                                                }
                                            }
                                        }
                                    } else if let DimRadialPhase::Placing { center, radius, is_diameter } = self.dim_radial_phase {
                                        // DIMRADIUS / DIMDIAMETER leader placement.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        self.place_dim_radial(center, radius, is_diameter, world);
                                    } else if self.text_phase == TextPhase::PlacingPosition {
                                        // TEXT insertion point pick — same snap logic as DIMALIGNED.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|(s, _)| s.world).unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible { self.snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            } else if self.hover_snap_kind.is_some() {
                                                if let Some(hw) = self.hover_world_pos {
                                                    world = hw;
                                                }
                                            }
                                        }
                                        self.deliver_point(world);
                                    } else if self.edit_text_phase == EditTextPhase::SelectingEntity {
                                        // EDITTEXT: find a text entity near the click.
                                        if let Some(id) = self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                            if self.is_entity_on_locked_layer(&id) {
                                                self.command_log.push(
                                                    "EDITTEXT: Entity is on a locked layer".to_string(),
                                                );
                                            } else if let Some(entity) = self.drawing.get_entity(&id) {
                                                if let EntityKind::Text { content, height, rotation, .. } = &entity.kind {
                                                    self.text_edit_dialog = Some(TextEditDialog {
                                                        id,
                                                        content: content.clone(),
                                                        height_str: format!("{:.4}", height),
                                                        rotation_str: format!("{:.2}", rotation.to_degrees()),
                                                        focus_requested: false,
                                                    });
                                                    self.edit_text_phase = EditTextPhase::Idle;
                                                } else {
                                                    self.command_log.push("EDITTEXT: That is not a text entity".to_string());
                                                }
                                            }
                                        } else {
                                            self.command_log.push("EDITTEXT: Nothing found near click".to_string());
                                        }
                                    } else if self.edit_dim_phase == EditDimPhase::SelectingEntity {
                                        if let Some(id) = self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                            if self.is_entity_on_locked_layer(&id) {
                                                self.command_log.push(
                                                    "EDITDIM: Entity is on a locked layer".to_string(),
                                                );
                                            } else if let Some(entity) = self.drawing.get_entity(&id) {
                                                match &entity.kind {
                                                    EntityKind::DimAligned { text_override, .. }
                                                    | EntityKind::DimLinear { text_override, .. }
                                                    | EntityKind::DimAngular { text_override, .. }
                                                    | EntityKind::DimRadial { text_override, .. } => {
                                                        self.dim_edit_dialog = Some(DimEditDialog {
                                                            id,
                                                            override_str: text_override.clone().unwrap_or_else(|| "<>".to_string()),
                                                            focus_requested: false,
                                                        });
                                                        self.edit_dim_phase = EditDimPhase::Idle;
                                                    }
                                                    _ => {
                                                        self.command_log.push("EDITDIM: That is not a dimension entity".to_string());
                                                    }
                                                }
                                            }
                                        } else {
                                            self.command_log.push("EDITDIM: Nothing found near click".to_string());
                                        }
                                    } else {
                                        match self.offset_phase {
                                            OffsetPhase::SelectingEntity => {
                                                match self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                                    Some(id) => {
                                                        if self.is_entity_on_locked_layer(&id) {
                                                            self.command_log.push(
                                                                "OFFSET: Entity is on a locked layer".to_string(),
                                                            );
                                                        } else {
                                                            self.offset_selected_entity = Some(id);
                                                            self.offset_phase = OffsetPhase::SelectingSide;
                                                            self.command_log.push("OFFSET: Click side to offset toward".to_string());
                                                        }
                                                    }
                                                    None => {
                                                        self.command_log.push("OFFSET: Nothing found near click".to_string());
                                                    }
                                                }
                                            }
                                            OffsetPhase::SelectingSide => {
                                                let rel = click_pos - response.rect.min;
                                                let world_click = screen_to_world(rel.x, rel.y, viewport);
                                                match self.apply_offset(world_click) {
                                                    Ok(entity) => {
                                                        self.push_undo();
                                                        self.drawing.add_entity(entity);
                                                        self.offset_selected_entity = None;
                                                        self.offset_phase = OffsetPhase::SelectingEntity;
                                                        self.command_log.push("OFFSET: Select entity to offset".to_string());
                                                    }
                                                    Err(msg) => {
                                                        self.command_log.push(msg);
                                                    }
                                                }
                                            }
                                            _ => {
                                                // Regular selection (also used in MOVE SelectingEntities).
                                                let shift = ui.input(|i| i.modifiers.shift);
                                                let id = self.entity_at_screen_pos(viewport, response.rect, click_pos);
                                                if self.array_phase == ArrayPhase::SelectingEntities
                                                    && !shift
                                                    && id.is_some()
                                                {
                                                    let picked = id.unwrap();
                                                    if self.try_start_array_edit_from_selection(&[picked]) {
                                                        self.selected_entities.clear();
                                                        self.selection = None;
                                                    } else {
                                                        self.select_entity_id(Some(picked), false);
                                                    }
                                                } else if id.is_some() || !shift {
                                                    self.select_entity_id(id, shift);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if response.drag_started_by(egui::PointerButton::Primary) {
                                // Allow drag selection when idle or in MOVE SelectingEntities.
                                let allow = matches!(self.offset_phase, OffsetPhase::Idle)
                                    && !matches!(self.move_phase, MovePhase::BasePoint | MovePhase::Destination)
                                    && !matches!(self.copy_phase, CopyPhase::BasePoint | CopyPhase::Destination)
                                    && !matches!(self.rotate_phase, RotatePhase::BasePoint | RotatePhase::Rotation)
                                    && !matches!(self.scale_phase, ScalePhase::BasePoint | ScalePhase::ReferencePoint | ScalePhase::Factor)
                                    && !matches!(self.mirror_phase, MirrorPhase::FirstAxisPoint | MirrorPhase::SecondAxisPoint)
                                    && matches!(self.fillet_phase, FilletPhase::Idle)
                                    && matches!(self.chamfer_phase, ChamferPhase::Idle)
                                    && matches!(self.polygon_phase, PolygonPhase::Idle)
                                    && matches!(self.ellipse_phase, EllipsePhase::Idle)
                                    && matches!(self.rectangle_phase, RectanglePhase::Idle)
                                    && matches!(self.array_phase, ArrayPhase::Idle | ArrayPhase::SelectingEntities)
                                    && matches!(self.pedit_phase, PeditPhase::Idle)
                                    && matches!(self.dim_phase, DimPhase::Idle)
                                    && matches!(self.dim_linear_phase, DimLinearPhase::Idle)
                                    && matches!(self.dim_angular_phase, DimAngularPhase::Idle)
                                    && matches!(self.dim_radial_phase, DimRadialPhase::Idle);
                                if allow {
                                    if let (Some(pos), Some(viewport)) =
                                        (response.interact_pointer_pos(), self.viewport.as_ref())
                                    {
                                        if let Some(handle) =
                                            self.pick_dim_grip_handle(viewport, response.rect, pos)
                                        {
                                            if self.is_entity_on_locked_layer(&handle.entity) {
                                                self.command_log.push(
                                                    "DIM: Entity is on a locked layer".to_string(),
                                                );
                                            } else {
                                                self.push_undo();
                                                self.dim_grip_drag = Some(handle);
                                                self.dim_grip_is_dragging = true;
                                            }
                                            self.selection_drag_start = None;
                                            self.selection_drag_current = None;
                                        } else {
                                            self.selection_drag_start = Some(pos);
                                            self.selection_drag_current = Some(pos);
                                        }
                                    }
                                }
                            }
                        }

                        // Right-click cancels the current command or tool.
                        if response.clicked_by(egui::PointerButton::Secondary) {
                            if self.dim_grip_drag.is_some() {
                                self.dim_grip_drag = None;
                                self.dim_grip_is_dragging = false;
                                self.command_log.push("*Cancel*".to_string());
                            } else if matches!(
                                self.array_phase,
                                ArrayPhase::RectXSpacingGrip
                                    | ArrayPhase::RectXCountGrip
                                    | ArrayPhase::RectYSpacingGrip
                                    | ArrayPhase::RectYCountGrip
                            ) {
                                self.array_phase = ArrayPhase::RectGripIdle;
                                self.command_log.push("ARRAY: Grip deactivated".to_string());
                            } else if self.from_phase != FromPhase::Idle {
                                self.exit_from();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.active_tool, ActiveTool::None) {
                                self.cancel_active_tool();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.trim_phase, TrimPhase::Idle) {
                                self.exit_trim();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.offset_phase, OffsetPhase::Idle) {
                                self.exit_offset();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.move_phase, MovePhase::Idle) {
                                self.exit_move();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.copy_phase, CopyPhase::Idle) {
                                self.exit_copy();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.rotate_phase, RotatePhase::Idle) {
                                self.exit_rotate();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.scale_phase, ScalePhase::Idle) {
                                self.exit_scale();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.mirror_phase, MirrorPhase::Idle) {
                                self.exit_mirror();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.fillet_phase, FilletPhase::Idle) {
                                self.exit_fillet();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.chamfer_phase, ChamferPhase::Idle) {
                                self.exit_chamfer();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.polygon_phase, PolygonPhase::Idle) {
                                self.exit_polygon();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.ellipse_phase, EllipsePhase::Idle) {
                                self.exit_ellipse();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.rectangle_phase, RectanglePhase::Idle) {
                                self.exit_rectangle();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.array_phase, ArrayPhase::Idle) {
                                self.exit_array();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.pedit_phase, PeditPhase::Idle) {
                                self.exit_pedit();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.extend_phase, ExtendPhase::Idle) {
                                self.exit_extend();
                                self.command_log.push("*Cancel*".to_string());
                            } else if self.has_active_dimension_command() {
                                self.exit_dim();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.text_phase, TextPhase::Idle) {
                                self.exit_text();
                                self.command_log.push("*Cancel*".to_string());
                            } else if self.text_edit_dialog.is_some() || !matches!(self.edit_text_phase, EditTextPhase::Idle) {
                                self.exit_edit_text();
                                self.command_log.push("*Cancel*".to_string());
                            } else if self.dim_edit_dialog.is_some() || !matches!(self.edit_dim_phase, EditDimPhase::Idle) {
                                self.exit_edit_dim();
                                self.command_log.push("*Cancel*".to_string());
                            }
                        }

                        if response.dragged_by(egui::PointerButton::Primary) {
                            if let Some(handle) = self.dim_grip_drag {
                                if self.dim_grip_is_dragging {
                                    if let (Some(pos), Some(viewport)) =
                                        (response.interact_pointer_pos(), self.viewport.as_ref())
                                    {
                                        let (world, snap_kind) =
                                            self.snapped_world_for_grip_drag(handle, viewport, response.rect, pos);
                                        let world = self.constrained_dim_grip_world(handle, world);
                                        self.hover_world_pos = Some(world);
                                        self.hover_snap_kind = snap_kind;
                                        self.snap_intersection_point = if snap_kind == Some(SnapKind::Intersection) {
                                            Some(world)
                                        } else {
                                            None
                                        };
                                        self.apply_dim_grip_drag(handle, world);
                                    }
                                }
                            } else if let Some(pos) = response.interact_pointer_pos() {
                                self.selection_drag_current = Some(pos);
                            }
                        }

                        if response.drag_stopped_by(egui::PointerButton::Primary) {
                            if self.dim_grip_drag.is_some() && self.dim_grip_is_dragging {
                                self.dim_grip_drag = None;
                                self.dim_grip_is_dragging = false;
                                self.selection_drag_start = None;
                                self.selection_drag_current = None;
                            } else if let (Some(start), Some(end)) = (
                                self.selection_drag_start.take(),
                                self.selection_drag_current.take(),
                            ) {
                                let drag_len = start.distance(end);
                                let shift = ui.input(|i| i.modifiers.shift);
                                let selection_data = if let Some(viewport) = self.viewport.as_ref() {
                                    if drag_len > 4.0 {
                                        let s0 = start - response.rect.min;
                                        let s1 = end - response.rect.min;
                                        let w0 = screen_to_world(s0.x, s0.y, viewport);
                                        let w1 = screen_to_world(s1.x, s1.y, viewport);
                                        let min_x = w0.x.min(w1.x);
                                        let min_y = w0.y.min(w1.y);
                                        let max_x = w0.x.max(w1.x);
                                        let max_y = w0.y.max(w1.y);
                                        let window_mode = end.x >= start.x;
                                        Some((self.entities_in_world_box(min_x, min_y, max_x, max_y, window_mode), None::<Selection>))
                                    } else {
                                        Some((Vec::new(), None))
                                    }
                                } else {
                                    None
                                };

                                if let Some((hits, single_pick)) = selection_data {
                                    if let Some(pick) = single_pick {
                                        self.selection = None;
                                        self.select_entity_id(Some(pick.entity), shift);
                                    } else {
                                        if drag_len > 4.0 {
                                            self.selection = None;
                                            if !shift {
                                                self.selected_entities.clear();
                                            }
                                            for id in hits {
                                                if shift && self.selected_entities.contains(&id) {
                                                    self.selected_entities.remove(&id);
                                                } else {
                                                    self.selected_entities.insert(id);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        self.selection_drag_start = None;
                        self.selection_drag_current = None;
                    }

                    if response.hovered() {
                        if let (Some(pointer_pos), Some(viewport)) =
                            (ui.input(|i| i.pointer.hover_pos()), self.viewport.as_ref())
                        {
                            if let Some(handle) =
                                self.pick_dim_grip_handle(viewport, response.rect, pointer_pos)
                            {
                                self.hover_dim_grip = Some(handle);
                                ui.output_mut(|o| {
                                    o.cursor_icon = if self.dim_grip_is_dragging {
                                        egui::CursorIcon::Grabbing
                                    } else {
                                        egui::CursorIcon::PointingHand
                                    };
                                });
                            }

                            // Clear stale snap state each hover frame.
                            self.snap_intersection_point = None;
                            self.hover_snap_kind = None;

                            let local = pointer_pos - response.rect.min;
                            let raw_world = screen_to_world(local.x, local.y, viewport);
                            let (hover_pick, mut world) = if let Some(handle) = self.dim_grip_drag {
                                let (w, kind) =
                                    self.snapped_world_for_grip_drag(handle, viewport, response.rect, pointer_pos);
                                let w = self.constrained_dim_grip_world(handle, w);
                                self.hover_snap_kind = kind;
                                if kind == Some(SnapKind::Intersection) {
                                    self.snap_intersection_point = Some(w);
                                }
                                (None, w)
                            } else {
                                let hover_pick = if self.snap_enabled {
                                    self.pick_entity_point(viewport, response.rect, pointer_pos)
                                } else {
                                    None
                                };
                                let world = hover_pick
                                    .as_ref()
                                    .map(|(s, _)| s.world)
                                    .unwrap_or_else(|| {
                                        if self.snap_enabled && self.grid_visible {
                                            self.snap_to_grid(raw_world)
                                        } else {
                                            raw_world
                                        }
                                    });
                                (hover_pick, world)
                            };

                            // Apply tool-specific snapping when no point was explicitly picked.
                            // Skip during FROM mode so the tool's distance/ortho don't corrupt the hover.
                            if hover_pick.is_none()
                                && self.dim_grip_drag.is_none()
                                && matches!(self.from_phase, FromPhase::Idle)
                            {
                                match &self.active_tool {
                                    ActiveTool::Line { start: Some(s) } => {
                                        if self.ortho_enabled {
                                            world =
                                                Self::snap_angle(*s, world, self.ortho_increment_deg);
                                        }
                                        if let Some(dist_world) = Self::apply_distance_override(
                                            *s,
                                            world,
                                            &self.distance_input,
                                        ) {
                                            world = dist_world;
                                        }
                                    }
                                    ActiveTool::Circle { center: Some(c) } => {
                                        if let Some(dist_world) = Self::apply_circle_distance_override(
                                            *c,
                                            world,
                                            &self.distance_input,
                                            self.circle_use_diameter,
                                        ) {
                                            world = dist_world;
                                        }
                                    }
                                    ActiveTool::Polyline { points } => {
                                        if let Some(last) = points.last() {
                                            if self.ortho_enabled {
                                                world = Self::snap_angle(
                                                    *last,
                                                    world,
                                                    self.ortho_increment_deg,
                                                );
                                            }
                                            if let Some(dist_world) = Self::apply_distance_override(
                                                *last,
                                                world,
                                                &self.distance_input,
                                            ) {
                                                world = dist_world;
                                            }
                                        }
                                    }
                                    ActiveTool::Arc { start: Some(s), mid: Some(m) } => {
                                        if let Some(arc_entity) =
                                            create_arc_from_three_points(*s, *m, world)
                                        {
                                            if let EntityKind::Arc {
                                                center,
                                                radius,
                                                start_angle,
                                                end_angle,
                                            } = arc_entity.kind
                                            {
                                                let center2: Vec2 = center.into();
                                                let sweep = end_angle - start_angle;
                                                let steps =
                                                    ((sweep.abs() * radius).max(12.0) as usize)
                                                        .clamp(12, 128);
                                                let painter = ui.painter_at(response.rect);
                                                let mut last_screen: Option<egui::Pos2> = None;
                                                for i in 0..=steps {
                                                    let t = i as f64 / steps as f64;
                                                    let ang = start_angle + sweep * t;
                                                    let px =
                                                        center2.x + radius * ang.cos();
                                                    let py =
                                                        center2.y + radius * ang.sin();
                                                    let (sx, sy) = world_to_screen(
                                                        px as f32,
                                                        py as f32,
                                                        viewport,
                                                    );
                                                    let pos = response.rect.min
                                                        + egui::vec2(sx, sy);
                                                    if let Some(prev) = last_screen {
                                                        painter.line_segment(
                                                            [prev, pos],
                                                            egui::Stroke::new(
                                                                2.0,
                                                                egui::Color32::from_rgb(
                                                                    230, 230, 230,
                                                                ),
                                                            ),
                                                        );
                                                    }
                                                    last_screen = Some(pos);
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }

                            // Intersection snap (priority 2: below entity point, above perp/tangent/nearest).
                            if hover_pick.is_none() && self.dim_grip_drag.is_none() && self.snap_enabled {
                                if let Some(snap_pt) = self.find_intersection_snap(viewport, response.rect, pointer_pos) {
                                    world = snap_pt;
                                    self.snap_intersection_point = Some(snap_pt);
                                    self.hover_snap_kind = Some(SnapKind::Intersection);
                                }
                            }

                            // Perpendicular snap (priority 3: foot on entity from last placed point).
                            if hover_pick.is_none()
                                && self.dim_grip_drag.is_none()
                                && self.snap_intersection_point.is_none()
                                && self.snap_enabled
                            {
                                if let Some(from_pt) = self.current_from_point() {
                                    if let Some(pt) = self.perpendicular_snap(viewport, response.rect, pointer_pos, from_pt) {
                                        world = pt;
                                        self.hover_snap_kind = Some(SnapKind::Perpendicular);
                                    }
                                }
                            }

                            // Tangent snap (priority 4: tangent point on circle/arc from last placed point).
                            if hover_pick.is_none()
                                && self.dim_grip_drag.is_none()
                                && self.snap_intersection_point.is_none()
                                && self.hover_snap_kind.is_none()
                                && self.snap_enabled
                            {
                                if let Some(from_pt) = self.current_from_point() {
                                    if let Some(pt) = self.tangent_snap(viewport, response.rect, pointer_pos, from_pt) {
                                        world = pt;
                                        self.hover_snap_kind = Some(SnapKind::Tangent);
                                    }
                                }
                            }

                            // Nearest snap (priority 5: closest point on any entity curve).
                            if hover_pick.is_none()
                                && self.dim_grip_drag.is_none()
                                && self.snap_intersection_point.is_none()
                                && self.hover_snap_kind.is_none()
                                && self.snap_enabled
                            {
                                if let Some(pt) = self.nearest_entity_snap(viewport, response.rect, pointer_pos) {
                                    world = pt;
                                    self.hover_snap_kind = Some(SnapKind::Nearest);
                                }
                            }

                            self.hover_world_pos = Some(world);
                            self.last_hover_world_pos = Some(world);

                            // FROM rubber-band: magenta marker at base, dashed line to hover.
                            if let Some(base) = self.from_base {
                                let painter = ui.painter_at(response.rect);
                                let (bsx, bsy) = world_to_screen(base.x as f32, base.y as f32, viewport);
                                let base_screen = response.rect.min + egui::vec2(bsx, bsy);
                                // In WaitingOffset: ortho-constrain the rubber-band tip so it
                                // visually matches what a plain-distance entry would produce.
                                let tip_world = if self.from_phase == FromPhase::WaitingOffset
                                    && self.ortho_enabled
                                    && hover_pick.is_none()
                                {
                                    Self::snap_angle(base, world, self.ortho_increment_deg)
                                } else {
                                    world
                                };
                                let (hsx, hsy) = world_to_screen(tip_world.x as f32, tip_world.y as f32, viewport);
                                let hover_screen = response.rect.min + egui::vec2(hsx, hsy);
                                // Base point: magenta X marker.
                                let mag = egui::Color32::from_rgb(220, 80, 220);
                                let r = 6.0_f32;
                                painter.line_segment([base_screen - egui::vec2(r, r), base_screen + egui::vec2(r, r)], egui::Stroke::new(2.0, mag));
                                painter.line_segment([base_screen - egui::vec2(r, -r), base_screen + egui::vec2(r, -r)], egui::Stroke::new(2.0, mag));
                                // Dashed rubber-band from base to (ortho-constrained) tip.
                                let delta = hover_screen - base_screen;
                                let len = (delta.x * delta.x + delta.y * delta.y).sqrt();
                                if len > 2.0 {
                                    let dash = 8.0_f32;
                                    let gap = 5.0_f32;
                                    let dir = delta / len;
                                    let mut t = 0.0_f32;
                                    while t < len {
                                        let t_end = (t + dash).min(len);
                                        painter.line_segment(
                                            [
                                                base_screen + dir * t,
                                                base_screen + dir * t_end,
                                            ],
                                            egui::Stroke::new(1.5, mag),
                                        );
                                        t += dash + gap;
                                    }
                                }
                            }

                            // MOVE / COPY / ROTATE / DIMALIGNED / DIMLINEAR / TEXT ghost preview.
                            self.draw_move_preview(ui, response.rect, viewport, world);
                            self.draw_copy_preview(ui, response.rect, viewport, world);
                            self.draw_rotate_preview(ui, response.rect, viewport, world);
                            self.draw_scale_preview(ui, response.rect, viewport, world);
                            self.draw_mirror_preview(ui, response.rect, viewport, world);
                            self.draw_dim_preview(ui, response.rect, viewport, world);
                            self.draw_dim_linear_preview(ui, response.rect, viewport, world);
                            self.draw_dim_angular_preview(ui, response.rect, viewport, world);
                            self.draw_dim_radial_preview(ui, response.rect, viewport, world);
                            self.draw_text_preview(ui, response.rect, viewport, world);
                            self.draw_polygon_preview(ui, response.rect, viewport, world);
                            self.draw_ellipse_preview(ui, response.rect, viewport, world);
                            self.draw_rectangle_preview(ui, response.rect, viewport, world);
                            self.draw_array_preview(ui, response.rect, viewport, world);

                            // Grid-snap dot (suppress when any entity/intersection/nearest/perp/tangent snap active).
                            if self.snap_enabled
                                && self.grid_visible
                                && self.dim_grip_drag.is_none()
                                && hover_pick.is_none()
                                && self.snap_intersection_point.is_none()
                                && self.hover_snap_kind.is_none()
                            {
                                let (sx, sy) =
                                    world_to_screen(world.x as f32, world.y as f32, viewport);
                                let marker = response.rect.min + egui::vec2(sx, sy);
                                ui.painter().circle_filled(
                                    marker,
                                    4.0,
                                    egui::Color32::from_rgb(0, 220, 120),
                                );
                            }

                            // Rubber-band preview for line tool once a start point is chosen.
                            if let ActiveTool::Line { start: Some(s) } = &self.active_tool {
                                let (sx1, sy1) =
                                    world_to_screen(s.x as f32, s.y as f32, viewport);
                                let (sx2, sy2) =
                                    world_to_screen(world.x as f32, world.y as f32, viewport);
                                let p1 = response.rect.min + egui::vec2(sx1, sy1);
                                let p2 = response.rect.min + egui::vec2(sx2, sy2);
                                ui.painter_at(response.rect).line_segment(
                                    [p1, p2],
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 230, 230)),
                                );
                            }

                            // Rubber-band preview for circle tool once a center is chosen.
                            if let ActiveTool::Circle { center: Some(c) } = &self.active_tool {
                                let radius = c.distance_to(&world);
                                if radius > f64::EPSILON {
                                    let (cx, cy) =
                                        world_to_screen(c.x as f32, c.y as f32, viewport);
                                    let (rx, ry) = world_to_screen(
                                        (c.x + radius) as f32,
                                        c.y as f32,
                                        viewport,
                                    );
                                    let screen_r =
                                        ((rx - cx).powi(2) + (ry - cy).powi(2)).sqrt();
                                    let center_pos = response.rect.min + egui::vec2(cx, cy);
                                    ui.painter_at(response.rect).circle_stroke(
                                        center_pos,
                                        screen_r,
                                        egui::Stroke::new(
                                            2.0,
                                            egui::Color32::from_rgb(230, 230, 230),
                                        ),
                                    );
                                }
                            }
                            // Polyline preview: existing segments and a tail to hover.
                            if let ActiveTool::Polyline { points } = &self.active_tool {
                                if !points.is_empty() {
                                    let painter = ui.painter_at(response.rect);
                                    let mut last = points[0];
                                    for p in points.iter().skip(1) {
                                        let (sx1, sy1) =
                                            world_to_screen(last.x as f32, last.y as f32, viewport);
                                        let (sx2, sy2) =
                                            world_to_screen(p.x as f32, p.y as f32, viewport);
                                        painter.line_segment(
                                            [
                                                response.rect.min + egui::vec2(sx1, sy1),
                                                response.rect.min + egui::vec2(sx2, sy2),
                                            ],
                                            egui::Stroke::new(
                                                2.0,
                                                egui::Color32::from_rgb(230, 230, 230),
                                            ),
                                        );
                                        last = *p;
                                    }
                                    let (sx1, sy1) =
                                        world_to_screen(last.x as f32, last.y as f32, viewport);
                                    let (sx2, sy2) =
                                        world_to_screen(world.x as f32, world.y as f32, viewport);
                                    painter.line_segment(
                                        [
                                            response.rect.min + egui::vec2(sx1, sy1),
                                            response.rect.min + egui::vec2(sx2, sy2),
                                        ],
                                        egui::Stroke::new(
                                            2.0,
                                            egui::Color32::from_rgb(200, 200, 200),
                                        ),
                                    );
                                }
                            }

                            ctx.request_repaint();
                        }

                        // Hover highlight for selectable line points (both idle and while drawing).
                        if let (Some(pointer_pos), Some(viewport)) =
                            (ui.input(|i| i.pointer.hover_pos()), self.viewport.as_ref())
                        {
                            if self.snap_enabled || matches!(self.active_tool, ActiveTool::None) {
                                if self.dim_grip_drag.is_none() {
                                    if let Some((candidate, kind)) =
                                        self.pick_entity_point(viewport, response.rect, pointer_pos)
                                    {
                                        Self::draw_snap_glyph(
                                            ui,
                                            response.rect,
                                            viewport,
                                            candidate.world,
                                            kind,
                                        );
                                    } else if let Some(snap_kind) = self.hover_snap_kind {
                                        if let Some(world) = self.hover_world_pos {
                                            Self::draw_snap_glyph(ui, response.rect, viewport, world, snap_kind);
                                        }
                                    }
                                } else if let Some(snap_kind) = self.hover_snap_kind {
                                    if let Some(world) = self.hover_world_pos {
                                        Self::draw_snap_glyph(ui, response.rect, viewport, world, snap_kind);
                                    }
                                }
                            }
                        }

        // Handle left-clicks for active drawing tools.
                        if response.clicked_by(egui::PointerButton::Primary)
                            && !matches!(self.active_tool, ActiveTool::None)
                        {
                            if let (Some(click_pos), Some(viewport)) =
                                (response.interact_pointer_pos(), self.viewport.as_ref())
                            {
                                    let local = click_pos - response.rect.min;
                                    let raw_world =
                                        screen_to_world(local.x, local.y, viewport);
                                let pick = if self.snap_enabled {
                                    self.pick_entity_point(viewport, response.rect, click_pos)
                                } else {
                                    None
                                };
                                    let mut world = pick
                                        .as_ref()
                                        .map(|(s, _)| s.world)
                                        .unwrap_or_else(|| {
                                            if self.snap_enabled && self.grid_visible {
                                                self.snap_to_grid(raw_world)
                                            } else {
                                                raw_world
                                            }
                                        });

                                    // Apply tool snapping if no pick override.
                                    // Skip during FROM mode: the FROM base/offset owns the click position.
                                    if pick.is_none() && matches!(self.from_phase, FromPhase::Idle) {
                                        match &self.active_tool {
                                            ActiveTool::Line { start: Some(s) } => {
                                                if self.ortho_enabled {
                                                    world = Self::snap_angle(
                                                        *s,
                                                        world,
                                                        self.ortho_increment_deg,
                                                    );
                                                }
                                                if let Some(dist_world) = Self::apply_distance_override(
                                                    *s,
                                                    world,
                                                    &self.distance_input,
                                                ) {
                                                    world = dist_world;
                                                }
                                            }
                                            ActiveTool::Circle { center: Some(c) } => {
                                                if let Some(dist_world) = Self::apply_circle_distance_override(
                                                    *c,
                                                    world,
                                                    &self.distance_input,
                                                    self.circle_use_diameter,
                                                ) {
                                                    world = dist_world;
                                                }
                                            }
                                            ActiveTool::Polyline { points } => {
                                                if let Some(last) = points.last() {
                                                    if self.ortho_enabled {
                                                        world = Self::snap_angle(
                                                            *last,
                                                            world,
                                                            self.ortho_increment_deg,
                                                        );
                                                    }
                                                    if let Some(dist_world) = Self::apply_distance_override(
                                                        *last,
                                                        world,
                                                        &self.distance_input,
                                                    ) {
                                                        world = dist_world;
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                // Apply hover snaps (intersection/perp/tangent/nearest) if no point-snap pick.
                                if pick.is_none() {
                                    if let Some(snap_pt) = self.snap_intersection_point {
                                        world = snap_pt;
                                    } else if self.hover_snap_kind.is_some() {
                                        if let Some(hw) = self.hover_world_pos {
                                            world = hw;
                                        }
                                    }
                                }

                                // Update snap marker when a point pick happens during drawing.
                                if let Some((p, _)) = pick {
                                    self.selection = Some(p);
                                }

                                // FROM: capture base point, then wait for typed offset.
                                if self.from_phase == FromPhase::WaitingBase {
                                    self.from_base = Some(world);
                                    self.from_phase = FromPhase::WaitingOffset;
                                    self.command_log.push(format!("  Base: {:.4}, {:.4}", world.x, world.y));
                                    self.command_log.push("FROM  Offset (@dx,dy  or  @dist<angle):".to_string());
                                } else if self.from_phase == FromPhase::WaitingOffset {
                                    // Click = use snapped cursor as the result directly.
                                    let result = world;
                                    self.apply_from_result_point(result);
                                } else {
                                match &mut self.active_tool {
                                    ActiveTool::Line { start } => {
                                        if start.is_none() {
                                            *start = Some(world);
                                            self.distance_input.clear();
                                            log::info!(
                                                "Line start set at ({:.3}, {:.3})",
                                                world.x,
                                                world.y
                                            );
                                        } else if let Some(s) = start.take() {
                                            let mut line = create_line(s, world);
                                            line.layer = self.current_layer;
                                            self.drawing.add_entity(line);
                                            log::info!(
                                                "Line created from ({:.3}, {:.3}) to ({:.3}, {:.3})",
                                                s.x,
                                                s.y,
                                                world.x,
                                                world.y
                                            );
                                            // Keep tool active and chain from the last endpoint.
                                            *start = Some(world);
                                            self.distance_input.clear();
                                        }
                                    }
                                    ActiveTool::Circle { center } => {
                                        if center.is_none() {
                                            *center = Some(world);
                                            log::info!(
                                                "Circle center set at ({:.3}, {:.3})",
                                                world.x,
                                                world.y
                                            );
                                        } else if let Some(c) = center.take() {
                                            let radius = c.distance_to(&world);
                                            if radius > f64::EPSILON {
                                                let mut circle = create_circle(c, radius);
                                                circle.layer = self.current_layer;
                                                self.drawing.add_entity(circle);
                                                log::info!(
                                                    "Circle created center ({:.3}, {:.3}) r={:.3}",
                                                    c.x,
                                                    c.y,
                                                    radius
                                                );
                                            }
                                        }
                                    }
                                    ActiveTool::Arc { start, mid } => {
                                        if start.is_none() {
                                            *start = Some(world);
                                            log::info!(
                                                "Arc start set at ({:.3}, {:.3})",
                                                world.x,
                                                world.y
                                            );
                                        } else if mid.is_none() {
                                            *mid = Some(world);
                                            log::info!(
                                                "Arc mid set at ({:.3}, {:.3})",
                                                world.x,
                                                world.y
                                            );
                                        } else if let (Some(s), Some(m)) = (start.take(), mid.take()) {
                                            let end = world;
                                            let arc = create_arc_from_three_points(s, m, end);
                                            if let Some(mut a) = arc {
                                                a.layer = self.current_layer;
                                                self.drawing.add_entity(a);
                                                log::info!(
                                                    "Arc created through start ({:.3}, {:.3}), mid ({:.3}, {:.3}), end ({:.3}, {:.3})",
                                                    s.x,
                                                    s.y,
                                                    m.x,
                                                    m.y,
                                                    end.x,
                                                    end.y
                                                );
                                            } else {
                                                log::warn!("Arc creation failed (collinear or invalid).");
                                            }
                                        }
                                    }
                                    ActiveTool::Polyline { points } => {
                                        points.push(world);
                                        self.distance_input.clear();
                                        log::info!(
                                            "Polyline point {} set at ({:.3}, {:.3})",
                                            points.len(),
                                            world.x,
                                            world.y
                                        );
                                    }
                                    ActiveTool::None => {}
                                }
                                // Clear typed input after every click placement
                                self.command_input.clear();
                                } // closes `else` (not FROM mode)
                            }
                        }

                        // Right-click: exit trim, finish polyline, or cancel tool.
                        if response.clicked_by(egui::PointerButton::Secondary) {
                            if !matches!(self.trim_phase, TrimPhase::Idle) {
                                self.exit_trim();
                            } else if matches!(
                                self.array_phase,
                                ArrayPhase::RectXSpacingGrip
                                    | ArrayPhase::RectXCountGrip
                                    | ArrayPhase::RectYSpacingGrip
                                    | ArrayPhase::RectYCountGrip
                            ) {
                                self.array_phase = ArrayPhase::RectGripIdle;
                                self.command_log.push("ARRAY: Grip deactivated".to_string());
                            } else if !matches!(self.scale_phase, ScalePhase::Idle) {
                                self.exit_scale();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.mirror_phase, MirrorPhase::Idle) {
                                self.exit_mirror();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.fillet_phase, FilletPhase::Idle) {
                                self.exit_fillet();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.chamfer_phase, ChamferPhase::Idle) {
                                self.exit_chamfer();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.polygon_phase, PolygonPhase::Idle) {
                                self.exit_polygon();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.ellipse_phase, EllipsePhase::Idle) {
                                self.exit_ellipse();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.rectangle_phase, RectanglePhase::Idle) {
                                self.exit_rectangle();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.array_phase, ArrayPhase::Idle) {
                                self.exit_array();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.pedit_phase, PeditPhase::Idle) {
                                self.exit_pedit();
                                self.command_log.push("*Cancel*".to_string());
                            } else if self.has_active_dimension_command() {
                                self.exit_dim();
                                self.command_log.push("*Cancel*".to_string());
                            } else {
                                match &self.active_tool {
                                    ActiveTool::Polyline { points } if points.len() >= 2 => {
                                        self.finalize_polyline(false);
                                    }
                                    _ => self.cancel_active_tool(),
                                }
                            }
                        }

                        let scroll_y = ui.input(|i| i.raw_scroll_delta.y);
                        if scroll_y.abs() > f32::EPSILON {
                            // Keep zoom step stable across platforms with different wheel scales.
                            let zoom_delta = (scroll_y * 0.001).clamp(-0.25, 0.25);
                            if let Some(viewport) = &mut self.viewport {
                                viewport.zoom_delta(zoom_delta);
                            }
                        }

                        let pan_delta = if response.dragged_by(egui::PointerButton::Middle) {
                            response.drag_delta()
                        } else {
                            egui::Vec2::ZERO
                        };
                        if pan_delta.length_sq() > 0.0 {
                            if let Some(viewport) = &mut self.viewport {
                                // Screen-space drag mapped to CAD-like "grab and move".
                                viewport.pan(
                                    -pan_delta.x * Self::PAN_SENSITIVITY,
                                    pan_delta.y * Self::PAN_SENSITIVITY,
                                );
                            }
                        }
                    }
                } else {
                    ui.label("Viewport texture registration failed.");
                }
            } else if let Some(err) = &self.viewport_init_error {
                ui.label(format!("Viewport initialization failed: {}", err));
            } else {
                ui.label("Initializing viewport...");
            }

        });

        // Persist app preferences (snap/ortho/grid/current file) when changed.
        self.persist_preferences_if_changed();
    }
}

impl CadKitApp {
    fn entity_bounds_world(kind: &EntityKind) -> Option<(f64, f64, f64, f64)> {
        match kind {
            EntityKind::Line { start, end } => {
                Some((start.x.min(end.x), start.y.min(end.y), start.x.max(end.x), start.y.max(end.y)))
            }
            EntityKind::Circle { center, radius } => Some((
                center.x - *radius,
                center.y - *radius,
                center.x + *radius,
                center.y + *radius,
            )),
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                let sweep = *end_angle - *start_angle;
                let steps = ((sweep.abs() * *radius).max(12.0) as usize).clamp(12, 128);
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let ang = *start_angle + sweep * t;
                    let x = center.x + *radius * ang.cos();
                    let y = center.y + *radius * ang.sin();
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                }
                Some((min_x, min_y, max_x, max_y))
            }
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
            EntityKind::DimAligned { start, end, offset, .. } => {
                let ddx = end.x - start.x;
                let ddy = end.y - start.y;
                let glen = (ddx*ddx + ddy*ddy).sqrt();
                if glen < 1e-9 { return None; }
                let perp = (-ddy/glen, ddx/glen);
                let dl1x = start.x + perp.0 * offset;
                let dl1y = start.y + perp.1 * offset;
                let dl2x = end.x   + perp.0 * offset;
                let dl2y = end.y   + perp.1 * offset;
                let min_x = start.x.min(end.x).min(dl1x).min(dl2x);
                let min_y = start.y.min(end.y).min(dl1y).min(dl2y);
                let max_x = start.x.max(end.x).max(dl1x).max(dl2x);
                let max_y = start.y.max(end.y).max(dl1y).max(dl2y);
                Some((min_x, min_y, max_x, max_y))
            }
            EntityKind::DimLinear { start, end, offset, horizontal, .. } => {
                let mid_x = (start.x + end.x) * 0.5;
                let mid_y = (start.y + end.y) * 0.5;
                let (dl1x, dl1y, dl2x, dl2y) = if *horizontal {
                    let x1 = start.x.min(end.x); let x2 = start.x.max(end.x);
                    (x1, mid_y + offset, x2, mid_y + offset)
                } else {
                    let y1 = start.y.min(end.y); let y2 = start.y.max(end.y);
                    (mid_x + offset, y1, mid_x + offset, y2)
                };
                let min_x = start.x.min(end.x).min(dl1x).min(dl2x);
                let min_y = start.y.min(end.y).min(dl1y).min(dl2y);
                let max_x = start.x.max(end.x).max(dl1x).max(dl2x);
                let max_y = start.y.max(end.y).max(dl1y).max(dl2y);
                Some((min_x, min_y, max_x, max_y))
            }
            EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, .. } => {
                let v: Vec2 = (*vertex).into();
                let p1: Vec2 = (*line1_pt).into();
                let p2: Vec2 = (*line2_pt).into();
                let (a1, a2) = angular_arc_angles(v, p1, p2);
                let pts = angular_arc_pts(v, a1, a2, *radius);
                let mut min_x = p1.x.min(p2.x);
                let mut min_y = p1.y.min(p2.y);
                let mut max_x = p1.x.max(p2.x);
                let mut max_y = p1.y.max(p2.y);
                for p in &pts {
                    min_x = min_x.min(p.x); min_y = min_y.min(p.y);
                    max_x = max_x.max(p.x); max_y = max_y.max(p.y);
                }
                Some((min_x, min_y, max_x, max_y))
            }
            EntityKind::DimRadial { center, radius, leader_pt, .. } => {
                let min_x = (center.x - *radius).min(leader_pt.x);
                let min_y = (center.y - *radius).min(leader_pt.y);
                let max_x = (center.x + *radius).max(leader_pt.x);
                let max_y = (center.y + *radius).max(leader_pt.y);
                Some((min_x, min_y, max_x, max_y))
            }
            EntityKind::Text { position, .. } => {
                Some((position.x, position.y, position.x, position.y))
            }
        }
    }

    fn entities_in_world_box(
        &self,
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
        window_mode: bool,
    ) -> Vec<Guid> {
        let mut hits = Vec::new();
        for e in self.drawing.visible_entities() {
            let Some((ex0, ey0, ex1, ey1)) = Self::entity_bounds_world(&e.kind) else {
                continue;
            };

            let hit = if window_mode {
                ex0 >= min_x && ey0 >= min_y && ex1 <= max_x && ey1 <= max_y
            } else {
                !(ex1 < min_x || ex0 > max_x || ey1 < min_y || ey0 > max_y)
            };

            if hit {
                hits.push(e.id);
            }
        }
        hits
    }

    fn snap_kind_from_label(label: &str) -> SnapKind {
        if label.contains("mid")    { SnapKind::Midpoint }
        else if label.contains("center") { SnapKind::Center }
        else if label.contains("east") || label.contains("west")
             || label.contains("north") || label.contains("south") { SnapKind::Quadrant }
        else { SnapKind::Endpoint }
    }

    /// Pick nearest entity point (endpoints, midpoints, centers, quadrants) in screen space.
    fn pick_entity_point(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<(Selection, SnapKind)> {
        self.pick_entity_point_excluding(viewport, rect, screen_pos, None)
    }

    /// Pick nearest object snap point, optionally excluding one entity id.
    fn pick_entity_point_excluding(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
        exclude_entity: Option<Guid>,
    ) -> Option<(Selection, SnapKind)> {
        let mut best: Option<(f32, Selection, SnapKind)> = None;

        for entity in self.drawing.visible_entities() {
            if Some(entity.id) == exclude_entity {
                continue;
            }
            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let s: Vec2 = (*start).into();
                    let e: Vec2 = (*end).into();
                    let mid = Vec2::new((s.x + e.x) * 0.5, (s.y + e.y) * 0.5);
                    self.push_pick_candidates(
                        &mut best,
                        viewport,
                        rect,
                        screen_pos,
                        entity.id,
                        &[("line start", s), ("line end", e), ("line mid", mid)],
                    );
                }
                EntityKind::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    let c: Vec2 = (*center).into();
                    let r = *radius;
                    let sa = *start_angle;
                    let ea = *end_angle;
                    let mid_ang = sa + (ea - sa) * 0.5;
                    let pts = [
                        ("arc start", Vec2::new(c.x + r * sa.cos(), c.y + r * sa.sin())),
                        ("arc mid", Vec2::new(c.x + r * mid_ang.cos(), c.y + r * mid_ang.sin())),
                        ("arc end", Vec2::new(c.x + r * ea.cos(), c.y + r * ea.sin())),
                    ];
                    self.push_pick_candidates(&mut best, viewport, rect, screen_pos, entity.id, &pts);
                }
                EntityKind::Circle { center, radius } => {
                    let c: Vec2 = (*center).into();
                    let r = *radius;
                    let pts = [
                        ("circle center", c),
                        ("circle east", Vec2::new(c.x + r, c.y)),
                        ("circle west", Vec2::new(c.x - r, c.y)),
                        ("circle north", Vec2::new(c.x, c.y + r)),
                        ("circle south", Vec2::new(c.x, c.y - r)),
                    ];
                    self.push_pick_candidates(&mut best, viewport, rect, screen_pos, entity.id, &pts);
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.is_empty() {
                        continue;
                    }
                    // Vertices (waypoints)
                    for (i, v) in vertices.iter().enumerate() {
                        let p: Vec2 = (*v).into();
                        let label = if i == 0 {
                            "poly start"
                        } else if i + 1 == vertices.len() && !*closed {
                            "poly end"
                        } else {
                            "poly vertex"
                        };
                        self.push_pick_candidates(
                            &mut best,
                            viewport,
                            rect,
                            screen_pos,
                            entity.id,
                            &[(label, p)],
                        );
                    }

                    // Midpoints of segments
                    let mut add_seg = |a: Vec2, b: Vec2| {
                        let mid = Vec2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
                        self.push_pick_candidates(
                            &mut best,
                            viewport,
                            rect,
                            screen_pos,
                            entity.id,
                            &[("poly mid", mid)],
                        );
                    };
                    for seg in vertices.windows(2) {
                        let a: Vec2 = seg[0].into();
                        let b: Vec2 = seg[1].into();
                        add_seg(a, b);
                    }
                    if *closed && vertices.len() >= 2 {
                        let a: Vec2 = vertices.last().unwrap().to_owned().into();
                        let b: Vec2 = vertices.first().unwrap().to_owned().into();
                        add_seg(a, b);
                    }
                }
                EntityKind::DimAligned { start, end, .. }
                | EntityKind::DimLinear { start, end, .. } => {
                    let s: Vec2 = (*start).into();
                    let e: Vec2 = (*end).into();
                    let mid = Vec2::new((s.x + e.x) * 0.5, (s.y + e.y) * 0.5);
                    self.push_pick_candidates(
                        &mut best, viewport, rect, screen_pos, entity.id,
                        &[("dim start", s), ("dim end", e), ("dim mid", mid)],
                    );
                }
                EntityKind::DimAngular { vertex, line1_pt, line2_pt, .. } => {
                    let v: Vec2 = (*vertex).into();
                    let p1: Vec2 = (*line1_pt).into();
                    let p2: Vec2 = (*line2_pt).into();
                    self.push_pick_candidates(
                        &mut best, viewport, rect, screen_pos, entity.id,
                        &[("center", v), ("dim start", p1), ("dim end", p2)],
                    );
                }
                EntityKind::DimRadial { center, leader_pt, .. } => {
                    let c: Vec2 = (*center).into();
                    let l: Vec2 = (*leader_pt).into();
                    self.push_pick_candidates(
                        &mut best, viewport, rect, screen_pos, entity.id,
                        &[("center", c), ("dim end", l)],
                    );
                }
                EntityKind::Text { position, .. } => {
                    let p: Vec2 = (*position).into();
                    self.push_pick_candidates(
                        &mut best, viewport, rect, screen_pos, entity.id,
                        &[("text origin", p)],
                    );
                }
            }
        }

        best.map(|(_, sel, kind)| (sel, kind))
    }

    fn push_pick_candidates(
        &self,
        best: &mut Option<(f32, Selection, SnapKind)>,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
        entity: Guid,
        candidates: &[(&'static str, Vec2)],
    ) {
        for (label, world) in candidates {
            let kind = Self::snap_kind_from_label(label);
            let (sx, sy) = world_to_screen(world.x as f32, world.y as f32, viewport);
            let pos = rect.min + egui::vec2(sx, sy);
            let dist = pos.distance(screen_pos);
            if dist <= Self::PICK_RADIUS {
                match best {
                    Some((best_dist, _, _)) if dist >= *best_dist => {}
                    _ => {
                        *best = Some((dist, Selection { entity, world: *world }, kind));
                    }
                }
            }
        }
    }

    /// Snap a target point to the nearest angle increment relative to a start.
    fn snap_angle(start: Vec2, target: Vec2, inc_deg: f64) -> Vec2 {
        let inc_rad = inc_deg.to_radians();
        if inc_rad <= f64::EPSILON {
            return target;
        }

        let dx = target.x - start.x;
        let dy = target.y - start.y;
        let r = (dx * dx + dy * dy).sqrt();
        if r <= f64::EPSILON {
            return target;
        }

        let angle = dy.atan2(dx);
        let snapped = (angle / inc_rad).round() * inc_rad;
        Vec2::new(
            start.x + snapped.cos() * r,
            start.y + snapped.sin() * r,
        )
    }

    /// Apply a typed distance to the current line direction, if possible.
    fn apply_distance_override(start: Vec2, target: Vec2, distance_text: &str) -> Option<Vec2> {
        let dist = distance_text.trim().parse::<f64>().ok()?;
        if dist <= f64::EPSILON {
            return None;
        }
        let dx = target.x - start.x;
        let dy = target.y - start.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= f64::EPSILON {
            return None;
        }
        let nx = dx / len;
        let ny = dy / len;
        Some(Vec2::new(start.x + nx * dist, start.y + ny * dist))
    }

    /// Apply circle distance override (radius or diameter) from center to target direction.
    fn apply_circle_distance_override(
        center: Vec2,
        target: Vec2,
        distance_text: &str,
        use_diameter: bool,
    ) -> Option<Vec2> {
        let dist = distance_text.trim().parse::<f64>().ok()?;
        if dist <= f64::EPSILON {
            return None;
        }
        let desired_radius = if use_diameter { dist * 0.5 } else { dist };
        if desired_radius <= f64::EPSILON {
            return None;
        }

        let dx = target.x - center.x;
        let dy = target.y - center.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= f64::EPSILON {
            return None;
        }
        let nx = dx / len;
        let ny = dy / len;
        Some(Vec2::new(
            center.x + nx * desired_radius,
            center.y + ny * desired_radius,
        ))
    }

    /// Split a line at interior intersection points; return all segments except the one containing
    /// the click projection.  Returns an empty vec if no interior intersections exist.
    fn trim_line(
        start: Vec3,
        end: Vec3,
        isect_pts: &[Vec3],
        click: Vec2,
        layer: u32,
    ) -> Vec<cadkit_2d_core::Entity> {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let len_sq = dx * dx + dy * dy;
        if len_sq < 1e-18 {
            return vec![];
        }

        // Project each intersection point onto the line parameter t ∈ [0, 1].
        let mut params: Vec<f64> = isect_pts
            .iter()
            .map(|p| ((p.x - start.x) * dx + (p.y - start.y) * dy) / len_sq)
            .filter(|&t| t > 1e-9 && t < 1.0 - 1e-9)
            .collect();
        if params.is_empty() {
            return vec![];
        }
        params.sort_by(|a, b| a.partial_cmp(b).unwrap());
        params.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

        // Build boundary parameter list [0, t0, t1, ..., 1].
        let bounds: Vec<f64> = std::iter::once(0.0_f64)
            .chain(params.iter().copied())
            .chain(std::iter::once(1.0_f64))
            .collect();

        // Click projection parameter (clamped for robustness).
        let t_click = (((click.x - start.x) * dx + (click.y - start.y) * dy) / len_sq)
            .clamp(0.0, 1.0);

        // Find which interval [bounds[i], bounds[i+1]] contains t_click.
        let skip_idx = bounds
            .windows(2)
            .position(|w| t_click >= w[0] && t_click <= w[1])
            .unwrap_or(0);

        // Return every segment except skip_idx.
        let mut result = Vec::new();
        for (i, w) in bounds.windows(2).enumerate() {
            if i == skip_idx {
                continue;
            }
            let p0 = Vec2::new(start.x + w[0] * dx, start.y + w[0] * dy);
            let p1 = Vec2::new(start.x + w[1] * dx, start.y + w[1] * dy);
            let dist_sq = (p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2);
            if dist_sq < 1e-18 {
                continue;
            }
            let mut e = create_line(p0, p1);
            e.layer = layer;
            result.push(e);
        }
        result
    }

    /// CCW offset of angle `a` from `base` in [0, 2π).
    fn ccw_from(base: f64, a: f64) -> f64 {
        let diff = a - base;
        let twopi = std::f64::consts::TAU;
        ((diff % twopi) + twopi) % twopi
    }

    /// Split an arc at interior intersection points; return all sub-arcs except the one
    /// containing the click's angular projection from the arc's center.
    fn trim_arc(
        center: Vec3,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        isect_pts: &[Vec3],
        click: Vec2,
        layer: u32,
    ) -> Vec<cadkit_2d_core::Entity> {
        let center_v2 = Vec2::new(center.x, center.y);
        let span = Self::ccw_from(start_angle, end_angle);
        if span < 1e-9 {
            return vec![];
        }

        // Convert intersection points to CCW offsets from start_angle.
        let mut offsets: Vec<f64> = isect_pts
            .iter()
            .map(|p| {
                let angle = (p.y - center.y).atan2(p.x - center.x);
                Self::ccw_from(start_angle, angle)
            })
            .filter(|&off| off > 1e-9 && off < span - 1e-9)
            .collect();
        if offsets.is_empty() {
            return vec![];
        }
        offsets.sort_by(|a, b| a.partial_cmp(b).unwrap());
        offsets.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

        // Boundary offsets [0, off0, off1, ..., span].
        let bounds: Vec<f64> = std::iter::once(0.0_f64)
            .chain(offsets.iter().copied())
            .chain(std::iter::once(span))
            .collect();

        // Click angular offset from start_angle (clamped to arc span).
        let click_angle = (click.y - center_v2.y).atan2(click.x - center_v2.x);
        let click_off = Self::ccw_from(start_angle, click_angle).min(span);

        let skip_idx = bounds
            .windows(2)
            .position(|w| click_off >= w[0] && click_off <= w[1])
            .unwrap_or(0);

        let mut result = Vec::new();
        for (i, w) in bounds.windows(2).enumerate() {
            if i == skip_idx {
                continue;
            }
            let a0 = start_angle + w[0];
            let a1 = start_angle + w[1];
            if (a1 - a0).abs() < 1e-9 {
                continue;
            }
            let mut e = create_arc(center_v2, radius, a0, a1);
            e.layer = layer;
            result.push(e);
        }
        result
    }

    /// Split a circle at intersection points; return all arcs except the one containing
    /// the click's angular position.  Requires at least 2 distinct intersection points.
    fn trim_circle(
        center: Vec3,
        radius: f64,
        isect_pts: &[Vec3],
        click: Vec2,
        layer: u32,
    ) -> Vec<cadkit_2d_core::Entity> {
        let center_v2 = Vec2::new(center.x, center.y);

        // Collect intersection angles sorted CCW in [0, 2π).
        let twopi = std::f64::consts::TAU;
        let mut angles: Vec<f64> = isect_pts
            .iter()
            .map(|p| {
                let a = (p.y - center.y).atan2(p.x - center.x);
                if a < 0.0 { a + twopi } else { a }
            })
            .collect();
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
        angles.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

        if angles.len() < 2 {
            return vec![];
        }

        // Use angles[0] as the base; compute offsets of all angles from it.
        let base = angles[0];
        let offsets: Vec<f64> = angles.iter().map(|&a| Self::ccw_from(base, a)).collect();
        // Full span is 2π.
        let mut bounds: Vec<f64> = offsets.clone();
        bounds.push(twopi); // wrap back to base

        // Click angle offset from base.
        let click_angle_raw = (click.y - center_v2.y).atan2(click.x - center_v2.x);
        let click_angle = if click_angle_raw < 0.0 { click_angle_raw + twopi } else { click_angle_raw };
        let click_off = Self::ccw_from(base, click_angle);

        let skip_idx = bounds
            .windows(2)
            .position(|w| click_off >= w[0] && click_off <= w[1])
            .unwrap_or(0);

        let mut result = Vec::new();
        for (i, w) in bounds.windows(2).enumerate() {
            if i == skip_idx {
                continue;
            }
            let a0 = base + w[0];
            let a1 = base + w[1];
            if (a1 - a0).abs() < 1e-9 {
                continue;
            }
            let mut e = create_arc(center_v2, radius, a0, a1);
            e.layer = layer;
            result.push(e);
        }
        result
    }

    /// Compute a trim operation (read-only).  Returns a `TrimResult` describing
    /// what should happen; the caller applies any mutations.
    ///
    /// Separating read from write lets the borrow checker accept field-level
    /// mutations (drawing, trim_cutting_edges, command_log) while a `&Viewport`
    /// borrowed from `self.viewport` is still in scope.
    fn compute_trim(
        &self,
        screen_pos: egui::Pos2,
        viewport: &Viewport,
        rect: egui::Rect,
    ) -> TrimResult {
        // 1. Find entity nearest click.
        let target_id = {
            let mut best: Option<(f32, Guid)> = None;
            for entity in self.drawing.visible_entities() {
                let d = Self::screen_dist_to_entity(&entity.kind, viewport, rect, screen_pos);
                if d <= Self::PICK_RADIUS {
                    if best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                        best = Some((d, entity.id));
                    }
                }
            }
            match best {
                Some((_, id)) => id,
                None => return TrimResult::Fail("TRIM: Nothing found near click".to_string()),
            }
        };

        // 2. Clone target data.
        let (target_kind, target_layer) = match self.drawing.get_entity(&target_id) {
            Some(e) => (e.kind.clone(), e.layer),
            None => return TrimResult::Fail("TRIM: Entity not found".to_string()),
        };

        // 3. Cutting edge prims — skip if prim is target itself.
        let cutting_prims: Vec<GeomPrim> = self
            .trim_cutting_edges
            .iter()
            .filter(|&&id| id != target_id)
            .filter_map(|id| self.drawing.get_entity(id))
            .filter_map(|e| Self::entity_to_geom_prim(&e.kind))
            .collect();

        // 4. Target prim.
        let target_prim = match Self::entity_to_geom_prim(&target_kind) {
            Some(p) => p,
            None => return TrimResult::Fail("TRIM: Unsupported entity type".to_string()),
        };

        // 5. Collect all intersection points.
        let mut isect_pts: Vec<Vec3> = Vec::new();
        for cut_prim in &cutting_prims {
            let result = Self::intersect_geom_prims(&target_prim, cut_prim, Self::GEOM_TOL);
            isect_pts.extend(result.points());
        }
        if isect_pts.is_empty() {
            return TrimResult::Fail("TRIM: No intersection with cutting edges".to_string());
        }

        // 6. Click world position.
        let local = screen_pos - rect.min;
        let click_world = screen_to_world(local.x, local.y, viewport);

        // 7. Compute new entities.
        let new_entities: Vec<cadkit_2d_core::Entity> = match &target_kind {
            EntityKind::Line { start, end } => {
                Self::trim_line(*start, *end, &isect_pts, click_world, target_layer)
            }
            EntityKind::Arc { center, radius, start_angle, end_angle } => Self::trim_arc(
                *center,
                *radius,
                *start_angle,
                *end_angle,
                &isect_pts,
                click_world,
                target_layer,
            ),
            EntityKind::Circle { center, radius } => {
                Self::trim_circle(*center, *radius, &isect_pts, click_world, target_layer)
            }
            EntityKind::Polyline { .. } => {
                return TrimResult::Fail(
                    "TRIM: Polyline trim not yet supported".to_string(),
                );
            }
            EntityKind::DimAligned { .. } | EntityKind::DimLinear { .. } | EntityKind::DimAngular { .. } | EntityKind::DimRadial { .. } => {
                return TrimResult::Fail(
                    "TRIM: Cannot trim dimension entities".to_string(),
                );
            }
            EntityKind::Text { .. } => {
                return TrimResult::Fail(
                    "TRIM: Cannot trim text entities".to_string(),
                );
            }
        };

        TrimResult::Apply { target_id, new_entities }
    }

    fn finalize_polyline(&mut self, closed: bool) {
        if let ActiveTool::Polyline { points } = &mut self.active_tool {
            if points.len() >= 2 {
                let verts: Vec<cadkit_types::Vec3> = points.drain(..).map(|p| p.into()).collect();
                self.push_undo();
                let count = verts.len();
                let entity = cadkit_2d_core::Entity::new(
                    EntityKind::Polyline {
                        vertices: verts,
                        closed,
                    },
                    self.current_layer,
                );
                    self.drawing.add_entity(entity);
                    log::info!(
                        "Polyline created ({} pts, closed={})",
                    count,
                    closed
                );
            } else {
                log::info!("Polyline not created (need at least 2 points)");
                points.clear();
            }
        }
    }

    /// JOIN selected touching lines/open-polylines into one open polyline.
    /// Requires one selected open polyline as the base chain.
    fn join_selected_into_polyline(&mut self) -> bool {
        let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
        let ids = self.filter_editable_entity_ids(&requested, "JOIN");
        if ids.len() < 2 {
            self.command_log
                .push("JOIN: Select an open polyline and touching segments".to_string());
            return false;
        }

        let base_id = ids.iter().copied().find(|id| {
            matches!(
                self.drawing.get_entity(id).map(|e| &e.kind),
                Some(EntityKind::Polyline { closed: false, .. })
            )
        });
        let Some(base_id) = base_id else {
            self.command_log
                .push("JOIN: Need one selected open polyline as base".to_string());
            return false;
        };

        let Some(base_entity) = self.drawing.get_entity(&base_id).cloned() else {
            self.command_log.push("JOIN: Base polyline missing".to_string());
            return false;
        };
        let (mut verts, layer) = match base_entity.kind {
            EntityKind::Polyline { vertices, closed } if !closed && vertices.len() >= 2 => {
                (vertices, base_entity.layer)
            }
            _ => {
                self.command_log
                    .push("JOIN: Base must be an open polyline with at least 2 vertices".to_string());
                return false;
            }
        };

        let mut remaining: Vec<(Guid, EntityKind)> = Vec::new();
        let mut skipped_non_joinable = 0usize;
        for id in ids {
            if id == base_id {
                continue;
            }
            if let Some(e) = self.drawing.get_entity(&id) {
                match &e.kind {
                    EntityKind::Line { .. } => remaining.push((id, e.kind.clone())),
                    EntityKind::Polyline { closed, vertices } if !*closed && vertices.len() >= 2 => {
                        remaining.push((id, e.kind.clone()))
                    }
                    _ => skipped_non_joinable += 1,
                }
            }
        }
        if remaining.is_empty() {
            self.command_log
                .push("JOIN: No joinable touching lines/polylines selected".to_string());
            return false;
        }

        let touch = |a: cadkit_types::Vec3, b: cadkit_types::Vec3| -> bool {
            (a.x - b.x).abs() <= 1e-6 && (a.y - b.y).abs() <= 1e-6
        };

        let mut remove_ids: Vec<Guid> = Vec::new();
        let mut joined = 0usize;
        loop {
            let mut progressed = false;
            for i in 0..remaining.len() {
                let (eid, kind) = &remaining[i];
                let head = verts[0];
                let tail = verts[verts.len() - 1];
                let mut consumed = false;

                match kind {
                    EntityKind::Line { start, end } => {
                        if touch(*start, tail) {
                            let last = verts.len() - 1;
                            verts[last] = *start;
                            verts.push(*end);
                            consumed = true;
                        } else if touch(*end, tail) {
                            let last = verts.len() - 1;
                            verts[last] = *end;
                            verts.push(*start);
                            consumed = true;
                        } else if touch(*end, head) {
                            verts[0] = *end;
                            verts.insert(0, *start);
                            consumed = true;
                        } else if touch(*start, head) {
                            verts[0] = *start;
                            verts.insert(0, *end);
                            consumed = true;
                        }
                    }
                    EntityKind::Polyline { vertices, closed } => {
                        if !*closed && vertices.len() >= 2 {
                            let first = vertices[0];
                            let last = vertices[vertices.len() - 1];
                            if touch(first, tail) {
                                let tail_i = verts.len() - 1;
                                verts[tail_i] = first;
                                verts.extend(vertices.iter().skip(1).copied());
                                consumed = true;
                            } else if touch(last, tail) {
                                let tail_i = verts.len() - 1;
                                verts[tail_i] = last;
                                verts.extend(vertices[..vertices.len() - 1].iter().rev().copied());
                                consumed = true;
                            } else if touch(last, head) {
                                verts[0] = last;
                                let mut prefix: Vec<cadkit_types::Vec3> =
                                    vertices[..vertices.len() - 1].to_vec();
                                prefix.append(&mut verts);
                                verts = prefix;
                                consumed = true;
                            } else if touch(first, head) {
                                verts[0] = first;
                                let mut prefix: Vec<cadkit_types::Vec3> =
                                    vertices[1..].iter().rev().copied().collect();
                                prefix.append(&mut verts);
                                verts = prefix;
                                consumed = true;
                            }
                        }
                    }
                    _ => {}
                }

                if consumed {
                    let (joined_id, _) = remaining.swap_remove(i);
                    remove_ids.push(joined_id);
                    joined += 1;
                    progressed = true;
                    break;
                } else {
                    let _ = eid;
                }
            }
            if !progressed {
                break;
            }
        }

        if joined == 0 {
            self.command_log
                .push("JOIN: No touching segments found at polyline ends".to_string());
            return false;
        }

        self.push_undo();
        let _ = self.drawing.remove_entity(&base_id);
        for id in &remove_ids {
            let _ = self.drawing.remove_entity(id);
        }
        let new_entity = Entity::new(
            EntityKind::Polyline {
                vertices: verts,
                closed: false,
            },
            layer,
        );
        let new_id = new_entity.id;
        self.drawing.add_entity(new_entity);
        self.selected_entities.clear();
        self.selected_entities.insert(new_id);

        self.command_log.push(format!(
            "JOIN: {} entit{} joined into polyline",
            joined,
            if joined == 1 { "y" } else { "ies" }
        ));
        if skipped_non_joinable > 0 {
            self.command_log.push(format!(
                "JOIN: {} non-joinable entit{} ignored",
                skipped_non_joinable,
                if skipped_non_joinable == 1 { "y" } else { "ies" }
            ));
        }
        true
    }

    fn pedit_touch(a: cadkit_types::Vec3, b: cadkit_types::Vec3) -> bool {
        (a.x - b.x).abs() <= 1e-6 && (a.y - b.y).abs() <= 1e-6
    }

    /// Join one touching line or arc into an open polyline's start/end.
    /// Returns the new polyline id on success.
    fn pedit_join_entity_into_polyline(&mut self, base_id: Guid, join_id: Guid) -> Option<Guid> {
        if base_id == join_id {
            self.command_log
                .push("PEDIT: Select a line or arc to join".to_string());
            return None;
        }
        if self.is_entity_on_locked_layer(&base_id) || self.is_entity_on_locked_layer(&join_id) {
            self.command_log
                .push("PEDIT: Cannot edit entities on locked layers".to_string());
            return None;
        }

        let Some(base_entity) = self.drawing.get_entity(&base_id).cloned() else {
            self.command_log.push("PEDIT: Base polyline missing".to_string());
            return None;
        };
        let Some(join_entity) = self.drawing.get_entity(&join_id).cloned() else {
            self.command_log.push("PEDIT: Entity to join missing".to_string());
            return None;
        };

        let (mut verts, closed, layer, color) = match base_entity.kind {
            EntityKind::Polyline { vertices, closed } => {
                (vertices, closed, base_entity.layer, base_entity.color)
            }
            _ => {
                self.command_log
                    .push("PEDIT: Base must be an open polyline".to_string());
                return None;
            }
        };
        if closed || verts.len() < 2 {
            self.command_log
                .push("PEDIT: Base must be an open polyline".to_string());
            return None;
        }

        let head = verts[0];
        let tail = verts[verts.len() - 1];
        let mut joined = false;

        let append_pts = |verts: &mut Vec<cadkit_types::Vec3>, pts: &[cadkit_types::Vec3]| {
            if pts.is_empty() {
                return;
            }
            let start_i = if Self::pedit_touch(verts[verts.len() - 1], pts[0]) {
                1
            } else {
                0
            };
            verts.extend(pts.iter().skip(start_i).copied());
        };
        let prepend_pts = |verts: &mut Vec<cadkit_types::Vec3>, pts: &[cadkit_types::Vec3]| {
            if pts.is_empty() {
                return;
            }
            let mut prefix = pts.to_vec();
            if Self::pedit_touch(prefix[prefix.len() - 1], verts[0]) {
                prefix.pop();
            }
            prefix.extend(verts.iter().copied());
            *verts = prefix;
        };

        match join_entity.kind {
            EntityKind::Line { start, end } => {
                if Self::pedit_touch(start, tail) {
                    append_pts(&mut verts, &[start, end]);
                    joined = true;
                } else if Self::pedit_touch(end, tail) {
                    append_pts(&mut verts, &[end, start]);
                    joined = true;
                } else if Self::pedit_touch(end, head) {
                    prepend_pts(&mut verts, &[start, end]);
                    joined = true;
                } else if Self::pedit_touch(start, head) {
                    prepend_pts(&mut verts, &[end, start]);
                    joined = true;
                }
            }
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let c = Vec2::new(center.x, center.y);
                let sa = start_angle;
                let mut ea = end_angle;
                if ea <= sa {
                    ea += std::f64::consts::TAU;
                }
                let arc_pts: Vec<cadkit_types::Vec3> = angular_arc_pts(c, sa, ea, radius)
                    .into_iter()
                    .map(|p| Vec3::xy(p.x, p.y))
                    .collect();
                if arc_pts.len() >= 2 {
                    let a0 = arc_pts[0];
                    let a1 = arc_pts[arc_pts.len() - 1];
                    if Self::pedit_touch(a0, tail) {
                        append_pts(&mut verts, &arc_pts);
                        joined = true;
                    } else if Self::pedit_touch(a1, tail) {
                        let rev: Vec<cadkit_types::Vec3> = arc_pts.iter().rev().copied().collect();
                        append_pts(&mut verts, &rev);
                        joined = true;
                    } else if Self::pedit_touch(a1, head) {
                        prepend_pts(&mut verts, &arc_pts);
                        joined = true;
                    } else if Self::pedit_touch(a0, head) {
                        let rev: Vec<cadkit_types::Vec3> = arc_pts.iter().rev().copied().collect();
                        prepend_pts(&mut verts, &rev);
                        joined = true;
                    }
                }
            }
            _ => {
                self.command_log
                    .push("PEDIT: Only line or arc can be joined".to_string());
                return None;
            }
        }

        if !joined {
            self.command_log
                .push("PEDIT: Entity must touch polyline at start or end".to_string());
            return None;
        }

        self.push_undo();
        let _ = self.drawing.remove_entity(&base_id);
        let _ = self.drawing.remove_entity(&join_id);
        let mut e = Entity::new(
            EntityKind::Polyline {
                vertices: verts,
                closed: false,
            },
            layer,
        );
        e.color = color;
        let new_id = e.id;
        self.drawing.add_entity(e);
        Some(new_id)
    }

    fn apply_polygon(&mut self, center: Vec2, edge_point: Vec2) -> bool {
        if self.polygon_sides < 3 {
            self.command_log
                .push("POLYGON: Number of sides must be >= 3".to_string());
            return false;
        }
        let edge_point = if self.ortho_enabled {
            Self::snap_angle(center, edge_point, self.ortho_increment_deg)
        } else {
            edge_point
        };
        let r = center.distance_to(&edge_point);
        if r <= 1e-9 || !r.is_finite() {
            self.command_log
                .push("POLYGON: Radius too small".to_string());
            return false;
        }
        let base = (edge_point.y - center.y).atan2(edge_point.x - center.x);
        let step = std::f64::consts::TAU / self.polygon_sides as f64;
        let mut verts: Vec<Vec3> = Vec::with_capacity(self.polygon_sides);
        for i in 0..self.polygon_sides {
            let a = base + i as f64 * step;
            verts.push(Vec3::xy(center.x + r * a.cos(), center.y + r * a.sin()));
        }
        self.push_undo();
        self.drawing.add_entity(Entity::new(
            EntityKind::Polyline {
                vertices: verts,
                closed: true,
            },
            self.current_layer,
        ));
        self.command_log.push(format!(
            "POLYGON: {} sides, r={:.4}",
            self.polygon_sides, r
        ));
        true
    }

    fn rectangle_points_from_diagonal(&self, first: Vec2, second: Vec2) -> Option<[Vec2; 4]> {
        let min_x = first.x.min(second.x);
        let max_x = first.x.max(second.x);
        let min_y = first.y.min(second.y);
        let max_y = first.y.max(second.y);
        if (max_x - min_x).abs() <= 1e-9 || (max_y - min_y).abs() <= 1e-9 {
            return None;
        }
        Some([
            Vec2::new(min_x, min_y),
            Vec2::new(max_x, min_y),
            Vec2::new(max_x, max_y),
            Vec2::new(min_x, max_y),
        ])
    }

    fn rectangle_points_from_dimensions(
        &self,
        first: Vec2,
        width: f64,
        height: f64,
        direction: Vec2,
    ) -> Option<[Vec2; 4]> {
        if width <= 1e-9 || height <= 1e-9 || !width.is_finite() || !height.is_finite() {
            return None;
        }
        let dir = if self.ortho_enabled {
            Self::snap_angle(first, direction, self.ortho_increment_deg)
        } else {
            direction
        };
        let dx = dir.x - first.x;
        let dy = dir.y - first.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len <= 1e-9 || !len.is_finite() {
            return None;
        }
        let ux = dx / len;
        let uy = dy / len;
        let left_x = -uy;
        let left_y = ux;
        let cross = dx * (direction.y - first.y) - dy * (direction.x - first.x);
        let side = if cross >= 0.0 { 1.0 } else { -1.0 };
        let hx = left_x * height * side;
        let hy = left_y * height * side;
        let wx = ux * width;
        let wy = uy * width;
        let p0 = first;
        let p1 = Vec2::new(first.x + wx, first.y + wy);
        let p2 = Vec2::new(p1.x + hx, p1.y + hy);
        let p3 = Vec2::new(first.x + hx, first.y + hy);
        Some([p0, p1, p2, p3])
    }

    fn add_rectangle_polyline(&mut self, pts: [Vec2; 4]) {
        let verts = vec![
            Vec3::xy(pts[0].x, pts[0].y),
            Vec3::xy(pts[1].x, pts[1].y),
            Vec3::xy(pts[2].x, pts[2].y),
            Vec3::xy(pts[3].x, pts[3].y),
        ];
        self.push_undo();
        self.drawing.add_entity(Entity::new(
            EntityKind::Polyline {
                vertices: verts,
                closed: true,
            },
            self.current_layer,
        ));
    }

    fn apply_rectangle_diagonal(&mut self, first: Vec2, second: Vec2) -> bool {
        if let Some(pts) = self.rectangle_points_from_diagonal(first, second) {
            self.add_rectangle_polyline(pts);
            let w = (pts[1].x - pts[0].x).abs();
            let h = (pts[3].y - pts[0].y).abs();
            self.command_log
                .push(format!("RECTANGLE: w={:.4}, h={:.4}", w, h));
            true
        } else {
            self.command_log
                .push("RECTANGLE: Opposite corner too close".to_string());
            false
        }
    }

    fn apply_rectangle_dimensions(
        &mut self,
        first: Vec2,
        width: f64,
        height: f64,
        direction: Vec2,
    ) -> bool {
        if let Some(pts) = self.rectangle_points_from_dimensions(first, width, height, direction) {
            self.add_rectangle_polyline(pts);
            self.command_log
                .push(format!("RECTANGLE: w={:.4}, h={:.4}", width, height));
            true
        } else {
            self.command_log
                .push("RECTANGLE: Invalid width/height or direction".to_string());
            false
        }
    }

    fn apply_ellipse(&mut self, center: Vec2, rx: f64, ry: f64) -> bool {
        if rx <= 1e-9 || ry <= 1e-9 || !rx.is_finite() || !ry.is_finite() {
            self.command_log
                .push("ELLIPSE: Radius/height too small".to_string());
            return false;
        }
        let steps = 96usize;
        let mut verts: Vec<Vec3> = Vec::with_capacity(steps);
        for i in 0..steps {
            let t = i as f64 / steps as f64;
            let a = t * std::f64::consts::TAU;
            verts.push(Vec3::xy(center.x + rx * a.cos(), center.y + ry * a.sin()));
        }
        self.push_undo();
        self.drawing.add_entity(Entity::new(
            EntityKind::Polyline {
                vertices: verts,
                closed: true,
            },
            self.current_layer,
        ));
        self.command_log.push(format!("ELLIPSE: rx={:.4}, ry={:.4}", rx, ry));
        true
    }

    // ── Snap math helpers ────────────────────────────────────────────────────

    /// Foot of perpendicular from `p` onto segment `[a, b]`, clamped to the segment.
    fn nearest_on_segment(p: Vec2, a: Vec2, b: Vec2) -> Vec2 {
        let abx = b.x - a.x; let aby = b.y - a.y;
        let len_sq = abx * abx + aby * aby;
        if len_sq < 1e-12 { return a; }
        let t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq;
        let t = t.clamp(0.0, 1.0);
        Vec2::new(a.x + t * abx, a.y + t * aby)
    }

    /// Foot of perpendicular from `p` onto the INFINITE line through `[a, b]`.
    fn perp_foot_on_line(p: Vec2, a: Vec2, b: Vec2) -> Option<Vec2> {
        let abx = b.x - a.x; let aby = b.y - a.y;
        let len_sq = abx * abx + aby * aby;
        if len_sq < 1e-12 { return None; }
        let t = ((p.x - a.x) * abx + (p.y - a.y) * aby) / len_sq;
        Some(Vec2::new(a.x + t * abx, a.y + t * aby))
    }

    /// Closest point on a circle's circumference to `p`.
    fn nearest_on_circle(p: Vec2, c: Vec2, r: f64) -> Option<Vec2> {
        let dx = p.x - c.x; let dy = p.y - c.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d < 1e-12 { return None; }
        Some(Vec2::new(c.x + r * dx / d, c.y + r * dy / d))
    }

    /// Closest point on an arc to `p` (clamped to arc angle range).
    fn nearest_on_arc(p: Vec2, c: Vec2, r: f64, start_angle: f64, end_angle: f64) -> Option<Vec2> {
        let dx = p.x - c.x; let dy = p.y - c.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d < 1e-12 { return None; }
        let mut angle = f64::atan2(dy, dx);
        // Arcs stored CCW (end_angle > start_angle). Normalise into range.
        while angle < start_angle { angle += std::f64::consts::TAU; }
        if angle <= end_angle {
            Some(Vec2::new(c.x + r * angle.cos(), c.y + r * angle.sin()))
        } else {
            // Return the nearer endpoint
            let ps = Vec2::new(c.x + r * start_angle.cos(), c.y + r * start_angle.sin());
            let pe = Vec2::new(c.x + r * end_angle.cos(),   c.y + r * end_angle.sin());
            let ds = (p.x - ps.x).powi(2) + (p.y - ps.y).powi(2);
            let de = (p.x - pe.x).powi(2) + (p.y - pe.y).powi(2);
            Some(if ds < de { ps } else { pe })
        }
    }

    /// Two tangent-touch points on circle `(c, r)` from external point `from_pt`.
    /// Returns empty vec when `from_pt` is inside or on the circle.
    fn tangent_points_to_circle(from_pt: Vec2, c: Vec2, r: f64) -> Vec<Vec2> {
        let dx = c.x - from_pt.x; let dy = c.y - from_pt.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d <= r + 1e-9 { return vec![]; }
        let phi = f64::atan2(dy, dx);
        let gamma = f64::asin((r / d).clamp(-1.0, 1.0));
        let tlen = (d * d - r * r).sqrt();
        vec![
            Vec2::new(from_pt.x + tlen * f64::cos(phi + gamma),
                      from_pt.y + tlen * f64::sin(phi + gamma)),
            Vec2::new(from_pt.x + tlen * f64::cos(phi - gamma),
                      from_pt.y + tlen * f64::sin(phi - gamma)),
        ]
    }

    // ── New snap functions ────────────────────────────────────────────────────

    /// Returns the most-recently-placed world point in the current drawing command,
    /// used as the "from" origin for perpendicular and tangent snaps.
    fn current_from_point(&self) -> Option<Vec2> {
        match &self.active_tool {
            ActiveTool::Line { start: Some(s) }    => Some(*s),
            ActiveTool::Polyline { points } if !points.is_empty() => points.last().copied(),
            _ => None,
        }
    }

    /// Snap to the closest point ON any entity's geometry (not just special points).
    fn nearest_entity_snap(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<Vec2> {
        let local = screen_pos - rect.min;
        let cw = screen_to_world(local.x, local.y, viewport);
        let cursor = Vec2::new(cw.x, cw.y);
        let mut best: Option<(f32, Vec2)> = None;

        for entity in self.drawing.visible_entities() {
            let foot: Option<Vec2> = match &entity.kind {
                EntityKind::Line { start, end } => {
                    let s: Vec2 = (*start).into();
                    let e: Vec2 = (*end).into();
                    Some(Self::nearest_on_segment(cursor, s, e))
                }
                EntityKind::Circle { center, radius } => {
                    let c: Vec2 = (*center).into();
                    Self::nearest_on_circle(cursor, c, *radius)
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let c: Vec2 = (*center).into();
                    Self::nearest_on_arc(cursor, c, *radius, *start_angle, *end_angle)
                }
                EntityKind::Polyline { vertices, closed } => {
                    if vertices.is_empty() { None } else {
                        let vv: Vec<Vec2> = vertices.iter().map(|v| (*v).into()).collect();
                        let mut min_d = f64::MAX;
                        let mut min_foot = None;
                        let mut check = |a: Vec2, b: Vec2| {
                            let f = Self::nearest_on_segment(cursor, a, b);
                            let d = (f.x - cursor.x).powi(2) + (f.y - cursor.y).powi(2);
                            if d < min_d { min_d = d; min_foot = Some(f); }
                        };
                        for w in vv.windows(2) { check(w[0], w[1]); }
                        if *closed && vv.len() >= 2 { check(*vv.last().unwrap(), vv[0]); }
                        min_foot
                    }
                }
                _ => None,
            };
            if let Some(w) = foot {
                let (sx, sy) = world_to_screen(w.x as f32, w.y as f32, viewport);
                let sp = rect.min + egui::vec2(sx, sy);
                let d = sp.distance(screen_pos);
                if d <= Self::PICK_RADIUS {
                    match best { Some((bd, _)) if d >= bd => {} _ => best = Some((d, w)) }
                }
            }
        }
        best.map(|(_, w)| w)
    }

    /// Snap to the perpendicular foot on an entity from `from_pt` (last placed point).
    fn perpendicular_snap(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
        from_pt: Vec2,
    ) -> Option<Vec2> {
        let mut best: Option<(f32, Vec2)> = None;

        for entity in self.drawing.visible_entities() {
            let foot: Option<Vec2> = match &entity.kind {
                EntityKind::Line { start, end } => {
                    let s: Vec2 = (*start).into();
                    let e: Vec2 = (*end).into();
                    Self::perp_foot_on_line(from_pt, s, e)
                }
                EntityKind::Circle { center, radius } => {
                    let c: Vec2 = (*center).into();
                    // Perpendicular from from_pt = closest point on circle along from_pt→center
                    Self::nearest_on_circle(from_pt, c, *radius)
                }
                _ => None,
            };
            if let Some(w) = foot {
                let (sx, sy) = world_to_screen(w.x as f32, w.y as f32, viewport);
                let sp = rect.min + egui::vec2(sx, sy);
                let d = sp.distance(screen_pos);
                if d <= Self::PICK_RADIUS {
                    match best { Some((bd, _)) if d >= bd => {} _ => best = Some((d, w)) }
                }
            }
        }
        best.map(|(_, w)| w)
    }

    /// Snap to the tangent-touch point on a circle/arc from `from_pt`.
    fn tangent_snap(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
        from_pt: Vec2,
    ) -> Option<Vec2> {
        let mut best: Option<(f32, Vec2)> = None;

        for entity in self.drawing.visible_entities() {
            let candidates: Vec<Vec2> = match &entity.kind {
                EntityKind::Circle { center, radius } => {
                    let c: Vec2 = (*center).into();
                    Self::tangent_points_to_circle(from_pt, c, *radius)
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let c: Vec2 = (*center).into();
                    Self::tangent_points_to_circle(from_pt, c, *radius)
                        .into_iter()
                        .filter(|pt| {
                            let mut a = f64::atan2(pt.y - c.y, pt.x - c.x);
                            while a < *start_angle { a += std::f64::consts::TAU; }
                            a <= *end_angle
                        })
                        .collect()
                }
                _ => vec![],
            };
            for w in candidates {
                let (sx, sy) = world_to_screen(w.x as f32, w.y as f32, viewport);
                let sp = rect.min + egui::vec2(sx, sy);
                let d = sp.distance(screen_pos);
                if d <= Self::PICK_RADIUS {
                    match best { Some((bd, _)) if d >= bd => {} _ => best = Some((d, w)) }
                }
            }
        }
        best.map(|(_, w)| w)
    }
}

/// Build an arc entity passing through start, mid, end (all on XY plane).
/// Returns None if the points are collinear or invalid.
fn create_arc_from_three_points(start: Vec2, mid: Vec2, end: Vec2) -> Option<cadkit_2d_core::Entity> {
    // Compute circle center from three points (circumcenter)
    let x1 = start.x;
    let y1 = start.y;
    let x2 = mid.x;
    let y2 = mid.y;
    let x3 = end.x;
    let y3 = end.y;

    let a = x1 * (y2 - y3) - y1 * (x2 - x3) + x2 * y3 - x3 * y2;
    if a.abs() < 1e-9 {
        return None; // Collinear
    }

    let b = (x1 * x1 + y1 * y1) * (y3 - y2)
        + (x2 * x2 + y2 * y2) * (y1 - y3)
        + (x3 * x3 + y3 * y3) * (y2 - y1);
    let c = (x1 * x1 + y1 * y1) * (x2 - x3)
        + (x2 * x2 + y2 * y2) * (x3 - x1)
        + (x3 * x3 + y3 * y3) * (x1 - x2);

    let cx = -b / (2.0 * a);
    let cy = -c / (2.0 * a);
    let center = Vec2::new(cx, cy);
    let r = center.distance_to(&start);
    if r <= f64::EPSILON {
        return None;
    }

    // Angles
    let ang_start = (y1 - cy).atan2(x1 - cx);
    let ang_end = (y3 - cy).atan2(x3 - cx);

    // Determine orientation (sign of sweep) so arc passes through mid.
    let cross = (mid.x - start.x) * (end.y - start.y) - (mid.y - start.y) * (end.x - start.x);
    let mut sweep = ang_end - ang_start;
    if cross > 0.0 {
        // CCW
        if sweep <= 0.0 {
            sweep += std::f64::consts::TAU;
        }
    } else {
        // CW
        if sweep >= 0.0 {
            sweep -= std::f64::consts::TAU;
        }
    }

    // Normalise to CCW: all arc geometry code (trim, extend, snap) assumes
    // end_angle > start_angle (positive CCW sweep).  A CW arc (sweep < 0)
    // represents the same visual segment as the CCW arc from (ang_start+sweep)
    // to ang_start — swap the endpoints so the span is always positive.
    let (final_start, final_end) = if sweep < 0.0 {
        (ang_start + sweep, ang_start)
    } else {
        (ang_start, ang_start + sweep)
    };

    Some(create_arc(center, r, final_start, final_end))
}

/// Convert HSV (hue 0-360, saturation 0-1, value 0-1) to RGB bytes.
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

/// Return an RGB colour for AutoCAD Color Index (ACI) 0-255.
///
/// Layout:
/// - 0        : black (ByBlock)
/// - 1-9      : standard colours
/// - 10-249   : 24 hue groups × 10 shade rows
/// - 250-255  : grayscale ramp
fn aci_color(idx: u8) -> [u8; 3] {
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
            // group = hue column (0-23), shade = row within group (0-9)
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
