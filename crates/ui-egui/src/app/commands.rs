use super::state::{
    ActiveTool, CopyPhase, DimAngularPhase, DimLinearPhase, DimPhase, DimRadialPhase, EditDimPhase, EditTextPhase,
    ArrayMode, ArrayPhase, ChamferPhase, EllipsePhase, ExtendPhase, FilletPhase, FromPhase, MirrorPhase, MovePhase,
    OffsetPhase, PeditPhase, PolygonPhase, RectanglePhase, RotatePhase, ScalePhase, TextPhase, TrimPhase,
};
use super::{create_arc_from_three_points, AiBackendMode, AiModelProfile, CadKitApp};
use cadkit_2d_core::{create_circle, create_line};
use cadkit_types::Vec2;
use serde_json::json;
use std::path::PathBuf;

impl CadKitApp {
    /// Execute a command-line alias similar to classic CAD workflows.
    pub(crate) fn execute_command_alias(&mut self, raw: &str) -> bool {
        let raw_trimmed = raw.trim();
        let cmd = raw_trimmed.to_ascii_lowercase();
        if cmd.is_empty() {
            return false;
        }
        let mut words = cmd.split_whitespace();
        let head = words.next().unwrap_or("");
        let arg1 = words.next();

        // OSNAP / OSMODE aliases (AutoCAD-style).
        if head == "osnap" {
            match arg1 {
                None => {
                    self.snap_enabled = !self.snap_enabled;
                    self.command_log.push(format!(
                        "OSNAP: {}",
                        if self.snap_enabled { "ON" } else { "OFF" }
                    ));
                }
                Some("on") | Some("1") => {
                    self.snap_enabled = true;
                    self.command_log.push("OSNAP: ON".to_string());
                }
                Some("off") | Some("0") => {
                    self.snap_enabled = false;
                    self.command_log.push("OSNAP: OFF".to_string());
                }
                Some(_) => {
                    self.command_log
                        .push("OSNAP: Use OSNAP [ON/OFF]".to_string());
                }
            }
            return true;
        }
        if head == "osmode" {
            const OSMODE_ENDPOINT: u32 = 1;
            const OSMODE_MIDPOINT: u32 = 2;
            const OSMODE_CENTER: u32 = 4;
            const OSMODE_QUADRANT: u32 = 16;
            const OSMODE_INTERSECTION: u32 = 32;
            const OSMODE_PERPENDICULAR: u32 = 128;
            const OSMODE_TANGENT: u32 = 256;
            const OSMODE_NEAREST: u32 = 512;
            const OSMODE_PARALLEL: u32 = 8192;
            match arg1 {
                None => {
                    let mut value = 0u32;
                    if self.snap_endpoint { value |= OSMODE_ENDPOINT; }
                    if self.snap_midpoint { value |= OSMODE_MIDPOINT; }
                    if self.snap_center { value |= OSMODE_CENTER; }
                    if self.snap_quadrant { value |= OSMODE_QUADRANT; }
                    if self.snap_intersection { value |= OSMODE_INTERSECTION; }
                    if self.snap_perpendicular { value |= OSMODE_PERPENDICULAR; }
                    if self.snap_tangent { value |= OSMODE_TANGENT; }
                    if self.snap_nearest { value |= OSMODE_NEAREST; }
                    if self.snap_parallel { value |= OSMODE_PARALLEL; }
                    self.command_log.push(format!("OSMODE={value}"));
                }
                Some(v) => {
                    match v.parse::<i64>() {
                        Ok(n) if n >= 0 => {
                            let value = n as u32;
                            self.snap_endpoint = (value & OSMODE_ENDPOINT) != 0;
                            self.snap_midpoint = (value & OSMODE_MIDPOINT) != 0;
                            self.snap_center = (value & OSMODE_CENTER) != 0;
                            self.snap_quadrant = (value & OSMODE_QUADRANT) != 0;
                            self.snap_intersection = (value & OSMODE_INTERSECTION) != 0;
                            self.snap_perpendicular = (value & OSMODE_PERPENDICULAR) != 0;
                            self.snap_tangent = (value & OSMODE_TANGENT) != 0;
                            self.snap_nearest = (value & OSMODE_NEAREST) != 0;
                            self.snap_parallel = (value & OSMODE_PARALLEL) != 0;
                            self.snap_enabled = value != 0;
                            self.command_log.push(format!("OSMODE set to {value}"));
                        }
                        _ => {
                            self.command_log
                                .push("OSMODE: Enter a non-negative integer (example: 175)".to_string());
                        }
                    }
                }
            }
            return true;
        }
        if head == "ltscale" || head == "lts" {
            match arg1 {
                None => {
                    self.command_log
                        .push(format!("LTSCALE={:.4}", self.drawing.linetype_scale));
                }
                Some(v) => match v.parse::<f64>() {
                    Ok(n) if n.is_finite() && n > 0.0 => {
                        self.drawing.linetype_scale = n.clamp(0.01, 1000.0);
                        self.command_log.push(format!(
                            "LTSCALE set to {:.4}",
                            self.drawing.linetype_scale
                        ));
                    }
                    _ => {
                        self.command_log.push(
                            "LTSCALE: Enter a positive number (example: LTSCALE 10)".to_string(),
                        );
                    }
                },
            }
            return true;
        }

        let keeps_dim_context = matches!(
            cmd.as_str(),
            "dal" | "dimaligned" | "dim"
                | "dli" | "dimlinear"
                | "dang" | "dimangular"
                | "dra" | "dimradius"
                | "ddi" | "dimdiameter"
                | "from" | "fr"
        ) || self.from_phase != FromPhase::Idle;
        if !keeps_dim_context {
            self.exit_dim();
        }
        let keeps_scale_context =
            matches!(cmd.as_str(), "sc" | "scale" | "from" | "fr")
                || self.scale_phase != ScalePhase::Idle;
        if !keeps_scale_context {
            self.exit_scale();
        }
        let keeps_mirror_context =
            matches!(cmd.as_str(), "mi" | "mirror" | "from" | "fr")
                || self.mirror_phase != MirrorPhase::Idle;
        if !keeps_mirror_context {
            self.exit_mirror();
        }
        let keeps_fillet_context =
            matches!(cmd.as_str(), "fi" | "fillet" | "from" | "fr")
                || self.fillet_phase != FilletPhase::Idle;
        if !keeps_fillet_context {
            self.exit_fillet();
        }
        let keeps_chamfer_context =
            matches!(cmd.as_str(), "cha" | "chamfer" | "from" | "fr")
                || self.chamfer_phase != ChamferPhase::Idle;
        if !keeps_chamfer_context {
            self.exit_chamfer();
        }
        let keeps_polygon_context =
            matches!(cmd.as_str(), "pol" | "polygon" | "from" | "fr")
                || self.polygon_phase != PolygonPhase::Idle;
        if !keeps_polygon_context {
            self.exit_polygon();
        }
        let keeps_ellipse_context =
            matches!(cmd.as_str(), "el" | "ellipse" | "elipse" | "from" | "fr")
                || self.ellipse_phase != EllipsePhase::Idle;
        if !keeps_ellipse_context {
            self.exit_ellipse();
        }
        let keeps_rectangle_context =
            matches!(cmd.as_str(), "rec" | "rect" | "rectangle" | "from" | "fr")
                || self.rectangle_phase != RectanglePhase::Idle;
        if !keeps_rectangle_context {
            self.exit_rectangle();
        }
        let keeps_array_context =
            matches!(cmd.as_str(), "ar" | "array" | "from" | "fr")
                || self.array_phase != ArrayPhase::Idle;
        if !keeps_array_context {
            self.exit_array();
        }
        let keeps_pedit_context =
            matches!(cmd.as_str(), "pe" | "pedit" | "from" | "fr")
                || self.pedit_phase != PeditPhase::Idle;
        if !keeps_pedit_context {
            self.exit_pedit();
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
            "el" | "ellipse" | "elipse" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.exit_fillet();
                self.exit_chamfer();
                self.exit_polygon();
                self.exit_pedit();
                self.ellipse_phase = EllipsePhase::Center;
                self.command_log
                    .push("ELLIPSE: Specify center point".to_string());
                log::info!("Command: ELLIPSE");
                true
            }
            "pol" | "polygon" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.exit_fillet();
                self.exit_chamfer();
                self.exit_pedit();
                self.polygon_phase = PolygonPhase::EnteringSides;
                self.command_log
                    .push(format!("POLYGON: Enter number of sides <{}>", self.polygon_sides));
                log::info!("Command: POLYGON");
                true
            }
            "rec" | "rect" | "rectangle" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.exit_fillet();
                self.exit_chamfer();
                self.exit_polygon();
                self.exit_ellipse();
                self.exit_pedit();
                self.rectangle_phase = RectanglePhase::FirstCorner;
                self.command_log
                    .push("RECTANGLE: Specify first corner".to_string());
                log::info!("Command: RECTANGLE");
                true
            }
            "ar" | "array" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.exit_fillet();
                self.exit_chamfer();
                self.exit_polygon();
                self.exit_ellipse();
                self.exit_rectangle();
                self.exit_pedit();
                self.array_mode = ArrayMode::Rectangular;
                self.array_entities.clear();
                self.array_edit_assoc = None;
                self.array_phase = ArrayPhase::SelectingEntities;
                self.command_log
                    .push("ARRAY: Select entities, press Enter to continue".to_string());
                log::info!("Command: ARRAY");
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
                self.exit_scale();
                self.exit_mirror();
                self.rotate_phase = RotatePhase::SelectingEntities;
                self.rotate_base_point = None;
                self.rotate_entities.clear();
                self.command_log
                    .push("ROTATE: Select entities, press Enter to continue".to_string());
                log::info!("Command: ROTATE");
                true
            }
            "sc" | "scale" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_mirror();
                self.scale_phase = ScalePhase::SelectingEntities;
                self.scale_base_point = None;
                self.scale_ref_point = None;
                self.scale_entities.clear();
                self.command_log
                    .push("SCALE: Select entities, press Enter to continue".to_string());
                log::info!("Command: SCALE");
                true
            }
            "mi" | "mirror" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.mirror_phase = MirrorPhase::SelectingEntities;
                self.mirror_axis_p1 = None;
                self.mirror_entities.clear();
                self.command_log
                    .push("MIRROR: Select entities, press Enter to continue".to_string());
                log::info!("Command: MIRROR");
                true
            }
            "fi" | "fillet" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.fillet_phase = FilletPhase::EnteringRadius;
                self.command_log
                    .push(format!("FILLET: Enter radius <{:.4}>", self.fillet_radius));
                log::info!("Command: FILLET");
                true
            }
            "cha" | "chamfer" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.exit_fillet();
                self.chamfer_phase = ChamferPhase::EnteringDistance;
                self.command_log
                    .push(format!(
                        "CHAMFER: Enter distances <{:.4},{:.4}>",
                        self.chamfer_distance1, self.chamfer_distance2
                    ));
                log::info!("Command: CHAMFER");
                true
            }
            "pe" | "pedit" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                self.exit_fillet();
                self.pedit_phase = PeditPhase::SelectingPolyline;
                self.command_log
                    .push("PEDIT: Select an open polyline".to_string());
                log::info!("Command: PEDIT");
                true
            }
            "j" | "join" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_scale();
                self.exit_mirror();
                let _ = self.join_selected_into_polyline();
                log::info!("Command: JOIN");
                true
            }
            "co" | "copy" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_scale();
                self.exit_mirror();
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
                if let DimPhase::SecondPoint { first } = self.dim_phase {
                    // Inside DIMALIGNED after first point: use first point as FROM base.
                    self.from_phase = FromPhase::WaitingOffset;
                    self.from_base = Some(first);
                    self.command_log
                        .push(format!("  Base: {:.4}, {:.4}", first.x, first.y));
                    self.command_log
                        .push("FROM  Offset (@dx,dy  or  @dist<angle  or click):".to_string());
                } else if let DimLinearPhase::SecondPoint { first } = self.dim_linear_phase {
                    // Inside DIMLINEAR after first point: use first point as FROM base.
                    self.from_phase = FromPhase::WaitingOffset;
                    self.from_base = Some(first);
                    self.command_log
                        .push(format!("  Base: {:.4}, {:.4}", first.x, first.y));
                    self.command_log
                        .push("FROM  Offset (@dx,dy  or  @dist<angle  or click):".to_string());
                } else if self.is_picking_point() {
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
            "svgout" => {
                self.export_svg();
                true
            }
            "pdfout" => {
                self.export_pdf();
                true
            }
            "dxfin" => {
                self.pending_dxf_import = true;
                true
            }
            "pyrun" | "py" => {
                self.run_python_script_file();
                true
            }
            "pycon" | "python" | "pythonconsole" => {
                self.python_console_open = true;
                true
            }
            "aicmd" | "ai" => {
                self.ai_command_open = true;
                true
            }
            "aihelp" => {
                self.insert_ai_help_into_prompt();
                self.ai_command_open = true;
                self.command_log
                    .push("AIHELP: Inserted CadKit API cheat-sheet into AICMD prompt".to_string());
                true
            }
            "mcp" | "mcpstatus" => {
                self.refresh_mcp_detection();
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
            "t" | "text" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.text_is_mtext = false;
                self.text_phase = TextPhase::PlacingPosition;
                self.command_log
                    .push("TEXT  Specify insertion point:".to_string());
                log::info!("Command: TEXT");
                true
            }
            "mt" | "mtext" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.text_is_mtext = true;
                self.text_phase = TextPhase::PlacingPosition;
                self.command_log
                    .push("MTEXT  Specify insertion point:".to_string());
                self.command_log
                    .push("MTEXT  Use \\P in input to create a new line".to_string());
                log::info!("Command: MTEXT");
                true
            }
            "ed" | "editdim" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_text();
                self.exit_edit_text();
                self.edit_dim_phase = EditDimPhase::SelectingEntity;
                self.dim_edit_dialog = None;
                self.command_log
                    .push("EDITDIM: Click a dimension entity to edit".to_string());
                log::info!("Command: EDITDIM");
                true
            }
            "et" | "edittext" => {
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.exit_text();
                self.edit_text_phase = EditTextPhase::SelectingEntity;
                self.text_edit_dialog = None;
                self.command_log
                    .push("EDITTEXT: Click a text entity to edit".to_string());
                log::info!("Command: EDITTEXT");
                true
            }
            "dal" | "dimaligned" | "dim" => {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_phase = DimPhase::FirstPoint;
                self.command_log
                    .push("DIMALIGNED: Specify first extension line origin".to_string());
                log::info!("Command: DIMALIGNED");
                true
            }
            "dli" | "dimlinear" => {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_linear_phase = DimLinearPhase::FirstPoint;
                self.command_log
                    .push("DIMLINEAR: Specify first extension line origin".to_string());
                log::info!("Command: DIMLINEAR");
                true
            }
            "dang" | "dimangular" => {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_angular_phase = DimAngularPhase::FirstEntity;
                self.command_log
                    .push("DIMANGULAR: Click the first line or polyline segment".to_string());
                log::info!("Command: DIMANGULAR");
                true
            }
            "dra" | "dimradius" => {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_radial_phase = DimRadialPhase::SelectingEntity { is_diameter: false };
                self.command_log
                    .push("DIMRADIUS: Click a circle or arc".to_string());
                log::info!("Command: DIMRADIUS");
                true
            }
            "ddi" | "dimdiameter" => {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_radial_phase = DimRadialPhase::SelectingEntity { is_diameter: true };
                self.command_log
                    .push("DIMDIAMETER: Click a circle or arc".to_string());
                log::info!("Command: DIMDIAMETER");
                true
            }
            "dimstyle" | "dst" => {
                self.open_dim_style_dialog();
                self.command_log.push("DIMSTYLE: Edit dimension style".to_string());
                true
            }
            "grid" | "gr" => {
                self.grid_visible = !self.grid_visible;
                self.command_log.push(format!(
                    "Grid {}",
                    if self.grid_visible { "ON" } else { "OFF" }
                ));
                true
            }
            "help" | "?" => {
                self.help_open = true;
                true
            }
            "bgcolor" => {
                self.bgcolor_picker_open = true;
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
    pub(crate) fn python_console_completions() -> &'static [&'static str] {
        &[
            "cad.line(x1, y1, x2, y2)",
            "cad.circle(cx, cy, radius)",
            "cad.arc(cx, cy, radius, start_deg, end_deg)",
            "cad.dim_linear(x1, y1, x2, y2, offset, True, None)",
            "cad.set_layer(layer_id)",
            "cad.select()",
            "cad.get_entity(entity_id)",
            "cad.entity_count()",
        ]
    }

    pub(crate) fn apply_python_console_completion(&mut self) -> bool {
        let text = self.python_console_input.as_str();
        let token_start = text
            .char_indices()
            .rev()
            .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.'))
            .map(|(i, ch)| i + ch.len_utf8())
            .unwrap_or(0);
        let token = &text[token_start..];
        let token_lower = token.to_ascii_lowercase();
        let mut chosen: Option<&str> = None;
        for cand in Self::python_console_completions() {
            let c = cand.to_ascii_lowercase();
            if c.starts_with(&token_lower) {
                chosen = Some(cand);
                break;
            }
            if let Some(stripped) = c.strip_prefix("cad.") {
                if stripped.starts_with(&token_lower) {
                    chosen = Some(cand);
                    break;
                }
            }
        }
        let Some(cand) = chosen else { return false };
        self.python_console_input.truncate(token_start);
        self.python_console_input.push_str(cand);
        if !self.python_console_input.ends_with('\n') {
            self.python_console_input.push('\n');
        }
        self.command_log.push(format!("PYCON: Completed '{}'", cand));
        true
    }

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

    fn extract_numbers(text: &str) -> Vec<f64> {
        let mut cleaned = String::with_capacity(text.len());
        for ch in text.chars() {
            if ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+') {
                cleaned.push(ch);
            } else {
                cleaned.push(' ');
            }
        }
        cleaned
            .split_whitespace()
            .filter_map(|t| t.parse::<f64>().ok())
            .collect()
    }

    fn try_generate_directional_line(prompt: &str) -> Option<String> {
        let t = prompt.trim().to_ascii_lowercase();
        if !t.contains("line") || !t.contains("from") {
            return None;
        }
        let has_dir = t.contains("left") || t.contains("right") || t.contains("up") || t.contains("down");
        if !has_dir {
            return None;
        }
        let nums = Self::extract_numbers(&t);
        if nums.len() < 3 {
            return None;
        }
        let x = nums[0];
        let y = nums[1];
        let dist = nums[2].abs();
        let (x2, y2) = if t.contains("left") {
            (x - dist, y)
        } else if t.contains("right") {
            (x + dist, y)
        } else if t.contains("down") {
            (x, y - dist)
        } else {
            // "up"
            (x, y + dist)
        };
        Some(format!("cad.line({}, {}, {}, {})\n", x, y, x2, y2))
    }

    fn try_generate_circle(prompt: &str) -> Option<String> {
        let t = prompt.trim().to_ascii_lowercase();
        if !t.contains("circle") {
            return None;
        }
        let nums = Self::extract_numbers(&t);
        if nums.len() < 3 {
            return None;
        }
        let cx = nums[0];
        let cy = nums[1];
        let mut radius = nums[2].abs();
        if t.contains("diameter") || t.contains("dia") || t.contains('⌀') {
            radius *= 0.5;
        }
        if radius <= f64::EPSILON {
            return None;
        }
        Some(format!("cad.circle({}, {}, {})\n", cx, cy, radius))
    }

    pub(crate) fn generate_python_from_nl_prompt(&self, prompt: &str) -> Result<String, String> {
        let t = prompt.trim().to_ascii_lowercase();
        if t.is_empty() {
            return Err("Enter a prompt first".to_string());
        }
        if let Some(code) = Self::try_generate_directional_line(&t) {
            return Ok(code);
        }
        if let Some(code) = Self::try_generate_circle(&t) {
            return Ok(code);
        }
        let nums = Self::extract_numbers(&t);

        if t.starts_with("line") {
            if nums.len() < 4 {
                return Err("LINE needs 4 numbers: x1 y1 x2 y2".to_string());
            }
            return Ok(format!(
                "cad.line({}, {}, {}, {})\n",
                nums[0], nums[1], nums[2], nums[3]
            ));
        }

        if t.starts_with("circle") {
            if nums.len() < 3 {
                return Err("CIRCLE needs 3 numbers: cx cy radius".to_string());
            }
            return Ok(format!(
                "cad.circle({}, {}, {})\n",
                nums[0], nums[1], nums[2].abs()
            ));
        }

        if t.starts_with("arc") {
            if nums.len() < 5 {
                return Err("ARC needs 5 numbers: cx cy radius start_deg end_deg".to_string());
            }
            return Ok(format!(
                "cad.arc({}, {}, {}, {}, {})\n",
                nums[0], nums[1], nums[2].abs(), nums[3], nums[4]
            ));
        }

        if t.starts_with("dim") || t.contains("dimlinear") || t.contains("dimension") {
            if nums.len() < 5 {
                return Err("DIMLINEAR needs 5 numbers: x1 y1 x2 y2 offset".to_string());
            }
            let horizontal = !t.contains("vertical");
            return Ok(format!(
                "cad.dim_linear({}, {}, {}, {}, {}, {}, None)\n",
                nums[0], nums[1], nums[2], nums[3], nums[4], horizontal
            ));
        }

        Err("Supported intents: line, circle, arc, dimlinear".to_string())
    }

    fn extract_python_code(text: &str) -> String {
        let s = text.trim();
        if let Some(start) = s.find("```") {
            let rest = &s[start + 3..];
            let rest = if let Some(r) = rest.strip_prefix("python") { r } else { rest };
            let rest = rest.strip_prefix('\n').unwrap_or(rest);
            if let Some(end) = rest.find("```") {
                return rest[..end].trim().to_string();
            }
        }
        s.to_string()
    }

    fn generate_python_with_lm_studio(&self, prompt: &str) -> Result<String, String> {
        let endpoint = self.ai_lmstudio_endpoint.trim();
        if endpoint.is_empty() {
            return Err("LM Studio endpoint is empty".to_string());
        }
        let model = if self.ai_lmstudio_model.trim().is_empty() {
            "local-model"
        } else {
            self.ai_lmstudio_model.trim()
        };
        let system_prompt = match self.ai_model_profile {
            AiModelProfile::StrictCadCode => {
                "Convert CAD user intent into Python code using CadKit API only. Output only python code. Use only cad.line, cad.circle, cad.arc, cad.dim_linear. No prose. No markdown. No explanations."
            }
            AiModelProfile::General => {
                "Convert CAD user intent into helpful CadKit Python code. Prefer cad.line, cad.circle, cad.arc, cad.dim_linear. Output python code."
            }
        };
        let payload = json!({
            "model": model,
            "temperature": 0.1,
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": prompt
                }
            ]
        });
        let body = ureq::post(endpoint)
            .set("Content-Type", "application/json")
            .send_string(&payload.to_string())
            .map_err(|e| format!("LM Studio request failed: {}", e))?
            .into_string()
            .map_err(|e| format!("LM Studio response read failed: {}", e))?;
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("LM Studio JSON parse failed: {}", e))?;
        let raw = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| "LM Studio response missing choices[0].message.content".to_string())?;
        let code = Self::extract_python_code(raw);
        if code.trim().is_empty() {
            return Err("LM Studio returned empty code".to_string());
        }
        Ok(format!("{}\n", code.trim_end()))
    }

    fn generate_python_with_phi3(&self, _prompt: &str) -> Result<String, String> {
        let p = self.ai_phi3_model_path.trim();
        if p.is_empty() {
            return Err("Phi-3 model path is empty".to_string());
        }
        let model_path = Self::expand_tilde_path(p);
        if !model_path.exists() {
            return Err(format!("Phi-3 model file not found: {}", model_path.display()));
        }
        let system_prompt = match self.ai_model_profile {
            AiModelProfile::StrictCadCode => {
                "Convert CAD user intent into Python code using CadKit API only. Output only python code. Use only cad.line, cad.circle, cad.arc, cad.dim_linear. No prose. No markdown. No explanations."
            }
            AiModelProfile::General => {
                "Convert CAD user intent into helpful CadKit Python code. Prefer cad.line, cad.circle, cad.arc, cad.dim_linear. Output python code."
            }
        };
        let full_prompt = format!(
            "System:\n{}\n\nUser:\n{}\n\nAssistant:\n",
            system_prompt, _prompt
        );

        let mut last_err = String::new();
        for bin in ["llama-cli", "llama"] {
            match std::process::Command::new(bin)
                .args([
                    "-m",
                    &model_path.to_string_lossy(),
                    "-p",
                    &full_prompt,
                    "-n",
                    "256",
                    "--temp",
                    "0.1",
                ])
                .output()
            {
                Ok(out) => {
                    if !out.status.success() {
                        let stderr = String::from_utf8_lossy(&out.stderr);
                        last_err = format!("{} exited with error: {}", bin, stderr.trim());
                        continue;
                    }
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let code = Self::extract_python_code(&stdout);
                    if code.trim().is_empty() {
                        return Err("Phi-3 returned empty code".to_string());
                    }
                    return Ok(format!("{}\n", code.trim_end()));
                }
                Err(e) => {
                    last_err = format!("Failed to launch {}: {}", bin, e);
                }
            }
        }

        if last_err.is_empty() {
            Err("Could not run Phi-3 runtime (llama-cli)".to_string())
        } else {
            Err(last_err)
        }
    }

    pub(crate) fn generate_ai_python_preview(&mut self, prompt: &str) -> Result<String, String> {
        if let Some(code) = Self::try_generate_directional_line(prompt) {
            self.command_log
                .push("AICMD: Used deterministic directional-line parser".to_string());
            return Ok(code);
        }
        if let Some(code) = Self::try_generate_circle(prompt) {
            self.command_log
                .push("AICMD: Used deterministic circle parser".to_string());
            return Ok(code);
        }
        match self.ai_backend_mode {
            AiBackendMode::LocalParser => self.generate_python_from_nl_prompt(prompt),
            AiBackendMode::LmStudio => self.generate_python_with_lm_studio(prompt),
            AiBackendMode::Phi3 => match self.generate_python_with_phi3(prompt) {
                Ok(code) => Ok(code),
                Err(e) => {
                    self.command_log.push(format!(
                        "AICMD: {}. Falling back to local parser",
                        e
                    ));
                    self.generate_python_from_nl_prompt(prompt)
                }
            },
            AiBackendMode::Mcp => Err(
                "MCP generation backend is not wired yet. Use LM Studio or Local Parser.".to_string(),
            ),
            AiBackendMode::Auto => {
                if self.ai_mcp_detected {
                    self.command_log.push(
                        "AICMD: MCP detected; using LM Studio/local fallback until MCP generation is wired"
                            .to_string(),
                    );
                }
                match self.generate_python_with_lm_studio(prompt) {
                    Ok(code) => Ok(code),
                    Err(lm_err) => {
                        self.command_log.push(format!(
                            "AICMD: LM Studio unavailable ({}), falling back to local parser",
                            lm_err
                        ));
                        self.generate_python_from_nl_prompt(prompt)
                    }
                }
            }
        }
    }

    pub(crate) fn ai_help_snippet() -> &'static str {
        "CadKit Python API quick reference:\n\
         - cad.line(x1, y1, x2, y2)\n\
         - cad.line((x1, y1), (x2, y2))\n\
         - cad.circle(cx, cy, radius)\n\
         - cad.circle((cx, cy), radius)\n\
         - cad.arc(cx, cy, radius, start_deg, end_deg)\n\
         - cad.arc((cx, cy), radius, start_deg, end_deg)\n\
         - cad.dim_linear(x1, y1, x2, y2, offset, horizontal=True, text_override=None)\n\
         - cad.set_layer(layer_id)\n\
         - cad.select(kind=None, layer=None)\n\
         - cad.get_entity(entity_id)\n\
         - cad.entity_count()\n\
         Rules:\n\
         - Output only executable Python lines.\n\
         - Do not use imports, file IO, eval/exec, or unknown cad.* functions.\n\
         - Prefer one command per line.\n\
         "
    }

    pub(crate) fn insert_ai_help_into_prompt(&mut self) {
        if !self.ai_command_prompt.trim().is_empty() {
            self.ai_command_prompt.push_str("\n\n");
        }
        self.ai_command_prompt.push_str(Self::ai_help_snippet());
    }

    pub(crate) fn validate_ai_preview_code(&self, src: &str) -> Result<(), String> {
        let allowed = [
            "line",
            "circle",
            "arc",
            "dim_linear",
            "set_layer",
            "select",
            "get_entity",
            "entity_count",
        ];
        for (idx, raw) in src.lines().enumerate() {
            let line_no = idx + 1;
            let t = raw.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            if t.starts_with("import ")
                || t.starts_with("from ")
                || t.contains("__")
                || t.contains(';')
                || t.contains("exec(")
                || t.contains("eval(")
                || t.contains("open(")
            {
                return Err(format!("Line {} contains disallowed Python construct", line_no));
            }
            if !t.starts_with("cad.") {
                return Err(format!(
                    "Line {} must start with cad.<command>(...)",
                    line_no
                ));
            }
            let Some(rest) = t.strip_prefix("cad.") else {
                return Err(format!("Line {} invalid cad call", line_no));
            };
            let Some((name, _)) = rest.split_once('(') else {
                return Err(format!("Line {} invalid function call syntax", line_no));
            };
            if !allowed.iter().any(|a| a == &name) {
                return Err(format!(
                    "Line {} uses unsupported cad command '{}'",
                    line_no, name
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn refresh_mcp_detection(&mut self) {
        let mut candidate_paths: Vec<PathBuf> = Vec::new();
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            candidate_paths.push(PathBuf::from(xdg).join("Claude").join("claude_desktop_config.json"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            candidate_paths.push(home.join(".config").join("Claude").join("claude_desktop_config.json"));
            candidate_paths.push(home.join(".claude").join("claude_desktop_config.json"));
        }
        // Preserve insertion order while removing duplicates.
        let mut unique: Vec<PathBuf> = Vec::new();
        for p in candidate_paths {
            if !unique.iter().any(|x| x == &p) {
                unique.push(p);
            }
        }

        let mut config_found = None::<PathBuf>;
        let mut mcp_configured = false;
        for p in unique {
            if !p.exists() {
                continue;
            }
            config_found = Some(p.clone());
            if let Ok(txt) = std::fs::read_to_string(&p) {
                let lower = txt.to_ascii_lowercase();
                if lower.contains("mcpservers") || lower.contains("\"mcp\"") {
                    mcp_configured = true;
                    break;
                }
            }
        }

        let claude_running = std::process::Command::new("ps")
            .args(["-A", "-o", "comm="])
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| {
                s.lines()
                    .any(|line| line.to_ascii_lowercase().contains("claude"))
            })
            .unwrap_or(false);

        let status = if mcp_configured && claude_running {
            "Claude MCP detected (configured + app running)".to_string()
        } else if mcp_configured {
            "Claude MCP configured (app not running)".to_string()
        } else if config_found.is_some() {
            "Claude config found, MCP not configured".to_string()
        } else {
            "Claude MCP not detected".to_string()
        };
        self.ai_mcp_detected = mcp_configured;
        self.ai_mcp_status = if let Some(path) = config_found {
            format!("{} [{}]", status, path.display())
        } else {
            status
        };
        self.command_log
            .push(format!("MCP: {}", self.ai_mcp_status));
    }

    fn expand_tilde_path(raw: &str) -> PathBuf {
        if let Some(rest) = raw.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return PathBuf::from(home).join(rest);
            }
        }
        PathBuf::from(raw)
    }
}
