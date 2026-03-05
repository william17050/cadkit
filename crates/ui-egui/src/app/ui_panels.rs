use super::state::{
    ActiveTool, CopyPhase, DimLinearPhase, DimPhase, EditDimPhase, EditTextPhase, ExtendPhase,
    FromPhase, MovePhase, OffsetPhase, RotatePhase, TextPhase, TrimPhase,
};
use super::CadKitApp;
use cadkit_2d_core::EntityKind;
use cadkit_types::Guid;
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
            if ui.button("T Text").clicked() {
                self.exit_dim();
                self.cancel_active_tool();
                self.exit_trim();
                self.exit_offset();
                self.exit_move();
                self.exit_extend();
                self.exit_copy();
                self.exit_rotate();
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
                self.copy_phase = CopyPhase::SelectingEntities;
                self.copy_base_point = None;
                self.copy_entities.clear();
                self.command_log.push("COPY: Select entities to copy, press Enter to continue".to_string());
            }
            if ui.button("🔄 Rotate").clicked() {
                self.exit_dim();
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
            let mut delete_layer: Option<u32> = None;
            let mut set_current: Option<u32> = None;
            let mut open_color_picker: Option<u32> = None;
            let mut commit_name: Option<(u32, String)> = None;
            let mut cancel_edit = false;
            let mut start_edit: Option<(u32, String)> = None;

            let mut assign_entity_layer: Option<u32> = None;
            let mut set_entity_bylayer = false;
            let mut open_entity_color_picker = false;

            ui.heading("Layers");
            ui.separator();

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
                                    EntityKind::Text { .. } => "Text",
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
                                            EntityKind::Text { position, content, height, rotation } => {
                                                ui.label("X:"); ui.label(format!("{:.4}", position.x)); ui.end_row();
                                                ui.label("Y:"); ui.label(format!("{:.4}", position.y)); ui.end_row();
                                                ui.label("Content:"); ui.label(content.as_str()); ui.end_row();
                                                ui.label("Height:"); ui.label(format!("{:.4}", height)); ui.end_row();
                                                ui.label("Rotation:"); ui.label(format!("{:.2}°", rotation.to_degrees())); ui.end_row();
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
            if let Some(id) = set_current {
                if self.is_layer_locked(id) {
                    self.command_log
                        .push("LAYER: Cannot set a locked layer as current".to_string());
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
                            ui.separator();
                            ui.checkbox(&mut self.grid_visible, "Grid");
                            ui.add(
                                egui::DragValue::new(&mut self.grid_spacing)
                                    .clamp_range(0.5..=500.0)
                                    .speed(0.5)
                                    .suffix("\""),
                            );
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

                        // Keep focus glued to the command line, unless the user
                        // is editing a layer name, has a colour picker open, or the
                        // Edit Text dialog is showing.
                        if self.layer_editing_id.is_none()
                            && self.layer_color_picking.is_none()
                            && self.text_edit_dialog.is_none()
                            && self.dim_edit_dialog.is_none()
                            && self.dim_style_dialog.is_none()
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
                            } else if self.text_phase == TextPhase::PlacingPosition {
                                // Empty Enter = confirm hover point as position.
                                if let Some(world) = self.hover_world_pos {
                                    self.text_phase = TextPhase::EnteringHeight { position: world };
                                    self.command_log.push(format!(
                                        "TEXT  Text height <{:.4}>:", self.last_text_height));
                                }
                            } else if let TextPhase::EnteringHeight { position } = self.text_phase {
                                // Empty Enter = use last_text_height.
                                let h = self.last_text_height;
                                self.text_phase = TextPhase::EnteringRotation { position, height: h };
                                self.command_log.push(format!(
                                    "TEXT  Rotation angle <{:.1}>:", h.to_degrees()));
                            } else if let TextPhase::EnteringRotation { position, height } = self.text_phase {
                                // Empty Enter = use last_text_rotation.
                                let r = self.last_text_rotation;
                                self.text_phase = TextPhase::TypingContent { position, height, rotation: r };
                                self.command_log.push("TEXT  Enter text:".to_string());
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
                                    "TEXT  Rotation angle <{:.1}>:", self.last_text_rotation.to_degrees()));
                                handled = true;
                            } else if let TextPhase::EnteringRotation { position, height } = self.text_phase {
                                let r = cmd.trim().parse::<f64>().unwrap_or(self.last_text_rotation.to_degrees()).to_radians();
                                self.last_text_rotation = r;
                                self.text_phase = TextPhase::TypingContent { position, height, rotation: r };
                                self.command_log.push("TEXT  Enter text:".to_string());
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
                                                content: cmd.clone(),
                                                height,
                                                rotation,
                                            },
                                            self.current_layer,
                                        );
                                        e.layer = self.current_layer;
                                        self.drawing.add_entity(e);
                                        self.command_log.push(format!("TEXT: placed \"{}\"", cmd));
                                    }
                                }
                                self.exit_text();
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
