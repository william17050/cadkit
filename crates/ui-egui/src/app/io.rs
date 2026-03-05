use super::{AppPrefs, CadKitApp};
use cadkit_2d_core::Drawing;
use cadkit_2d_core::DxfImportResult;
use eframe::egui;
use std::path::PathBuf;

impl CadKitApp {
    const MAX_RECENT_FILES: usize = 8;

    fn prefs_path() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config").join("cadkit").join("prefs.json"))
    }

    fn collect_preferences(&self) -> AppPrefs {
        AppPrefs {
            snap_enabled: self.snap_enabled,
            ortho_enabled: self.ortho_enabled,
            grid_visible: self.grid_visible,
            grid_spacing: self.grid_spacing,
            current_file: self.current_file.clone(),
            recent_files: self.recent_files.clone(),
            dim_style: self.dim_style.clone(),
        }
    }

    fn touch_recent_file(&mut self, path: &str) {
        let p = path.to_string();
        self.recent_files.retain(|x| x != &p);
        self.recent_files.insert(0, p);
        self.recent_files.truncate(Self::MAX_RECENT_FILES);
    }

    pub(crate) fn clear_recent_files(&mut self) {
        self.recent_files.clear();
    }

    pub(crate) fn prune_recent_files(&mut self) {
        let mut out = Vec::with_capacity(Self::MAX_RECENT_FILES);
        for p in &self.recent_files {
            if out.iter().any(|x: &String| x == p) {
                continue;
            }
            out.push(p.clone());
            if out.len() >= Self::MAX_RECENT_FILES {
                break;
            }
        }
        self.recent_files = out;
    }

    pub(crate) fn open_path(&mut self, ctx: &egui::Context, path_str: &str) {
        match Drawing::load_from_file(path_str) {
            Ok(drawing) => {
                self.drawing = drawing;
                self.current_file = Some(path_str.to_string());
                self.touch_recent_file(path_str);
                self.selected_entities.clear();
                self.selection = None;
                self.command_log.push(format!("Opened: {}", path_str));
                Self::update_title(ctx, path_str);
            }
            Err(e) => self.command_log.push(format!("Open failed: {}", e)),
        }
    }

    pub(crate) fn load_preferences(&mut self) {
        let Some(path) = Self::prefs_path() else {
            self.last_saved_prefs = Some(self.collect_preferences());
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(json) => match serde_json::from_str::<AppPrefs>(&json) {
                Ok(prefs) => {
                    self.snap_enabled = prefs.snap_enabled;
                    self.ortho_enabled = prefs.ortho_enabled;
                    self.grid_visible = prefs.grid_visible;
                    self.grid_spacing = prefs.grid_spacing.max(0.5);
                    self.current_file = prefs.current_file.clone();
                    self.recent_files = prefs.recent_files;
                    self.dim_style = prefs.dim_style;
                    self.prune_recent_files();
                    self.last_saved_prefs = Some(self.collect_preferences());
                }
                Err(e) => {
                    log::warn!("Failed to parse prefs at {}: {}", path.display(), e);
                    self.last_saved_prefs = Some(self.collect_preferences());
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.last_saved_prefs = Some(self.collect_preferences());
            }
            Err(e) => {
                log::warn!("Failed to read prefs at {}: {}", path.display(), e);
                self.last_saved_prefs = Some(self.collect_preferences());
            }
        }
    }

    pub(crate) fn persist_preferences_if_changed(&mut self) {
        let prefs = self.collect_preferences();
        if self.last_saved_prefs.as_ref() == Some(&prefs) {
            return;
        }
        let Some(path) = Self::prefs_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!("Failed to create prefs directory {}: {}", parent.display(), e);
                return;
            }
        }
        match serde_json::to_string_pretty(&prefs) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    log::warn!("Failed to write prefs at {}: {}", path.display(), e);
                    return;
                }
                self.last_saved_prefs = Some(prefs);
            }
            Err(e) => {
                log::warn!("Failed to serialize prefs: {}", e);
            }
        }
    }

    /// Save to the current file path, or run Save As if none is set.
    pub(crate) fn save(&mut self, ctx: &egui::Context) {
        if let Some(path) = self.current_file.clone() {
            match self.drawing.save_to_file(&path) {
                Ok(()) => {
                    self.command_log.push(format!("Saved: {}", path));
                    self.touch_recent_file(&path);
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
                    self.touch_recent_file(&path_str);
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
            self.open_path(ctx, &path_str);
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
