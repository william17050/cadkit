//! CadKit - Main application entry point

use cadkit_2d_core::{create_arc, create_circle, create_line, Drawing, DxfImportResult, Entity, EntityKind};
// create_arc_from_three_points helper lives below in this file (UI layer-specific).
use cadkit_geometry::{
    Arc as GeomArc, Circle as GeomCircle, Intersects, Line as GeomLine,
    Polyline as GeomPolyline,
};
use cadkit_render_wgpu::{font, screen_to_world, world_to_screen, Viewport};
use cadkit_types::{Guid, Vec2, Vec3};
use eframe::egui;
use egui_wgpu::wgpu;
use std::collections::HashSet;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("CadKit - 2D CAD Platform"),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    
    eframe::run_native(
        "CadKit",
        options,
        Box::new(|cc| Box::new(CadKitApp::new(cc))),
    )
}

struct CadKitApp {
    drawing: Drawing,
    command_input: String,
    viewport: Option<Viewport>,
    viewport_texture_id: Option<egui::TextureId>,
    viewport_init_error: Option<String>,
    hover_world_pos: Option<cadkit_types::Vec2>,
    snap_enabled: bool,
    active_tool: ActiveTool,
    selection: Option<Selection>,
    selected_entities: HashSet<Guid>,
    selection_drag_start: Option<egui::Pos2>,
    selection_drag_current: Option<egui::Pos2>,
    ortho_enabled: bool,
    ortho_increment_deg: f64,
    distance_input: String,
    circle_use_diameter: bool,
    command_log: Vec<String>,
    snap_intersection_point: Option<Vec2>,
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
    from_phase: FromPhase,
    from_base: Option<Vec2>,
    dim_phase: DimPhase,
    current_file: Option<String>,
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
}

#[derive(Debug, Clone)]
enum ActiveTool {
    None,
    Line { start: Option<Vec2> },
    Circle { center: Option<Vec2> },
    Arc { start: Option<Vec2>, mid: Option<Vec2> },
    Polyline { points: Vec<Vec2> },
}

#[derive(Debug, Clone, PartialEq)]
enum TrimPhase {
    Idle,
    SelectingEdges,
    Trimming,
}

#[derive(Debug, Clone, PartialEq)]
enum OffsetPhase {
    Idle,
    EnteringDistance,
    SelectingEntity,
    SelectingSide,
}

#[derive(Debug, Clone, PartialEq)]
enum MovePhase {
    Idle,
    SelectingEntities,
    BasePoint,
    Destination,
}

#[derive(Debug, Clone, PartialEq)]
enum ExtendPhase {
    Idle,
    SelectingBoundaries,
    Extending,
}

#[derive(Debug, Clone, PartialEq)]
enum CopyPhase {
    Idle,
    SelectingEntities,
    BasePoint,
    Destination,
}

#[derive(Debug, Clone, PartialEq)]
enum RotatePhase {
    Idle,
    SelectingEntities,
    BasePoint,
    Rotation,
}

/// FROM tracking: lets the user pick a base snap point then type an offset from it.
/// Triggered by typing "from" or "fr" while any point-pick is expected.
#[derive(Debug, Clone, PartialEq)]
enum FromPhase {
    Idle,
    WaitingBase,   // picked base snap point first
    WaitingOffset, // now type @dx,dy or @dist<angle, or click for raw position
}

/// DimLinear placement workflow phases.
#[derive(Debug, Clone, PartialEq)]
enum DimPhase {
    Idle,
    FirstPoint,
    SecondPoint { first: Vec2 },
    Placing { first: Vec2, second: Vec2 },
}

/// Result of a read-only trim computation; mutations are applied by the caller.
enum TrimResult {
    /// Operation failed; the string is the log message.
    Fail(String),
    /// Apply: remove `target_id`, add `new_entities`.
    Apply {
        target_id: Guid,
        new_entities: Vec<cadkit_2d_core::Entity>,
    },
}

#[derive(Debug, Clone)]
struct Selection {
    entity: Guid,
    world: Vec2,
}

/// Geometry-crate primitive, used for intersection dispatch.
enum GeomPrim {
    Line(GeomLine),
    Circle(GeomCircle),
    Arc(GeomArc),
    Polyline(GeomPolyline),
}

/// Minimum screen-space distance from `p` to the segment `[a, b]`.
fn point_to_segment_dist(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.x * ab.x + ab.y * ab.y;
    if len_sq < f32::EPSILON {
        return p.distance(a);
    }
    let ap = p - a;
    let t = ((ap.x * ab.x + ap.y * ab.y) / len_sq).clamp(0.0, 1.0);
    let closest = a + egui::vec2(ab.x * t, ab.y * t);
    p.distance(closest)
}

impl Default for CadKitApp {
    fn default() -> Self {
        let drawing = Drawing::new("New Drawing".to_string());

        Self {
            drawing,
            command_input: String::new(),
            viewport: None,
            viewport_texture_id: None,
            viewport_init_error: None,
            hover_world_pos: None,
            snap_enabled: true,
            active_tool: ActiveTool::None,
            selection: None,
            selected_entities: HashSet::new(),
            selection_drag_start: None,
            selection_drag_current: None,
            ortho_enabled: true,
            ortho_increment_deg: 90.0,
            distance_input: String::new(),
            circle_use_diameter: false,
            command_log: Vec::new(),
            snap_intersection_point: None,
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
            from_phase: FromPhase::Idle,
            from_base: None,
            dim_phase: DimPhase::Idle,
            current_file: None,
            current_layer: 0,
            next_layer_number: 1,
            layer_color_picking: None,
            layer_editing_id: None,
            layer_editing_text: String::new(),
            layer_editing_original: String::new(),
            properties_split: 0.55,
            entity_color_picker_open: false,
            pending_dxf_import: false,
        }
    }
}

impl CadKitApp {
    const PAN_SENSITIVITY: f32 = 0.3;
    const GRID_SPACING: f64 = 12.0;
    const GRID_MAX_POINTS: usize = 20_000;
    const PICK_RADIUS: f32 = 16.0; // screen-space pixels
    const GEOM_TOL: f64 = 1e-9;

    fn cancel_active_tool(&mut self) {
        self.active_tool = ActiveTool::None;
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

    fn exit_dim(&mut self) {
        self.dim_phase = DimPhase::Idle;
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
                    || !matches!(self.dim_phase, DimPhase::Idle)
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
                    let mut line = create_line(s, world);
                    line.layer = layer;
                    self.drawing.add_entity(line);
                    *start = Some(world);
                    self.distance_input.clear();
                    self.command_log.push(format!("  End: {:.4}, {:.4}", world.x, world.y));
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
        }
    }

    /// Execute a command-line alias similar to classic CAD workflows.
    fn execute_command_alias(&mut self, raw: &str) -> bool {
        let cmd = raw.trim().to_ascii_lowercase();
        if cmd.is_empty() {
            return false;
        }

        match cmd.as_str() {
            "l" | "line" => {
                self.active_tool = ActiveTool::Line { start: None };
                self.distance_input.clear();
                self.command_log.push("LINE".to_string());
                log::info!("Command: LINE");
                true
            }
            "c" => {
                // "C" closes an in-progress polyline; otherwise starts a circle
                let close_poly = matches!(
                    &self.active_tool,
                    ActiveTool::Polyline { points } if points.len() >= 2
                );
                if close_poly {
                    self.finalize_polyline(true);
                    self.command_log.push("Polyline closed.".to_string());
                } else {
                    self.active_tool = ActiveTool::Circle { center: None };
                    self.distance_input.clear();
                    self.command_log.push("CIRCLE".to_string());
                    log::info!("Command: CIRCLE");
                }
                true
            }
            "circle" => {
                self.active_tool = ActiveTool::Circle { center: None };
                self.distance_input.clear();
                self.command_log.push("CIRCLE".to_string());
                log::info!("Command: CIRCLE");
                true
            }
            "pl" | "pline" | "polyline" => {
                self.active_tool = ActiveTool::Polyline { points: Vec::new() };
                self.distance_input.clear();
                self.command_log.push("PLINE".to_string());
                log::info!("Command: PLINE");
                true
            }
            "a" | "arc" => {
                self.active_tool = ActiveTool::Arc { start: None, mid: None };
                self.distance_input.clear();
                self.command_log.push("ARC".to_string());
                log::info!("Command: ARC");
                true
            }
            "tr" | "trim" => {
                self.cancel_active_tool();
                self.trim_phase = TrimPhase::SelectingEdges;
                self.trim_cutting_edges.clear();
                self.command_log.push("TRIM: Select cutting edges, press Enter to continue".to_string());
                log::info!("Command: TRIM");
                true
            }
            "ex" | "extend" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.extend_phase = ExtendPhase::SelectingBoundaries;
                self.extend_boundary_edges.clear();
                self.command_log.push("EXTEND: Select boundary edges, press Enter to continue".to_string());
                log::info!("Command: EXTEND");
                true
            }
            "m" | "move" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_copy();
                self.move_phase = MovePhase::SelectingEntities;
                self.move_base_point = None;
                self.move_entities.clear();
                self.command_log.push("MOVE: Select entities to move, press Enter to continue".to_string());
                log::info!("Command: MOVE");
                true
            }
            "ro" | "rotate" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.rotate_phase = RotatePhase::SelectingEntities;
                self.rotate_base_point = None;
                self.rotate_entities.clear();
                self.command_log.push("ROTATE: Select entities, press Enter to continue".to_string());
                log::info!("Command: ROTATE");
                true
            }
            "co" | "copy" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.copy_phase = CopyPhase::SelectingEntities;
                self.copy_base_point = None;
                self.copy_entities.clear();
                self.command_log.push("COPY: Select entities to copy, press Enter to continue".to_string());
                log::info!("Command: COPY");
                true
            }
            "o" | "offset" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.offset_phase = OffsetPhase::EnteringDistance;
                self.offset_distance = None;
                self.offset_selected_entity = None;
                self.command_log.push("OFFSET: Enter distance".to_string());
                log::info!("Command: OFFSET");
                true
            }
            "esc" | "cancel" => {
                self.cancel_active_tool();
                self.command_log.push("*Cancel*".to_string());
                log::info!("Command: CANCEL");
                true
            }
            "la" | "layer" => {
                self.command_log.push("LAYER: Use the layer panel on the right to manage layers".to_string());
                true
            }
            "from" | "fr" => {
                if self.is_picking_point() {
                    self.from_phase = FromPhase::WaitingBase;
                    self.from_base = None;
                    self.command_log.push("FROM  Base point (snap to geometry):".to_string());
                } else {
                    self.command_log.push("FROM: Not active during a point-pick step".to_string());
                }
                true
            }
            "dxfout" => {
                self.export_dxf();
                true
            }
            "dxfin" => {
                self.pending_dxf_import = true;
                true
            }
            "dli" | "dimlinear" | "dim" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_phase = DimPhase::FirstPoint;
                self.command_log.push("DIMLINEAR: Specify first extension line origin".to_string());
                log::info!("Command: DIMLINEAR");
                true
            }
            _ => false,
        }
    }

    fn parse_xy(text: &str) -> Option<Vec2> {
        let (x, y) = text.split_once(',')?;
        let x = x.trim().parse::<f64>().ok()?;
        let y = y.trim().parse::<f64>().ok()?;
        Some(Vec2::new(x, y))
    }

    fn resolve_typed_point(text: &str, base: Option<Vec2>) -> Option<Vec2> {
        let t = text.trim();
        if t.is_empty() {
            return None;
        }

        if let Some(rest) = t.strip_prefix('@') {
            if let Some((dist_s, ang_s)) = rest.split_once('<') {
                let dist = dist_s.trim().parse::<f64>().ok()?;
                let ang_deg = ang_s.trim().parse::<f64>().ok()?;
                let base = base?;
                let ang = ang_deg.to_radians();
                return Some(Vec2::new(
                    base.x + dist * ang.cos(),
                    base.y + dist * ang.sin(),
                ));
            }
            let delta = Self::parse_xy(rest)?;
            let base = base?;
            return Some(Vec2::new(base.x + delta.x, base.y + delta.y));
        }

        Self::parse_xy(t)
    }

    fn apply_typed_point_input(&mut self, raw: &str) -> bool {
        let text = raw.trim();
        if text.is_empty() {
            return false;
        }

        match &mut self.active_tool {
            ActiveTool::Line { start } => {
                let base = *start;
                let world = if let Some(w) = Self::resolve_typed_point(text, base) {
                    w
                } else if let (Ok(dist), Some(b), Some(hover)) =
                    (text.parse::<f64>(), base, self.hover_world_pos)
                {
                    // Direct distance entry: type a number, mouse sets direction
                    if dist <= f64::EPSILON { return false; }
                    let dx = hover.x - b.x;
                    let dy = hover.y - b.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= f64::EPSILON { return false; }
                    let mut w = Vec2::new(b.x + dx / len * dist, b.y + dy / len * dist);
                    if self.ortho_enabled {
                        w = Self::snap_angle(b, w, self.ortho_increment_deg);
                    }
                    w
                } else {
                    return false;
                };

                if start.is_none() {
                    *start = Some(world);
                    self.distance_input.clear();
                    self.command_log.push(format!("  Start: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Line start set at ({:.3}, {:.3})", world.x, world.y);
                } else if let Some(s) = start.take() {
                    let mut line = create_line(s, world);
                    line.layer = self.current_layer;
                    self.drawing.add_entity(line);
                    *start = Some(world);
                    self.distance_input.clear();
                    self.command_log.push(format!("  End: {:.4}, {:.4}", world.x, world.y));
                    log::info!(
                        "Line created from ({:.3}, {:.3}) to ({:.3}, {:.3})",
                        s.x,
                        s.y,
                        world.x,
                        world.y
                    );
                }
                true
            }
            ActiveTool::Circle { center } => {
                let base = *center;
                let world = if let Some(w) = Self::resolve_typed_point(text, base) {
                    w
                } else if let (Ok(val), Some(c)) = (text.parse::<f64>(), base) {
                    // Plain number with center set → radius (or diameter) input
                    if val <= f64::EPSILON { return false; }
                    let desired_r = if self.circle_use_diameter { val * 0.5 } else { val };
                    let hover = self.hover_world_pos.unwrap_or(Vec2::new(c.x + desired_r, c.y));
                    let dx = hover.x - c.x;
                    let dy = hover.y - c.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    let (nx, ny) = if len > f64::EPSILON { (dx / len, dy / len) } else { (1.0, 0.0) };
                    Vec2::new(c.x + nx * desired_r, c.y + ny * desired_r)
                } else {
                    return false;
                };

                if center.is_none() {
                    *center = Some(world);
                    self.distance_input.clear();
                    self.command_log.push(format!("  Center: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Circle center set at ({:.3}, {:.3})", world.x, world.y);
                } else if let Some(c) = center.take() {
                    let radius = c.distance_to(&world);
                    if radius > f64::EPSILON {
                        let mut circle = create_circle(c, radius);
                        circle.layer = self.current_layer;
                        self.drawing.add_entity(circle);
                        self.command_log.push(format!("  Radius: {:.4}", radius));
                        log::info!(
                            "Circle created center ({:.3}, {:.3}) r={:.3}",
                            c.x,
                            c.y,
                            radius
                        );
                    }
                    self.distance_input.clear();
                }
                true
            }
            ActiveTool::Arc { start, mid } => {
                let base = if mid.is_some() {
                    *mid
                } else if start.is_some() {
                    *start
                } else {
                    None
                };
                let Some(world) = Self::resolve_typed_point(text, base) else {
                    return false;
                };

                if start.is_none() {
                    *start = Some(world);
                    self.command_log.push(format!("  Start: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Arc start set at ({:.3}, {:.3})", world.x, world.y);
                } else if mid.is_none() {
                    *mid = Some(world);
                    self.command_log.push(format!("  Mid: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Arc mid set at ({:.3}, {:.3})", world.x, world.y);
                } else if let (Some(s), Some(m)) = (start.take(), mid.take()) {
                    if let Some(mut a) = create_arc_from_three_points(s, m, world) {
                        a.layer = self.current_layer;
                        self.drawing.add_entity(a);
                        self.command_log.push(format!("  End: {:.4}, {:.4}", world.x, world.y));
                        log::info!(
                            "Arc created through start ({:.3}, {:.3}), mid ({:.3}, {:.3}), end ({:.3}, {:.3})",
                            s.x,
                            s.y,
                            m.x,
                            m.y,
                            world.x,
                            world.y
                        );
                    } else {
                        self.command_log.push("  *Invalid arc (collinear points)*".to_string());
                        log::warn!("Arc creation failed (collinear or invalid).");
                    }
                }
                true
            }
            ActiveTool::Polyline { points } => {
                let base = points.last().copied();
                let world = if let Some(w) = Self::resolve_typed_point(text, base) {
                    w
                } else if let (Ok(dist), Some(b), Some(hover)) =
                    (text.parse::<f64>(), base, self.hover_world_pos)
                {
                    if dist <= f64::EPSILON { return false; }
                    let dx = hover.x - b.x;
                    let dy = hover.y - b.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= f64::EPSILON { return false; }
                    let mut w = Vec2::new(b.x + dx / len * dist, b.y + dy / len * dist);
                    if self.ortho_enabled {
                        w = Self::snap_angle(b, w, self.ortho_increment_deg);
                    }
                    w
                } else {
                    return false;
                };

                points.push(world);
                self.distance_input.clear();
                self.command_log.push(format!("  Pt {}: {:.4}, {:.4}", points.len(), world.x, world.y));
                log::info!(
                    "Polyline point {} set at ({:.3}, {:.3})",
                    points.len(),
                    world.x,
                    world.y
                );
                true
            }
            ActiveTool::None => false,
        }
    }

    fn tool_uses_distance_input(&self) -> bool {
        match &self.active_tool {
            ActiveTool::Line { start: Some(_) } => true,
            ActiveTool::Circle { center: Some(_) } => true,
            ActiveTool::Polyline { points } => !points.is_empty(),
            _ => false,
        }
    }

    /// Request focus on the command line input if nothing else currently has it.
    fn auto_focus_command_line(&self, ctx: &egui::Context) {
        if !ctx.wants_keyboard_input() {
            ctx.memory_mut(|m| m.request_focus(egui::Id::new("cmd_input")));
        }
    }

    fn new(cc: &eframe::CreationContext<'_>) -> Self {
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

    fn add_test_circle(&mut self) {
        let offset = self.drawing.entity_count() as f64 * 15.0;
        let circle = create_circle(Vec2::new(20.0 + offset, 20.0 + offset), 18.0);
        self.drawing.add_entity(circle);
    }

    fn add_test_arc(&mut self) {
        let offset = self.drawing.entity_count() as f64 * 12.0;
        let arc = create_arc(
            Vec2::new(-20.0 + offset, 10.0 + offset),
            22.0,
            0.0,
            std::f64::consts::PI * 1.5,
        );
        self.drawing.add_entity(arc);
    }

    fn snap_to_grid(world: cadkit_types::Vec2) -> cadkit_types::Vec2 {
        let gx = (world.x / Self::GRID_SPACING).round() * Self::GRID_SPACING;
        let gy = (world.y / Self::GRID_SPACING).round() * Self::GRID_SPACING;
        cadkit_types::Vec2::new(gx, gy)
    }

    fn draw_grid_overlay(ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        let (w, h) = viewport.size();
        if w == 0 || h == 0 {
            return;
        }

        let top_left = screen_to_world(0.0, 0.0, viewport);
        let bottom_right = screen_to_world(w as f32, h as f32, viewport);
        let min_x = top_left.x.min(bottom_right.x);
        let max_x = top_left.x.max(bottom_right.x);
        let min_y = top_left.y.min(bottom_right.y);
        let max_y = top_left.y.max(bottom_right.y);

        let spacing = Self::GRID_SPACING;
        let start_x = (min_x / spacing).floor() * spacing;
        let end_x = (max_x / spacing).ceil() * spacing;
        let start_y = (min_y / spacing).floor() * spacing;
        let end_y = (max_y / spacing).ceil() * spacing;

        let nx = (((end_x - start_x) / spacing).max(0.0) as usize).saturating_add(1);
        let ny = (((end_y - start_y) / spacing).max(0.0) as usize).saturating_add(1);
        if nx.saturating_mul(ny) > Self::GRID_MAX_POINTS {
            return;
        }

        // Clip grid drawing to the viewport rectangle so it doesn't bleed into
        // side panels or surrounding UI.
        let painter = ui.painter_at(rect);
        let color = egui::Color32::from_gray(95);
        let mut gx = start_x;
        while gx <= end_x + f64::EPSILON {
            let mut gy = start_y;
            while gy <= end_y + f64::EPSILON {
                let (sx, sy) = world_to_screen(gx as f32, gy as f32, viewport);
                let pos = rect.min + egui::vec2(sx, sy);
                painter.circle_filled(pos, 1.5, color);
                gy += spacing;
            }
            gx += spacing;
        }
    }

    fn select_entity_id(&mut self, entity: Option<Guid>, additive: bool) {
        match (entity, additive) {
            (Some(id), true) => {
                if self.selected_entities.contains(&id) {
                    self.selected_entities.remove(&id);
                } else {
                    self.selected_entities.insert(id);
                }
            }
            (Some(id), false) => {
                self.selected_entities.clear();
                self.selected_entities.insert(id);
            }
            (None, false) => self.selected_entities.clear(),
            (None, true) => {}
        }
    }

    fn draw_selected_entities_overlay(&self, ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
        if self.selected_entities.is_empty() {
            return;
        }

        let painter = ui.painter_at(rect);
        let stroke = egui::Stroke::new(2.5, egui::Color32::from_rgb(0, 200, 255));

        for entity in self.drawing.visible_entities() {
            if !self.selected_entities.contains(&entity.id) {
                continue;
            }

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
                    let (rx, ry) = world_to_screen((c.x + r) as f32, c.y as f32, viewport);
                    let screen_r = ((rx - cx).powi(2) + (ry - cy).powi(2)).sqrt();
                    painter.circle_stroke(rect.min + egui::vec2(cx, cy), screen_r, stroke);
                }
                EntityKind::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
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
                    if vertices.len() < 2 {
                        continue;
                    }
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
                    if *closed {
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
                EntityKind::DimLinear { start, end, offset, .. } => {
                    let sx = start.x as f32; let sy = start.y as f32;
                    let ex = end.x as f32;   let ey = end.y as f32;
                    let ddx = ex - sx; let ddy = ey - sy;
                    let len = (ddx*ddx + ddy*ddy).sqrt();
                    if len < 1e-6 { continue; }
                    let perp = [-ddy/len, ddx/len];
                    let off = *offset as f32;
                    let (dl1x, dl1y) = world_to_screen(sx + perp[0]*off, sy + perp[1]*off, viewport);
                    let (dl2x, dl2y) = world_to_screen(ex + perp[0]*off, ey + perp[1]*off, viewport);
                    let (sx1, sy1) = world_to_screen(sx, sy, viewport);
                    let (sx2, sy2) = world_to_screen(ex, ey, viewport);
                    painter.line_segment([rect.min + egui::vec2(dl1x, dl1y), rect.min + egui::vec2(dl2x, dl2y)], stroke);
                    painter.line_segment([rect.min + egui::vec2(sx1, sy1), rect.min + egui::vec2(dl1x, dl1y)], stroke);
                    painter.line_segment([rect.min + egui::vec2(sx2, sy2), rect.min + egui::vec2(dl2x, dl2y)], stroke);
                }
            }
        }
    }

    fn draw_tick_marker(
        ui: &egui::Ui,
        rect: egui::Rect,
        viewport: &Viewport,
        world: Vec2,
        color: egui::Color32,
    ) {
        let (sx, sy) = world_to_screen(world.x as f32, world.y as f32, viewport);
        let center = rect.min + egui::vec2(sx, sy);
        let r = 7.0;
        let painter = ui.painter_at(rect);
        painter.line_segment(
            [center + egui::vec2(-r, -r), center + egui::vec2(r, r)],
            egui::Stroke::new(2.0, color),
        );
        painter.line_segment(
            [center + egui::vec2(-r, r), center + egui::vec2(r, -r)],
            egui::Stroke::new(2.0, color),
        );
    }

    fn current_prompt(&self) -> &'static str {
        // FROM mode overrides all other prompts.
        if self.from_phase == FromPhase::WaitingBase {
            return "FROM  Base point (snap to geometry):";
        }
        if self.from_phase == FromPhase::WaitingOffset {
            return "FROM  Offset (@dx,dy  or  @dist<angle  or click):";
        }
        match &self.active_tool {
            ActiveTool::None => match self.trim_phase {
                TrimPhase::Idle => match self.offset_phase {
                    OffsetPhase::Idle => match self.move_phase {
                        MovePhase::Idle => match self.extend_phase {
                        ExtendPhase::Idle => match self.copy_phase {
                            CopyPhase::Idle => match self.rotate_phase {
                                RotatePhase::Idle => match self.dim_phase {
                                    DimPhase::Idle => "Command:",
                                    DimPhase::FirstPoint => "DIMLINEAR  Specify first extension line origin:",
                                    DimPhase::SecondPoint { .. } => "DIMLINEAR  Specify second extension line origin:",
                                    DimPhase::Placing { .. } => "DIMLINEAR  Specify dimension line location:",
                                },
                                RotatePhase::SelectingEntities => "ROTATE  Select entities, press Enter to continue:",
                                RotatePhase::BasePoint => "ROTATE  Pick base point:",
                                RotatePhase::Rotation => "ROTATE  Specify angle (degrees) or click:",
                            },
                            CopyPhase::SelectingEntities => "COPY  Select entities, press Enter to continue:",
                            CopyPhase::BasePoint => "COPY  Pick base point:",
                            CopyPhase::Destination => "COPY  Pick destination (Enter to finish):",
                        },
                        ExtendPhase::SelectingBoundaries => "EXTEND  Select boundary edges (Enter when done):",
                        ExtendPhase::Extending => "EXTEND  Click near line endpoint to extend:",
                    },
                        MovePhase::SelectingEntities => "MOVE  Select entities, press Enter to continue:",
                        MovePhase::BasePoint => "MOVE  Pick base point:",
                        MovePhase::Destination => "MOVE  Pick destination point:",
                    },
                    OffsetPhase::EnteringDistance => "OFFSET  Enter distance:",
                    OffsetPhase::SelectingEntity => "OFFSET  Select entity to offset:",
                    OffsetPhase::SelectingSide => "OFFSET  Click side to offset toward:",
                },
                TrimPhase::SelectingEdges => "TRIM  Select cutting edges (Enter when done):",
                TrimPhase::Trimming => "TRIM  Click entity side to trim (Esc/Enter to exit):",
            },
            ActiveTool::Line { start: None } => "LINE  Specify first point:",
            ActiveTool::Line { start: Some(_) } => "LINE  Specify next point (Esc to finish):",
            ActiveTool::Circle { center: None } => "CIRCLE  Specify center point:",
            ActiveTool::Circle { center: Some(_) } => "CIRCLE  Specify radius:",
            ActiveTool::Arc { start: None, .. } => "ARC  Specify start point:",
            ActiveTool::Arc { start: Some(_), mid: None } => "ARC  Specify second point:",
            ActiveTool::Arc { start: Some(_), mid: Some(_) } => "ARC  Specify end point:",
            ActiveTool::Polyline { points } => match points.len() {
                0 => "PLINE  Specify start point:",
                _ => "PLINE  Specify next point  [C=Close  RClick/Enter=Done]:",
            },
        }
    }

    /// Save to the current file path, or run Save As if none is set.
    fn save(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.current_file.clone() {
            match self.drawing.save_to_file(&path) {
                Ok(()) => {
                    self.command_log.push(format!("Saved: {}", path));
                    Self::update_title(ctx, &path);
                }
                Err(e) => self.command_log.push(format!("Save failed: {}", e)),
            }
        } else {
            self.save_as(ctx);
        }
    }

    /// Open a Save As dialog and write the file.
    fn save_as(&mut self, ctx: &egui::Context) {
        let path = rfd::FileDialog::new()
            .set_title("Save Drawing As")
            .add_filter("CadKit Drawing", &["json"])
            .save_file();
        if let Some(path) = path {
            let path_str = path.to_string_lossy().to_string();
            match self.drawing.save_to_file(&path_str) {
                Ok(()) => {
                    self.current_file = Some(path_str.clone());
                    self.command_log.push(format!("Saved: {}", path_str));
                    Self::update_title(ctx, &path_str);
                }
                Err(e) => self.command_log.push(format!("Save failed: {}", e)),
            }
        }
    }

    /// Open a file dialog and load a drawing.
    fn open(&mut self, ctx: &egui::Context) {
        let path = rfd::FileDialog::new()
            .set_title("Open Drawing")
            .add_filter("CadKit Drawing", &["json"])
            .pick_file();
        if let Some(path) = path {
            let path_str = path.to_string_lossy().to_string();
            match Drawing::load_from_file(&path_str) {
                Ok(drawing) => {
                    self.drawing = drawing;
                    self.current_file = Some(path_str.clone());
                    self.selected_entities.clear();
                    self.selection = None;
                    self.command_log.push(format!("Opened: {}", path_str));
                    Self::update_title(ctx, &path_str);
                }
                Err(e) => self.command_log.push(format!("Open failed: {}", e)),
            }
        }
    }

    /// Export the current drawing to a DXF file.
    fn export_dxf(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Export Drawing as DXF")
            .add_filter("DXF Drawing", &["dxf"])
            .save_file();
        if let Some(path) = path {
            let path_str = path.to_string_lossy().to_string();
            match self.drawing.save_to_dxf(&path_str) {
                Ok(n) => self.command_log.push(format!("DXF: Exported {} entities to {}", n, path_str)),
                Err(e) => self.command_log.push(format!("DXF: Export failed - {}", e)),
            }
        }
    }

    /// Import a DXF file, replacing the current drawing.
    fn import_dxf(&mut self, ctx: &egui::Context) {
        let path = rfd::FileDialog::new()
            .set_title("Import DXF File")
            .add_filter("DXF Drawing", &["dxf"])
            .pick_file();
        if let Some(path) = path {
            let path_str = path.to_string_lossy().to_string();
            match Drawing::load_from_dxf(&path_str) {
                Ok(DxfImportResult { drawing, entity_count, layer_count, skipped_entity_types }) => {
                    self.drawing = drawing;
                    self.current_file = None;
                    self.selected_entities.clear();
                    self.selection = None;
                    self.command_log.push(format!(
                        "DXF: Imported {} entities, {} layers from {}",
                        entity_count, layer_count, path_str
                    ));
                    for t in &skipped_entity_types {
                        self.command_log.push(format!("DXF: Warning - skipped unsupported entity type: {}", t));
                    }
                    Self::update_title(ctx, &path_str);
                }
                Err(e) => self.command_log.push(format!("DXF: Import failed - {}", e)),
            }
        }
    }

    fn update_title(ctx: &egui::Context, path: &str) {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            format!("CadKit - {}", name),
        ));
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
        let ids: Vec<Guid> = self.move_entities.clone();
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
                    EntityKind::DimLinear { start, end, text_pos, .. } => {
                        start.x += dx; start.y += dy;
                        end.x += dx;   end.y += dy;
                        text_pos.x += dx; text_pos.y += dy;
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
                EntityKind::DimLinear { start, end, offset, .. } => {
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
        let ids: Vec<Guid> = self.copy_entities.clone();
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
                    EntityKind::DimLinear { start, end, offset, text_override, text_pos } => EntityKind::DimLinear {
                        start: Vec3::xy(start.x + dx, start.y + dy),
                        end:   Vec3::xy(end.x   + dx, end.y   + dy),
                        offset: *offset,
                        text_override: text_override.clone(),
                        text_pos: Vec3::xy(text_pos.x + dx, text_pos.y + dy),
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
                EntityKind::DimLinear { start, end, offset, .. } => {
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

        let ids: Vec<Guid> = self.rotate_entities.clone();
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
                    EntityKind::DimLinear { start, end, text_pos, .. } => {
                        *start    = rotate_pt(*start,    base.x, base.y, cos_a, sin_a);
                        *end      = rotate_pt(*end,      base.x, base.y, cos_a, sin_a);
                        *text_pos = rotate_pt(*text_pos, base.x, base.y, cos_a, sin_a);
                        // offset scalar is preserved by rotation (see geometry proof)
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
                EntityKind::DimLinear { start, end, offset, .. } => {
                    let (rs1x, rs1y) = rot(*start);
                    let (rs2x, rs2y) = rot(*end);
                    // Compute rotated dim line endpoints
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
            }
        }
    }

    /// Place a DimLinear entity. Called when the user clicks the dimension line location.
    /// After placement, resets to FirstPoint so the user can continue dimensioning.
    fn place_dim_linear(&mut self, first: Vec2, second: Vec2, offset_world: Vec2) {
        let dx = second.x - first.x;
        let dy = second.y - first.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-6 {
            self.command_log.push("DIMLINEAR: Degenerate dimension, ignored".to_string());
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
        let entity = Entity::new(
            EntityKind::DimLinear {
                start: Vec3::xy(first.x, first.y),
                end: Vec3::xy(second.x, second.y),
                offset,
                text_override: None,
                text_pos,
            },
            self.current_layer,
        );
        self.drawing.add_entity(entity);
        self.command_log.push(format!("DIMLINEAR: Distance = {:.4}", len));
        // Stay in FirstPoint so user can chain dimensions.
        self.dim_phase = DimPhase::FirstPoint;
    }

    /// Draw the DIMLINEAR rubber-band preview during SecondPoint and Placing phases.
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

                // Dimension text via stroke font
                let dist_text = format!("{:.3}", len);
                let tc = [(dl1[0] + dl2[0]) as f32 * 0.5, (dl1[1] + dl2[1]) as f32 * 0.5];
                let sign = if offset >= 0.0 { 1.0_f32 } else { -1.0 };
                let up = [perp[0] as f32 * sign, perp[1] as f32 * sign];
                let dir_f = [dir[0] as f32, dir[1] as f32];
                for (fp1, fp2) in font::text_segments(&dist_text, tc, dir_f, up, 5.0) {
                    let (fsx1, fsy1) = world_to_screen(fp1[0], fp1[1], viewport);
                    let (fsx2, fsy2) = world_to_screen(fp2[0], fp2[1], viewport);
                    painter.line_segment(
                        [rect.min + egui::vec2(fsx1, fsy1), rect.min + egui::vec2(fsx2, fsy2)],
                        ghost_stroke,
                    );
                }
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
    /// Returns (entity_id, is_start, new_point) or an error message.
    fn compute_extend(
        &self,
        screen_pos: egui::Pos2,
        viewport: &Viewport,
        rect: egui::Rect,
    ) -> Result<(Guid, bool, Vec2), String> {
        // 1. Find the nearest line endpoint within PICK_RADIUS.
        let mut best_ep: Option<(f32, Guid, bool)> = None; // (dist, id, is_start)
        for entity in self.drawing.visible_entities() {
            let EntityKind::Line { start, end } = &entity.kind else { continue };
            for (pt, is_start) in [(*start, true), (*end, false)] {
                let (sx, sy) = world_to_screen(pt.x as f32, pt.y as f32, viewport);
                let screen_pt = rect.min + egui::vec2(sx, sy);
                let d = screen_pos.distance(screen_pt);
                if d <= Self::PICK_RADIUS {
                    if best_ep.as_ref().map_or(true, |(bd, _, _)| d < *bd) {
                        best_ep = Some((d, entity.id, is_start));
                    }
                }
            }
        }
        let (_, eid, is_start) = best_ep
            .ok_or_else(|| "EXTEND: Click near a line endpoint".to_string())?;

        // 2. Read the line geometry.
        let entity = self
            .drawing
            .get_entity(&eid)
            .ok_or_else(|| "EXTEND: Entity not found".to_string())?;
        let (clicked_pt, other_pt) = match &entity.kind {
            EntityKind::Line { start, end } => {
                if is_start { (*start, *end) } else { (*end, *start) }
            }
            _ => return Err("EXTEND: Not a line".to_string()),
        };

        // Direction: from other_pt toward clicked_pt and beyond.
        let dx = clicked_pt.x - other_pt.x;
        let dy = clicked_pt.y - other_pt.y;
        let seg_len = (dx * dx + dy * dy).sqrt();
        if seg_len < 1e-9 {
            return Err("EXTEND: Degenerate line".to_string());
        }
        let dir_x = dx / seg_len;
        let dir_y = dy / seg_len;

        // 3. Build an extended ray as a very long GeomLine.
        let far = 1_000_000.0_f64;
        let far_pt = cadkit_types::Vec3::xy(
            other_pt.x + dir_x * far,
            other_pt.y + dir_y * far,
        );
        let ray = GeomLine::new(other_pt, far_pt);

        // 4. Intersect ray with each boundary edge; keep nearest point beyond clicked_pt.
        let mut best_new_pt: Option<Vec2> = None;
        let mut best_dot = f64::INFINITY;
        for &bid in &self.extend_boundary_edges {
            if bid == eid { continue; } // skip self
            let Some(boundary) = self.drawing.get_entity(&bid) else { continue };
            let Some(bprim) = Self::entity_to_geom_prim(&boundary.kind) else { continue };
            let isect = Self::intersect_geom_prims(&GeomPrim::Line(ray), &bprim, Self::GEOM_TOL);
            for pt in isect.points() {
                // dot > epsilon means the point is strictly beyond clicked_pt.
                let dot = (pt.x - clicked_pt.x) * dir_x + (pt.y - clicked_pt.y) * dir_y;
                if dot > 1e-6 && dot < best_dot {
                    best_dot = dot;
                    best_new_pt = Some(Vec2::new(pt.x, pt.y));
                }
            }
        }

        best_new_pt
            .map(|p| (eid, is_start, p))
            .ok_or_else(|| "EXTEND: No intersection found beyond endpoint".to_string())
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
                EntityKind::DimLinear { .. } => {}
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
            EntityKind::DimLinear { .. } => {
                Err("OFFSET: Cannot offset dimension entities".to_string())
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
                EntityKind::DimLinear { .. } => {}
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
            EntityKind::DimLinear { .. } => {}
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

    /// Convert an EntityKind to a cadkit_geometry primitive for intersection testing.
    fn entity_to_geom_prim(kind: &EntityKind) -> Option<GeomPrim> {
        match kind {
            EntityKind::Line { start, end } => {
                Some(GeomPrim::Line(GeomLine::new(*start, *end)))
            }
            EntityKind::Circle { center, radius } => {
                Some(GeomPrim::Circle(GeomCircle::new(*center, *radius)))
            }
            EntityKind::Arc { center, radius, start_angle, end_angle } => {
                Some(GeomPrim::Arc(GeomArc::new(*center, *radius, *start_angle, *end_angle)))
            }
            EntityKind::Polyline { vertices, closed } => {
                Some(GeomPrim::Polyline(GeomPolyline::new(vertices.clone(), *closed)))
            }
            EntityKind::DimLinear { .. } => None,
        }
    }

    /// Dispatch intersection between any two geometry primitives.
    fn intersect_geom_prims(
        a: &GeomPrim,
        b: &GeomPrim,
        tol: f64,
    ) -> cadkit_geometry::Intersection {
        match (a, b) {
            (GeomPrim::Line(la), GeomPrim::Line(lb))     => la.intersect(lb, tol),
            (GeomPrim::Line(l),  GeomPrim::Circle(c))    => l.intersect(c, tol),
            (GeomPrim::Line(l),  GeomPrim::Arc(a))       => l.intersect(a, tol),
            (GeomPrim::Line(l),  GeomPrim::Polyline(p))  => l.intersect(p, tol),
            (GeomPrim::Circle(c), GeomPrim::Line(l))     => l.intersect(c, tol),
            (GeomPrim::Circle(ca), GeomPrim::Circle(cb)) => ca.intersect(cb, tol),
            (GeomPrim::Circle(c), GeomPrim::Arc(a))      => a.intersect(c, tol),
            (GeomPrim::Circle(c), GeomPrim::Polyline(p)) => p.intersect(c, tol),
            (GeomPrim::Arc(a),  GeomPrim::Line(l))       => l.intersect(a, tol),
            (GeomPrim::Arc(a),  GeomPrim::Circle(c))     => a.intersect(c, tol),
            (GeomPrim::Arc(aa), GeomPrim::Arc(ab))       => aa.intersect(ab, tol),
            (GeomPrim::Arc(a),  GeomPrim::Polyline(p))   => p.intersect(a, tol),
            (GeomPrim::Polyline(p), GeomPrim::Line(l))   => p.intersect(l, tol),
            (GeomPrim::Polyline(p), GeomPrim::Circle(c)) => p.intersect(c, tol),
            (GeomPrim::Polyline(p), GeomPrim::Arc(a))    => p.intersect(a, tol),
            (GeomPrim::Polyline(pa), GeomPrim::Polyline(pb)) => pa.intersect(pb, tol),
        }
    }

    /// Screen-space distance from `screen_pos` to the nearest point on `kind`.
    fn screen_dist_to_entity(
        kind: &EntityKind,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> f32 {
        match kind {
            EntityKind::Line { start, end } => {
                let (x1, y1) = world_to_screen(start.x as f32, start.y as f32, viewport);
                let (x2, y2) = world_to_screen(end.x as f32, end.y as f32, viewport);
                let p1 = rect.min + egui::vec2(x1, y1);
                let p2 = rect.min + egui::vec2(x2, y2);
                point_to_segment_dist(screen_pos, p1, p2)
            }
            EntityKind::Circle { center, radius } => {
                let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                let (rx, _ry) =
                    world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                let screen_r = (rx - cx).abs();
                let c_screen = rect.min + egui::vec2(cx, cy);
                (screen_pos.distance(c_screen) - screen_r).abs()
            }
            EntityKind::Arc { center, radius, start_angle, end_angle } => {
                let (cx, cy) = world_to_screen(center.x as f32, center.y as f32, viewport);
                let (rx, _ry) =
                    world_to_screen((center.x + radius) as f32, center.y as f32, viewport);
                let screen_r = (rx - cx).abs();
                let c_screen = rect.min + egui::vec2(cx, cy);

                // Convert screen_pos back to world to check if the angle is within the arc span.
                let rel = screen_pos - c_screen;
                let click_angle = (rel.y as f64).atan2(rel.x as f64);
                let span = Self::ccw_from(*start_angle, *end_angle);
                let angle_in_span = Self::ccw_from(*start_angle, click_angle) <= span;

                if angle_in_span {
                    // Cursor projects onto the arc — use radial distance.
                    (screen_pos.distance(c_screen) - screen_r).abs()
                } else {
                    // Cursor is off the arc's angular span — distance to nearest endpoint.
                    let (ex1, ey1) = world_to_screen(
                        (center.x + radius * start_angle.cos()) as f32,
                        (center.y + radius * start_angle.sin()) as f32,
                        viewport,
                    );
                    let (ex2, ey2) = world_to_screen(
                        (center.x + radius * end_angle.cos()) as f32,
                        (center.y + radius * end_angle.sin()) as f32,
                        viewport,
                    );
                    let p1 = rect.min + egui::vec2(ex1, ey1);
                    let p2 = rect.min + egui::vec2(ex2, ey2);
                    screen_pos.distance(p1).min(screen_pos.distance(p2))
                }
            }
            EntityKind::Polyline { vertices, closed } => {
                if vertices.len() < 2 {
                    return f32::INFINITY;
                }
                let mut min_d = f32::INFINITY;
                let pairs: Box<dyn Iterator<Item = (&cadkit_types::Vec3, &cadkit_types::Vec3)>> =
                    if *closed {
                        Box::new(
                            vertices.windows(2).map(|w| (&w[0], &w[1])).chain(
                                vertices.last().zip(vertices.first()),
                            ),
                        )
                    } else {
                        Box::new(vertices.windows(2).map(|w| (&w[0], &w[1])))
                    };
                for (a, b) in pairs {
                    let (x1, y1) = world_to_screen(a.x as f32, a.y as f32, viewport);
                    let (x2, y2) = world_to_screen(b.x as f32, b.y as f32, viewport);
                    let p1 = rect.min + egui::vec2(x1, y1);
                    let p2 = rect.min + egui::vec2(x2, y2);
                    min_d = min_d.min(point_to_segment_dist(screen_pos, p1, p2));
                }
                min_d
            }
            EntityKind::DimLinear { start, end, offset, .. } => {
                let sx = start.x as f32; let sy = start.y as f32;
                let ex = end.x as f32;   let ey = end.y as f32;
                let ddx = ex - sx; let ddy = ey - sy;
                let len = (ddx*ddx + ddy*ddy).sqrt();
                if len < 1e-6 { return f32::INFINITY; }
                let perp = [-ddy/len, ddx/len];
                let off = *offset as f32;
                let (dl1x, dl1y) = world_to_screen(sx + perp[0]*off, sy + perp[1]*off, viewport);
                let (dl2x, dl2y) = world_to_screen(ex + perp[0]*off, ey + perp[1]*off, viewport);
                point_to_segment_dist(
                    screen_pos,
                    rect.min + egui::vec2(dl1x, dl1y),
                    rect.min + egui::vec2(dl2x, dl2y),
                )
            }
        }
    }

    /// Find the nearest intersection snap point to the cursor when a drawing tool is active.
    ///
    /// Finds the entity nearest the screen cursor, intersects it with all other entities,
    /// and returns the closest intersection point within PICK_RADIUS pixels (if any).
    fn find_intersection_snap(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<Vec2> {
        if matches!(self.active_tool, ActiveTool::None) {
            return None;
        }

        let entities: Vec<_> = self.drawing.visible_entities().collect();
        if entities.len() < 2 {
            return None;
        }

        // Find the entity whose geometry is nearest the cursor in screen space.
        let nearest_idx = entities
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let d = Self::screen_dist_to_entity(&e.kind, viewport, rect, screen_pos);
                (i, d)
            })
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)?;

        let nearest_prim = Self::entity_to_geom_prim(&entities[nearest_idx].kind)?;
        let nearest_id = entities[nearest_idx].id;

        // Collect intersection points of nearest entity with all others.
        let mut candidates: Vec<Vec2> = Vec::new();
        for entity in &entities {
            if entity.id == nearest_id {
                continue;
            }
            if let Some(other_prim) = Self::entity_to_geom_prim(&entity.kind) {
                let result =
                    Self::intersect_geom_prims(&nearest_prim, &other_prim, Self::GEOM_TOL);
                for pt in result.points() {
                    candidates.push(Vec2::new(pt.x, pt.y));
                }
            }
        }

        // Return the candidate closest to the cursor within PICK_RADIUS.
        candidates
            .into_iter()
            .filter_map(|w| {
                let (sx, sy) = world_to_screen(w.x as f32, w.y as f32, viewport);
                let s_pos = rect.min + egui::vec2(sx, sy);
                let d = s_pos.distance(screen_pos);
                if d <= Self::PICK_RADIUS {
                    Some((d, w))
                } else {
                    None
                }
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, w)| w)
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
            } else if !matches!(self.extend_phase, ExtendPhase::Idle) {
                self.exit_extend();
                self.command_log.push("*Cancel*".to_string());
            } else if !matches!(self.dim_phase, DimPhase::Idle) {
                self.exit_dim();
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
            let ids: Vec<Guid> = self.selected_entities.iter().copied().collect();
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

        // Top menu bar
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.drawing = Drawing::new("New Drawing".to_string());
                        self.current_file = None;
                        self.selected_entities.clear();
                        self.selection = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title("CadKit".to_string()));
                        ui.close_menu();
                    }
                    if ui.button("Open...").clicked() {
                        ui.close_menu();
                        self.open(ctx);
                    }
                    ui.separator();
                    if ui.button("Save       Ctrl+S").clicked() {
                        ui.close_menu();
                        self.save(ctx);
                    }
                    if ui.button("Save As...").clicked() {
                        ui.close_menu();
                        self.save_as(ctx);
                    }
                    ui.separator();
                    if ui.button("Export DXF...").clicked() {
                        ui.close_menu();
                        self.export_dxf();
                    }
                    if ui.button("Import DXF...").clicked() {
                        ui.close_menu();
                        self.import_dxf(ctx);
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                
                ui.menu_button("Draw", |ui| {
                    if ui.button("Line").clicked() {
                        log::info!("Line tool selected");
                        let line = create_line(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
                        self.drawing.add_entity(line);
                        ui.close_menu();
                    }
                    if ui.button("Circle").clicked() {
                        log::info!("Circle tool selected");
                        self.add_test_circle();
                        ui.close_menu();
                    }
                    if ui.button("Arc").clicked() {
                        log::info!("Arc tool selected");
                        self.add_test_arc();
                        ui.close_menu();
                    }
                });
                
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        // TODO: About dialog
                    }
                });
            });
        });
        
        // Left sidebar - tools
        egui::SidePanel::left("tools").default_width(150.0).show(ctx, |ui| {
            ui.heading("Draw");
            ui.separator();
            
            if ui.button("📏 Line").clicked() {
                match self.active_tool {
                    ActiveTool::Line { .. } => {
                        log::info!("Line tool canceled");
                        self.cancel_active_tool();
                    }
                    _ => {
                        log::info!("Line tool selected");
                        self.active_tool = ActiveTool::Line { start: None };
                    }
                }
            }
            if ui.button("⭕ Circle").clicked() {
                match self.active_tool {
                    ActiveTool::Circle { .. } => {
                        log::info!("Circle tool canceled");
                        self.cancel_active_tool();
                    }
                    _ => {
                        log::info!("Circle tool selected");
                        self.active_tool = ActiveTool::Circle { center: None };
                        self.distance_input.clear();
                    }
                }
            }
            if ui.button("🧵 Polyline").clicked() {
                match self.active_tool {
                    ActiveTool::Polyline { .. } => {
                        log::info!("Polyline tool canceled");
                        self.cancel_active_tool();
                    }
                    _ => {
                        log::info!("Polyline tool selected");
                        self.active_tool = ActiveTool::Polyline { points: Vec::new() };
                        self.distance_input.clear();
                    }
                }
            }
            if ui.button("◜ Arc").clicked() {
                match self.active_tool {
                    ActiveTool::Arc { .. } => {
                        log::info!("Arc tool canceled");
                        self.cancel_active_tool();
                    }
                    _ => {
                        log::info!("Arc tool selected (start-mid-end)");
                        self.active_tool = ActiveTool::Arc { start: None, mid: None };
                        self.distance_input.clear();
                    }
                }
            }
            
            ui.add_space(20.0);
            ui.heading("Dimension");
            ui.separator();

            if ui.button("📐 Linear Dim").clicked() {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_phase = DimPhase::FirstPoint;
                self.command_log.push("DIMLINEAR: Specify first extension line origin".to_string());
                log::info!("Command: DIMLINEAR");
            }

            ui.add_space(20.0);
            ui.heading("Modify");
            ui.separator();
            
            if ui.button("✂ Trim").clicked() {
                self.cancel_active_tool();
                self.trim_phase = TrimPhase::SelectingEdges;
                self.trim_cutting_edges.clear();
                self.command_log.push("TRIM: Select cutting edges, press Enter to continue".to_string());
                log::info!("Command: TRIM");
            }
            if ui.button("↔ Extend").clicked() {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.extend_phase = ExtendPhase::SelectingBoundaries;
                self.extend_boundary_edges.clear();
                self.command_log.push("EXTEND: Select boundary edges, press Enter to continue".to_string());
                log::info!("Command: EXTEND");
            }
            if ui.button("⊙ Offset").clicked() {
                self.cancel_active_tool();
                self.exit_trim();
                self.offset_phase = OffsetPhase::EnteringDistance;
                self.offset_distance = None;
                self.offset_selected_entity = None;
                self.command_log.push("OFFSET: Enter distance".to_string());
                log::info!("Command: OFFSET");
            }
            if ui.button("➡️ Move").clicked() {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.move_phase = MovePhase::SelectingEntities;
                self.move_base_point = None;
                self.move_entities.clear();
                self.command_log.push("MOVE: Select entities to move, press Enter to continue".to_string());
                log::info!("Command: MOVE");
            }
            if ui.button("📋 Copy").clicked() {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.copy_phase = CopyPhase::SelectingEntities;
                self.copy_base_point = None;
                self.copy_entities.clear();
                self.command_log.push("COPY: Select entities to copy, press Enter to continue".to_string());
                log::info!("Command: COPY");
            }
            if ui.button("🔄 Rotate").clicked() {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.rotate_phase = RotatePhase::SelectingEntities;
                self.rotate_base_point = None;
                self.rotate_entities.clear();
                self.command_log.push("ROTATE: Select entities, press Enter to continue".to_string());
                log::info!("Command: ROTATE");
            }
            if ui.button("🗑️ Delete").clicked() {
                let ids: Vec<Guid> = self.selected_entities.iter().copied().collect();
                for id in &ids {
                    let _ = self.drawing.remove_entity(id);
                }
                if !ids.is_empty() {
                    log::info!("Deleted {} selected entit{}", ids.len(), if ids.len() == 1 { "y" } else { "ies" });
                }
                self.selected_entities.clear();
                self.selection = None;
            }
        });
        
        // Right sidebar - layer manager
        egui::SidePanel::right("properties").default_width(240.0).show(ctx, |ui| {
            let total_h = ui.available_height();

            // Collect layer ids sorted so the list is stable.
            let mut layer_ids: Vec<u32> = self.drawing.layers().map(|l| l.id).collect();
            layer_ids.sort_unstable();

            // Deferred layer mutations.
            let mut toggle_visible:  Option<u32>             = None;
            let mut toggle_locked:   Option<u32>             = None;
            let mut delete_layer:    Option<u32>             = None;
            let mut set_current:     Option<u32>             = None;
            let mut open_color_picker: Option<u32>           = None;
            let mut commit_name:     Option<(u32, String)>   = None;
            let mut cancel_edit                              = false;
            let mut start_edit:      Option<(u32, String)>   = None;

            // Deferred entity mutations.
            let mut assign_entity_layer: Option<u32>         = None;
            let mut set_entity_bylayer                       = false;
            let mut open_entity_color_picker                 = false;

            // ── LAYERS SECTION ──────────────────────────────────────────────
            ui.heading("Layers");
            ui.separator();

            // Active layer indicator.
            if let Some(cur) = self.drawing.get_layer(self.current_layer) {
                let c = cur.color;
                let name = cur.name.clone();
                ui.horizontal(|ui| {
                    ui.label("Active:");
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                    ui.label(egui::RichText::new(name).strong().color(egui::Color32::from_rgb(160, 210, 255)));
                });
            }
            ui.add_space(2.0);

            // Layers scroll list — height controlled by the split handle.
            let layers_list_h = (total_h * self.properties_split - 70.0).max(40.0);
            egui::ScrollArea::vertical()
                .id_source("layer_scroll")
                .max_height(layers_list_h)
                .show(ui, |ui| {
                    for &id in &layer_ids {
                        let (name, visible, locked, color, is_current) = match self.drawing.get_layer(id) {
                            Some(l) => (l.name.clone(), l.visible, l.locked, l.color, self.current_layer == id),
                            None => continue,
                        };
                        let row_color = if is_current {
                            egui::Color32::from_rgb(40, 55, 80)
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        egui::Frame::none().fill(row_color).show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let eye = if visible { "👁" } else { "🚫" };
                                if ui.small_button(eye).on_hover_text("Toggle visibility").clicked() {
                                    toggle_visible = Some(id);
                                }
                                let swatch = egui::Color32::from_rgb(color[0], color[1], color[2]);
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                                ui.painter().rect_filled(rect, 2.0, swatch);
                                ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_gray(120)));
                                if resp.on_hover_text("Change colour").clicked() {
                                    open_color_picker = Some(id);
                                }
                                if self.layer_editing_id == Some(id) {
                                    let edit_resp = ui.add(
                                        egui::TextEdit::singleline(&mut self.layer_editing_text)
                                            .desired_width(80.0),
                                    );
                                    edit_resp.request_focus();
                                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let esc   = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                    if enter || (edit_resp.lost_focus() && !esc) {
                                        commit_name = Some((id, self.layer_editing_text.trim().to_string()));
                                    } else if esc {
                                        cancel_edit = true;
                                    }
                                } else {
                                    let label_text = if is_current {
                                        egui::RichText::new(&name).strong().color(egui::Color32::from_rgb(160, 210, 255))
                                    } else {
                                        egui::RichText::new(&name)
                                    };
                                    let resp = ui.add(egui::Label::new(label_text).sense(egui::Sense::click()));
                                    if resp.double_clicked() {
                                        start_edit = Some((id, name.clone()));
                                    } else if resp.clicked() {
                                        set_current = Some(id);
                                    }
                                    resp.on_hover_text("Click to set current · Double-click to rename");
                                }
                                let lock_icon = if locked { "🔒" } else { "🔓" };
                                if ui.small_button(lock_icon).on_hover_text("Toggle lock").clicked() {
                                    toggle_locked = Some(id);
                                }
                                if id != 0 {
                                    if ui.small_button("✕").on_hover_text("Delete layer").clicked() {
                                        delete_layer = Some(id);
                                    }
                                }
                            });
                        });
                    }
                });

            if ui.small_button("+ New Layer").clicked() {
                let name = format!("Layer {}", self.next_layer_number);
                self.next_layer_number += 1;
                let new_id = self.drawing.add_layer(name);
                self.current_layer = new_id;
            }

            // ── DRAG HANDLE ─────────────────────────────────────────────────
            ui.add_space(2.0);
            let (drag_rect, drag_resp) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 6.0),
                egui::Sense::drag(),
            );
            let handle_color = if drag_resp.hovered() || drag_resp.dragged() {
                egui::Color32::from_gray(90)
            } else {
                egui::Color32::from_gray(55)
            };
            ui.painter().rect_filled(drag_rect, 0.0, handle_color);
            for i in -3i32..=3 {
                let cx = drag_rect.center().x + i as f32 * 5.0;
                ui.painter().circle_filled(
                    egui::pos2(cx, drag_rect.center().y),
                    1.5,
                    egui::Color32::from_gray(130),
                );
            }
            let drag_delta = drag_resp.drag_delta().y;
            let is_dragging = drag_resp.on_hover_cursor(egui::CursorIcon::ResizeVertical).dragged();
            if is_dragging {
                let new_layers_h = layers_list_h + drag_delta;
                self.properties_split = ((new_layers_h + 70.0) / total_h).clamp(0.15, 0.85);
            }
            ui.add_space(2.0);

            // ── PROPERTIES SECTION ──────────────────────────────────────────
            ui.label(egui::RichText::new("Properties").strong());
            ui.separator();

            egui::ScrollArea::vertical()
                .id_source("props_scroll")
                .show(ui, |ui| {
                    let sel_count = self.selected_entities.len();

                    if sel_count == 0 {
                        ui.label(egui::RichText::new("No selection").color(egui::Color32::from_gray(110)));
                    } else {
                        // Gather common color + layer across selection.
                        let mut common_color:  Option<Option<[u8; 3]>> = None;
                        let mut color_mixed                            = false;
                        let mut common_layer:  Option<u32>             = None;
                        let mut layer_mixed                            = false;

                        for id in &self.selected_entities {
                            if let Some(e) = self.drawing.get_entity(id) {
                                match common_color {
                                    None                               => common_color = Some(e.color),
                                    Some(c) if c == e.color            => {}
                                    _                                  => color_mixed = true,
                                }
                                match common_layer {
                                    None                               => common_layer = Some(e.layer),
                                    Some(l) if l == e.layer            => {}
                                    _                                  => layer_mixed = true,
                                }
                            }
                        }

                        // Single-entity geometry block.
                        if sel_count == 1 {
                            let eid = *self.selected_entities.iter().next().unwrap();
                            if let Some(entity) = self.drawing.get_entity(&eid) {
                                let type_name = match &entity.kind {
                                    EntityKind::Line { .. }      => "Line",
                                    EntityKind::Circle { .. }    => "Circle",
                                    EntityKind::Arc { .. }       => "Arc",
                                    EntityKind::Polyline { .. }  => "Polyline",
                                    EntityKind::DimLinear { .. } => "DimLinear",
                                };
                                ui.label(egui::RichText::new(type_name).strong());
                                egui::Grid::new("entity_geom")
                                    .num_columns(2)
                                    .spacing([4.0, 2.0])
                                    .show(ui, |ui| {
                                        match &entity.kind {
                                            EntityKind::Line { start, end } => {
                                                let dx  = end.x - start.x;
                                                let dy  = end.y - start.y;
                                                let len = (dx * dx + dy * dy).sqrt();
                                                ui.label("Start X:"); ui.label(format!("{:.4}", start.x)); ui.end_row();
                                                ui.label("Start Y:"); ui.label(format!("{:.4}", start.y)); ui.end_row();
                                                ui.label("End X:");   ui.label(format!("{:.4}", end.x));   ui.end_row();
                                                ui.label("End Y:");   ui.label(format!("{:.4}", end.y));   ui.end_row();
                                                ui.label("Length:");  ui.label(format!("{:.4}", len));     ui.end_row();
                                            }
                                            EntityKind::Circle { center, radius } => {
                                                ui.label("Center X:"); ui.label(format!("{:.4}", center.x));    ui.end_row();
                                                ui.label("Center Y:"); ui.label(format!("{:.4}", center.y));    ui.end_row();
                                                ui.label("Radius:");        ui.label(format!("{:.4}", radius));                          ui.end_row();
                                                ui.label("Diameter:");      ui.label(format!("{:.4}", radius * 2.0));                    ui.end_row();
                                                ui.label("Circumference:"); ui.label(format!("{:.4}", std::f64::consts::TAU * radius)); ui.end_row();
                                            }
                                            EntityKind::Arc { center, radius, start_angle, end_angle } => {
                                                let span_rad = (end_angle - start_angle).abs();
                                                let span_deg = span_rad.to_degrees();
                                                let arc_len  = radius * span_rad;
                                                ui.label("Center X:");  ui.label(format!("{:.4}", center.x));                  ui.end_row();
                                                ui.label("Center Y:");  ui.label(format!("{:.4}", center.y));                  ui.end_row();
                                                ui.label("Radius:");    ui.label(format!("{:.4}", radius));                     ui.end_row();
                                                ui.label("Start Ang:"); ui.label(format!("{:.2}°", start_angle.to_degrees())); ui.end_row();
                                                ui.label("End Ang:");   ui.label(format!("{:.2}°", end_angle.to_degrees()));   ui.end_row();
                                                ui.label("Span:");      ui.label(format!("{:.2}°", span_deg));                 ui.end_row();
                                                ui.label("Arc Length:"); ui.label(format!("{:.4}", arc_len));                  ui.end_row();
                                            }
                                            EntityKind::Polyline { vertices, closed } => {
                                                ui.label("Points:"); ui.label(vertices.len().to_string()); ui.end_row();
                                                ui.label("Closed:"); ui.label(if *closed { "Yes" } else { "No" }); ui.end_row();
                                            }
                                            EntityKind::DimLinear { start, end, offset, text_override, .. } => {
                                                let dx = end.x - start.x;
                                                let dy = end.y - start.y;
                                                let dist = (dx*dx + dy*dy).sqrt();
                                                ui.label("Start X:"); ui.label(format!("{:.4}", start.x)); ui.end_row();
                                                ui.label("Start Y:"); ui.label(format!("{:.4}", start.y)); ui.end_row();
                                                ui.label("End X:");   ui.label(format!("{:.4}", end.x));   ui.end_row();
                                                ui.label("End Y:");   ui.label(format!("{:.4}", end.y));   ui.end_row();
                                                ui.label("Distance:"); ui.label(format!("{:.4}", dist));   ui.end_row();
                                                ui.label("Offset:");  ui.label(format!("{:.4}", offset));  ui.end_row();
                                                if let Some(t) = text_override {
                                                    ui.label("Text:"); ui.label(t.as_str()); ui.end_row();
                                                }
                                            }
                                        }
                                    });
                                ui.separator();
                            }
                        } else {
                            ui.label(egui::RichText::new(format!("{sel_count} entities selected")).small()
                                .color(egui::Color32::from_gray(150)));
                            ui.separator();
                        }

                        // ── Layer combo ──────────────────────────────────────
                        let layer_display = if layer_mixed {
                            "*varies*".to_string()
                        } else if let Some(lid) = common_layer {
                            self.drawing.get_layer(lid).map(|l| l.name.clone()).unwrap_or_else(|| lid.to_string())
                        } else {
                            "—".to_string()
                        };
                        ui.horizontal(|ui| {
                            ui.label("Layer:");
                            egui::ComboBox::from_id_source("prop_layer_combo")
                                .selected_text(&layer_display)
                                .width(110.0)
                                .show_ui(ui, |ui| {
                                    for &lid in &layer_ids {
                                        if let Some(layer) = self.drawing.get_layer(lid) {
                                            let is_sel = common_layer == Some(lid) && !layer_mixed;
                                            if ui.selectable_label(is_sel, layer.name.clone()).clicked() {
                                                assign_entity_layer = Some(lid);
                                            }
                                        }
                                    }
                                });
                        });

                        // ── Color row ────────────────────────────────────────
                        // Effective color for single / common selection.
                        let entity_custom_color: Option<[u8; 3]> = if color_mixed {
                            None
                        } else {
                            common_color.flatten()
                        };
                        let layer_color: Option<[u8; 3]> = if !layer_mixed {
                            common_layer.and_then(|lid| self.drawing.get_layer(lid).map(|l| l.color))
                        } else {
                            None
                        };
                        let bylayer_active = !color_mixed && common_color == Some(None);

                        ui.horizontal(|ui| {
                            ui.label("Color:");

                            // ByLayer toggle
                            if ui.selectable_label(bylayer_active, "ByLayer")
                                .on_hover_text("Use layer colour")
                                .clicked()
                                && !bylayer_active
                            {
                                set_entity_bylayer = true;
                            }

                            // Colour swatch — shows entity override or (dimmed) layer fallback.
                            let swatch_rgb = entity_custom_color
                                .or(layer_color)
                                .unwrap_or([128, 128, 128]);
                            let swatch_c = egui::Color32::from_rgb(swatch_rgb[0], swatch_rgb[1], swatch_rgb[2]);

                            let (rect, resp) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
                            ui.painter().rect_filled(rect, 2.0, swatch_c);
                            let (stroke_w, stroke_c) = if entity_custom_color.is_some() && !color_mixed {
                                (2.0, egui::Color32::WHITE)
                            } else {
                                (1.0, egui::Color32::from_gray(100))
                            };
                            ui.painter().rect_stroke(rect, 2.0, egui::Stroke::new(stroke_w, stroke_c));

                            if color_mixed {
                                ui.label(egui::RichText::new("varies").small().color(egui::Color32::from_gray(120)));
                            }

                            if resp.on_hover_text("Set custom entity colour").clicked() {
                                open_entity_color_picker = true;
                            }
                        });
                    }
                });

            // ── Apply layer mutations ────────────────────────────────────────
            if let Some(id) = toggle_visible {
                if let Some(l) = self.drawing.get_layer_mut(id) { l.visible = !l.visible; }
            }
            if let Some(id) = toggle_locked {
                if let Some(l) = self.drawing.get_layer_mut(id) { l.locked = !l.locked; }
            }
            if let Some(id) = set_current {
                self.current_layer = id;
            }
            if let Some(id) = open_color_picker {
                self.layer_color_picking = Some(id);
            }
            if let Some((id, new_name)) = commit_name {
                if new_name.is_empty() {
                    self.command_log.push("LAYER: Name cannot be empty".to_string());
                    self.layer_editing_id = None;
                } else if self.drawing.layers().any(|l| l.name == new_name && l.id != id) {
                    self.command_log.push("LAYER: Layer name already exists".to_string());
                    self.layer_editing_id = None;
                } else {
                    if let Some(l) = self.drawing.get_layer_mut(id) { l.name = new_name; }
                    self.layer_editing_id = None;
                }
            }
            if cancel_edit {
                self.layer_editing_text = self.layer_editing_original.clone();
                self.layer_editing_id = None;
            }
            if let Some((id, original)) = start_edit {
                self.layer_editing_id = Some(id);
                self.layer_editing_text = original.clone();
                self.layer_editing_original = original;
            }
            if let Some(id) = delete_layer {
                if id == 0 {
                    self.command_log.push("LAYER: Cannot delete default layer".to_string());
                } else if self.drawing.entities().any(|e| e.layer == id) {
                    self.command_log.push("LAYER: Cannot delete layer with entities".to_string());
                } else {
                    self.drawing.remove_layer(id);
                    if self.current_layer == id { self.current_layer = 0; }
                    if self.layer_editing_id == Some(id) { self.layer_editing_id = None; }
                    if self.layer_color_picking == Some(id) { self.layer_color_picking = None; }
                }
            }

            // ── Apply entity mutations ───────────────────────────────────────
            if let Some(lid) = assign_entity_layer {
                let ids: Vec<Guid> = self.selected_entities.iter().copied().collect();
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) { e.layer = lid; }
                }
            }
            if set_entity_bylayer {
                let ids: Vec<Guid> = self.selected_entities.iter().copied().collect();
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) { e.color = None; }
                }
            }
            if open_entity_color_picker {
                self.entity_color_picker_open = true;
            }
        });

        // Colour picker popup window — full 255-colour ACI palette.
        if let Some(pick_id) = self.layer_color_picking {
            let mut still_open = true;
            let mut picked_color: Option<[u8; 3]> = None;
            egui::Window::new("Layer Colour")
                .open(&mut still_open)
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    let current_color = self.drawing.get_layer(pick_id)
                        .map(|l| l.color)
                        .unwrap_or([255, 255, 255]);

                    // Standard colours 1-9
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

                    // Main grid: 24 hue columns × 10 shade rows (indices 10-249)
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

                    // Grayscale ramp 250-255
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
                if let Some(l) = self.drawing.get_layer_mut(pick_id) {
                    l.color = rgb;
                }
                self.layer_color_picking = None;
            }
            if !still_open {
                self.layer_color_picking = None;
            }
        }

        // Entity colour picker popup — same ACI grid, applies to selected entities.
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
                let ids: Vec<Guid> = self.selected_entities.iter().copied().collect();
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
        egui::TopBottomPanel::bottom("command_line")
            .min_height(110.0)
            .frame(
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(20, 20, 20))
                    .inner_margin(egui::Margin::same(6.0)),
            )
            .show(ctx, |ui| {
                let input_row_height = 24.0;
                let history_height = ui.available_height() - input_row_height - 10.0;

                // Scrollable history + live prompt
                egui::ScrollArea::vertical()
                    .id_source("cmd_scroll")
                    .max_height(history_height.max(0.0))
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.command_log {
                            ui.label(
                                egui::RichText::new(line)
                                    .color(egui::Color32::from_gray(190))
                                    .monospace()
                                    .size(12.0),
                            );
                        }
                        // Live prompt shown in yellow
                        ui.label(
                            egui::RichText::new(self.current_prompt())
                                .color(egui::Color32::from_rgb(255, 210, 60))
                                .monospace()
                                .size(12.0),
                        );
                    });

                ui.separator();

                // Input row
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("›")
                            .color(egui::Color32::from_rgb(80, 220, 80))
                            .size(18.0),
                    );
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.command_input)
                            .id(egui::Id::new("cmd_input"))
                            .frame(false)
                            .text_color(egui::Color32::WHITE)
                            .desired_width(f32::INFINITY),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        let cmd = self.command_input.clone();
                        if cmd.trim().is_empty() {
                            // Empty Enter: handle phase transitions.
                            if self.trim_phase == TrimPhase::SelectingEdges {
                                if self.trim_cutting_edges.is_empty() {
                                    self.command_log.push("TRIM: No cutting edges selected".to_string());
                                } else {
                                    self.trim_phase = TrimPhase::Trimming;
                                    self.command_log.push("TRIM: Click entity side to trim".to_string());
                                }
                            } else if self.trim_phase == TrimPhase::Trimming {
                                self.exit_trim();
                                self.command_log.push("TRIM done.".to_string());
                            } else if self.extend_phase == ExtendPhase::SelectingBoundaries {
                                if self.extend_boundary_edges.is_empty() {
                                    self.command_log.push("EXTEND: No boundary edges selected".to_string());
                                } else {
                                    self.extend_phase = ExtendPhase::Extending;
                                    self.command_log.push("EXTEND: Click near line endpoint to extend".to_string());
                                }
                            } else if self.extend_phase == ExtendPhase::Extending {
                                self.exit_extend();
                                self.command_log.push("EXTEND done.".to_string());
                            } else if self.move_phase == MovePhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("MOVE: No entities selected".to_string());
                                } else {
                                    self.move_entities = self.selected_entities.iter().copied().collect();
                                    self.move_phase = MovePhase::BasePoint;
                                    self.command_log.push("MOVE: Pick base point".to_string());
                                }
                            } else if self.move_phase == MovePhase::BasePoint {
                                // Enter = confirm current hover position as base point.
                                if let Some(world) = self.hover_world_pos {
                                    self.move_base_point = Some(world);
                                    self.move_phase = MovePhase::Destination;
                                    self.command_log.push("MOVE: Pick destination point".to_string());
                                }
                            } else if self.move_phase == MovePhase::Destination {
                                // Enter = confirm current hover position as destination.
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_move(world);
                                }
                            } else if self.copy_phase == CopyPhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("COPY: No entities selected".to_string());
                                } else {
                                    self.copy_entities = self.selected_entities.iter().copied().collect();
                                    self.copy_phase = CopyPhase::BasePoint;
                                    self.command_log.push("COPY: Pick base point".to_string());
                                }
                            } else if self.copy_phase == CopyPhase::BasePoint {
                                // Enter = confirm current hover position as base point.
                                if let Some(world) = self.hover_world_pos {
                                    self.copy_base_point = Some(world);
                                    self.copy_phase = CopyPhase::Destination;
                                    self.command_log.push("COPY: Pick destination (RClick/Enter=done)".to_string());
                                }
                            } else if self.copy_phase == CopyPhase::Destination {
                                if let Some(world) = self.hover_world_pos {
                                    // Enter with hover = copy to that point, stay for more.
                                    self.apply_copy(world);
                                } else {
                                    // Enter with no hover = done with copies.
                                    self.exit_copy();
                                    self.command_log.push("COPY done.".to_string());
                                }
                            } else if self.rotate_phase == RotatePhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("ROTATE: No entities selected".to_string());
                                } else {
                                    self.rotate_entities = self.selected_entities.iter().copied().collect();
                                    self.rotate_phase = RotatePhase::BasePoint;
                                    self.command_log.push("ROTATE: Pick base point".to_string());
                                }
                            } else if self.rotate_phase == RotatePhase::BasePoint {
                                // Enter = confirm current hover position as base point.
                                if let Some(world) = self.hover_world_pos {
                                    self.rotate_base_point = Some(world);
                                    self.rotate_phase = RotatePhase::Rotation;
                                    self.command_log.push("ROTATE: Specify angle (degrees) or click".to_string());
                                }
                            } else if self.rotate_phase == RotatePhase::Rotation {
                                // Enter = confirm current hover position as rotation target.
                                if let (Some(world), Some(base)) = (self.hover_world_pos, self.rotate_base_point) {
                                    let angle = (world.y - base.y).atan2(world.x - base.x);
                                    self.apply_rotate(angle);
                                }
                            } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                if let Some(world) = self.hover_world_pos {
                                    if matches!(self.dim_phase, DimPhase::FirstPoint) {
                                        self.dim_phase = DimPhase::SecondPoint { first: world };
                                        self.command_log.push(format!("DIMLINEAR: First point ({:.4}, {:.4})", world.x, world.y));
                                    } else if let DimPhase::SecondPoint { first } = self.dim_phase {
                                        self.dim_phase = DimPhase::Placing { first, second: world };
                                        self.command_log.push(format!("DIMLINEAR: Second point ({:.4}, {:.4})", world.x, world.y));
                                    } else if let DimPhase::Placing { first, second } = self.dim_phase {
                                        self.place_dim_linear(first, second, world);
                                    }
                                }
                            } else {
                                // Enter confirms current hover position (same as clicking at cursor).
                                let hover = self.hover_world_pos;
                                let should_finish = matches!(
                                    &self.active_tool,
                                    ActiveTool::Polyline { points } if points.len() >= 2
                                );
                                if should_finish {
                                    self.finalize_polyline(false);
                                    self.command_log.push("Polyline finished.".to_string());
                                } else if let Some(world) = hover {
                                    match &mut self.active_tool {
                                        ActiveTool::Line { start } => {
                                            if start.is_none() {
                                                *start = Some(world);
                                            } else if let Some(s) = start.take() {
                                                let mut line = create_line(s, world);
                                                line.layer = self.current_layer;
                                                self.drawing.add_entity(line);
                                                *start = Some(world);
                                                self.distance_input.clear();
                                            }
                                        }
                                        ActiveTool::Circle { center } => {
                                            if center.is_none() {
                                                *center = Some(world);
                                            } else if let Some(c) = center.take() {
                                                let radius = c.distance_to(&world);
                                                if radius > f64::EPSILON {
                                                    let mut circle = create_circle(c, radius);
                                                    circle.layer = self.current_layer;
                                                    self.drawing.add_entity(circle);
                                                }
                                            }
                                        }
                                        ActiveTool::Arc { start, mid } => {
                                            if start.is_none() {
                                                *start = Some(world);
                                            } else if mid.is_none() {
                                                *mid = Some(world);
                                            } else if let (Some(s), Some(m)) = (start.take(), mid.take()) {
                                                let arc = create_arc_from_three_points(s, m, world);
                                                if let Some(mut a) = arc {
                                                    a.layer = self.current_layer;
                                                    self.drawing.add_entity(a);
                                                }
                                            }
                                        }
                                        ActiveTool::Polyline { points } => {
                                            points.push(world);
                                            self.distance_input.clear();
                                        }
                                        ActiveTool::None => {}
                                    }
                                }
                            }
                        } else {
                            self.command_log.push(format!("› {}", cmd.trim()));
                            // FROM mode intercepts all typed input in both WaitingBase and WaitingOffset.
                            let handled = if self.from_phase == FromPhase::WaitingBase {
                                // Accept absolute x,y as the base point.
                                if let Some(base) = Self::resolve_typed_point(cmd.trim(), None) {
                                    self.from_base = Some(base);
                                    self.from_phase = FromPhase::WaitingOffset;
                                    self.command_log.push(format!("  Base: {:.4}, {:.4}", base.x, base.y));
                                    self.command_log.push("FROM  Offset (@dx,dy  or  @dist<angle):".to_string());
                                } else {
                                    self.command_log.push("  *FROM: enter x,y for base point*".to_string());
                                }
                                true
                            } else if self.from_phase == FromPhase::WaitingOffset {
                                if let Some(result) = Self::resolve_typed_point(cmd.trim(), self.from_base) {
                                    // @dx,dy  /  @dist<angle  /  x,y
                                    self.exit_from();
                                    self.deliver_point(result);
                                } else if let (Ok(dist), Some(base), Some(hover)) =
                                    (cmd.trim().parse::<f64>(), self.from_base, self.hover_world_pos)
                                {
                                    // Plain number: travel `dist` in the direction base→cursor,
                                    // ortho-constrained when enabled.
                                    if dist > f64::EPSILON {
                                        let mut target = hover;
                                        if self.ortho_enabled {
                                            target = Self::snap_angle(base, hover, self.ortho_increment_deg);
                                        }
                                        let dx = target.x - base.x;
                                        let dy = target.y - base.y;
                                        let len = (dx * dx + dy * dy).sqrt();
                                        if len > f64::EPSILON {
                                            let result = Vec2::new(
                                                base.x + dx / len * dist,
                                                base.y + dy / len * dist,
                                            );
                                            self.exit_from();
                                            self.deliver_point(result);
                                        }
                                    }
                                } else {
                                    self.command_log.push("  *FROM: @dx,dy | @dist<angle | dist | x,y*".to_string());
                                }
                                true
                            // "from"/"fr" must be checked before MOVE/COPY/ROTATE handlers,
                            // because those always return handled=true even on parse failure.
                            } else if matches!(cmd.trim().to_ascii_lowercase().as_str(), "from" | "fr")
                                && self.is_picking_point()
                            {
                                self.from_phase = FromPhase::WaitingBase;
                                self.from_base = None;
                                self.command_log.push("FROM  Base point (snap to geometry):".to_string());
                                true
                            // MOVE coordinate entry intercepts before generic input handlers.
                            } else if self.move_phase == MovePhase::BasePoint {
                                if let Some(world) = Self::resolve_typed_point(cmd.trim(), None) {
                                    self.move_base_point = Some(world);
                                    self.move_phase = MovePhase::Destination;
                                    self.command_log.push("MOVE: Pick destination point".to_string());
                                } else {
                                    self.command_log.push("  *Invalid coordinate*".to_string());
                                }
                                true
                            } else if self.move_phase == MovePhase::Destination {
                                if let Some(world) = Self::resolve_typed_point(cmd.trim(), self.move_base_point) {
                                    self.apply_move(world);
                                } else if let (Ok(dist), Some(base), Some(hover)) =
                                    (cmd.trim().parse::<f64>(), self.move_base_point, self.hover_world_pos)
                                {
                                    if dist > f64::EPSILON {
                                        let dx = hover.x - base.x;
                                        let dy = hover.y - base.y;
                                        let len = (dx * dx + dy * dy).sqrt();
                                        if len > f64::EPSILON {
                                            let dest = Vec2::new(
                                                base.x + dx / len * dist,
                                                base.y + dy / len * dist,
                                            );
                                            self.apply_move(dest);
                                        }
                                    }
                                } else {
                                    self.command_log.push("  *Invalid coordinate*".to_string());
                                }
                                true
                            // COPY coordinate entry.
                            } else if self.copy_phase == CopyPhase::BasePoint {
                                if let Some(world) = Self::resolve_typed_point(cmd.trim(), None) {
                                    self.copy_base_point = Some(world);
                                    self.copy_phase = CopyPhase::Destination;
                                    self.command_log.push("COPY: Pick destination (Enter to finish)".to_string());
                                } else {
                                    self.command_log.push("  *Invalid coordinate*".to_string());
                                }
                                true
                            } else if self.copy_phase == CopyPhase::Destination {
                                if let Some(world) = Self::resolve_typed_point(cmd.trim(), self.copy_base_point) {
                                    self.apply_copy(world);
                                } else if let (Ok(dist), Some(base), Some(hover)) =
                                    (cmd.trim().parse::<f64>(), self.copy_base_point, self.hover_world_pos)
                                {
                                    if dist > f64::EPSILON {
                                        let dx = hover.x - base.x;
                                        let dy = hover.y - base.y;
                                        let len = (dx * dx + dy * dy).sqrt();
                                        if len > f64::EPSILON {
                                            self.apply_copy(Vec2::new(
                                                base.x + dx / len * dist,
                                                base.y + dy / len * dist,
                                            ));
                                        }
                                    }
                                } else {
                                    self.command_log.push("  *Invalid coordinate*".to_string());
                                }
                                true
                            // ROTATE angle entry.
                            } else if self.rotate_phase == RotatePhase::BasePoint {
                                if let Some(world) = Self::resolve_typed_point(cmd.trim(), None) {
                                    self.rotate_base_point = Some(world);
                                    self.rotate_phase = RotatePhase::Rotation;
                                    self.command_log.push("ROTATE: Specify angle (degrees) or click".to_string());
                                } else {
                                    self.command_log.push("  *Invalid coordinate*".to_string());
                                }
                                true
                            } else if self.rotate_phase == RotatePhase::Rotation {
                                if let Ok(deg) = cmd.trim().parse::<f64>() {
                                    self.apply_rotate(deg.to_radians());
                                } else {
                                    self.command_log.push("  *Invalid angle*".to_string());
                                }
                                true
                            // DIMLINEAR coordinate entry.
                            } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                if matches!(self.dim_phase, DimPhase::FirstPoint) {
                                    if let Some(world) = Self::resolve_typed_point(cmd.trim(), None) {
                                        self.dim_phase = DimPhase::SecondPoint { first: world };
                                        self.command_log.push(format!("DIMLINEAR: First point ({:.4}, {:.4})", world.x, world.y));
                                    } else {
                                        self.command_log.push("  *Invalid coordinate*".to_string());
                                    }
                                } else if let DimPhase::SecondPoint { first } = self.dim_phase {
                                    if let Some(world) = Self::resolve_typed_point(cmd.trim(), None) {
                                        self.dim_phase = DimPhase::Placing { first, second: world };
                                        self.command_log.push(format!("DIMLINEAR: Second point ({:.4}, {:.4})", world.x, world.y));
                                    } else {
                                        self.command_log.push("  *Invalid coordinate*".to_string());
                                    }
                                } else if let DimPhase::Placing { first, second } = self.dim_phase {
                                    // Can type a number to set offset distance, or x,y for exact placement.
                                    if let Some(world) = Self::resolve_typed_point(cmd.trim(), None) {
                                        self.place_dim_linear(first, second, world);
                                    } else {
                                        self.command_log.push("  *Invalid coordinate*".to_string());
                                    }
                                }
                                true
                            // OFFSET distance entry intercepts before generic input handlers.
                            } else if self.offset_phase == OffsetPhase::EnteringDistance {
                                match cmd.trim().parse::<f64>() {
                                    Ok(d) if d > 0.0 => {
                                        self.offset_distance = Some(d);
                                        self.offset_phase = OffsetPhase::SelectingEntity;
                                        self.command_log.push("OFFSET: Select entity to offset".to_string());
                                    }
                                    _ => {
                                        self.command_log.push("OFFSET: Invalid distance".to_string());
                                    }
                                }
                                true
                            } else {
                                false
                            };
                            if !handled && !self.apply_typed_point_input(&cmd) && !self.execute_command_alias(&cmd) {
                                self.command_log.push("  *Unknown command*".to_string());
                                log::warn!("Unknown command/input: {}", cmd.trim());
                            }
                            if self.command_log.len() > 200 {
                                self.command_log.drain(0..50);
                            }
                        }
                        self.command_input.clear();
                        response.request_focus();
                    }
                });
            });
        
        // Central panel - viewport
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_size();
            let width = available.x.max(1.0) as u32;
            let height = available.y.max(1.0) as u32;
            self.hover_world_pos = None;
            self.snap_intersection_point = None;

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
                        Self::draw_grid_overlay(ui, response.rect, viewport);
                        self.draw_selected_entities_overlay(ui, response.rect, viewport);
                        self.draw_arc_input_ticks(ui, response.rect, viewport);
                        self.draw_trim_overlay(ui, response.rect, viewport);
                        self.draw_offset_overlay(ui, response.rect, viewport);
                        self.draw_extend_overlay(ui, response.rect, viewport);
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
                                    if self.from_phase == FromPhase::WaitingBase || self.from_phase == FromPhase::WaitingOffset {
                                        // FROM base/offset pick in idle mode — same snap as MOVE.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|p| p.world).unwrap_or_else(|| {
                                            if self.snap_enabled { Self::snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
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
                                            self.exit_from();
                                            self.deliver_point(result);
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
                                            Ok((eid, is_start, new_pt)) => {
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
                                        let mut world = pick.as_ref().map(|p| p.world).unwrap_or_else(|| {
                                            if self.snap_enabled { Self::snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
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
                                        let mut world = pick.as_ref().map(|p| p.world).unwrap_or_else(|| {
                                            if self.snap_enabled { Self::snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
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
                                        let mut world = pick.as_ref().map(|p| p.world).unwrap_or_else(|| {
                                            if self.snap_enabled { Self::snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
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
                                    } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                        // DIMLINEAR point pick — same snap logic as MOVE/COPY/ROTATE.
                                        let local = click_pos - response.rect.min;
                                        let raw_world = screen_to_world(local.x, local.y, viewport);
                                        let pick = self.pick_entity_point(viewport, response.rect, click_pos);
                                        let mut world = pick.as_ref().map(|p| p.world).unwrap_or_else(|| {
                                            if self.snap_enabled { Self::snap_to_grid(raw_world) } else { raw_world }
                                        });
                                        if pick.is_none() {
                                            if let Some(snap_pt) = self.snap_intersection_point {
                                                world = snap_pt;
                                            }
                                        }
                                        if matches!(self.dim_phase, DimPhase::FirstPoint) {
                                            self.dim_phase = DimPhase::SecondPoint { first: world };
                                            self.command_log.push(format!("DIMLINEAR: First point ({:.4}, {:.4})", world.x, world.y));
                                        } else if let DimPhase::SecondPoint { first } = self.dim_phase {
                                            self.dim_phase = DimPhase::Placing { first, second: world };
                                            self.command_log.push(format!("DIMLINEAR: Second point ({:.4}, {:.4})", world.x, world.y));
                                        } else if let DimPhase::Placing { first, second } = self.dim_phase {
                                            self.place_dim_linear(first, second, world);
                                        }
                                    } else {
                                        match self.offset_phase {
                                            OffsetPhase::SelectingEntity => {
                                                match self.entity_at_screen_pos(viewport, response.rect, click_pos) {
                                                    Some(id) => {
                                                        self.offset_selected_entity = Some(id);
                                                        self.offset_phase = OffsetPhase::SelectingSide;
                                                        self.command_log.push("OFFSET: Click side to offset toward".to_string());
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
                                                if id.is_some() || !shift {
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
                                    && matches!(self.dim_phase, DimPhase::Idle);
                                if allow {
                                    if let Some(pos) = response.interact_pointer_pos() {
                                        self.selection_drag_start = Some(pos);
                                        self.selection_drag_current = Some(pos);
                                    }
                                }
                            }
                        }

                        // Right-click cancels the current command or tool.
                        if response.clicked_by(egui::PointerButton::Secondary) {
                            if self.from_phase != FromPhase::Idle {
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
                            } else if !matches!(self.extend_phase, ExtendPhase::Idle) {
                                self.exit_extend();
                                self.command_log.push("*Cancel*".to_string());
                            } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                self.exit_dim();
                                self.command_log.push("*Cancel*".to_string());
                            }
                        }

                        if response.dragged_by(egui::PointerButton::Primary) {
                            if let Some(pos) = response.interact_pointer_pos() {
                                self.selection_drag_current = Some(pos);
                            }
                        }

                        if response.drag_stopped_by(egui::PointerButton::Primary) {
                            if let (Some(start), Some(end)) = (
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
                            let local = pointer_pos - response.rect.min;
                            let raw_world = screen_to_world(local.x, local.y, viewport);
                            let hover_pick = if self.snap_enabled {
                                self.pick_entity_point(viewport, response.rect, pointer_pos)
                            } else {
                                None
                            };
                            let mut world = hover_pick
                                .as_ref()
                                .map(|p| p.world)
                                .unwrap_or_else(|| {
                                    if self.snap_enabled {
                                        Self::snap_to_grid(raw_world)
                                    } else {
                                        raw_world
                                    }
                                });

                            // Apply tool-specific snapping when no point was explicitly picked.
                            if hover_pick.is_none() {
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

                            // Intersection snap: overrides grid/ortho snap, lower priority than point snap.
                            if hover_pick.is_none() && self.snap_enabled {
                                let isect_snap = self.find_intersection_snap(
                                    viewport,
                                    response.rect,
                                    pointer_pos,
                                );
                                if let Some(snap_pt) = isect_snap {
                                    world = snap_pt;
                                    self.snap_intersection_point = Some(snap_pt);
                                }
                            }

                            self.hover_world_pos = Some(world);

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

                            // MOVE / COPY / ROTATE / DIMLINEAR ghost preview.
                            self.draw_move_preview(ui, response.rect, viewport, world);
                            self.draw_copy_preview(ui, response.rect, viewport, world);
                            self.draw_rotate_preview(ui, response.rect, viewport, world);
                            self.draw_dim_preview(ui, response.rect, viewport, world);

                            // Grid-snap dot (suppress when intersection snap is active).
                            if self.snap_enabled
                                && hover_pick.is_none()
                                && self.snap_intersection_point.is_none()
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

                            // Intersection snap X marker (cyan).
                            if let Some(snap_pt) = self.snap_intersection_point {
                                Self::draw_tick_marker(
                                    ui,
                                    response.rect,
                                    viewport,
                                    snap_pt,
                                    egui::Color32::from_rgb(0, 230, 230),
                                );
                            }

                            ctx.request_repaint();
                        }

                        // Hover highlight for selectable line points (both idle and while drawing).
                        if let (Some(pointer_pos), Some(viewport)) =
                            (ui.input(|i| i.pointer.hover_pos()), self.viewport.as_ref())
                        {
                            if self.snap_enabled || matches!(self.active_tool, ActiveTool::None) {
                                if let Some(candidate) =
                                    self.pick_entity_point(viewport, response.rect, pointer_pos)
                                {
                                    let (sx, sy) = world_to_screen(
                                        candidate.world.x as f32,
                                        candidate.world.y as f32,
                                        viewport,
                                    );
                                    let pos = response.rect.min + egui::vec2(sx, sy);
                                    let painter = ui.painter_at(response.rect);
                                    painter.circle_filled(
                                        pos,
                                        6.0,
                                        egui::Color32::from_rgb(255, 200, 40),
                                    );
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
                                        .map(|p| p.world)
                                        .unwrap_or_else(|| {
                                            if self.snap_enabled {
                                                Self::snap_to_grid(raw_world)
                                            } else {
                                                raw_world
                                            }
                                        });

                                    // Apply tool snapping if no pick override.
                                    if pick.is_none() {
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
                                // Apply intersection snap if no point-snap pick.
                                if pick.is_none() {
                                    if let Some(snap_pt) = self.snap_intersection_point {
                                        world = snap_pt;
                                    }
                                }

                                // Update snap marker when a point pick happens during drawing.
                                if let Some(p) = pick {
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
                                    self.exit_from();
                                    self.deliver_point(result);
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

            if let Some(viewport) = &self.viewport {
                let zoom = viewport.zoom;
                let pan_x = viewport.pan_x;
                let pan_y = viewport.pan_y;
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.snap_enabled, "Snap (12\")");
                    ui.separator();
                    ui.label(format!(
                        "Zoom: {:.2}x  Pan: ({:.2}, {:.2})",
                        zoom, pan_x, pan_y
                    ));
                    if let Some(world) = self.hover_world_pos {
                        ui.separator();
                        ui.label(format!("X: {:.3}  Y: {:.3}", world.x, world.y));
                    }
                    ui.separator();
                    ui.checkbox(&mut self.ortho_enabled, "Ortho");
                    ui.add(
                        egui::DragValue::new(&mut self.ortho_increment_deg)
                            .clamp_range(0.1..=360.0)
                            .speed(1.0)
                            .suffix("°"),
                    );
                    for preset in [90.0, 45.0, 22.5] {
                        if ui.small_button(format!("{:.1}°", preset)).clicked() {
                            self.ortho_increment_deg = preset;
                            self.ortho_enabled = true;
                        }
                    }
                    if let ActiveTool::Circle { center: Some(_) } = self.active_tool {
                        ui.separator();
                        ui.checkbox(&mut self.circle_use_diameter, "⌀ Diameter");
                    }
                    ui.separator();
                    ui.label(match &self.active_tool {
                        ActiveTool::None => "Tool: none".to_string(),
                        ActiveTool::Line { start: None } => "Tool: line (pick start)".to_string(),
                        ActiveTool::Line { start: Some(s) } => format!(
                            "Tool: line (start at {:.3}, {:.3})",
                            s.x, s.y
                        ),
                        ActiveTool::Circle { center: None } => "Tool: circle (pick center)".to_string(),
                        ActiveTool::Circle { center: Some(c) } => format!(
                            "Tool: circle (center at {:.3}, {:.3})",
                            c.x, c.y
                        ),
                        ActiveTool::Arc { start: None, .. } => "Tool: arc (pick start)".to_string(),
                        ActiveTool::Arc { start: Some(s), mid: None } => format!(
                            "Tool: arc (start {:.3}, {:.3}, pick mid)",
                            s.x, s.y
                        ),
                        ActiveTool::Arc { start: Some(s), mid: Some(m) } => format!(
                            "Tool: arc (start {:.3}, {:.3}, mid {:.3}, {:.3}, pick end)",
                            s.x, s.y, m.x, m.y
                        ),
                        ActiveTool::Polyline { points } => {
                            match points.len() {
                                0 => "Tool: polyline (pick start)".to_string(),
                                1 => format!(
                                    "Tool: polyline (start {:.3}, {:.3}, pick next)",
                                    points[0].x, points[0].y
                                ),
                                n => format!("Tool: polyline ({} pts, pick next / right-click to finish / 'C' to close)", n),
                            }
                        }
                    });
                    if !self.selected_entities.is_empty() {
                        ui.separator();
                        ui.label(format!("Selected: {}", self.selected_entities.len()));
                    }
                });
            }
        });
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
            EntityKind::DimLinear { start, end, offset, .. } => {
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

    /// Pick nearest entity point (line endpoints/midpoint, arc endpoints/mid-angle, circle center) in screen space.
    fn pick_entity_point(
        &self,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
    ) -> Option<Selection> {
        let mut best: Option<(f32, Selection)> = None;

        for entity in self.drawing.visible_entities() {
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
                EntityKind::DimLinear { start, end, .. } => {
                    let s: Vec2 = (*start).into();
                    let e: Vec2 = (*end).into();
                    let mid = Vec2::new((s.x + e.x) * 0.5, (s.y + e.y) * 0.5);
                    self.push_pick_candidates(
                        &mut best, viewport, rect, screen_pos, entity.id,
                        &[("dim start", s), ("dim end", e), ("dim mid", mid)],
                    );
                }
            }
        }

        best.map(|(_, sel)| sel)
    }

    fn push_pick_candidates(
        &self,
        best: &mut Option<(f32, Selection)>,
        viewport: &Viewport,
        rect: egui::Rect,
        screen_pos: egui::Pos2,
        entity: Guid,
        candidates: &[(&'static str, Vec2)],
    ) {
        for (_label, world) in candidates {
            let (sx, sy) = world_to_screen(world.x as f32, world.y as f32, viewport);
            let pos = rect.min + egui::vec2(sx, sy);
            let dist = pos.distance(screen_pos);
            if dist <= Self::PICK_RADIUS {
                match best {
                    Some((best_dist, _)) if dist >= *best_dist => {}
                    _ => {
                        *best = Some((
                            dist,
                            Selection {
                                entity,
                                world: *world,
                            },
                        ));
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
            EntityKind::DimLinear { .. } => {
                return TrimResult::Fail(
                    "TRIM: Cannot trim dimension entities".to_string(),
                );
            }
        };

        TrimResult::Apply { target_id, new_entities }
    }

    fn finalize_polyline(&mut self, closed: bool) {
        if let ActiveTool::Polyline { points } = &mut self.active_tool {
            if points.len() >= 2 {
                let verts: Vec<cadkit_types::Vec3> =
                    points.iter().map(|p| (*p).into()).collect();
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
                    points.len(),
                    closed
                );
            } else {
                log::info!("Polyline not created (need at least 2 points)");
            }
            points.clear();
        }
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

    let end_angle = ang_start + sweep;

    Some(create_arc(center, r, ang_start, end_angle))
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
