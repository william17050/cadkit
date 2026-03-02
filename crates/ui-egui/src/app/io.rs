use super::CadKitApp;
use cadkit_2d_core::Drawing;
use cadkit_2d_core::DxfImportResult;
use eframe::egui;

impl CadKitApp {
    /// Save to the current file path, or run Save As if none is set.
    pub(crate) fn save(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn save_as(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn open(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn export_dxf(&mut self) {
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
    pub(crate) fn import_dxf(&mut self, ctx: &egui::Context) {
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

    pub(crate) fn update_title(ctx: &egui::Context, path: &str) {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(
            format!("CadKit - {}", name),
        ));
    }
}
