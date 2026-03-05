use super::state::{ActiveTool, GeomPrim, SnapKind};
use super::CadKitApp;
use cadkit_geometry::{
    Arc as GeomArc, Circle as GeomCircle, Intersects, Line as GeomLine, Polyline as GeomPolyline,
};
use cadkit_render_wgpu::{screen_to_world, world_to_screen, Viewport};
use cadkit_types::Vec2;
use cadkit_2d_core::EntityKind;
use eframe::egui;

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

impl CadKitApp {
    pub(crate) fn draw_grid_overlay(ui: &egui::Ui, rect: egui::Rect, viewport: &Viewport) {
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

    pub(crate) fn draw_selected_entities_overlay(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        viewport: &Viewport,
    ) {
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
                EntityKind::DimAligned { start, end, offset, .. } => {
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
                EntityKind::Text { position, .. } => {
                    // Draw a small selection box around the insertion point.
                    let (sx, sy) = world_to_screen(position.x as f32, position.y as f32, viewport);
                    let pos = rect.min + egui::vec2(sx, sy);
                    painter.rect_stroke(
                        egui::Rect::from_center_size(pos, egui::vec2(12.0, 12.0)),
                        2.0,
                        stroke,
                    );
                }
            }
        }
    }

    pub(crate) fn draw_tick_marker(
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

    /// Draw a per-type snap glyph at `world` in yellow (AutoCAD-style icons).
    pub(crate) fn draw_snap_glyph(
        ui: &egui::Ui,
        rect: egui::Rect,
        viewport: &Viewport,
        world: Vec2,
        kind: SnapKind,
    ) {
        let (sx, sy) = world_to_screen(world.x as f32, world.y as f32, viewport);
        let pos = rect.min + egui::vec2(sx, sy);
        let painter = ui.painter_at(rect);
        let yellow = egui::Color32::from_rgb(255, 220, 40);
        let stroke = egui::Stroke::new(1.5, yellow);

        match kind {
            SnapKind::Endpoint => {
                // Hollow square
                let h = 5.0_f32;
                let c = [
                    pos + egui::vec2(-h, -h), pos + egui::vec2(h, -h),
                    pos + egui::vec2(h,  h),  pos + egui::vec2(-h, h),
                ];
                painter.line_segment([c[0], c[1]], stroke);
                painter.line_segment([c[1], c[2]], stroke);
                painter.line_segment([c[2], c[3]], stroke);
                painter.line_segment([c[3], c[0]], stroke);
            }
            SnapKind::Midpoint => {
                // Upward triangle
                let r = 6.0_f32;
                let p1 = pos + egui::vec2(0.0, -r);
                let p2 = pos + egui::vec2(r * 0.866, r * 0.5);
                let p3 = pos + egui::vec2(-r * 0.866, r * 0.5);
                painter.line_segment([p1, p2], stroke);
                painter.line_segment([p2, p3], stroke);
                painter.line_segment([p3, p1], stroke);
            }
            SnapKind::Center => {
                // Circle with crosshairs
                painter.circle_stroke(pos, 6.0, stroke);
                let r = 9.0_f32;
                painter.line_segment([pos - egui::vec2(r, 0.0), pos + egui::vec2(r, 0.0)], stroke);
                painter.line_segment([pos - egui::vec2(0.0, r), pos + egui::vec2(0.0, r)], stroke);
            }
            SnapKind::Quadrant => {
                // Diamond
                let r = 6.0_f32;
                let top   = pos + egui::vec2(0.0, -r);
                let right = pos + egui::vec2(r,   0.0);
                let bot   = pos + egui::vec2(0.0,  r);
                let left  = pos + egui::vec2(-r,  0.0);
                painter.line_segment([top,   right], stroke);
                painter.line_segment([right, bot  ], stroke);
                painter.line_segment([bot,   left ], stroke);
                painter.line_segment([left,  top  ], stroke);
            }
            SnapKind::Intersection => {
                // X mark
                let r = 7.0_f32;
                painter.line_segment([pos + egui::vec2(-r, -r), pos + egui::vec2(r, r)], stroke);
                painter.line_segment([pos + egui::vec2(-r,  r), pos + egui::vec2(r,-r)], stroke);
            }
            SnapKind::Nearest => {
                // Circle with inner X
                painter.circle_stroke(pos, 6.0, stroke);
                let r = 4.0_f32;
                painter.line_segment([pos + egui::vec2(-r, -r), pos + egui::vec2(r, r)], stroke);
                painter.line_segment([pos + egui::vec2(-r,  r), pos + egui::vec2(r,-r)], stroke);
            }
            SnapKind::Perpendicular => {
                // Right-angle symbol: vertical arm + horizontal arm + corner square
                let r = 7.0_f32;
                painter.line_segment([pos + egui::vec2(0.0, -r), pos], stroke);
                painter.line_segment([pos, pos + egui::vec2(r, 0.0)], stroke);
                let sq = 3.0_f32;
                painter.line_segment([pos + egui::vec2(sq, 0.0), pos + egui::vec2(sq, -sq)], stroke);
                painter.line_segment([pos + egui::vec2(sq, -sq), pos + egui::vec2(0.0, -sq)], stroke);
            }
            SnapKind::Tangent => {
                // Small circle with horizontal tangent line above it
                painter.circle_stroke(pos, 5.0, stroke);
                painter.line_segment(
                    [pos + egui::vec2(-8.0, -7.0), pos + egui::vec2(8.0, -7.0)],
                    stroke,
                );
            }
        }
    }

    pub(crate) fn screen_dist_to_entity(
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
                // World Y is up but screen Y is down, so negate rel.y to get the world-space angle.
                let rel = screen_pos - c_screen;
                let click_angle = (-rel.y as f64).atan2(rel.x as f64);
                let span = Self::ccw_from(*start_angle, *end_angle);
                let angle_in_span = Self::ccw_from(*start_angle, click_angle) <= span;

                if angle_in_span {
                    (screen_pos.distance(c_screen) - screen_r).abs()
                } else {
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
            EntityKind::DimAligned { start, end, offset, .. } => {
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
            EntityKind::Text { position, .. } => {
                let (sx, sy) = world_to_screen(position.x as f32, position.y as f32, viewport);
                screen_pos.distance(rect.min + egui::vec2(sx, sy))
            }
        }
    }

    /// Find the nearest intersection snap point to the cursor when a drawing tool is active.
    pub(crate) fn find_intersection_snap(
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

    pub(crate) fn entity_to_geom_prim(kind: &EntityKind) -> Option<GeomPrim> {
        match kind {
            EntityKind::Line { start, end } => Some(GeomPrim::Line(GeomLine::new(
                *start,
                *end,
            ))),
            EntityKind::Circle { center, radius } => {
                Some(GeomPrim::Circle(GeomCircle::new(*center, *radius)))
            }
            EntityKind::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => Some(GeomPrim::Arc(GeomArc::new(
                *center,
                *radius,
                *start_angle,
                *end_angle,
            ))),
            EntityKind::Polyline { vertices, closed } => Some(GeomPrim::Polyline(
                GeomPolyline::new(vertices.clone(), *closed),
            )),
            EntityKind::DimAligned { .. } | EntityKind::Text { .. } => None,
        }
    }

    pub(crate) fn intersect_geom_prims(a: &GeomPrim, b: &GeomPrim, tol: f64) -> cadkit_geometry::Intersection {
        match (a, b) {
            (GeomPrim::Line(la), GeomPrim::Line(lb)) => la.intersect(lb, tol),
            (GeomPrim::Line(l), GeomPrim::Circle(c)) | (GeomPrim::Circle(c), GeomPrim::Line(l)) => {
                l.intersect(c, tol)
            }
            (GeomPrim::Line(l), GeomPrim::Arc(a)) | (GeomPrim::Arc(a), GeomPrim::Line(l)) => {
                l.intersect(a, tol)
            }
            (GeomPrim::Circle(ca), GeomPrim::Circle(cb)) => ca.intersect(cb, tol),
            (GeomPrim::Circle(c), GeomPrim::Arc(a)) | (GeomPrim::Arc(a), GeomPrim::Circle(c)) => {
                c.intersect(a, tol)
            }
            (GeomPrim::Arc(aa), GeomPrim::Arc(ab)) => aa.intersect(ab, tol),
            (GeomPrim::Line(l), GeomPrim::Polyline(p)) | (GeomPrim::Polyline(p), GeomPrim::Line(l)) => {
                l.intersect(p, tol)
            }
            (GeomPrim::Circle(c), GeomPrim::Polyline(p)) | (GeomPrim::Polyline(p), GeomPrim::Circle(c)) => {
                c.intersect(p, tol)
            }
            (GeomPrim::Arc(a), GeomPrim::Polyline(p)) | (GeomPrim::Polyline(p), GeomPrim::Arc(a)) => {
                a.intersect(p, tol)
            }
            (GeomPrim::Polyline(pa), GeomPrim::Polyline(pb)) => pa.intersect(pb, tol),
        }
    }
}
