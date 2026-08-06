use super::*;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct QaAutomationBridge {
    root_dir: PathBuf,
    commands_dir: PathBuf,
    processed_dir: PathBuf,
    results_dir: PathBuf,
    state_path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum QaAutomationCommand {
    Status,
    RunCommand { text: String },
    Ortho { enabled: Option<bool> },
    Polar { enabled: Option<bool> },
    PolarAngle { degrees: f64 },
    HoverWorldSnapped { x: f64, y: f64 },
    ClickWorldSnapped { x: f64, y: f64, additive: Option<bool> },
    ClickWorld { x: f64, y: f64, additive: Option<bool> },
    DragSelectWorld {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        shift: Option<bool>,
    },
    Escape,
    Delete,
    Undo,
    Redo,
    Zoom { delta: f32 },
    PanScreen { dx: f32, dy: f32 },
    Recovery { decision: QaRecoveryDecision },
    Quit,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum QaRecoveryDecision {
    Restore,
    Discard,
    Later,
}

#[derive(Serialize)]
struct QaCommandResult {
    ok: bool,
    message: String,
    processed_at_unix_ms: u128,
    state_path: String,
}

#[derive(Serialize)]
struct QaStateSnapshot {
    enabled: bool,
    root_dir: String,
    generated_at_unix_ms: u128,
    recovery_prompt_open: bool,
    active_tool: String,
    point_mode: String,
    selected_count: usize,
    selected_entity_ids: Vec<String>,
    entity_count: usize,
    entity_kinds: Vec<QaEntitySummary>,
    command_input: String,
    command_log_tail: Vec<String>,
    current_layer: u32,
    snap_enabled: bool,
    ortho_enabled: bool,
    polar_enabled: bool,
    polar_angle_deg: f64,
    grid_visible: bool,
    hover_world: Option<[f64; 2]>,
    hover_snap_kind: Option<String>,
    viewport: Option<QaViewportSnapshot>,
}

#[derive(Serialize)]
struct QaEntitySummary {
    id: String,
    kind: String,
    layer: u32,
}

#[derive(Serialize)]
struct QaViewportSnapshot {
    width: u32,
    height: u32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
}

impl QaAutomationBridge {
    pub(super) fn from_env() -> Option<Self> {
        let raw = match env::var("CADKIT_QA_DIR") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => return None,
        };

        match Self::new(PathBuf::from(raw)) {
            Ok(bridge) => Some(bridge),
            Err(err) => {
                eprintln!("CadKit QA automation disabled: {err}");
                None
            }
        }
    }

    fn new(root_dir: PathBuf) -> Result<Self, String> {
        let commands_dir = root_dir.join("commands");
        let processed_dir = root_dir.join("processed");
        let results_dir = root_dir.join("results");
        fs::create_dir_all(&commands_dir)
            .map_err(|e| format!("failed to create commands dir: {e}"))?;
        fs::create_dir_all(&processed_dir)
            .map_err(|e| format!("failed to create processed dir: {e}"))?;
        fs::create_dir_all(&results_dir)
            .map_err(|e| format!("failed to create results dir: {e}"))?;
        Ok(Self {
            state_path: root_dir.join("state.json"),
            root_dir,
            commands_dir,
            processed_dir,
            results_dir,
        })
    }

    fn pending_command_files(&self) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = match fs::read_dir(&self.commands_dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
                .collect(),
            Err(_) => Vec::new(),
        };
        files.sort();
        files
    }

    fn read_command(&self, path: &Path) -> Result<QaAutomationCommand, String> {
        let raw = fs::read_to_string(path)
            .map_err(|e| format!("failed to read command {}: {e}", path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|e| format!("failed to parse command {}: {e}", path.display()))
    }

    fn result_path_for(&self, command_path: &Path) -> PathBuf {
        let stem = command_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("command");
        self.results_dir.join(format!("{stem}.result.json"))
    }

    fn archive_path_for(&self, command_path: &Path) -> PathBuf {
        self.processed_dir.join(
            command_path
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("command.json")),
        )
    }

    fn write_result(&self, command_path: &Path, ok: bool, message: String) {
        let payload = QaCommandResult {
            ok,
            message,
            processed_at_unix_ms: unix_time_ms(),
            state_path: self.state_path.display().to_string(),
        };
        if let Ok(text) = serde_json::to_string_pretty(&payload) {
            let _ = fs::write(self.result_path_for(command_path), text + "\n");
        }
    }

    fn archive_command(&self, command_path: &Path) {
        let archive = self.archive_path_for(command_path);
        let _ = fs::rename(command_path, archive);
    }

    fn write_state(&self, snapshot: &QaStateSnapshot) {
        if let Ok(text) = serde_json::to_string_pretty(snapshot) {
            let _ = fs::write(&self.state_path, text + "\n");
        }
    }
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

impl CadKitApp {
    pub(super) fn process_qa_automation(&mut self, ctx: &egui::Context) {
        let pending = match self.qa_automation.as_ref() {
            Some(bridge) => bridge.pending_command_files(),
            None => return,
        };
        if pending.is_empty() {
            return;
        }

        for path in pending {
            let command = match self.qa_automation.as_ref().unwrap().read_command(&path) {
                Ok(command) => command,
                Err(err) => {
                    self.qa_automation.as_ref().unwrap().write_result(&path, false, err);
                    self.qa_automation.as_ref().unwrap().archive_command(&path);
                    continue;
                }
            };

            let result = self.execute_qa_automation_command(ctx, command);
            match result {
                Ok(message) => self
                    .qa_automation
                    .as_ref()
                    .unwrap()
                    .write_result(&path, true, message),
                Err(err) => self
                    .qa_automation
                    .as_ref()
                    .unwrap()
                    .write_result(&path, false, err),
            }
            self.qa_automation.as_ref().unwrap().archive_command(&path);
            self.publish_qa_automation_state();
        }

        ctx.request_repaint();
    }

    pub(super) fn publish_qa_automation_state(&self) {
        let Some(bridge) = self.qa_automation.as_ref() else {
            return;
        };
        bridge.write_state(&self.qa_state_snapshot());
    }

    fn execute_qa_automation_command(
        &mut self,
        ctx: &egui::Context,
        command: QaAutomationCommand,
    ) -> Result<String, String> {
        match command {
            QaAutomationCommand::Status => Ok("State snapshot refreshed".to_string()),
            QaAutomationCommand::RunCommand { text } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before other actions"
                            .to_string(),
                    );
                }
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    return Err("Command text must not be empty".to_string());
                }
                let handled = self.execute_command_alias(trimmed);
                if handled {
                    Ok(format!("Executed command alias: {trimmed}"))
                } else {
                    Err(format!("Command was not recognized: {trimmed}"))
                }
            }
            QaAutomationCommand::Ortho { enabled } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before other actions"
                            .to_string(),
                    );
                }
                let state = match enabled {
                    Some(enabled) => self.set_ortho_enabled(enabled),
                    None => self.toggle_ortho_enabled(),
                };
                Ok(format!(
                    "Ortho {}",
                    if state { "ON" } else { "OFF" }
                ))
            }
            QaAutomationCommand::Polar { enabled } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before other actions"
                            .to_string(),
                    );
                }
                let state = match enabled {
                    Some(enabled) => self.set_polar_enabled(enabled),
                    None => self.toggle_polar_enabled(),
                };
                Ok(format!(
                    "Polar {}",
                    if state { "ON" } else { "OFF" }
                ))
            }
            QaAutomationCommand::PolarAngle { degrees } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before other actions"
                            .to_string(),
                    );
                }
                let value = self.set_polar_angle_deg(degrees);
                Ok(format!("Polar angle {:.1}°", value))
            }
            QaAutomationCommand::HoverWorldSnapped { x, y } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before hover actions"
                            .to_string(),
                    );
                }
                self.qa_hover_world_snapped(Vec2::new(x, y))
            }
            QaAutomationCommand::ClickWorldSnapped { x, y, additive } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before viewport clicks"
                            .to_string(),
                    );
                }
                self.qa_click_world_snapped(Vec2::new(x, y), additive.unwrap_or(false))
            }
            QaAutomationCommand::ClickWorld { x, y, additive } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before viewport clicks"
                            .to_string(),
                    );
                }
                self.qa_click_world(Vec2::new(x, y), additive.unwrap_or(false))
            }
            QaAutomationCommand::DragSelectWorld {
                x0,
                y0,
                x1,
                y1,
                shift,
            } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before drag selection"
                            .to_string(),
                    );
                }
                self.qa_drag_select_world(
                    Vec2::new(x0, y0),
                    Vec2::new(x1, y1),
                    shift.unwrap_or(false),
                )
            }
            QaAutomationCommand::Escape => {
                if self.recovery_prompt_open {
                    self.command_log.push("Recovery: Deferred for this session".to_string());
                    self.recovery_prompt_open = false;
                    Ok("Dismissed recovery prompt using Escape semantics".to_string())
                } else {
                    self.handle_escape_action();
                    Ok("Escape processed".to_string())
                }
            }
            QaAutomationCommand::Delete => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before Delete"
                            .to_string(),
                    );
                }
                self.delete_selected_via_keyboard();
                Ok("Delete processed".to_string())
            }
            QaAutomationCommand::Undo => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before Undo"
                            .to_string(),
                    );
                }
                self.undo();
                Ok("Undo processed".to_string())
            }
            QaAutomationCommand::Redo => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before Redo"
                            .to_string(),
                    );
                }
                self.redo();
                Ok("Redo processed".to_string())
            }
            QaAutomationCommand::Zoom { delta } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before Zoom"
                            .to_string(),
                    );
                }
                let Some(viewport) = self.viewport.as_mut() else {
                    return Err("Viewport is not initialized".to_string());
                };
                viewport.zoom_delta(delta);
                Ok(format!("Zoom delta applied: {delta:.4}"))
            }
            QaAutomationCommand::PanScreen { dx, dy } => {
                if self.recovery_prompt_open {
                    return Err(
                        "Recovery prompt is open; send a recovery command before Pan"
                            .to_string(),
                    );
                }
                let Some(viewport) = self.viewport.as_mut() else {
                    return Err("Viewport is not initialized".to_string());
                };
                viewport.pan(-dx * Self::PAN_SENSITIVITY, dy * Self::PAN_SENSITIVITY);
                Ok(format!("Pan applied from screen delta ({dx:.2}, {dy:.2})"))
            }
            QaAutomationCommand::Recovery { decision } => match decision {
                QaRecoveryDecision::Restore => {
                    if self.recovery_prompt_open {
                        self.restore_recovery_snapshot(ctx);
                        self.recovery_prompt_open = false;
                        Ok("Recovery snapshot restored".to_string())
                    } else {
                        Err("Recovery prompt is not currently open".to_string())
                    }
                }
                QaRecoveryDecision::Discard => {
                    if self.recovery_prompt_open {
                        self.delete_recovery_snapshot();
                        self.command_log
                            .push("Recovery: Snapshot discarded".to_string());
                        self.recovery_prompt_open = false;
                        Ok("Recovery snapshot discarded".to_string())
                    } else {
                        Err("Recovery prompt is not currently open".to_string())
                    }
                }
                QaRecoveryDecision::Later => {
                    if self.recovery_prompt_open {
                        self.command_log
                            .push("Recovery: Deferred for this session".to_string());
                        self.recovery_prompt_open = false;
                        Ok("Recovery prompt deferred".to_string())
                    } else {
                        Err("Recovery prompt is not currently open".to_string())
                    }
                }
            },
            QaAutomationCommand::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                Ok("Close command sent".to_string())
            }
        }
    }

    fn qa_click_world(&mut self, world: Vec2, additive: bool) -> Result<String, String> {
        self.hover_world_pos = Some(world);
        self.last_hover_world_pos = Some(world);

        if self.qa_uses_point_delivery() {
            self.deliver_point(world);
            return Ok(format!("Delivered point at ({:.4}, {:.4})", world.x, world.y));
        }

        if self.qa_requires_unsupported_click_mode() {
            return Err(
                "Automation click is not implemented for the current interaction mode"
                    .to_string(),
            );
        }

        let id = self.qa_entity_at_world(world);
        self.selection = None;
        self.select_entity_id(id, additive);
        match id {
            Some(id) => Ok(format!("Selected entity {id}")),
            None => Ok("Cleared selection by clicking empty space".to_string()),
        }
    }

    fn qa_hover_world_snapped(&mut self, approx_world: Vec2) -> Result<String, String> {
        let (world, snap_kind) = self.qa_resolve_hover_world_from_approx_world(approx_world)?;
        let snap_label = snap_kind
            .map(Self::qa_snap_kind_name)
            .unwrap_or("none");
        Ok(format!(
            "Resolved hover to ({:.4}, {:.4}) with snap {}",
            world.x, world.y, snap_label
        ))
    }

    fn qa_click_world_snapped(
        &mut self,
        approx_world: Vec2,
        additive: bool,
    ) -> Result<String, String> {
        let (world, snap_kind) = self.qa_resolve_hover_world_from_approx_world(approx_world)?;
        if self.qa_uses_point_delivery() {
            self.deliver_point(world);
            return Ok(format!(
                "Delivered snapped point at ({:.4}, {:.4}) with snap {}",
                world.x,
                world.y,
                snap_kind.map(Self::qa_snap_kind_name).unwrap_or("none")
            ));
        }
        self.qa_click_world(world, additive)
    }

    fn qa_drag_select_world(
        &mut self,
        start: Vec2,
        end: Vec2,
        shift: bool,
    ) -> Result<String, String> {
        if self.qa_requires_unsupported_click_mode() || self.qa_uses_point_delivery() {
            return Err(
                "Automation drag selection is only available from idle selection mode"
                    .to_string(),
            );
        }

        let min_x = start.x.min(end.x);
        let min_y = start.y.min(end.y);
        let max_x = start.x.max(end.x);
        let max_y = start.y.max(end.y);
        let window_mode = end.x >= start.x;
        let hits = self.entities_in_world_box(min_x, min_y, max_x, max_y, window_mode);
        if !shift {
            self.selected_entities.clear();
        }
        for id in hits {
            let group = self.expand_assoc_group_ids(id);
            let any_selected = group.iter().any(|gid| self.selected_entities.contains(gid));
            if shift && any_selected {
                for gid in group {
                    self.selected_entities.remove(&gid);
                }
            } else {
                for gid in group {
                    self.selected_entities.insert(gid);
                }
            }
        }
        self.selection = None;
        Ok(format!(
            "Drag selection applied; selected count is now {}",
            self.selected_entities.len()
        ))
    }

    fn qa_entity_at_world(&self, world: Vec2) -> Option<Guid> {
        let viewport = self.viewport.as_ref()?;
        let rect = self.qa_viewport_rect(viewport);
        let (sx, sy) = world_to_screen(world.x as f32, world.y as f32, viewport);
        let pos = rect.min + egui::vec2(sx, sy);
        self.entity_at_screen_pos(viewport, rect, pos)
    }

    fn qa_viewport_rect(&self, viewport: &Viewport) -> egui::Rect {
        let (width, height) = viewport.size();
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32))
    }

    fn qa_resolve_hover_world_from_approx_world(
        &mut self,
        approx_world: Vec2,
    ) -> Result<(Vec2, Option<SnapKind>), String> {
        let Some(viewport) = self.viewport.as_ref() else {
            return Err("Viewport is not initialized".to_string());
        };
        let rect = self.qa_viewport_rect(viewport);
        let (sx, sy) = world_to_screen(approx_world.x as f32, approx_world.y as f32, viewport);
        let screen_pos = rect.min + egui::vec2(sx, sy);

        self.snap_intersection_point = None;
        self.hover_snap_kind = None;

        let local = screen_pos - rect.min;
        let raw_world = screen_to_world(local.x, local.y, viewport);
        let ortho_base = if self.dim_grip_drag.is_none() && matches!(self.from_phase, FromPhase::Idle)
        {
            self.ortho_snap_base_point()
        } else {
            None
        };

        let (hover_pick, mut world) = if let Some(base) = ortho_base {
            let (w, kind) =
                self.resolve_ortho_snap_for_line_like(viewport, rect, screen_pos, base, raw_world);
            self.hover_snap_kind = kind;
            if kind == Some(SnapKind::Intersection) {
                self.snap_intersection_point = Some(w);
            }
            (None, w)
        } else if let Some((pick, kind)) = self.pick_entity_point(viewport, rect, screen_pos) {
            self.hover_snap_kind = Some(kind);
            (Some(pick.clone()), pick.world)
        } else {
            (None, raw_world)
        };

        if hover_pick.is_none()
            && self.dim_grip_drag.is_none()
            && matches!(self.from_phase, FromPhase::Idle)
        {
            match &self.active_tool {
                ActiveTool::Line { start: Some(s) } => {
                    if self.directional_snap_enabled() && ortho_base.is_none() {
                        world = self.ortho_snap(*s, world);
                    }
                    if let Some(dist_world) =
                        Self::apply_distance_override(*s, world, &self.distance_input)
                    {
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
                        if self.directional_snap_enabled() && ortho_base.is_none() {
                            world = self.ortho_snap(*last, world);
                        }
                        if let Some(dist_world) =
                            Self::apply_distance_override(*last, world, &self.distance_input)
                        {
                            world = dist_world;
                        }
                    }
                }
                _ => {}
            }
        }

        if hover_pick.is_none()
            && self.dim_grip_drag.is_none()
            && ortho_base.is_none()
            && self.snap_enabled
            && self.snap_intersection
        {
            if let Some(snap_pt) = self.find_intersection_snap(viewport, rect, screen_pos) {
                world = snap_pt;
                self.snap_intersection_point = Some(snap_pt);
                self.hover_snap_kind = Some(SnapKind::Intersection);
            }
        }

        if hover_pick.is_none()
            && self.dim_grip_drag.is_none()
            && ortho_base.is_none()
            && self.snap_intersection_point.is_none()
            && self.hover_snap_kind.is_none()
            && self.snap_enabled
            && self.snap_perpendicular
        {
            if let Some(from_pt) = self.current_from_point() {
                if let Some(pt) = self.perpendicular_snap(viewport, rect, screen_pos, from_pt) {
                    world = pt;
                    self.hover_snap_kind = Some(SnapKind::Perpendicular);
                }
            }
        }

        if hover_pick.is_none()
            && self.dim_grip_drag.is_none()
            && ortho_base.is_none()
            && self.snap_intersection_point.is_none()
            && self.hover_snap_kind.is_none()
            && self.snap_enabled
            && (self.snap_parallel || self.snap_perpendicular)
        {
            if let Some(from_pt) = self.current_from_point() {
                if let Some((pt, kind)) =
                    self.tracking_snap(viewport, rect, screen_pos, from_pt)
                {
                    world = pt;
                    self.hover_snap_kind = Some(kind);
                }
            }
        }

        if hover_pick.is_none()
            && self.dim_grip_drag.is_none()
            && ortho_base.is_none()
            && self.snap_intersection_point.is_none()
            && self.hover_snap_kind.is_none()
            && self.snap_enabled
            && self.snap_tangent
        {
            if let Some(from_pt) = self.current_from_point() {
                if let Some(pt) = self.tangent_snap(viewport, rect, screen_pos, from_pt) {
                    world = pt;
                    self.hover_snap_kind = Some(SnapKind::Tangent);
                }
            }
        }

        if hover_pick.is_none()
            && self.dim_grip_drag.is_none()
            && ortho_base.is_none()
            && self.snap_intersection_point.is_none()
            && self.hover_snap_kind.is_none()
            && self.snap_enabled
            && self.snap_nearest
        {
            if let Some(pt) = self.nearest_entity_snap(viewport, rect, screen_pos) {
                world = pt;
                self.hover_snap_kind = Some(SnapKind::Nearest);
            }
        }

        self.hover_world_pos = Some(world);
        self.last_hover_world_pos = Some(world);
        Ok((world, self.hover_snap_kind))
    }

    fn qa_uses_point_delivery(&self) -> bool {
        !matches!(self.active_tool, ActiveTool::None)
            || !matches!(self.move_phase, MovePhase::Idle)
            || !matches!(self.stretch_phase, StretchPhase::Idle)
            || !matches!(self.copy_phase, CopyPhase::Idle)
            || !matches!(self.rotate_phase, RotatePhase::Idle)
            || !matches!(self.scale_phase, ScalePhase::Idle)
            || !matches!(self.mirror_phase, MirrorPhase::Idle)
            || !matches!(self.polygon_phase, PolygonPhase::Idle)
            || !matches!(self.isocircle_phase, IsocirclePhase::Idle)
            || !matches!(self.ellipse_phase, EllipsePhase::Idle)
            || !matches!(self.rectangle_phase, RectanglePhase::Idle)
            || !matches!(self.block_phase, BlockPhase::Idle)
            || !matches!(self.insert_phase, InsertPhase::Idle)
    }

    fn qa_requires_unsupported_click_mode(&self) -> bool {
        !matches!(self.trim_phase, TrimPhase::Idle)
            || !matches!(self.offset_phase, OffsetPhase::Idle)
            || !matches!(self.extend_phase, ExtendPhase::Idle)
            || !matches!(self.fillet_phase, FilletPhase::Idle)
            || !matches!(self.chamfer_phase, ChamferPhase::Idle)
            || !matches!(self.array_phase, ArrayPhase::Idle | ArrayPhase::SelectingEntities)
            || !matches!(self.pedit_phase, PeditPhase::Idle)
            || !matches!(self.boundary_phase, BoundaryPhase::Idle)
            || !matches!(self.hatch_phase, HatchPhase::Idle)
            || !matches!(self.isoextrude_phase, IsoExtrudePhase::Idle)
            || !matches!(self.dwiso_side_phase, DwIsoSidePhase::Idle)
            || self.has_active_dimension_command()
            || !matches!(self.text_phase, TextPhase::Idle)
            || self.text_edit_dialog.is_some()
            || !matches!(self.edit_text_phase, EditTextPhase::Idle)
            || self.dim_edit_dialog.is_some()
            || !matches!(self.edit_dim_phase, EditDimPhase::Idle)
            || self.dim_grip_drag.is_some()
    }

    fn qa_state_snapshot(&self) -> QaStateSnapshot {
        let viewport = self.viewport.as_ref().map(|viewport| {
            let (width, height) = viewport.size();
            QaViewportSnapshot {
                width,
                height,
                zoom: viewport.zoom,
                pan_x: viewport.pan_x,
                pan_y: viewport.pan_y,
            }
        });
        let entities: Vec<&Entity> = self.drawing.entities().collect();
        let entity_kinds = entities
            .iter()
            .map(|entity| QaEntitySummary {
                id: entity.id.to_string(),
                kind: Self::qa_entity_kind_name(&entity.kind).to_string(),
                layer: entity.layer,
            })
            .collect();
        QaStateSnapshot {
            enabled: true,
            root_dir: self
                .qa_automation
                .as_ref()
                .map(|bridge| bridge.root_dir.display().to_string())
                .unwrap_or_default(),
            generated_at_unix_ms: unix_time_ms(),
            recovery_prompt_open: self.recovery_prompt_open,
            active_tool: Self::qa_active_tool_name(&self.active_tool).to_string(),
            point_mode: self.qa_point_mode_name(),
            selected_count: self.selected_entities.len(),
            selected_entity_ids: self
                .selected_entities
                .iter()
                .map(|id| id.to_string())
                .collect(),
            entity_count: entities.len(),
            entity_kinds,
            command_input: self.command_input.clone(),
            command_log_tail: self
                .command_log
                .iter()
                .rev()
                .take(12)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            current_layer: self.current_layer,
            snap_enabled: self.snap_enabled,
            ortho_enabled: self.axis_ortho_enabled,
            polar_enabled: self.ortho_enabled,
            polar_angle_deg: self.ortho_increment_deg,
            grid_visible: self.grid_visible,
            hover_world: self
                .hover_world_pos
                .or(self.last_hover_world_pos)
                .map(|w| [w.x, w.y]),
            hover_snap_kind: self.hover_snap_kind.map(|kind| Self::qa_snap_kind_name(kind).to_string()),
            viewport,
        }
    }

    fn qa_active_tool_name(tool: &ActiveTool) -> &'static str {
        match tool {
            ActiveTool::None => "none",
            ActiveTool::Line { .. } => "line",
            ActiveTool::Circle { .. } => "circle",
            ActiveTool::Arc { .. } => "arc",
            ActiveTool::Polyline { .. } => "polyline",
        }
    }

    fn qa_point_mode_name(&self) -> String {
        if !matches!(self.active_tool, ActiveTool::None) {
            return format!("tool:{}", Self::qa_active_tool_name(&self.active_tool));
        }
        if !matches!(self.move_phase, MovePhase::Idle) {
            return format!("move:{:?}", self.move_phase);
        }
        if !matches!(self.copy_phase, CopyPhase::Idle) {
            return format!("copy:{:?}", self.copy_phase);
        }
        if !matches!(self.rotate_phase, RotatePhase::Idle) {
            return format!("rotate:{:?}", self.rotate_phase);
        }
        if !matches!(self.scale_phase, ScalePhase::Idle) {
            return format!("scale:{:?}", self.scale_phase);
        }
        if !matches!(self.mirror_phase, MirrorPhase::Idle) {
            return format!("mirror:{:?}", self.mirror_phase);
        }
        if !matches!(self.stretch_phase, StretchPhase::Idle) {
            return format!("stretch:{:?}", self.stretch_phase);
        }
        if !matches!(self.polygon_phase, PolygonPhase::Idle) {
            return format!("polygon:{:?}", self.polygon_phase);
        }
        if !matches!(self.ellipse_phase, EllipsePhase::Idle) {
            return format!("ellipse:{:?}", self.ellipse_phase);
        }
        if !matches!(self.rectangle_phase, RectanglePhase::Idle) {
            return format!("rectangle:{:?}", self.rectangle_phase);
        }
        if !matches!(self.insert_phase, InsertPhase::Idle) {
            return format!("insert:{:?}", self.insert_phase);
        }
        if !matches!(self.block_phase, BlockPhase::Idle) {
            return format!("block:{:?}", self.block_phase);
        }
        "idle_selection".to_string()
    }

    fn qa_entity_kind_name(kind: &EntityKind) -> &'static str {
        match kind {
            EntityKind::Line { .. } => "line",
            EntityKind::Arc { .. } => "arc",
            EntityKind::Circle { .. } => "circle",
            EntityKind::Polyline { .. } => "polyline",
            EntityKind::DimAligned { .. } => "dim_aligned",
            EntityKind::DimLinear { .. } => "dim_linear",
            EntityKind::DimAngular { .. } => "dim_angular",
            EntityKind::DimRadial { .. } => "dim_radial",
            EntityKind::Text { .. } => "text",
            EntityKind::Insert { .. } => "insert",
        }
    }

    fn qa_snap_kind_name(kind: SnapKind) -> &'static str {
        match kind {
            SnapKind::Endpoint => "endpoint",
            SnapKind::Midpoint => "midpoint",
            SnapKind::Center => "center",
            SnapKind::Quadrant => "quadrant",
            SnapKind::Intersection => "intersection",
            SnapKind::Parallel => "parallel",
            SnapKind::Nearest => "nearest",
            SnapKind::Perpendicular => "perpendicular",
            SnapKind::Tangent => "tangent",
        }
    }

    pub(super) fn handle_escape_action(&mut self) {
        if !self.command_input.is_empty() {
            self.command_input.clear();
        } else if self.from_phase != FromPhase::Idle {
            self.exit_from();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.trim_phase, TrimPhase::Idle) {
            self.exit_trim();
            self.command_log.push("*Cancel*".to_string());
        } else if matches!(self.offset_phase, OffsetPhase::PickingDistanceB { .. }) {
            self.offset_phase = OffsetPhase::EnteringDistance;
            self.command_log
                .push("OFFSET: Enter distance or click two points:".to_string());
        } else if !matches!(self.offset_phase, OffsetPhase::Idle) {
            self.exit_offset();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.move_phase, MovePhase::Idle) {
            self.exit_move();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.stretch_phase, StretchPhase::Idle) {
            self.exit_stretch();
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
        } else if !matches!(self.isocircle_phase, IsocirclePhase::Idle) {
            self.exit_isocircle();
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
        } else if !matches!(self.boundary_phase, BoundaryPhase::Idle) {
            self.exit_boundary();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.hatch_phase, HatchPhase::Idle) {
            self.exit_hatch();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.isoextrude_phase, IsoExtrudePhase::Idle) {
            self.exit_isoextrude();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.dwiso_side_phase, DwIsoSidePhase::Idle) {
            self.exit_dwiso_side();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.block_phase, BlockPhase::Idle) {
            self.exit_block();
            self.command_log.push("*Cancel*".to_string());
        } else if !matches!(self.insert_phase, InsertPhase::Idle) {
            self.exit_insert();
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
        } else if self.text_edit_dialog.is_some()
            || !matches!(self.edit_text_phase, EditTextPhase::Idle)
        {
            self.exit_edit_text();
            self.command_log.push("*Cancel*".to_string());
        } else if self.dim_edit_dialog.is_some()
            || !matches!(self.edit_dim_phase, EditDimPhase::Idle)
        {
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

    pub(super) fn delete_selected_via_keyboard(&mut self) {
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
}
