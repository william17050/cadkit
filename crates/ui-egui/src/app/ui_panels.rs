use super::state::{
    ActiveTool, CopyPhase, DimAngularPhase, DimLinearPhase, DimPhase, DimRadialPhase, EditDimPhase, EditTextPhase,
    ArrayMode, ArrayPhase, BlockPhase, BoundaryPhase, ChamferPhase, EllipsePhase, ExtendPhase, FilletPhase, FromPhase, HatchPhase, InsertPhase, MirrorPhase, MovePhase,
    OffsetPhase, PeditPhase, PolygonPhase, RectanglePhase, RotatePhase, ScalePhase, TextPhase, TrimPhase,
};
use super::CadKitApp;
use cadkit_2d_core::{EntityKind, Linetype};
use cadkit_types::{Guid, Vec2};
use eframe::egui;

impl CadKitApp {
    pub fn draw_ui_panels(&mut self, ctx: &egui::Context) {
        self.draw_menu_bar(ctx);
        self.draw_left_toolbar(ctx);
        self.draw_command_line(ctx); // must come before right_panel so available_height() is correct
        self.draw_right_panel(ctx);
    }

    fn draw_menu_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    self.prune_recent_files();
                    if ui.button("New").clicked() {
                        self.exit_dim();
                        self.drawing = cadkit_2d_core::Drawing::new("New Drawing".to_string());
                        self.current_file = None;
                        self.selected_entities.clear();
                        self.selection = None;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Title("CadKit".to_string()));
                        ui.close_menu();
                    }
                    if ui.button("Open...").clicked() {
                        self.exit_dim();
                        ui.close_menu();
                        self.open(ctx);
                    }
                    ui.menu_button("Recent Files", |ui| {
                        if self.recent_files.is_empty() {
                            ui.label(egui::RichText::new("No recent files").italics());
                        } else {
                            let items: Vec<String> = self.recent_files.clone();
                            let mut remove_missing = false;
                            for path in items {
                                let label = std::path::Path::new(&path)
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| path.clone());
                                if std::path::Path::new(&path).exists() {
                                    if ui.button(label).on_hover_text(path.clone()).clicked() {
                                        ui.close_menu();
                                        self.open_path(ctx, &path);
                                    }
                                } else {
                                    ui.add_enabled(
                                        false,
                                        egui::Button::new(format!("{label} (missing)")),
                                    )
                                    .on_hover_text(path.clone());
                                }
                            }
                            ui.separator();
                            if ui.button("Remove Missing").clicked() {
                                remove_missing = true;
                            }
                            if ui.button("Clear Recent").clicked() {
                                self.clear_recent_files();
                                ui.close_menu();
                            }
                            if remove_missing {
                                self.recent_files.retain(|p| std::path::Path::new(p).exists());
                            }
                        }
                    });
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
                    if ui.button("Export SVG...").clicked() {
                        ui.close_menu();
                        self.export_svg();
                    }
                    if ui.button("Export PDF...").clicked() {
                        ui.close_menu();
                        self.export_pdf();
                    }
                    if ui.button("Import DXF...").clicked() {
                        ui.close_menu();
                        self.import_dxf(ctx);
                    }
                    if ui.button("Run Python Script...").clicked() {
                        ui.close_menu();
                        self.run_python_script_file();
                    }
                    if ui.button("Python Console...").clicked() {
                        ui.close_menu();
                        self.python_console_open = true;
                    }
                    if ui.button("AI Command...").clicked() {
                        ui.close_menu();
                        self.ai_command_open = true;
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });

                ui.menu_button("Draw", |ui| {
                    if ui.button("Line").clicked() {
                        self.exit_dim();
                        self.active_tool = ActiveTool::Line { start: None };
                        self.distance_input.clear();
                        ui.close_menu();
                    }
                    if ui.button("Circle").clicked() {
                        self.exit_dim();
                        self.active_tool = ActiveTool::Circle { center: None };
                        self.distance_input.clear();
                        ui.close_menu();
                    }
                    if ui.button("Arc").clicked() {
                        self.exit_dim();
                        self.active_tool = ActiveTool::Arc { start: None, mid: None };
                        self.distance_input.clear();
                        ui.close_menu();
                    }
                    if ui.button("Rectangle").clicked() {
                        self.exit_dim();
                        self.cancel_active_tool();
                        self.rectangle_phase = RectanglePhase::FirstCorner;
                        self.command_log
                            .push("RECTANGLE: Specify first corner".to_string());
                        ui.close_menu();
                    }
                    if ui.button("Ellipse").clicked() {
                        self.exit_dim();
                        self.cancel_active_tool();
                        self.ellipse_phase = EllipsePhase::Center;
                        self.command_log
                            .push("ELLIPSE: Specify center point".to_string());
                        ui.close_menu();
                    }
                    if ui.button("Polygon").clicked() {
                        self.exit_dim();
                        self.cancel_active_tool();
                        self.polygon_phase = PolygonPhase::EnteringSides;
                        self.command_log
                            .push(format!("POLYGON: Enter number of sides <{}>", self.polygon_sides));
                        ui.close_menu();
                    }
                });

                ui.menu_button("Help", |ui| {
                    if ui.button("Commands...").clicked() {
                        self.help_open = true;
                        ui.close_menu();
                    }
                });
            });
        });
    }

    fn draw_left_toolbar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("tools").default_width(150.0).show(ctx, |ui| {
            ui.heading("Draw");
            ui.separator();

            if ui.button("📏 Line").clicked() {
                self.exit_dim();
                match self.active_tool {
                    ActiveTool::Line { .. } => self.cancel_active_tool(),
                    _ => {
                        self.active_tool = ActiveTool::Line { start: None };
                    }
                }
            }
            if ui.button("⭕ Circle").clicked() {
                self.exit_dim();
                match self.active_tool {
                    ActiveTool::Circle { .. } => self.cancel_active_tool(),
                    _ => {
                        self.active_tool = ActiveTool::Circle { center: None };
                        self.distance_input.clear();
                    }
                }
            }
            if ui.button("🧵 Polyline").clicked() {
                self.exit_dim();
                match self.active_tool {
                    ActiveTool::Polyline { .. } => self.cancel_active_tool(),
                    _ => {
                        self.active_tool = ActiveTool::Polyline { points: Vec::new() };
                        self.distance_input.clear();
                    }
                }
            }
            if ui.button("◜ Arc").clicked() {
                self.exit_dim();
                match self.active_tool {
                    ActiveTool::Arc { .. } => self.cancel_active_tool(),
                    _ => {
                        self.active_tool = ActiveTool::Arc { start: None, mid: None };
                        self.distance_input.clear();
                    }
                }
            }
            if ui.button("▭ Rectangle").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.rectangle_phase = RectanglePhase::FirstCorner;
                self.command_log
                    .push("RECTANGLE: Specify first corner".to_string());
            }
            if ui.button("⬭ Ellipse").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.ellipse_phase = EllipsePhase::Center;
                self.command_log
                    .push("ELLIPSE: Specify center point".to_string());
            }
            if ui.button("⬡ Polygon").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.polygon_phase = PolygonPhase::EnteringSides;
                self.command_log
                    .push(format!("POLYGON: Enter number of sides <{}>", self.polygon_sides));
            }
            if ui.button("T Text").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.text_is_mtext = false;
                self.text_phase = TextPhase::PlacingPosition;
                self.command_log.push("TEXT  Specify insertion point:".to_string());
            }
            if ui.button("✏ Edit Text").clicked() {
                self.exit_dim();
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
                self.command_log.push("EDITTEXT: Click a text entity to edit".to_string());
            }

            ui.add_space(20.0);
            ui.heading("Blocks");
            ui.separator();

            if ui.button("🧱 BMake").clicked() {
                let _ = self.execute_command_alias("bmake");
            }

            let block_names = self.drawing.block_names();
            if !block_names.is_empty()
                && (self.block_palette_selected.is_empty()
                    || !block_names.iter().any(|n| n == &self.block_palette_selected))
            {
                self.block_palette_selected = block_names[0].clone();
            }
            if block_names.is_empty() {
                ui.label(egui::RichText::new("No blocks defined yet").italics());
            } else {
                if ui.button("⬇ Insert Selected").clicked() {
                    let cmd = format!("insert {}", self.block_palette_selected);
                    let _ = self.execute_command_alias(&cmd);
                }

                egui::CollapsingHeader::new("Block Palette")
                    .id_source("left_blocks_palette")
                    .default_open(true)
                    .show(ui, |ui| {
                        for name in &block_names {
                            if ui
                                .selectable_label(self.block_palette_selected == *name, name)
                                .clicked()
                            {
                                self.block_palette_selected = name.clone();
                            }
                        }
                    });
            }

            ui.add_space(20.0);
            ui.heading("Dimension");
            ui.separator();

            if ui.button("📐 Dim Aligned").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_phase = DimPhase::FirstPoint;
                self.command_log.push("DIMALIGNED: Specify first extension line origin".to_string());
            }
            if ui.button("↔ Dim Linear").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_linear_phase = DimLinearPhase::FirstPoint;
                self.command_log.push("DIMLINEAR: Specify first extension line origin".to_string());
            }
            if ui.button("∠ Dim Angular").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_angular_phase = DimAngularPhase::FirstEntity;
                self.command_log.push("DIMANGULAR: Click the first line or polyline segment".to_string());
            }
            if ui.button("R Dim Radius").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_radial_phase = DimRadialPhase::SelectingEntity { is_diameter: false };
                self.command_log.push("DIMRADIUS: Click a circle or arc".to_string());
            }
            if ui.button("Ø Dim Diameter").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.dim_radial_phase = DimRadialPhase::SelectingEntity { is_diameter: true };
                self.command_log.push("DIMDIAMETER: Click a circle or arc".to_string());
            }
            if ui.button("✏ Edit Dim").clicked() {
                self.exit_dim();
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
                self.command_log.push("EDITDIM: Click a dimension entity to edit".to_string());
            }
            if ui.button("⚙ DimStyle").clicked() {
                self.open_dim_style_dialog();
                self.command_log.push("DIMSTYLE: Edit dimension style".to_string());
            }

            ui.add_space(20.0);
            ui.heading("Modify");
            ui.separator();

            if ui.button("✂ Trim").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.trim_phase = TrimPhase::SelectingEdges;
                self.trim_cutting_edges.clear();
                self.command_log.push("TRIM: Select cutting edges, press Enter to continue".to_string());
            }
            if ui.button("↔ Extend").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.extend_phase = ExtendPhase::SelectingBoundaries;
                self.extend_boundary_edges.clear();
                self.command_log.push("EXTEND: Select boundary edges, press Enter to continue".to_string());
            }
            if ui.button("⬚ Boundary").clicked() {
                self.exit_dim();
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
                self.exit_array();
                self.exit_pedit();
                self.boundary_phase = BoundaryPhase::PickingPoint;
                self.command_log.push("BOUNDARY: Click an internal point".to_string());
            }
            if ui.button("//// Hatch").clicked() {
                self.exit_dim();
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
                self.exit_array();
                self.exit_pedit();
                self.exit_boundary();
                self.hatch_dialog_open = true;
                self.hatch_phase = HatchPhase::PickingPoint;
                self.command_log
                    .push("HATCH: Dialog open. Click internal point or adjust settings first".to_string());
            }
            if ui.button("⊙ Offset").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.offset_phase = OffsetPhase::EnteringDistance;
                self.offset_distance = None;
                self.offset_selected_entity = None;
                self.command_log.push("OFFSET: Enter distance".to_string());
            }
            if ui.button("➡️ Move").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_scale();
                self.move_phase = MovePhase::SelectingEntities;
                self.move_base_point = None;
                self.move_entities.clear();
                self.command_log.push("MOVE: Select entities to move, press Enter to continue".to_string());
            }
            if ui.button("📋 Copy").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_scale();
                self.copy_phase = CopyPhase::SelectingEntities;
                self.copy_base_point = None;
                self.copy_entities.clear();
                self.command_log.push("COPY: Select entities to copy, press Enter to continue".to_string());
            }
            if ui.button("▦ Array").clicked() {
                self.exit_dim();
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
                self.command_log.push("ARRAY: Select entities, press Enter to continue".to_string());
            }
            if ui.button("🔄 Rotate").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_scale();
                self.rotate_phase = RotatePhase::SelectingEntities;
                self.rotate_base_point = None;
                self.rotate_entities.clear();
                self.command_log.push("ROTATE: Select entities, press Enter to continue".to_string());
            }
            if ui.button("📏 Scale").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
                self.scale_phase = ScalePhase::SelectingEntities;
                self.scale_base_point = None;
                self.scale_ref_point = None;
                self.scale_entities.clear();
                self.command_log.push("SCALE: Select entities, press Enter to continue".to_string());
            }
            if ui.button("🪞 Mirror").clicked() {
                self.exit_dim();
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
                self.command_log.push("MIRROR: Select entities, press Enter to continue".to_string());
            }
            if ui.button("◜ Fillet").clicked() {
                self.exit_dim();
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
                self.command_log.push(format!("FILLET: Enter radius <{:.4}>", self.fillet_radius));
            }
            if ui.button("⟂ Chamfer").clicked() {
                self.exit_dim();
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
            }
            if ui.button("🗑️ Delete").clicked() {
                let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                let ids = self.filter_editable_entity_ids(&requested, "DELETE");
                if !ids.is_empty() {
                    self.push_undo();
                    for id in &ids {
                        let _ = self.drawing.remove_entity(id);
                    }
                    self.selected_entities.clear();
                    self.selection = None;
                }
            }
        });
    }

    fn draw_right_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("properties").default_width(240.0).show(ctx, |ui| {
            let total_h = ui.available_height();

            let mut layer_ids: Vec<u32> = self.drawing.layers().map(|l| l.id).collect();
            layer_ids.sort_unstable();

            let mut toggle_visible: Option<u32> = None;
            let mut toggle_locked: Option<u32> = None;
            let mut toggle_frozen: Option<u32> = None;
            let mut delete_layer: Option<u32> = None;
            let mut set_current: Option<u32> = None;
            let mut open_color_picker: Option<u32> = None;
            let mut commit_name: Option<(u32, String)> = None;
            let mut cancel_edit = false;
            let mut start_edit: Option<(u32, String)> = None;

            let mut assign_entity_layer: Option<u32> = None;
            let mut assign_entity_linetype: Option<Linetype> = None;
            let mut assign_entity_linetype_bylayer: Option<bool> = None;
            let mut assign_entity_linetype_scale: Option<Option<f64>> = None;
            let mut set_entity_bylayer = false;
            let mut open_entity_color_picker = false;

            ui.heading("Layers");
            ui.separator();

            if let Some(cur) = self.drawing.get_layer(self.current_layer) {
                let c = cur.color;
                let name = cur.name.clone();
                let mut layer_lt = cur.linetype;
                let mut layer_lts = cur.linetype_scale;
                ui.horizontal(|ui| {
                    ui.label("Active:");
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                    ui.label(egui::RichText::new(name).strong().color(egui::Color32::from_rgb(160, 210, 255)));
                });
                ui.horizontal(|ui| {
                    ui.label("Layer LT:")
                        .on_hover_text("Default linetype used by entities set to ByLayer.");
                    egui::ComboBox::from_id_source("current_layer_linetype")
                        .selected_text(layer_lt.as_str())
                        .width(96.0)
                        .show_ui(ui, |ui| {
                            for lt in [Linetype::Continuous, Linetype::Hidden, Linetype::Center] {
                                if ui.selectable_label(layer_lt == lt, lt.as_str()).clicked() {
                                    layer_lt = lt;
                                }
                            }
                        });
                    ui.add(
                        egui::DragValue::new(&mut layer_lts)
                            .prefix("S ")
                            .clamp_range(0.01..=1000.0)
                            .speed(0.1),
                    )
                    .on_hover_text("Layer linetype scale. Used when entity LTScale is ByLayer.");
                });
                if let Some(l) = self.drawing.get_layer_mut(self.current_layer) {
                    l.linetype = layer_lt;
                    l.linetype_scale = layer_lts.clamp(0.01, 1000.0);
                }
            }
            ui.add_space(2.0);

            let layers_list_h = (total_h * self.properties_split - 70.0).max(40.0);
            egui::ScrollArea::vertical()
                .id_source("layer_scroll")
                .max_height(layers_list_h)
                .show(ui, |ui| {
                    for &id in &layer_ids {
                        let (name, visible, locked, frozen, color, is_current) = match self.drawing.get_layer(id) {
                            Some(l) => (l.name.clone(), l.visible, l.locked, l.frozen, l.color, self.current_layer == id),
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
                                    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
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
                                let freeze_icon = if frozen { "❄" } else { "·" };
                                if ui.small_button(freeze_icon).on_hover_text("Toggle freeze").clicked() {
                                    toggle_frozen = Some(id);
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

            ui.label(egui::RichText::new("Properties").strong());
            ui.separator();

            egui::ScrollArea::vertical()
                .id_source("props_scroll")
                .show(ui, |ui| {
                    let sel_count = self.selected_entities.len();

                    if sel_count == 0 {
                        ui.label(egui::RichText::new("No selection").color(egui::Color32::from_gray(110)));
                    } else {
                        let mut common_color: Option<Option<[u8; 3]>> = None;
                        let mut color_mixed = false;
                        let mut common_layer: Option<u32> = None;
                        let mut layer_mixed = false;
                        let mut common_linetype: Option<Linetype> = None;
                        let mut linetype_mixed = false;
                        let mut common_linetype_by_layer: Option<bool> = None;
                        let mut linetype_bylayer_mixed = false;
                        let mut common_linetype_scale: Option<Option<f64>> = None;
                        let mut linetype_scale_mixed = false;
                        let mut linetype_supported = true;

                        for id in &self.selected_entities {
                            if let Some(e) = self.drawing.get_entity(id) {
                                match common_color {
                                    None => common_color = Some(e.color),
                                    Some(c) if c == e.color => {}
                                    _ => color_mixed = true,
                                }
                                match common_layer {
                                    None => common_layer = Some(e.layer),
                                    Some(l) if l == e.layer => {}
                                    _ => layer_mixed = true,
                                }
                                match common_linetype {
                                    None => common_linetype = Some(e.linetype),
                                    Some(lt) if lt == e.linetype => {}
                                    _ => linetype_mixed = true,
                                }
                                match common_linetype_by_layer {
                                    None => common_linetype_by_layer = Some(e.linetype_by_layer),
                                    Some(v) if v == e.linetype_by_layer => {}
                                    _ => linetype_bylayer_mixed = true,
                                }
                                match common_linetype_scale {
                                    None => common_linetype_scale = Some(e.linetype_scale),
                                    Some(v) if v == e.linetype_scale => {}
                                    _ => linetype_scale_mixed = true,
                                }
                                if !matches!(
                                    e.kind,
                                    EntityKind::Line { .. }
                                        | EntityKind::Circle { .. }
                                        | EntityKind::Arc { .. }
                                        | EntityKind::Polyline { .. }
                                ) {
                                    linetype_supported = false;
                                }
                            }
                        }

                        if sel_count == 1 {
                            let eid = *self.selected_entities.iter().next().unwrap();
                            if let Some(entity) = self.drawing.get_entity(&eid) {
                                let type_name = match &entity.kind {
                                    EntityKind::Line { .. } => "Line",
                                    EntityKind::Circle { .. } => "Circle",
                                    EntityKind::Arc { .. } => "Arc",
                                    EntityKind::Polyline { .. } => "Polyline",
                                    EntityKind::DimAligned { .. } => "DimAligned",
                                    EntityKind::DimLinear { horizontal, .. } => if *horizontal { "DimLinear (H)" } else { "DimLinear (V)" },
                                    EntityKind::DimAngular { .. } => "DimAngular",
                                    EntityKind::DimRadial { is_diameter, .. } => if *is_diameter { "DimDiameter" } else { "DimRadius" },
                                    EntityKind::Text { .. } => "Text",
                                    EntityKind::Insert { .. } => "Insert",
                                };
                                ui.label(egui::RichText::new(type_name).strong());
                                egui::Grid::new("entity_geom")
                                    .num_columns(2)
                                    .spacing([4.0, 2.0])
                                    .show(ui, |ui| {
                                        match &entity.kind {
                                            EntityKind::Line { start, end } => {
                                                let dx = end.x - start.x;
                                                let dy = end.y - start.y;
                                                let len = (dx * dx + dy * dy).sqrt();
                                                ui.label("Start X:"); ui.label(format!("{:.4}", start.x)); ui.end_row();
                                                ui.label("Start Y:"); ui.label(format!("{:.4}", start.y)); ui.end_row();
                                                ui.label("End X:"); ui.label(format!("{:.4}", end.x)); ui.end_row();
                                                ui.label("End Y:"); ui.label(format!("{:.4}", end.y)); ui.end_row();
                                                ui.label("Length:"); ui.label(format!("{:.4}", len)); ui.end_row();
                                            }
                                            EntityKind::Circle { center, radius } => {
                                                ui.label("Center X:"); ui.label(format!("{:.4}", center.x)); ui.end_row();
                                                ui.label("Center Y:"); ui.label(format!("{:.4}", center.y)); ui.end_row();
                                                ui.label("Radius:"); ui.label(format!("{:.4}", radius)); ui.end_row();
                                                ui.label("Diameter:"); ui.label(format!("{:.4}", radius * 2.0)); ui.end_row();
                                                ui.label("Circumference:"); ui.label(format!("{:.4}", std::f64::consts::TAU * radius)); ui.end_row();
                                            }
                                            EntityKind::Arc { center, radius, start_angle, end_angle } => {
                                                let span_rad = (end_angle - start_angle).abs();
                                                let span_deg = span_rad.to_degrees();
                                                let arc_len = radius * span_rad;
                                                ui.label("Center X:"); ui.label(format!("{:.4}", center.x)); ui.end_row();
                                                ui.label("Center Y:"); ui.label(format!("{:.4}", center.y)); ui.end_row();
                                                ui.label("Radius:"); ui.label(format!("{:.4}", radius)); ui.end_row();
                                                ui.label("Start Ang:"); ui.label(format!("{:.2}°", start_angle.to_degrees())); ui.end_row();
                                                ui.label("End Ang:"); ui.label(format!("{:.2}°", end_angle.to_degrees())); ui.end_row();
                                                ui.label("Span:"); ui.label(format!("{:.2}°", span_deg)); ui.end_row();
                                                ui.label("Arc Length:"); ui.label(format!("{:.4}", arc_len)); ui.end_row();
                                            }
                                            EntityKind::Polyline { vertices, closed } => {
                                                ui.label("Points:"); ui.label(vertices.len().to_string()); ui.end_row();
                                                ui.label("Closed:"); ui.label(if *closed { "Yes" } else { "No" }); ui.end_row();
                                            }
                                            EntityKind::DimAligned { start, end, offset, text_override, .. } => {
                                                let dx = end.x - start.x;
                                                let dy = end.y - start.y;
                                                let dist = (dx * dx + dy * dy).sqrt();
                                                ui.label("Start X:"); ui.label(format!("{:.4}", start.x)); ui.end_row();
                                                ui.label("Start Y:"); ui.label(format!("{:.4}", start.y)); ui.end_row();
                                                ui.label("End X:"); ui.label(format!("{:.4}", end.x)); ui.end_row();
                                                ui.label("End Y:"); ui.label(format!("{:.4}", end.y)); ui.end_row();
                                                ui.label("Distance:"); ui.label(format!("{:.4}", dist)); ui.end_row();
                                                ui.label("Offset:"); ui.label(format!("{:.4}", offset)); ui.end_row();
                                                if let Some(t) = text_override {
                                                    ui.label("Text:"); ui.label(t.as_str()); ui.end_row();
                                                }
                                            }
                                            EntityKind::DimLinear { start, end, offset, text_override, horizontal, .. } => {
                                                let dist = if *horizontal {
                                                    (end.x - start.x).abs()
                                                } else {
                                                    (end.y - start.y).abs()
                                                };
                                                ui.label("Axis:"); ui.label(if *horizontal { "Horizontal" } else { "Vertical" }); ui.end_row();
                                                ui.label("Start X:"); ui.label(format!("{:.4}", start.x)); ui.end_row();
                                                ui.label("Start Y:"); ui.label(format!("{:.4}", start.y)); ui.end_row();
                                                ui.label("End X:"); ui.label(format!("{:.4}", end.x)); ui.end_row();
                                                ui.label("End Y:"); ui.label(format!("{:.4}", end.y)); ui.end_row();
                                                ui.label("Measured:"); ui.label(format!("{:.4}", dist)); ui.end_row();
                                                ui.label("Offset:"); ui.label(format!("{:.4}", offset)); ui.end_row();
                                                if let Some(t) = text_override {
                                                    ui.label("Text:"); ui.label(t.as_str()); ui.end_row();
                                                }
                                            }
                                            EntityKind::DimAngular { vertex, line1_pt, line2_pt, radius, text_override, .. } => {
                                                use std::f64::consts::TAU;
                                                let a1 = (line1_pt.y - vertex.y).atan2(line1_pt.x - vertex.x);
                                                let mut a2 = (line2_pt.y - vertex.y).atan2(line2_pt.x - vertex.x);
                                                if a2 <= a1 { a2 += TAU; }
                                                let angle_deg = (a2 - a1).to_degrees();
                                                ui.label("Vertex X:"); ui.label(format!("{:.4}", vertex.x)); ui.end_row();
                                                ui.label("Vertex Y:"); ui.label(format!("{:.4}", vertex.y)); ui.end_row();
                                                ui.label("Angle:"); ui.label(format!("{:.2}°", angle_deg)); ui.end_row();
                                                ui.label("Arc Radius:"); ui.label(format!("{:.4}", radius)); ui.end_row();
                                                if let Some(t) = text_override {
                                                    ui.label("Text:"); ui.label(t.as_str()); ui.end_row();
                                                }
                                            }
                                            EntityKind::DimRadial { center, radius, leader_pt, is_diameter, text_override, .. } => {
                                                let val = if *is_diameter { radius * 2.0 } else { *radius };
                                                ui.label("Type:"); ui.label(if *is_diameter { "Diameter" } else { "Radius" }); ui.end_row();
                                                ui.label("Center X:"); ui.label(format!("{:.4}", center.x)); ui.end_row();
                                                ui.label("Center Y:"); ui.label(format!("{:.4}", center.y)); ui.end_row();
                                                ui.label("Leader X:"); ui.label(format!("{:.4}", leader_pt.x)); ui.end_row();
                                                ui.label("Leader Y:"); ui.label(format!("{:.4}", leader_pt.y)); ui.end_row();
                                                ui.label("Measured:"); ui.label(format!("{:.4}", val)); ui.end_row();
                                                if let Some(t) = text_override {
                                                    ui.label("Text:"); ui.label(t.as_str()); ui.end_row();
                                                }
                                            }
                                            EntityKind::Text { position, content, height, rotation, font_name } => {
                                                ui.label("X:"); ui.label(format!("{:.4}", position.x)); ui.end_row();
                                                ui.label("Y:"); ui.label(format!("{:.4}", position.y)); ui.end_row();
                                                ui.label("Content:"); ui.label(content.as_str()); ui.end_row();
                                                ui.label("Height:"); ui.label(format!("{:.4}", height)); ui.end_row();
                                                ui.label("Rotation:"); ui.label(format!("{:.2}°", rotation.to_degrees())); ui.end_row();
                                                ui.label("Font:"); ui.label(font_name.as_str()); ui.end_row();
                                            }
                                            EntityKind::Insert { name, position, rotation, scale_x, scale_y } => {
                                                ui.label("Block:"); ui.label(name.as_str()); ui.end_row();
                                                ui.label("X:"); ui.label(format!("{:.4}", position.x)); ui.end_row();
                                                ui.label("Y:"); ui.label(format!("{:.4}", position.y)); ui.end_row();
                                                ui.label("Rotation:"); ui.label(format!("{:.2}°", rotation.to_degrees())); ui.end_row();
                                                ui.label("Scale X:"); ui.label(format!("{:.4}", scale_x)); ui.end_row();
                                                ui.label("Scale Y:"); ui.label(format!("{:.4}", scale_y)); ui.end_row();
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

                        if linetype_supported {
                            let linetype_display = if linetype_bylayer_mixed || linetype_mixed {
                                "*varies*".to_string()
                            } else if common_linetype_by_layer == Some(true) {
                                "ByLayer".to_string()
                            } else {
                                common_linetype
                                    .unwrap_or(Linetype::Continuous)
                                    .as_str()
                                    .to_string()
                            };
                            ui.horizontal(|ui| {
                                ui.label("Linetype:")
                                    .on_hover_text("ByLayer uses the layer linetype. Other values override per entity.");
                                egui::ComboBox::from_id_source("prop_linetype_combo")
                                    .selected_text(linetype_display)
                                    .width(110.0)
                                    .show_ui(ui, |ui| {
                                        let selected_bylayer = !linetype_bylayer_mixed
                                            && common_linetype_by_layer == Some(true);
                                        if ui.selectable_label(selected_bylayer, "ByLayer").clicked() {
                                            assign_entity_linetype_bylayer = Some(true);
                                        }
                                        let choices = [
                                            Linetype::Continuous,
                                            Linetype::Hidden,
                                            Linetype::Center,
                                        ];
                                        for lt in choices {
                                            let selected = !linetype_bylayer_mixed
                                                && common_linetype_by_layer == Some(false)
                                                && !linetype_mixed
                                                && common_linetype == Some(lt);
                                            if ui.selectable_label(selected, lt.as_str()).clicked() {
                                                assign_entity_linetype_bylayer = Some(false);
                                                assign_entity_linetype = Some(lt);
                                            }
                                        }
                                    });
                            });
                            ui.horizontal(|ui| {
                                ui.label("LTScale:")
                                    .on_hover_text("ByLayer uses the layer LT scale. Numeric value overrides per entity.");
                                if linetype_scale_mixed {
                                    ui.label(
                                        egui::RichText::new("varies")
                                            .small()
                                            .color(egui::Color32::from_gray(120)),
                                    );
                                } else if common_linetype_scale == Some(None) {
                                    ui.label(
                                        egui::RichText::new("ByLayer")
                                            .small()
                                            .color(egui::Color32::from_gray(120)),
                                    );
                                }
                                if ui
                                    .small_button("ByLayer")
                                    .on_hover_text("Use layer linetype scale for selected entities.")
                                    .clicked()
                                {
                                    assign_entity_linetype_scale = Some(None);
                                }
                                let mut lt_scale_edit = common_linetype_scale
                                    .flatten()
                                    .or_else(|| {
                                        common_layer.and_then(|lid| {
                                            self.drawing.get_layer(lid).map(|l| l.linetype_scale)
                                        })
                                    })
                                    .unwrap_or(1.0);
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut lt_scale_edit)
                                            .clamp_range(0.01..=1000.0)
                                            .speed(0.1),
                                    )
                                    .changed()
                                {
                                    assign_entity_linetype_scale =
                                        Some(Some(lt_scale_edit.clamp(0.01, 1000.0)));
                                }
                            });
                        } else {
                            ui.horizontal(|ui| {
                                ui.label("Linetype:");
                                ui.label(egui::RichText::new("N/A").color(egui::Color32::from_gray(120)));
                            });
                        }

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
                            if ui.selectable_label(bylayer_active, "ByLayer")
                                .on_hover_text("Use layer colour")
                                .clicked()
                                && !bylayer_active
                            {
                                set_entity_bylayer = true;
                            }

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

            if let Some(id) = toggle_visible {
                if let Some(l) = self.drawing.get_layer_mut(id) {
                    l.visible = !l.visible;
                }
            }
            if let Some(id) = toggle_locked {
                if let Some(l) = self.drawing.get_layer_mut(id) {
                    l.locked = !l.locked;
                }
                if self.current_layer == id && self.is_layer_locked(id) {
                    if let Some(fallback) = layer_ids
                        .iter()
                        .copied()
                        .find(|lid| !self.is_layer_locked(*lid))
                    {
                        self.current_layer = fallback;
                        self.command_log.push(format!(
                            "LAYER: Current layer was locked; switched current to {}",
                            fallback
                        ));
                    } else {
                        if let Some(l) = self.drawing.get_layer_mut(id) {
                            l.locked = false;
                        }
                        self.command_log
                            .push("LAYER: At least one unlocked layer is required".to_string());
                    }
                }
            }
            if let Some(id) = toggle_frozen {
                let mut is_now_frozen = false;
                if let Some(l) = self.drawing.get_layer_mut(id) {
                    l.frozen = !l.frozen;
                    is_now_frozen = l.frozen;
                }
                if is_now_frozen {
                    self.selected_entities
                        .retain(|eid| self.drawing.get_entity(eid).map(|e| e.layer != id).unwrap_or(true));
                    if self.current_layer == id {
                        if let Some(fallback) = layer_ids
                            .iter()
                            .copied()
                            .find(|lid| !self.is_layer_locked(*lid))
                        {
                            self.current_layer = fallback;
                            self.command_log.push(format!(
                                "LAYER: Current layer was frozen; switched current to {}",
                                fallback
                            ));
                        } else {
                            if let Some(l) = self.drawing.get_layer_mut(id) {
                                l.frozen = false;
                            }
                            self.command_log
                                .push("LAYER: At least one thawed unlocked layer is required".to_string());
                        }
                    }
                }
            }
            if let Some(id) = set_current {
                if self.is_layer_locked(id) {
                    self.command_log
                        .push("LAYER: Cannot set a locked/frozen layer as current".to_string());
                } else {
                    self.current_layer = id;
                }
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
                    if let Some(l) = self.drawing.get_layer_mut(id) {
                        l.name = new_name;
                    }
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
                    let _ = self.drawing.remove_layer(id);
                }
            }

            if let Some(lid) = assign_entity_layer {
                if self.is_layer_locked(lid) {
                    self.command_log
                        .push("PROPERTIES: Cannot assign entities to a locked layer".to_string());
                } else {
                    let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                    let ids = self.filter_editable_entity_ids(&requested, "PROPERTIES");
                    for id in &ids {
                        if let Some(e) = self.drawing.get_entity_mut(id) {
                            e.layer = lid;
                        }
                    }
                }
            }

            if let Some(lt) = assign_entity_linetype {
                let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                let ids = self.filter_editable_entity_ids(&requested, "PROPERTIES");
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) {
                        e.linetype = lt;
                    }
                }
            }
            if let Some(bylayer) = assign_entity_linetype_bylayer {
                let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                let ids = self.filter_editable_entity_ids(&requested, "PROPERTIES");
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) {
                        e.linetype_by_layer = bylayer;
                    }
                }
            }
            if let Some(scale_override) = assign_entity_linetype_scale {
                let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                let ids = self.filter_editable_entity_ids(&requested, "PROPERTIES");
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) {
                        e.linetype_scale = scale_override;
                    }
                }
            }

            if set_entity_bylayer {
                let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                let ids = self.filter_editable_entity_ids(&requested, "PROPERTIES");
                for id in &ids {
                    if let Some(e) = self.drawing.get_entity_mut(id) {
                        e.color = None;
                    }
                }
            }

            if open_entity_color_picker {
                self.entity_color_picker_open = true;
            }
        });
    }

    fn draw_command_line(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("command_line")
            .min_height(60.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // ── Status bar (full-width, no overflow into right panel) ──────────
                    if let Some(viewport) = &self.viewport {
                        let zoom = viewport.zoom;
                        let pan_x = viewport.pan_x;
                        let pan_y = viewport.pan_y;
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut self.snap_enabled, "Snap (F3)");
                            ui.menu_button("Snap Modes", |ui| {
                                ui.set_min_width(180.0);
                                ui.checkbox(&mut self.snap_endpoint, "Endpoint");
                                ui.checkbox(&mut self.snap_midpoint, "Midpoint");
                                ui.checkbox(&mut self.snap_center, "Center");
                                ui.checkbox(&mut self.snap_quadrant, "Quadrant");
                                ui.checkbox(&mut self.snap_intersection, "Intersection");
                                ui.checkbox(&mut self.snap_parallel, "Parallel");
                                ui.checkbox(&mut self.snap_perpendicular, "Perpendicular");
                                ui.checkbox(&mut self.snap_tangent, "Tangent");
                                ui.checkbox(&mut self.snap_nearest, "Nearest");
                                ui.separator();
                                if ui.button("All On").clicked() {
                                    self.set_all_snap_modes(true);
                                }
                                if ui.button("All Off").clicked() {
                                    self.set_all_snap_modes(false);
                                }
                            });
                            ui.separator();
                            ui.checkbox(&mut self.grid_visible, "Grid");
                            ui.add(
                                egui::DragValue::new(&mut self.grid_spacing)
                                    .clamp_range(0.5..=500.0)
                                    .speed(0.5)
                                    .suffix("\""),
                            );
                            ui.separator();
                            ui.label("LT Scale");
                            let mut lt = self.drawing.linetype_scale;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut lt)
                                        .clamp_range(0.01..=1000.0)
                                        .speed(0.1),
                                )
                                .changed()
                            {
                                self.drawing.linetype_scale = lt.clamp(0.01, 1000.0);
                            }
                            ui.separator();
                            ui.label(format!("Zoom: {:.2}x  Pan: ({:.2}, {:.2})", zoom, pan_x, pan_y));
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
                            for preset in [90.0_f64, 45.0, 22.5] {
                                if ui.small_button(format!("{preset:.1}°")).clicked() {
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
                                ActiveTool::Line { start: Some(s) } => format!("Tool: line ({:.3}, {:.3})", s.x, s.y),
                                ActiveTool::Circle { center: None } => "Tool: circle (pick center)".to_string(),
                                ActiveTool::Circle { center: Some(c) } => format!("Tool: circle ({:.3}, {:.3})", c.x, c.y),
                                ActiveTool::Arc { start: None, .. } => "Tool: arc (pick start)".to_string(),
                                ActiveTool::Arc { start: Some(s), mid: None } => format!("Tool: arc start ({:.3}, {:.3})", s.x, s.y),
                                ActiveTool::Arc { start: Some(_), mid: Some(_) } => "Tool: arc (pick end)".to_string(),
                                ActiveTool::Polyline { points } => format!("Tool: polyline ({} pts)", points.len()),
                            });
                            if !self.selected_entities.is_empty() {
                                ui.separator();
                                ui.label(format!("Sel: {}", self.selected_entities.len()));
                            }
                        });
                        ui.separator();
                    }
                    // ── Command input ────────────────────────────────────────────────
                    let prompt = self.current_prompt();
                    let mut committed_cmd: Option<String> = None;

                    ui.horizontal(|ui| {
                        // Ensure bright text for visibility on dark themes.
                        let old_override = ui.visuals_mut().override_text_color;
                        ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

                        ui.label(egui::RichText::new(prompt).monospace());

                        let edit = ui.add_sized(
                            [ui.available_width(), 26.0],
                            egui::TextEdit::singleline(&mut self.command_input)
                                .font(egui::FontId::monospace(15.0))
                                .hint_text("type command or value…")
                                .text_color(egui::Color32::WHITE)
                                .frame(true)
                                .id(egui::Id::new("cmd_input")),
                        );

                        // Keep focus glued to the command line unless the user is
                        // interacting with another text-heavy dialog/window.
                        if self.layer_editing_id.is_none()
                            && self.layer_color_picking.is_none()
                            && self.text_edit_dialog.is_none()
                            && self.dim_edit_dialog.is_none()
                            && self.dim_style_dialog.is_none()
                            && !self.python_console_open
                            && !self.ai_command_open
                        {
                            ui.memory_mut(|m| m.request_focus(edit.id));
                        }

                        let enter = edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if enter {
                            committed_cmd = Some(self.command_input.trim().to_string());
                            self.command_input.clear();
                        }
                        ui.visuals_mut().override_text_color = old_override;
                    });

                    if let Some(cmd) = committed_cmd {
                        if cmd.is_empty() {
                            // Empty Enter advances active multi-step commands (trim/extend/move/copy/rotate/dim).
                            if let Some(handle) = self.dim_grip_drag {
                                if !self.dim_grip_is_dragging {
                                    if let Some(world) = self.hover_world_pos {
                                        self.push_undo();
                                        self.apply_dim_grip_drag(handle, world);
                                        self.dim_grip_drag = None;
                                        self.dim_grip_is_dragging = false;
                                    } else {
                                        self.command_log.push(
                                            "DIM GRIP: move cursor or type distance".to_string(),
                                        );
                                    }
                                }
                            } else if self.from_phase == FromPhase::WaitingBase {
                                if let Some(world) = self.hover_world_pos {
                                    self.from_base = Some(world);
                                    self.from_phase = FromPhase::WaitingOffset;
                                    self.command_log.push(format!(
                                        "  Base: {:.4}, {:.4}",
                                        world.x, world.y
                                    ));
                                    self.command_log.push(
                                        "FROM  Offset (@dx,dy  or  @dist<angle):".to_string(),
                                    );
                                } else {
                                    self.command_log
                                        .push("  *FROM: move cursor to pick base*".to_string());
                                }
                            } else if self.from_phase == FromPhase::WaitingOffset {
                                if let (Some(base), Some(hover)) = (
                                    self.from_base,
                                    self.hover_world_pos.or(self.last_hover_world_pos),
                                ) {
                                    let mut target = hover;
                                    if self.ortho_enabled {
                                        target = Self::snap_angle(base, hover, self.ortho_increment_deg);
                                    }
                                    self.apply_from_result_point(target);
                                } else {
                                    self.command_log.push(
                                        "  *FROM: move cursor or type offset/distance*".to_string(),
                                    );
                                }
                            } else
                            if self.move_phase == MovePhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("MOVE: No entities selected".to_string());
                                } else {
                                    let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                                    self.move_entities = self.filter_editable_entity_ids(&requested, "MOVE");
                                    if self.move_entities.is_empty() {
                                        self.command_log.push("MOVE: No editable entities selected".to_string());
                                    } else {
                                        self.move_phase = MovePhase::BasePoint;
                                        self.command_log.push("MOVE: Pick base point".to_string());
                                    }
                                }
                            } else if self.move_phase == MovePhase::BasePoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.move_base_point = Some(world);
                                    self.move_phase = MovePhase::Destination;
                                    self.command_log.push("MOVE: Pick destination point".to_string());
                                }
                            } else if self.move_phase == MovePhase::Destination {
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_move(world);
                                } else {
                                    self.exit_move();
                                }
                            } else if self.copy_phase == CopyPhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("COPY: No entities selected".to_string());
                                } else {
                                    let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                                    self.copy_entities = self.filter_editable_entity_ids(&requested, "COPY");
                                    if self.copy_entities.is_empty() {
                                        self.command_log.push("COPY: No editable entities selected".to_string());
                                    } else {
                                        self.copy_phase = CopyPhase::BasePoint;
                                        self.command_log.push("COPY: Pick base point".to_string());
                                    }
                                }
                            } else if self.copy_phase == CopyPhase::BasePoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.copy_base_point = Some(world);
                                    self.copy_phase = CopyPhase::Destination;
                                    self.command_log.push("COPY: Pick destination (RClick/Enter=done)".to_string());
                                }
                            } else if self.copy_phase == CopyPhase::Destination {
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_copy(world);
                                } else {
                                    self.exit_copy();
                                    self.command_log.push("COPY done.".to_string());
                                }
                            } else if self.array_phase == ArrayPhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("ARRAY: No entities selected".to_string());
                                } else {
                                    let requested: Vec<Guid> =
                                        self.selected_entities.iter().copied().collect();
                                    if self.try_start_array_edit_from_selection(&requested) {
                                    } else {
                                        self.array_entities =
                                            self.filter_editable_entity_ids(&requested, "ARRAY");
                                        if self.array_entities.is_empty() {
                                            self.command_log
                                                .push("ARRAY: No editable entities selected".to_string());
                                        } else {
                                            self.array_phase = ArrayPhase::ChoosingType;
                                            self.command_log.push(
                                                "ARRAY: Choose type [Rectangular/Polar] <Rectangular>"
                                                    .to_string(),
                                            );
                                        }
                                    }
                                }
                            } else if self.array_phase == ArrayPhase::ChoosingType {
                                self.array_mode = ArrayMode::Rectangular;
                                self.array_phase = ArrayPhase::RectBasePoint;
                                self.command_log.push("ARRAY: Specify base point".to_string());
                            } else if self.array_phase == ArrayPhase::RectEnteringCount
                                || self.array_phase == ArrayPhase::RectEnteringSpacing
                            {
                                self.array_phase = ArrayPhase::RectBasePoint;
                                self.command_log.push("ARRAY: Specify base point".to_string());
                            } else if self.array_phase == ArrayPhase::RectBasePoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.array_center = Some(world);
                                    self.array_rect_dir_point =
                                        Some(Vec2::new(world.x + self.array_rect_dx.abs().max(1.0), world.y));
                                    self.array_phase = ArrayPhase::RectGripIdle;
                                    self.command_log.push(
                                        "ARRAY: Grips visible. Click any grip to activate/edit. Press Enter to apply"
                                            .to_string(),
                                    );
                                }
                            } else if matches!(
                                self.array_phase,
                                ArrayPhase::RectGripIdle
                                    | ArrayPhase::RectXSpacingGrip
                                    | ArrayPhase::RectXCountGrip
                                    | ArrayPhase::RectYSpacingGrip
                                    | ArrayPhase::RectYCountGrip
                            ) {
                                if let Some(world) = self.hover_world_pos {
                                    let _ = self.update_array_rect_from_world(world);
                                }
                                if let (Some(base), Some(dirp)) = (self.array_center, self.array_rect_dir_point) {
                                    if self.apply_array_rectangular(base, dirp) {
                                        self.exit_array();
                                    }
                                } else {
                                    self.command_log.push(
                                        "ARRAY: Set base point and direction first".to_string(),
                                    );
                                }
                            } else if self.array_phase == ArrayPhase::PolarEnteringCount {
                                self.array_phase = ArrayPhase::PolarEnteringAngle;
                                self.command_log.push(format!(
                                    "ARRAY: Enter fill angle degrees <{:.4}>",
                                    self.array_polar_angle_deg
                                ));
                            } else if self.array_phase == ArrayPhase::PolarEnteringAngle {
                                self.array_phase = ArrayPhase::PolarCenter;
                                self.command_log.push("ARRAY: Specify center point".to_string());
                            } else if self.array_phase == ArrayPhase::PolarCenter {
                                if let Some(world) = self.hover_world_pos {
                                    self.array_center = Some(world);
                                    self.array_phase = ArrayPhase::PolarBasePoint;
                                    self.command_log
                                        .push("ARRAY: Specify base/reference point".to_string());
                                }
                            } else if self.array_phase == ArrayPhase::PolarBasePoint {
                                if let (Some(world), Some(center)) = (self.hover_world_pos, self.array_center) {
                                    if self.apply_array_polar(center, world) {
                                        self.exit_array();
                                    }
                                }
                            } else if self.rotate_phase == RotatePhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("ROTATE: No entities selected".to_string());
                                } else {
                                    let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                                    self.rotate_entities = self.filter_editable_entity_ids(&requested, "ROTATE");
                                    if self.rotate_entities.is_empty() {
                                        self.command_log.push("ROTATE: No editable entities selected".to_string());
                                    } else {
                                        self.rotate_phase = RotatePhase::BasePoint;
                                        self.command_log.push("ROTATE: Pick base point".to_string());
                                    }
                                }
                            } else if self.rotate_phase == RotatePhase::BasePoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.rotate_base_point = Some(world);
                                    self.rotate_phase = RotatePhase::Rotation;
                                    self.command_log.push("ROTATE: Specify angle (degrees) or click".to_string());
                                }
                            } else if self.rotate_phase == RotatePhase::Rotation {
                                if let (Some(world), Some(base)) = (self.hover_world_pos, self.rotate_base_point) {
                                    let angle = (world.y - base.y).atan2(world.x - base.x);
                                    self.apply_rotate(angle);
                                } else {
                                    self.exit_rotate();
                                }
                            } else if self.scale_phase == ScalePhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("SCALE: No entities selected".to_string());
                                } else {
                                    let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                                    self.scale_entities = self.filter_editable_entity_ids(&requested, "SCALE");
                                    if self.scale_entities.is_empty() {
                                        self.command_log.push("SCALE: No editable entities selected".to_string());
                                    } else {
                                        self.scale_phase = ScalePhase::BasePoint;
                                        self.command_log.push("SCALE: Pick base point".to_string());
                                    }
                                }
                            } else if self.scale_phase == ScalePhase::BasePoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.scale_base_point = Some(world);
                                    self.scale_phase = ScalePhase::ReferencePoint;
                                    self.command_log.push("SCALE: Pick reference point".to_string());
                                }
                            } else if self.scale_phase == ScalePhase::ReferencePoint {
                                if let (Some(world), Some(base)) = (self.hover_world_pos, self.scale_base_point) {
                                    if base.distance_to(&world) > 1e-9 {
                                        self.scale_ref_point = Some(world);
                                        self.scale_phase = ScalePhase::Factor;
                                        self.command_log.push("SCALE: Specify factor or pick point".to_string());
                                    } else {
                                        self.command_log.push("SCALE: Reference point too close to base".to_string());
                                    }
                                }
                            } else if self.scale_phase == ScalePhase::Factor {
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_scale_from_point(world);
                                }
                            } else if self.mirror_phase == MirrorPhase::SelectingEntities {
                                if self.selected_entities.is_empty() {
                                    self.command_log.push("MIRROR: No entities selected".to_string());
                                } else {
                                    let requested: Vec<Guid> = self.selected_entities.iter().copied().collect();
                                    self.mirror_entities = self.filter_editable_entity_ids(&requested, "MIRROR");
                                    if self.mirror_entities.is_empty() {
                                        self.command_log.push("MIRROR: No editable entities selected".to_string());
                                    } else {
                                        self.mirror_phase = MirrorPhase::FirstAxisPoint;
                                        self.command_log.push("MIRROR: Pick first axis point".to_string());
                                    }
                                }
                            } else if self.mirror_phase == MirrorPhase::FirstAxisPoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.mirror_axis_p1 = Some(world);
                                    self.mirror_phase = MirrorPhase::SecondAxisPoint;
                                    self.command_log.push("MIRROR: Pick second axis point".to_string());
                                }
                            } else if self.mirror_phase == MirrorPhase::SecondAxisPoint {
                                if let (Some(world), Some(p1)) = (self.hover_world_pos, self.mirror_axis_p1) {
                                    let axis_p2 = if self.ortho_enabled {
                                        Self::snap_angle(p1, world, self.ortho_increment_deg)
                                    } else {
                                        world
                                    };
                                    self.apply_mirror(p1, axis_p2);
                                } else {
                                    self.exit_mirror();
                                }
                            } else if self.fillet_phase == FilletPhase::EnteringRadius {
                                self.fillet_phase = FilletPhase::FirstEntity;
                                self.command_log.push(format!(
                                    "FILLET: Radius {:.4}. Select first line or polyline segment",
                                    self.fillet_radius
                                ));
                            } else if self.fillet_phase == FilletPhase::FirstEntity {
                                self.exit_fillet();
                                self.command_log.push("FILLET cancelled.".to_string());
                            } else if matches!(self.fillet_phase, FilletPhase::SecondEntity { .. }) {
                                self.fillet_phase = FilletPhase::FirstEntity;
                                self.command_log
                                    .push("FILLET: Select first line or polyline segment".to_string());
                            } else if self.chamfer_phase == ChamferPhase::EnteringDistance {
                                self.chamfer_phase = ChamferPhase::FirstEntity;
                                self.command_log.push(format!(
                                    "CHAMFER: Distances {:.4},{:.4}. Select first line or polyline segment",
                                    self.chamfer_distance1, self.chamfer_distance2
                                ));
                            } else if self.chamfer_phase == ChamferPhase::FirstEntity {
                                self.exit_chamfer();
                                self.command_log.push("CHAMFER cancelled.".to_string());
                            } else if matches!(self.chamfer_phase, ChamferPhase::SecondEntity { .. }) {
                                self.chamfer_phase = ChamferPhase::FirstEntity;
                                self.command_log
                                    .push("CHAMFER: Select first line or polyline segment".to_string());
                            } else if self.polygon_phase == PolygonPhase::EnteringSides {
                                self.polygon_phase = PolygonPhase::Center;
                                self.command_log.push(format!(
                                    "POLYGON: {} sides. Specify center point",
                                    self.polygon_sides
                                ));
                            } else if self.polygon_phase == PolygonPhase::Center {
                                if let Some(world) = self.hover_world_pos {
                                    self.polygon_phase = PolygonPhase::Radius { center: world };
                                    self.command_log.push("POLYGON: Specify radius point".to_string());
                                } else {
                                    self.exit_polygon();
                                    self.command_log.push("POLYGON cancelled.".to_string());
                                }
                            } else if matches!(self.polygon_phase, PolygonPhase::Radius { .. }) {
                                if let (PolygonPhase::Radius { center }, Some(world)) =
                                    (self.polygon_phase.clone(), self.hover_world_pos)
                                {
                                    if self.apply_polygon(center, world) {
                                        self.polygon_phase = PolygonPhase::Center;
                                    }
                                } else {
                                    self.polygon_phase = PolygonPhase::Center;
                                    self.command_log.push("POLYGON: Specify center point".to_string());
                                }
                            } else if self.ellipse_phase == EllipsePhase::Center {
                                if let Some(world) = self.hover_world_pos {
                                    self.ellipse_phase = EllipsePhase::RadiusX { center: world };
                                    self.command_log.push("ELLIPSE: Specify radius from center".to_string());
                                } else {
                                    self.exit_ellipse();
                                    self.command_log.push("ELLIPSE cancelled.".to_string());
                                }
                            } else if let EllipsePhase::RadiusX { center } = self.ellipse_phase {
                                if let Some(world) = self.hover_world_pos {
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
                                } else {
                                    self.exit_ellipse();
                                    self.command_log.push("ELLIPSE cancelled.".to_string());
                                }
                            } else if let EllipsePhase::RadiusY { center, rx } = self.ellipse_phase {
                                if let Some(world) = self.hover_world_pos {
                                    let p = if self.ortho_enabled {
                                        Self::snap_angle(center, world, self.ortho_increment_deg)
                                    } else {
                                        world
                                    };
                                    let ry = center.distance_to(&p);
                                    if self.apply_ellipse(center, rx, ry) {
                                        self.ellipse_phase = EllipsePhase::Center;
                                    }
                                } else {
                                    self.ellipse_phase = EllipsePhase::Center;
                                    self.command_log.push("ELLIPSE: Specify center point".to_string());
                                }
                            } else if self.rectangle_phase == RectanglePhase::FirstCorner {
                                if let Some(world) = self.hover_world_pos {
                                    self.rectangle_phase = RectanglePhase::SecondCorner { first: world };
                                    self.command_log
                                        .push("RECTANGLE: Specify opposite corner or [D=Dimensions]".to_string());
                                } else {
                                    self.exit_rectangle();
                                    self.command_log.push("RECTANGLE cancelled.".to_string());
                                }
                            } else if let RectanglePhase::SecondCorner { first } = self.rectangle_phase {
                                if let Some(world) = self.hover_world_pos {
                                    if self.apply_rectangle_diagonal(first, world) {
                                        self.rectangle_phase = RectanglePhase::FirstCorner;
                                    }
                                } else {
                                    self.rectangle_phase = RectanglePhase::FirstCorner;
                                    self.command_log.push("RECTANGLE: Specify first corner".to_string());
                                }
                            } else if let RectanglePhase::EnteringDimensions { first } = self.rectangle_phase {
                                let w = self.rectangle_width.max(1e-9);
                                let h = self.rectangle_height.max(1e-9);
                                self.rectangle_phase = RectanglePhase::Direction { first, width: w, height: h };
                                self.command_log.push("RECTANGLE: Specify direction point".to_string());
                            } else if let RectanglePhase::Direction { first, width, height } = self.rectangle_phase {
                                if let Some(world) = self.hover_world_pos {
                                    if self.apply_rectangle_dimensions(first, width, height, world) {
                                        self.rectangle_phase = RectanglePhase::FirstCorner;
                                    }
                                } else {
                                    self.rectangle_phase = RectanglePhase::FirstCorner;
                                    self.command_log.push("RECTANGLE: Specify first corner".to_string());
                                }
                            } else if self.pedit_phase == PeditPhase::SelectingPolyline {
                                self.exit_pedit();
                                self.command_log.push("PEDIT cancelled.".to_string());
                            } else if matches!(self.pedit_phase, PeditPhase::Joining { .. }) {
                                self.exit_pedit();
                                self.command_log.push("PEDIT done.".to_string());
                            } else if self.boundary_phase == BoundaryPhase::PickingPoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_boundary_pick(world);
                                } else {
                                    self.exit_boundary();
                                    self.command_log.push("BOUNDARY cancelled.".to_string());
                                }
                            } else if self.hatch_phase == HatchPhase::PickingPoint {
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_hatch_pick(world);
                                } else {
                                    self.exit_hatch();
                                    self.command_log.push("HATCH cancelled.".to_string());
                                }
                            } else if matches!(self.block_phase, BlockPhase::PickBase { .. }) {
                                if let Some(world) = self.hover_world_pos {
                                    self.apply_block_base_pick(world);
                                } else {
                                    self.exit_block();
                                    self.command_log.push("BLOCK cancelled.".to_string());
                                }
                            } else if matches!(self.block_phase, BlockPhase::EnterName { .. }) {
                                self.command_log
                                    .push("BLOCK: Enter a block name".to_string());
                            } else if matches!(self.insert_phase, InsertPhase::PickPoint { .. }) {
                                self.exit_insert();
                                self.command_log.push("INSERT done.".to_string());
                            } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                if matches!(self.dim_phase, DimPhase::FirstPoint) {
                                    self.exit_dim();
                                    self.command_log.push("DIMALIGNED done.".to_string());
                                } else if let Some(world) = self.hover_world_pos {
                                    if let DimPhase::SecondPoint { first } = self.dim_phase {
                                        self.dim_phase = DimPhase::Placing { first, second: world };
                                        self.command_log.push(format!("DIMALIGNED: Second point ({:.4}, {:.4})", world.x, world.y));
                                    } else if let DimPhase::Placing { first, second } = self.dim_phase {
                                        self.place_dim_aligned(first, second, world);
                                    }
                                }
                            } else if !matches!(self.dim_linear_phase, DimLinearPhase::Idle) {
                                if matches!(self.dim_linear_phase, DimLinearPhase::FirstPoint) {
                                    self.exit_dim();
                                    self.command_log.push("DIMLINEAR done.".to_string());
                                } else if let Some(world) = self.hover_world_pos {
                                    if let DimLinearPhase::SecondPoint { first } = self.dim_linear_phase {
                                        self.dim_linear_phase = DimLinearPhase::Placing { first, second: world };
                                        self.command_log.push(format!("DIMLINEAR: Second point ({:.4}, {:.4})", world.x, world.y));
                                    } else if let DimLinearPhase::Placing { first, second } = self.dim_linear_phase {
                                        self.place_dim_linear(first, second, world);
                                    }
                                }
                            } else if !matches!(self.dim_angular_phase, DimAngularPhase::Idle) {
                                // FirstEntity / SecondEntity: Enter cancels (entity-pick, no hover point).
                                // Placing: Enter confirms hover world position as arc radius click.
                                if let DimAngularPhase::Placing { vertex, line1_pt, line2_pt } = self.dim_angular_phase {
                                    if let Some(world) = self.hover_world_pos {
                                        self.place_dim_angular(vertex, line1_pt, line2_pt, world);
                                    }
                                } else {
                                    self.exit_dim();
                                    self.command_log.push("DIMANGULAR cancelled.".to_string());
                                }
                            } else if self.text_phase == TextPhase::PlacingPosition {
                                // Empty Enter = confirm hover point as position.
                                if let Some(world) = self.hover_world_pos {
                                    self.text_phase = TextPhase::EnteringHeight { position: world };
                                    self.command_log.push(format!(
                                        "{}  Text height <{:.4}>:",
                                        if self.text_is_mtext { "MTEXT" } else { "TEXT" },
                                        self.last_text_height
                                    ));
                                }
                            } else if let TextPhase::EnteringHeight { position } = self.text_phase {
                                // Empty Enter = use last_text_height.
                                let h = self.last_text_height;
                                self.text_phase = TextPhase::EnteringRotation { position, height: h };
                                self.command_log.push(format!(
                                    "{}  Rotation angle <{:.1}>:",
                                    if self.text_is_mtext { "MTEXT" } else { "TEXT" },
                                    h.to_degrees()
                                ));
                            } else if let TextPhase::EnteringRotation { position, height } = self.text_phase {
                                // Empty Enter = use last_text_rotation.
                                let r = self.last_text_rotation;
                                self.text_phase = TextPhase::TypingContent { position, height, rotation: r };
                                self.command_log.push(
                                    if self.text_is_mtext {
                                        "MTEXT  Enter text (use \\P for new line):".to_string()
                                    } else {
                                        "TEXT  Enter text:".to_string()
                                    },
                                );
                            } else if let TextPhase::TypingContent { .. } = self.text_phase {
                                // Empty Enter in content phase = cancel.
                                self.exit_text();
                                self.command_log.push("*Cancel*".to_string());
                            } else if self.trim_phase == TrimPhase::SelectingEdges {
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
                                    self.command_log.push("EXTEND: Click near line or arc endpoint to extend".to_string());
                                }
                            } else if self.extend_phase == ExtendPhase::Extending {
                                self.exit_extend();
                                self.command_log.push("EXTEND done.".to_string());
                            }
                        } else {
                            // Echo typed command to log for visibility.
                            self.command_log.push(format!("› {}", cmd));

                            let mut handled = false;

                            if self.apply_typed_dim_grip_input(&cmd) {
                                handled = true;
                            } else if self.execute_command_alias(&cmd) {
                                handled = true;
                            } else if self.from_phase == FromPhase::WaitingBase {
                                if let Some(base) = Self::resolve_typed_point(&cmd, None) {
                                    self.from_base = Some(base);
                                    self.from_phase = FromPhase::WaitingOffset;
                                    self.command_log
                                        .push(format!("  Base: {:.4}, {:.4}", base.x, base.y));
                                    self.command_log.push(
                                        "FROM  Offset (@dx,dy  or  @dist<angle):".to_string(),
                                    );
                                } else {
                                    self.command_log
                                        .push("  *FROM: enter x,y for base point*".to_string());
                                }
                                handled = true;
                            } else if self.from_phase == FromPhase::WaitingOffset {
                                if let Some(result) = Self::resolve_typed_point(&cmd, self.from_base) {
                                    self.apply_from_result_point(result);
                                } else if let (Ok(dist), Some(base), Some(hover)) = (
                                    cmd.trim().parse::<f64>(),
                                    self.from_base,
                                    self.hover_world_pos.or(self.last_hover_world_pos),
                                )
                                {
                                    if dist > f64::EPSILON {
                                        let mut target = hover;
                                        if self.ortho_enabled {
                                            target =
                                                Self::snap_angle(base, hover, self.ortho_increment_deg);
                                        }
                                        let dx = target.x - base.x;
                                        let dy = target.y - base.y;
                                        let len = (dx * dx + dy * dy).sqrt();
                                        if len > f64::EPSILON {
                                            let result = cadkit_types::Vec2::new(
                                                base.x + dx / len * dist,
                                                base.y + dy / len * dist,
                                            );
                                            self.apply_from_result_point(result);
                                        } else {
                                            self.command_log
                                                .push("  *FROM: need cursor direction*".to_string());
                                        }
                                    }
                                } else {
                                    self.command_log
                                        .push("  *FROM: enter @dx,dy or @dist<angle*".to_string());
                                }
                                handled = true;
                            } else if self.boundary_phase == BoundaryPhase::PickingPoint {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.apply_boundary_pick(world);
                                } else {
                                    self.command_log
                                        .push("BOUNDARY: Enter point as x,y or @x,y".to_string());
                                }
                                handled = true;
                            } else if self.hatch_phase == HatchPhase::PickingPoint {
                                let raw = cmd.trim();
                                if let Some(world) = Self::resolve_typed_point(raw, None) {
                                    self.apply_hatch_pick(world);
                                } else if let Some((a, b)) = raw.split_once(',') {
                                    let s = a.trim().parse::<f64>().ok();
                                    let ang = b.trim().parse::<f64>().ok();
                                    if let (Some(spacing), Some(angle)) = (s, ang) {
                                        if spacing > 1e-9 {
                                            self.hatch_spacing = spacing;
                                            self.hatch_angle_deg = angle;
                                            self.command_log.push(format!(
                                                "HATCH: Spacing {:.4}, angle {:.1}deg",
                                                self.hatch_spacing, self.hatch_angle_deg
                                            ));
                                        } else {
                                            self.command_log.push("HATCH: Spacing must be > 0".to_string());
                                        }
                                    } else {
                                        self.command_log.push(
                                            "HATCH: Enter point x,y or spacing,angle".to_string(),
                                        );
                                    }
                                } else {
                                    self.command_log.push(
                                        "HATCH: Enter point x,y or spacing,angle".to_string(),
                                    );
                                }
                                handled = true;
                            } else if matches!(self.block_phase, BlockPhase::PickBase { .. }) {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.apply_block_base_pick(world);
                                } else {
                                    self.command_log
                                        .push("BLOCK: Enter base point as x,y or click".to_string());
                                }
                                handled = true;
                            } else if matches!(self.block_phase, BlockPhase::EnterName { .. }) {
                                if cmd.trim().is_empty() {
                                    self.command_log
                                        .push("BLOCK: Name cannot be empty".to_string());
                                } else {
                                    let _ = self.execute_command_alias(&cmd);
                                }
                                handled = true;
                            } else if matches!(self.insert_phase, InsertPhase::PickPoint { .. }) {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.apply_insert_pick(world);
                                } else {
                                    self.command_log
                                        .push("INSERT: Enter insertion point as x,y or click".to_string());
                                }
                                handled = true;
                            } else if self.fillet_phase == FilletPhase::EnteringRadius {
                                if let Ok(r) = cmd.trim().parse::<f64>() {
                                    if r > f64::EPSILON {
                                        self.fillet_radius = r;
                                        self.fillet_phase = FilletPhase::FirstEntity;
                                        self.command_log.push(format!(
                                            "FILLET: Radius {:.4}. Select first line or polyline segment",
                                            self.fillet_radius
                                        ));
                                    } else {
                                        self.command_log.push("  *FILLET: radius must be > 0*".to_string());
                                    }
                                } else {
                                    self.command_log.push("  *FILLET: enter numeric radius*".to_string());
                                }
                                handled = true;
                            } else if self.chamfer_phase == ChamferPhase::EnteringDistance {
                                let raw = cmd.trim();
                                let parsed = if let Some((a, b)) = raw.split_once(',') {
                                    let d1 = a.trim().parse::<f64>().ok();
                                    let d2 = b.trim().parse::<f64>().ok();
                                    match (d1, d2) {
                                        (Some(x), Some(y)) => Some((x, y)),
                                        _ => None,
                                    }
                                } else {
                                    raw.parse::<f64>().ok().map(|d| (d, d))
                                };
                                if let Some((d1, d2)) = parsed {
                                    if d1 >= 0.0 && d2 >= 0.0 {
                                        self.chamfer_distance1 = d1;
                                        self.chamfer_distance2 = d2;
                                        self.chamfer_phase = ChamferPhase::FirstEntity;
                                        self.command_log.push(format!(
                                            "CHAMFER: Distances {:.4},{:.4}. Select first line or polyline segment",
                                            self.chamfer_distance1, self.chamfer_distance2
                                        ));
                                    } else {
                                        self.command_log
                                            .push("  *CHAMFER: distances must be >= 0*".to_string());
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *CHAMFER: enter d or d1,d2 (example: 2 or 2,5)*".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.polygon_phase == PolygonPhase::EnteringSides {
                                if let Ok(n) = cmd.trim().parse::<usize>() {
                                    if n >= 3 {
                                        self.polygon_sides = n;
                                        self.polygon_phase = PolygonPhase::Center;
                                        self.command_log.push(format!(
                                            "POLYGON: {} sides. Specify center point",
                                            self.polygon_sides
                                        ));
                                    } else {
                                        self.command_log
                                            .push("  *POLYGON: sides must be >= 3*".to_string());
                                    }
                                } else {
                                    self.command_log
                                        .push("  *POLYGON: enter integer side count*".to_string());
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::ChoosingType {
                                let raw = cmd.trim().to_ascii_lowercase();
                                if raw.is_empty() || raw.starts_with('r') {
                                    self.array_mode = ArrayMode::Rectangular;
                                    self.array_phase = ArrayPhase::RectBasePoint;
                                    self.command_log.push("ARRAY: Specify base point".to_string());
                                } else if raw.starts_with('p') {
                                    self.array_mode = ArrayMode::Polar;
                                    self.array_phase = ArrayPhase::PolarEnteringCount;
                                    self.command_log.push(format!(
                                        "ARRAY: Enter item count <{}>",
                                        self.array_polar_count
                                    ));
                                } else {
                                    self.command_log.push("  *ARRAY: type R or P*".to_string());
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectEnteringCount {
                                let raw = cmd.trim();
                                let parsed = if let Some((a, b)) = raw.split_once(',') {
                                    let c = a.trim().parse::<usize>().ok();
                                    let r = b.trim().parse::<usize>().ok();
                                    match (c, r) {
                                        (Some(c), Some(r)) => Some((c, r)),
                                        _ => None,
                                    }
                                } else {
                                    raw.parse::<usize>().ok().map(|c| (c, self.array_rect_rows))
                                };
                                if let Some((c, r)) = parsed {
                                    if c >= 1 && r >= 1 && !(c == 1 && r == 1) {
                                        self.array_rect_columns = c;
                                        self.array_rect_rows = r;
                                        self.array_phase = ArrayPhase::RectEnteringSpacing;
                                        self.command_log.push(format!(
                                            "ARRAY: Enter spacing dx,dy <{:.4},{:.4}>",
                                            self.array_rect_dx, self.array_rect_dy
                                        ));
                                    } else {
                                        self.command_log.push(
                                            "  *ARRAY: columns/rows must be >=1, not both 1*".to_string(),
                                        );
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *ARRAY: enter columns,rows (example: 4,3)*".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectEnteringSpacing {
                                let raw = cmd.trim();
                                let parsed = if let Some((a, b)) = raw.split_once(',') {
                                    let dx = a.trim().parse::<f64>().ok();
                                    let dy = b.trim().parse::<f64>().ok();
                                    match (dx, dy) {
                                        (Some(dx), Some(dy)) => Some((dx, dy)),
                                        _ => None,
                                    }
                                } else {
                                    raw.parse::<f64>().ok().map(|d| (d, d))
                                };
                                if let Some((dx, dy)) = parsed {
                                    if dx.abs() > 1e-9 || dy.abs() > 1e-9 {
                                        self.array_rect_dx = dx;
                                        self.array_rect_dy = dy;
                                        self.array_phase = ArrayPhase::RectBasePoint;
                                        self.command_log.push("ARRAY: Specify base point".to_string());
                                    } else {
                                        self.command_log.push("  *ARRAY: spacing too small*".to_string());
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *ARRAY: enter dx,dy (example: 10,8)*".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectBasePoint {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.array_center = Some(world);
                                    self.array_rect_dir_point =
                                        Some(Vec2::new(world.x + self.array_rect_dx.abs().max(1.0), world.y));
                                    self.array_phase = ArrayPhase::RectGripIdle;
                                    self.command_log.push(
                                        "ARRAY: Grips visible. Click any grip to activate/edit. Press Enter to apply"
                                            .to_string(),
                                    );
                                } else {
                                    self.command_log.push("  *ARRAY: enter x,y for base point*".to_string());
                                }
                                handled = true;
                            } else if matches!(
                                self.array_phase,
                                ArrayPhase::RectGripIdle
                                    | ArrayPhase::RectXSpacingGrip
                                    | ArrayPhase::RectXCountGrip
                                    | ArrayPhase::RectYSpacingGrip
                                    | ArrayPhase::RectYCountGrip
                            ) && cmd.trim().eq_ignore_ascii_case("e") {
                                if let Some(aid) = self.array_edit_assoc {
                                    if self.explode_assoc_rect_array(aid) {
                                        self.command_log
                                            .push("ARRAY: Exploded associative array".to_string());
                                        self.exit_array();
                                    } else {
                                        self.command_log
                                            .push("ARRAY: Nothing to explode".to_string());
                                    }
                                } else {
                                    self.command_log
                                        .push("ARRAY: Current selection is not an associative array".to_string());
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectGripIdle {
                                self.command_log
                                    .push("  *ARRAY: activate a grip first (click grip, then type)*".to_string());
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectXSpacingGrip {
                                let mut applied = false;
                                if let (Some(world), Some(base)) =
                                    (Self::resolve_typed_point(&cmd, None), self.array_center)
                                {
                                    let dir = if self.ortho_enabled {
                                        Self::snap_angle(base, world, self.ortho_increment_deg)
                                    } else {
                                        world
                                    };
                                    let d = base.distance_to(&dir);
                                    if d > 1e-9 {
                                        self.array_rect_dx = d;
                                        self.array_rect_dir_point = Some(dir);
                                        applied = true;
                                    } else {
                                        self.command_log
                                            .push("  *ARRAY: spacing must be > 0*".to_string());
                                    }
                                } else if let Ok(d) = cmd.trim().parse::<f64>() {
                                    if d > 1e-9 {
                                        self.array_rect_dx = d;
                                        if let Some(base) = self.array_center {
                                            self.array_rect_dir_point = Some(Vec2::new(base.x + d, base.y));
                                        }
                                        applied = true;
                                    } else {
                                        self.command_log
                                            .push("  *ARRAY: spacing must be > 0*".to_string());
                                    }
                                } else {
                                    self.command_log
                                        .push("  *ARRAY: enter spacing distance or x,y direction*".to_string());
                                }
                                if applied {
                                    self.array_phase = ArrayPhase::RectGripIdle;
                                    self.command_log.push(
                                        "ARRAY: X spacing set. Grip released (click another grip or Enter)".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectXCountGrip {
                                let raw = cmd.trim();
                                let mut applied = false;
                                let dist_qty = raw.split_once(',').and_then(|(a, b)| {
                                    Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<usize>().ok()?))
                                });
                                if let Some((d, q)) = dist_qty {
                                    if d > 1e-9 && q >= 1 {
                                        self.array_rect_dx = d;
                                        self.array_rect_columns = q.max(1);
                                        applied = true;
                                    } else {
                                        self.command_log.push(
                                            "  *ARRAY: use positive dist and quantity >= 1*".to_string(),
                                        );
                                    }
                                } else if let Ok(q) = raw.parse::<usize>() {
                                    if q >= 1 {
                                        self.array_rect_columns = q;
                                        applied = true;
                                    } else {
                                        self.command_log
                                            .push("  *ARRAY: quantity must be >= 1*".to_string());
                                    }
                                } else if let (Some(world), Some(base), Some(dirp)) =
                                    (Self::resolve_typed_point(raw, None), self.array_center, self.array_rect_dir_point)
                                {
                                    let vx = dirp.x - base.x;
                                    let vy = dirp.y - base.y;
                                    let len = (vx * vx + vy * vy).sqrt();
                                    if len > 1e-9 && self.array_rect_dx.abs() > 1e-9 {
                                        let ux = vx / len;
                                        let uy = vy / len;
                                        let proj = ((world.x - base.x) * ux + (world.y - base.y) * uy).abs();
                                        self.array_rect_columns =
                                            (proj / self.array_rect_dx.abs()).floor() as usize + 1;
                                        applied = true;
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *ARRAY: enter quantity or dist,qty (example: 12,6)*".to_string(),
                                    );
                                }
                                if applied {
                                    self.array_phase = ArrayPhase::RectGripIdle;
                                    self.command_log.push(
                                        "ARRAY: X count set. Grip released (click another grip or Enter)".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectYSpacingGrip {
                                let mut applied = false;
                                if let Ok(d) = cmd.trim().parse::<f64>() {
                                    if d > 1e-9 {
                                        self.array_rect_dy = d * self.array_rect_y_sign;
                                        applied = true;
                                    } else {
                                        self.command_log
                                            .push("  *ARRAY: spacing must be > 0*".to_string());
                                    }
                                } else if let (Some(world), Some(base), Some(dirp)) =
                                    (Self::resolve_typed_point(&cmd, None), self.array_center, self.array_rect_dir_point)
                                {
                                    let vx = dirp.x - base.x;
                                    let vy = dirp.y - base.y;
                                    let len = (vx * vx + vy * vy).sqrt();
                                    if len > 1e-9 {
                                        let ux = vx / len;
                                        let uy = vy / len;
                                        let px = -uy;
                                        let py = ux;
                                        let proj = (world.x - base.x) * px + (world.y - base.y) * py;
                                        let d = proj.abs();
                                        if d > 1e-9 {
                                            self.array_rect_y_sign = if proj >= 0.0 { 1.0 } else { -1.0 };
                                            self.array_rect_dy = d * self.array_rect_y_sign;
                                            applied = true;
                                        }
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *ARRAY: enter spacing distance or x,y vertical grip*".to_string(),
                                    );
                                }
                                if applied {
                                    self.array_phase = ArrayPhase::RectGripIdle;
                                    self.command_log.push(
                                        "ARRAY: Y spacing set. Grip released (click another grip or Enter)".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::RectYCountGrip {
                                let raw = cmd.trim();
                                let mut applied = false;
                                let dist_qty = raw.split_once(',').and_then(|(a, b)| {
                                    Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<usize>().ok()?))
                                });
                                if let Some((d, q)) = dist_qty {
                                    if d > 1e-9 && q >= 1 {
                                        self.array_rect_dy = d * self.array_rect_y_sign;
                                        self.array_rect_rows = q.max(1);
                                        applied = true;
                                    } else {
                                        self.command_log.push(
                                            "  *ARRAY: use positive dist and quantity >= 1*".to_string(),
                                        );
                                    }
                                } else if let Ok(q) = raw.parse::<usize>() {
                                    if q >= 1 {
                                        self.array_rect_rows = q;
                                        applied = true;
                                    } else {
                                        self.command_log
                                            .push("  *ARRAY: quantity must be >= 1*".to_string());
                                    }
                                } else if let (Some(world), Some(base), Some(dirp)) =
                                    (Self::resolve_typed_point(raw, None), self.array_center, self.array_rect_dir_point)
                                {
                                    let vx = dirp.x - base.x;
                                    let vy = dirp.y - base.y;
                                    let len = (vx * vx + vy * vy).sqrt();
                                    if len > 1e-9 && self.array_rect_dy.abs() > 1e-9 {
                                        let ux = vx / len;
                                        let uy = vy / len;
                                        let px = -uy;
                                        let py = ux;
                                        let proj = ((world.x - base.x) * px + (world.y - base.y) * py).abs();
                                        self.array_rect_rows =
                                            (proj / self.array_rect_dy.abs()).floor() as usize + 1;
                                        applied = true;
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *ARRAY: enter quantity or dist,qty (example: 8,4)*".to_string(),
                                    );
                                }
                                if applied {
                                    self.array_phase = ArrayPhase::RectGripIdle;
                                    self.command_log.push(
                                        "ARRAY: Y count set. Grip released (click another grip or Enter)".to_string(),
                                    );
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::PolarEnteringCount {
                                if let Ok(n) = cmd.trim().parse::<usize>() {
                                    if n >= 2 {
                                        self.array_polar_count = n;
                                        self.array_phase = ArrayPhase::PolarEnteringAngle;
                                        self.command_log.push(format!(
                                            "ARRAY: Enter fill angle degrees <{:.4}>",
                                            self.array_polar_angle_deg
                                        ));
                                    } else {
                                        self.command_log.push("  *ARRAY: count must be >= 2*".to_string());
                                    }
                                } else {
                                    self.command_log.push("  *ARRAY: enter integer count*".to_string());
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::PolarEnteringAngle {
                                if let Ok(a) = cmd.trim().parse::<f64>() {
                                    if a.abs() > 1e-9 {
                                        self.array_polar_angle_deg = a;
                                        self.array_phase = ArrayPhase::PolarCenter;
                                        self.command_log.push("ARRAY: Specify center point".to_string());
                                    } else {
                                        self.command_log.push("  *ARRAY: angle must be non-zero*".to_string());
                                    }
                                } else {
                                    self.command_log.push("  *ARRAY: enter angle in degrees*".to_string());
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::PolarCenter {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.array_center = Some(world);
                                    self.array_phase = ArrayPhase::PolarBasePoint;
                                    self.command_log.push("ARRAY: Specify base/reference point".to_string());
                                } else {
                                    self.command_log.push("  *ARRAY: enter x,y for center*".to_string());
                                }
                                handled = true;
                            } else if self.array_phase == ArrayPhase::PolarBasePoint {
                                if let (Some(world), Some(center)) =
                                    (Self::resolve_typed_point(&cmd, None), self.array_center)
                                {
                                    if self.apply_array_polar(center, world) {
                                        self.exit_array();
                                    }
                                } else {
                                    self.command_log
                                        .push("  *ARRAY: enter x,y for base/reference point*".to_string());
                                }
                                handled = true;
                            } else if self.rectangle_phase == RectanglePhase::FirstCorner {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.rectangle_phase = RectanglePhase::SecondCorner { first: world };
                                    self.command_log
                                        .push("RECTANGLE: Specify opposite corner or [D=Dimensions]".to_string());
                                } else {
                                    self.command_log.push("  *RECTANGLE: enter x,y for first corner*".to_string());
                                }
                                handled = true;
                            } else if let RectanglePhase::SecondCorner { first } = self.rectangle_phase {
                                let raw = cmd.trim().to_ascii_lowercase();
                                if matches!(raw.as_str(), "d" | "dim" | "dims" | "dimensions") {
                                    self.rectangle_phase = RectanglePhase::EnteringDimensions { first };
                                    self.command_log.push(format!(
                                        "RECTANGLE: Enter dimensions w,h <{:.4},{:.4}>",
                                        self.rectangle_width, self.rectangle_height
                                    ));
                                } else if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    if self.apply_rectangle_diagonal(first, world) {
                                        self.rectangle_phase = RectanglePhase::FirstCorner;
                                    }
                                } else {
                                    self.command_log.push(
                                        "  *RECTANGLE: enter x,y or D for dimensions mode*".to_string(),
                                    );
                                }
                                handled = true;
                            } else if let RectanglePhase::EnteringDimensions { first } = self.rectangle_phase {
                                let raw = cmd.trim();
                                let parsed = raw
                                    .split_once(',')
                                    .and_then(|(a, b)| Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?)));
                                if let Some((w, h)) = parsed {
                                    if w > 0.0 && h > 0.0 {
                                        self.rectangle_width = w;
                                        self.rectangle_height = h;
                                        self.rectangle_phase = RectanglePhase::Direction { first, width: w, height: h };
                                        self.command_log.push("RECTANGLE: Specify direction point".to_string());
                                    } else {
                                        self.command_log.push("  *RECTANGLE: width,height must be > 0*".to_string());
                                    }
                                } else {
                                    self.command_log
                                        .push("  *RECTANGLE: enter width,height (example: 10,5)*".to_string());
                                }
                                handled = true;
                            } else if let RectanglePhase::Direction { first, width, height } = self.rectangle_phase {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    if self.apply_rectangle_dimensions(first, width, height, world) {
                                        self.rectangle_phase = RectanglePhase::FirstCorner;
                                    }
                                } else {
                                    self.command_log.push("  *RECTANGLE: enter x,y for direction point*".to_string());
                                }
                                handled = true;
                            } else if !matches!(self.dim_phase, DimPhase::Idle) {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    if matches!(self.dim_phase, DimPhase::FirstPoint) {
                                        self.dim_phase = DimPhase::SecondPoint { first: world };
                                        self.command_log.push(format!("DIMALIGNED: First point ({:.4}, {:.4})", world.x, world.y));
                                    } else if let DimPhase::SecondPoint { first } = self.dim_phase {
                                        self.dim_phase = DimPhase::Placing { first, second: world };
                                        self.command_log.push(format!("DIMALIGNED: Second point ({:.4}, {:.4})", world.x, world.y));
                                    } else if let DimPhase::Placing { first, second } = self.dim_phase {
                                        self.place_dim_aligned(first, second, world);
                                    }
                                } else {
                                    self.command_log.push("  *DIMALIGNED: enter x,y for point*".to_string());
                                }
                                handled = true;
                            } else if let TextPhase::EnteringHeight { position } = self.text_phase {
                                let h = cmd.trim().parse::<f64>().unwrap_or(self.last_text_height).max(0.001);
                                self.last_text_height = h;
                                self.text_phase = TextPhase::EnteringRotation { position, height: h };
                                self.command_log.push(format!(
                                    "{}  Rotation angle <{:.1}>:",
                                    if self.text_is_mtext { "MTEXT" } else { "TEXT" },
                                    self.last_text_rotation.to_degrees()
                                ));
                                handled = true;
                            } else if let TextPhase::EnteringRotation { position, height } = self.text_phase {
                                let r = cmd.trim().parse::<f64>().unwrap_or(self.last_text_rotation.to_degrees()).to_radians();
                                self.last_text_rotation = r;
                                self.text_phase = TextPhase::TypingContent { position, height, rotation: r };
                                self.command_log.push(
                                    if self.text_is_mtext {
                                        "MTEXT  Enter text (use \\P for new line):".to_string()
                                    } else {
                                        "TEXT  Enter text:".to_string()
                                    },
                                );
                                handled = true;
                            } else if let TextPhase::TypingContent { position, height, rotation } = self.text_phase {
                                // The typed text becomes the entity content.
                                if !cmd.is_empty() {
                                    use cadkit_2d_core::{Entity, EntityKind};
                                    use cadkit_types::Vec3;
                                    if self.is_layer_locked(self.current_layer) {
                                        self.command_log.push("TEXT: Current layer is locked".to_string());
                                    } else {
                                        self.push_undo();
                                        let mut e = Entity::new(
                                            EntityKind::Text {
                                                position: Vec3::xy(position.x, position.y),
                                                content: if self.text_is_mtext {
                                                    cmd.replace("\\P", "\n").replace("\\p", "\n")
                                                } else {
                                                    cmd.clone()
                                                },
                                                height,
                                                rotation,
                                                font_name: "STANDARD".to_string(),
                                            },
                                            self.current_layer,
                                        );
                                        e.layer = self.current_layer;
                                        self.drawing.add_entity(e);
                                        self.command_log.push(format!(
                                            "{}: placed \"{}\"",
                                            if self.text_is_mtext { "MTEXT" } else { "TEXT" },
                                            cmd
                                        ));
                                    }
                                }
                                self.exit_text();
                                handled = true;
                            } else if self.scale_phase == ScalePhase::Factor {
                                if let Ok(factor) = cmd.trim().parse::<f64>() {
                                    self.apply_scale_factor(factor);
                                } else {
                                    self.command_log.push("  *SCALE: enter numeric factor (e.g. 2.0)*".to_string());
                                }
                                handled = true;
                            } else if self.mirror_phase == MirrorPhase::FirstAxisPoint {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    self.mirror_axis_p1 = Some(world);
                                    self.mirror_phase = MirrorPhase::SecondAxisPoint;
                                    self.command_log.push("MIRROR: Pick second axis point".to_string());
                                } else {
                                    self.command_log.push("  *MIRROR: enter x,y for first axis point*".to_string());
                                }
                                handled = true;
                            } else if self.mirror_phase == MirrorPhase::SecondAxisPoint {
                                if let Some(world) = Self::resolve_typed_point(&cmd, None) {
                                    if let Some(p1) = self.mirror_axis_p1 {
                                        self.apply_mirror(p1, world);
                                    }
                                } else {
                                    self.command_log.push("  *MIRROR: enter x,y for second axis point*".to_string());
                                }
                                handled = true;
                            } else if self.apply_typed_point_input(&cmd) {
                                handled = true;
                            }

                            if !handled {
                                self.command_log.push(format!("*Unknown command*: {cmd}"));
                            }
                        }
                    }

                    ui.add_space(6.0);
                    let log_height = 80.0;
                    egui::ScrollArea::vertical()
                        .max_height(log_height)
                        .id_source("command_log")
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in self.command_log.iter().rev() {
                                ui.label(egui::RichText::new(line).monospace());
                            }
                        });
                });
            });
    }
}
