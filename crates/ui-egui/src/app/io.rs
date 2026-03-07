use super::{AppPrefs, CadKitApp};
use cadkit_2d_core::Drawing;
use cadkit_2d_core::DxfImportResult;
use cadkit_2d_core::EntityKind;
use eframe::egui;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

impl CadKitApp {
    const MAX_RECENT_FILES: usize = 8;

    fn prefs_path() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config").join("cadkit").join("prefs.json"))
    }

    fn recovery_path() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".config").join("cadkit").join("recovery.json"))
    }

    fn collect_preferences(&self) -> AppPrefs {
        AppPrefs {
            snap_enabled: self.snap_enabled,
            snap_endpoint: self.snap_endpoint,
            snap_midpoint: self.snap_midpoint,
            snap_center: self.snap_center,
            snap_quadrant: self.snap_quadrant,
            snap_intersection: self.snap_intersection,
            snap_parallel: self.snap_parallel,
            snap_perpendicular: self.snap_perpendicular,
            snap_tangent: self.snap_tangent,
            snap_nearest: self.snap_nearest,
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
                    self.snap_endpoint = prefs.snap_endpoint;
                    self.snap_midpoint = prefs.snap_midpoint;
                    self.snap_center = prefs.snap_center;
                    self.snap_quadrant = prefs.snap_quadrant;
                    self.snap_intersection = prefs.snap_intersection;
                    self.snap_parallel = prefs.snap_parallel;
                    self.snap_perpendicular = prefs.snap_perpendicular;
                    self.snap_tangent = prefs.snap_tangent;
                    self.snap_nearest = prefs.snap_nearest;
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
                    self.delete_recovery_snapshot();
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
                    self.delete_recovery_snapshot();
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

    /// Export visible geometry to an SVG file (paths only).
    pub(crate) fn export_svg(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Export Drawing as SVG")
            .add_filter("SVG", &["svg"])
            .save_file();
        let Some(path) = path else { return };
        let path_str = path.to_string_lossy().to_string();

        match self.build_svg_document() {
            Ok(svg) => match std::fs::write(&path, svg) {
                Ok(()) => self.command_log.push(format!("SVG: Exported to {}", path_str)),
                Err(e) => self.command_log.push(format!("SVG: Export failed - {}", e)),
            },
            Err(e) => self.command_log.push(format!("SVG: Export failed - {}", e)),
        }
    }

    /// Export visible geometry to a single-page vector PDF.
    pub(crate) fn export_pdf(&mut self) {
        let path = rfd::FileDialog::new()
            .set_title("Export Drawing as PDF")
            .add_filter("PDF", &["pdf"])
            .save_file();
        let Some(path) = path else { return };
        let path_str = path.to_string_lossy().to_string();

        match self.build_pdf_document() {
            Ok(pdf) => match std::fs::write(&path, pdf) {
                Ok(()) => self.command_log.push(format!("PDF: Exported to {}", path_str)),
                Err(e) => self.command_log.push(format!("PDF: Export failed - {}", e)),
            },
            Err(e) => self.command_log.push(format!("PDF: Export failed - {}", e)),
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

    pub(crate) fn recovery_snapshot_exists(&self) -> bool {
        Self::recovery_path()
            .map(|p| p.exists())
            .unwrap_or(false)
    }

    pub(crate) fn autosave_recovery_if_due(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.autosave_last_at) < Duration::from_secs(20) {
            return;
        }
        self.autosave_last_at = now;

        // Skip empty untitled drawings to avoid noisy/meaningless recovery files.
        if self.current_file.is_none() && self.drawing.entity_count() == 0 {
            return;
        }

        let Some(path) = Self::recovery_path() else { return };
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                log::warn!(
                    "Recovery: failed to create directory {}: {}",
                    parent.display(),
                    e
                );
                return;
            }
        }
        if let Err(e) = self.drawing.save_to_file(&path.to_string_lossy()) {
            log::warn!(
                "Recovery: auto-save failed at {}: {}",
                path.display(),
                e
            );
        }
    }

    pub(crate) fn restore_recovery_snapshot(&mut self, ctx: &egui::Context) {
        let Some(path) = Self::recovery_path() else {
            self.command_log
                .push("Recovery: Could not resolve recovery path".to_string());
            return;
        };
        let path_str = path.to_string_lossy().to_string();
        match Drawing::load_from_file(&path_str) {
            Ok(drawing) => {
                self.drawing = drawing;
                self.current_file = None;
                self.selected_entities.clear();
                self.selection = None;
                self.command_log
                    .push("Recovery: Snapshot restored (unsaved drawing)".to_string());
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                    "CadKit - Recovered".to_string(),
                ));
            }
            Err(e) => {
                self.command_log
                    .push(format!("Recovery: Restore failed - {}", e));
            }
        }
    }

    pub(crate) fn delete_recovery_snapshot(&mut self) {
        let Some(path) = Self::recovery_path() else { return };
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => log::warn!(
                "Recovery: failed to delete {}: {}",
                path.display(),
                e
            ),
        }
    }

    fn build_svg_document(&self) -> Result<String, String> {
        #[derive(Clone)]
        struct SvgPath {
            d: String,
            rgb: [u8; 3],
        }

        let mut paths: Vec<SvgPath> = Vec::new();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = -f64::INFINITY;
        let mut max_y = -f64::INFINITY;

        let mut push_poly = |pts: &[(f64, f64)], closed: bool, rgb: [u8; 3]| {
            if pts.len() < 2 {
                return;
            }
            for (x, y) in pts {
                min_x = min_x.min(*x);
                min_y = min_y.min(*y);
                max_x = max_x.max(*x);
                max_y = max_y.max(*y);
            }
            let mut d = String::new();
            let _ = write!(d, "M {:.6} {:.6}", pts[0].0, pts[0].1);
            for p in &pts[1..] {
                let _ = write!(d, " L {:.6} {:.6}", p.0, p.1);
            }
            if closed {
                d.push_str(" Z");
            }
            paths.push(SvgPath { d, rgb });
        };

        for e in self.drawing.visible_entities() {
            let layer_rgb = self
                .drawing
                .get_layer(e.layer)
                .map(|l| l.color)
                .unwrap_or([255, 255, 255]);
            let rgb = e.color.unwrap_or(layer_rgb);

            match &e.kind {
                EntityKind::Line { start, end } => {
                    push_poly(&[(start.x, start.y), (end.x, end.y)], false, rgb);
                }
                EntityKind::Polyline { vertices, closed } => {
                    let pts: Vec<(f64, f64)> = vertices.iter().map(|v| (v.x, v.y)).collect();
                    push_poly(&pts, *closed, rgb);
                }
                EntityKind::Circle { center, radius } => {
                    let steps = 96usize;
                    let pts: Vec<(f64, f64)> = (0..steps)
                        .map(|i| {
                            let a = (i as f64 / steps as f64) * std::f64::consts::TAU;
                            (center.x + radius * a.cos(), center.y + radius * a.sin())
                        })
                        .collect();
                    push_poly(&pts, true, rgb);
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let sweep = (end_angle - start_angle).max(1e-6);
                    let steps = ((sweep.abs() * radius.abs()).max(12.0) as usize).clamp(12, 256);
                    let pts: Vec<(f64, f64)> = (0..=steps)
                        .map(|i| {
                            let t = i as f64 / steps as f64;
                            let a = start_angle + sweep * t;
                            (center.x + radius * a.cos(), center.y + radius * a.sin())
                        })
                        .collect();
                    push_poly(&pts, false, rgb);
                }
                EntityKind::DimAligned { .. }
                | EntityKind::DimLinear { .. }
                | EntityKind::DimAngular { .. }
                | EntityKind::DimRadial { .. }
                | EntityKind::Text { .. } => {}
            }
        }

        if paths.is_empty() {
            return Err("No exportable path geometry found".to_string());
        }

        let mut width = (max_x - min_x).max(1.0);
        let mut height = (max_y - min_y).max(1.0);
        let pad = width.max(height) * 0.02;
        min_x -= pad;
        min_y -= pad;
        width += pad * 2.0;
        height += pad * 2.0;
        let y_top = min_y + height;

        let mut out = String::new();
        out.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        out.push('\n');
        let _ = write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" viewBox="{:.6} {:.6} {:.6} {:.6}" fill="none" stroke-linecap="round" stroke-linejoin="round">"#,
            min_x, min_y, width, height
        );
        out.push('\n');
        let stroke_w = (width.max(height) * 0.0015).max(0.01);
        for p in &paths {
            let y_flip_d = flip_path_y(&p.d, y_top);
            let _ = write!(
                out,
                r#"<path d="{}" stroke="rgb({},{},{})" stroke-width="{:.6}"/>"#,
                y_flip_d, p.rgb[0], p.rgb[1], p.rgb[2], stroke_w
            );
            out.push('\n');
        }
        out.push_str("</svg>\n");
        Ok(out)
    }

    fn build_pdf_document(&self) -> Result<Vec<u8>, String> {
        #[derive(Clone)]
        struct PdfPath {
            pts: Vec<(f64, f64)>,
            closed: bool,
            rgb: [u8; 3],
        }

        let mut paths: Vec<PdfPath> = Vec::new();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = -f64::INFINITY;
        let mut max_y = -f64::INFINITY;

        let mut push_poly = |pts: &[(f64, f64)], closed: bool, rgb: [u8; 3]| {
            if pts.len() < 2 {
                return;
            }
            for (x, y) in pts {
                min_x = min_x.min(*x);
                min_y = min_y.min(*y);
                max_x = max_x.max(*x);
                max_y = max_y.max(*y);
            }
            paths.push(PdfPath {
                pts: pts.to_vec(),
                closed,
                rgb,
            });
        };

        for e in self.drawing.visible_entities() {
            let layer_rgb = self
                .drawing
                .get_layer(e.layer)
                .map(|l| l.color)
                .unwrap_or([255, 255, 255]);
            let rgb = e.color.unwrap_or(layer_rgb);

            match &e.kind {
                EntityKind::Line { start, end } => {
                    push_poly(&[(start.x, start.y), (end.x, end.y)], false, rgb);
                }
                EntityKind::Polyline { vertices, closed } => {
                    let pts: Vec<(f64, f64)> = vertices.iter().map(|v| (v.x, v.y)).collect();
                    push_poly(&pts, *closed, rgb);
                }
                EntityKind::Circle { center, radius } => {
                    let steps = 96usize;
                    let pts: Vec<(f64, f64)> = (0..steps)
                        .map(|i| {
                            let a = (i as f64 / steps as f64) * std::f64::consts::TAU;
                            (center.x + radius * a.cos(), center.y + radius * a.sin())
                        })
                        .collect();
                    push_poly(&pts, true, rgb);
                }
                EntityKind::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                } => {
                    let sweep = (end_angle - start_angle).max(1e-6);
                    let steps = ((sweep.abs() * radius.abs()).max(12.0) as usize).clamp(12, 256);
                    let pts: Vec<(f64, f64)> = (0..=steps)
                        .map(|i| {
                            let t = i as f64 / steps as f64;
                            let a = start_angle + sweep * t;
                            (center.x + radius * a.cos(), center.y + radius * a.sin())
                        })
                        .collect();
                    push_poly(&pts, false, rgb);
                }
                EntityKind::DimAligned { .. }
                | EntityKind::DimLinear { .. }
                | EntityKind::DimAngular { .. }
                | EntityKind::DimRadial { .. }
                | EntityKind::Text { .. } => {}
            }
        }

        if paths.is_empty() {
            return Err("No exportable path geometry found".to_string());
        }

        let mut width = (max_x - min_x).max(1.0);
        let mut height = (max_y - min_y).max(1.0);
        let pad = width.max(height) * 0.02;
        min_x -= pad;
        min_y -= pad;
        width += pad * 2.0;
        height += pad * 2.0;

        let stroke_w = (width.max(height) * 0.0015).max(0.10);
        let mut content = String::new();
        content.push_str("1 J 1 j\n");
        for p in &paths {
            // White/light CAD strokes disappear on typical white PDF pages.
            // For print-friendly output, remap near-white strokes to black.
            let stroke_rgb = if (p.rgb[0] as u16 + p.rgb[1] as u16 + p.rgb[2] as u16) >= 735 {
                [0u8, 0u8, 0u8]
            } else {
                p.rgb
            };
            let _ = write!(
                content,
                "{:.6} {:.6} {:.6} RG\n{:.6} w\n",
                stroke_rgb[0] as f64 / 255.0,
                stroke_rgb[1] as f64 / 255.0,
                stroke_rgb[2] as f64 / 255.0,
                stroke_w
            );
            if let Some(first) = p.pts.first() {
                let _ = write!(content, "{:.6} {:.6} m\n", first.0 - min_x, first.1 - min_y);
                for pt in &p.pts[1..] {
                    let _ = write!(content, "{:.6} {:.6} l\n", pt.0 - min_x, pt.1 - min_y);
                }
                if p.closed {
                    content.push_str("h\n");
                }
                content.push_str("S\n");
            }
        }

        Ok(build_single_page_pdf(width, height, content.as_bytes()))
    }
}

fn flip_path_y(path_d: &str, y_top: f64) -> String {
    // Very small parser for our own generated "M x y L x y ... Z" strings.
    let mut out = String::new();
    let toks: Vec<&str> = path_d.split_whitespace().collect();
    let mut i = 0usize;
    while i < toks.len() {
        match toks[i] {
            "M" | "L" => {
                if i + 2 < toks.len() {
                    let x = toks[i + 1].parse::<f64>().unwrap_or(0.0);
                    let y = toks[i + 2].parse::<f64>().unwrap_or(0.0);
                    let fy = y_top - y;
                    let _ = write!(out, "{} {:.6} {:.6} ", toks[i], x, fy);
                    i += 3;
                } else {
                    break;
                }
            }
            "Z" => {
                out.push('Z');
                i += 1;
            }
            _ => i += 1,
        }
    }
    out.trim_end().to_string()
}

fn build_single_page_pdf(page_w: f64, page_h: f64, stream_content: &[u8]) -> Vec<u8> {
    let page_w = page_w.max(1.0);
    let page_h = page_h.max(1.0);
    let obj1 = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    let obj2 = b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec();
    let obj3 = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.6} {:.6}] /Resources << /ProcSet [/PDF] >> /Contents 4 0 R >>",
        page_w, page_h
    )
    .into_bytes();
    let mut obj4 = format!("<< /Length {} >>\nstream\n", stream_content.len()).into_bytes();
    obj4.extend_from_slice(stream_content);
    obj4.extend_from_slice(b"\nendstream");
    let objects = vec![obj1, obj2, obj3, obj4];

    let mut out = Vec::<u8>::new();
    out.extend_from_slice(b"%PDF-1.4\n%\xFF\xFF\xFF\xFF\n");

    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len() + 1);
    offsets.push(0);
    for (idx, obj) in objects.iter().enumerate() {
        let obj_num = idx + 1;
        offsets.push(out.len());
        out.extend_from_slice(format!("{obj_num} 0 obj\n").as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }

    let xref_offset = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    out
}
