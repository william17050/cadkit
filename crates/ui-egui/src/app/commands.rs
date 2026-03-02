use super::state::{
    ActiveTool, CopyPhase, DimPhase, ExtendPhase, FromPhase, MovePhase, OffsetPhase, RotatePhase,
    TrimPhase,
};
use super::{create_arc_from_three_points, CadKitApp};
use cadkit_2d_core::{create_circle, create_line};
use cadkit_types::Vec2;

impl CadKitApp {
    /// Execute a command-line alias similar to classic CAD workflows.
    pub(crate) fn execute_command_alias(&mut self, raw: &str) -> bool {
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
                self.command_log
                    .push("TRIM: Select cutting edges, press Enter to continue".to_string());
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
                self.command_log
                    .push("EXTEND: Select boundary edges, press Enter to continue".to_string());
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
                self.command_log
                    .push("MOVE: Select entities to move, press Enter to continue".to_string());
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
                self.command_log
                    .push("ROTATE: Select entities, press Enter to continue".to_string());
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
                self.command_log
                    .push("COPY: Select entities to copy, press Enter to continue".to_string());
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
                self.command_log
                    .push("LAYER: Use the layer panel on the right to manage layers".to_string());
                true
            }
            "from" | "fr" => {
                if self.is_picking_point() {
                    self.from_phase = FromPhase::WaitingBase;
                    self.from_base = None;
                    self.command_log
                        .push("FROM  Base point (snap to geometry):".to_string());
                } else {
                    self.command_log
                        .push("FROM: Not active during a point-pick step".to_string());
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
            "u" | "undo" => {
                self.undo();
                true
            }
            "r" | "redo" => {
                self.redo();
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
                self.command_log
                    .push("DIMLINEAR: Specify first extension line origin".to_string());
                log::info!("Command: DIMLINEAR");
                true
            }
            _ => false,
        }
    }

    pub(crate) fn tool_uses_distance_input(&self) -> bool {
        match &self.active_tool {
            ActiveTool::Line { start: Some(_) } => true,
            ActiveTool::Circle { center: Some(_) } => true,
            ActiveTool::Polyline { points } => !points.is_empty(),
            _ => false,
        }
    }

    pub(crate) fn apply_typed_point_input(&mut self, raw: &str) -> bool {
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
                    if dist <= f64::EPSILON {
                        return false;
                    }
                    let dx = hover.x - b.x;
                    let dy = hover.y - b.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= f64::EPSILON {
                        return false;
                    }
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
                    self.command_log
                        .push(format!("  Start: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Line start set at ({:.3}, {:.3})", world.x, world.y);
                } else if let Some(s) = start.take() {
                    let mut line = create_line(s, world);
                    line.layer = self.current_layer;
                    self.drawing.add_entity(line);
                    *start = Some(world);
                    self.distance_input.clear();
                    self.command_log
                        .push(format!("  End: {:.4}, {:.4}", world.x, world.y));
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
                    if val <= f64::EPSILON {
                        return false;
                    }
                    let desired_r = if self.circle_use_diameter { val * 0.5 } else { val };
                    let hover =
                        self.hover_world_pos
                            .unwrap_or(Vec2::new(c.x + desired_r, c.y));
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
                    self.command_log
                        .push(format!("  Center: {:.4}, {:.4}", world.x, world.y));
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
                    self.command_log
                        .push(format!("  Start: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Arc start set at ({:.3}, {:.3})", world.x, world.y);
                } else if mid.is_none() {
                    *mid = Some(world);
                    self.command_log
                        .push(format!("  Mid: {:.4}, {:.4}", world.x, world.y));
                    log::info!("Arc mid set at ({:.3}, {:.3})", world.x, world.y);
                } else if let (Some(s), Some(m)) = (start.take(), mid.take()) {
                    if let Some(mut a) = create_arc_from_three_points(s, m, world) {
                        a.layer = self.current_layer;
                        self.drawing.add_entity(a);
                        self.command_log
                            .push(format!("  End: {:.4}, {:.4}", world.x, world.y));
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
                    if dist <= f64::EPSILON {
                        return false;
                    }
                    let dx = hover.x - b.x;
                    let dy = hover.y - b.y;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len <= f64::EPSILON {
                        return false;
                    }
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
                self.command_log
                    .push(format!("  Pt {}: {:.4}, {:.4}", points.len(), world.x, world.y));
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
}

impl CadKitApp {
    fn parse_xy(text: &str) -> Option<Vec2> {
        let (x, y) = text.split_once(',')?;
        let x = x.trim().parse::<f64>().ok()?;
        let y = y.trim().parse::<f64>().ok()?;
        Some(Vec2::new(x, y))
    }

    pub(crate) fn resolve_typed_point(text: &str, base: Option<Vec2>) -> Option<Vec2> {
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
}
