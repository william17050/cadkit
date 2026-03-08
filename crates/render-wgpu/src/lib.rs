//! 2D CAD Viewport Renderer using wgpu
//!
//! This module handles:
//! - wgpu device initialization
//! - Rendering entities to texture
//! - Pan/zoom camera control

/// Canvas clear colour as **linear** RGB, matching the wgpu `LoadOp::Clear` value.
pub const CANVAS_CLEAR_LINEAR: f64 = 0.08;

/// Canvas background colour as **sRGB** bytes (gamma-corrected equivalent of
/// `CANVAS_CLEAR_LINEAR`), suitable for use as an egui `Color32`.
///
/// Derivation: `round(0.08^(1/2.2) * 255) = 81`
pub const CANVAS_BG_SRGB: [u8; 3] = [81, 81, 81];

pub mod font;
pub mod vertex;

use cadkit_2d_core::{Drawing, EntityKind, Linetype};
use cadkit_types::Vec2;
use std::sync::Arc;
use vertex::{Vertex, ViewTransform};
use wgpu::util::DeviceExt;

#[derive(Clone, Copy)]
struct DashPattern {
    segments: &'static [f64],
}

pub struct Viewport {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    render_pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    transform_buffer: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,

    // Camera state
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,

    /// Canvas clear colour in **linear** RGB. Default: `CANVAS_CLEAR_LINEAR` for each channel.
    pub clear_color: [f32; 3],

    width: u32,
    height: u32,
}

impl Viewport {
    const ARC_SAMPLE_LEN: f64 = 0.25;

    fn linetype_pattern(linetype: Linetype) -> Option<DashPattern> {
        match linetype {
            Linetype::Continuous => None,
            Linetype::Hidden => Some(DashPattern { segments: &[0.25, 0.125] }),
            Linetype::Center => Some(DashPattern {
                segments: &[0.75, 0.125, 0.125, 0.125],
            }),
        }
    }

    fn push_segment(vertices: &mut Vec<Vertex>, a: Vec2, b: Vec2, c: [f32; 3]) {
        vertices.push(Vertex::new(a.x as f32, a.y as f32, c[0], c[1], c[2]));
        vertices.push(Vertex::new(b.x as f32, b.y as f32, c[0], c[1], c[2]));
    }

    fn push_linetype_path(
        vertices: &mut Vec<Vertex>,
        points: &[Vec2],
        closed: bool,
        c: [f32; 3],
        linetype: Linetype,
        linetype_scale: f64,
    ) {
        if points.len() < 2 {
            return;
        }
        let Some(pattern) = Self::linetype_pattern(linetype) else {
            for w in points.windows(2) {
                Self::push_segment(vertices, w[0], w[1], c);
            }
            if closed {
                Self::push_segment(vertices, points[points.len() - 1], points[0], c);
            }
            return;
        };

        if pattern.segments.is_empty()
            || pattern.segments.iter().any(|v| *v <= 1e-9 || !v.is_finite())
        {
            for w in points.windows(2) {
                Self::push_segment(vertices, w[0], w[1], c);
            }
            if closed {
                Self::push_segment(vertices, points[points.len() - 1], points[0], c);
            }
            return;
        }

        let mut dash_idx = 0usize;
        let mut dash_pos = 0.0f64;
        let scale = linetype_scale.max(1e-6);
        let mut emit_patterned = |a: Vec2, b: Vec2| {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let seg_len = (dx * dx + dy * dy).sqrt();
            if seg_len <= 1e-12 {
                return;
            }
            let ux = dx / seg_len;
            let uy = dy / seg_len;
            let mut along = 0.0f64;
            while along < seg_len - 1e-12 {
                let draw = dash_idx % 2 == 0;
                let seg_pat = pattern.segments[dash_idx] * scale;
                let remain_pat = seg_pat - dash_pos;
                let remain_seg = seg_len - along;
                let step = remain_pat.min(remain_seg);
                if draw && step > 1e-12 {
                    let p0 = Vec2::new(a.x + ux * along, a.y + uy * along);
                    let p1 = Vec2::new(a.x + ux * (along + step), a.y + uy * (along + step));
                    Self::push_segment(vertices, p0, p1, c);
                }
                along += step;
                dash_pos += step;
                if dash_pos >= seg_pat - 1e-12 {
                    dash_pos = 0.0;
                    dash_idx = (dash_idx + 1) % pattern.segments.len();
                }
            }
        };

        for w in points.windows(2) {
            emit_patterned(w[0], w[1]);
        }
        if closed {
            emit_patterned(points[points.len() - 1], points[0]);
        }
    }

    fn sample_arc_points(center: Vec2, radius: f64, start_angle: f64, end_angle: f64) -> Vec<Vec2> {
        let span = end_angle - start_angle;
        if span.abs() <= f64::EPSILON || radius <= 1e-9 {
            return Vec::new();
        }
        let total_len = span.abs() * radius;
        let steps = ((total_len / Self::ARC_SAMPLE_LEN).ceil() as usize).clamp(16, 720);
        let mut pts = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let a = start_angle + span * t;
            pts.push(Vec2::new(center.x + radius * a.cos(), center.y + radius * a.sin()));
        }
        pts
    }

    fn sample_circle_points(center: Vec2, radius: f64) -> Vec<Vec2> {
        let circumference = std::f64::consts::TAU * radius.abs();
        let steps = ((circumference / Self::ARC_SAMPLE_LEN).ceil() as usize).clamp(24, 900);
        let mut pts = Vec::with_capacity(steps);
        for i in 0..steps {
            let t = i as f64 / steps as f64;
            let a = std::f64::consts::TAU * t;
            pts.push(Vec2::new(center.x + radius * a.cos(), center.y + radius * a.sin()));
        }
        pts
    }

    /// Create new viewport with given dimensions
    pub async fn new(width: u32, height: u32) -> anyhow::Result<Self> {
        // Create wgpu instance
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        
        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;
        
        // Request device and queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: Some("CAD Viewport Device"),
                },
                None,
            )
            .await?;
        
        Self::from_device_queue(Arc::new(device), Arc::new(queue), width, height)
    }

    /// Create a viewport using an existing wgpu device/queue (for eframe/egui-wgpu integration).
    pub fn new_with_device(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        Self::from_device_queue(device, queue, width, height)
    }

    fn from_device_queue(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("CAD Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        
        // Create transform uniform buffer
        let transform = ViewTransform::identity();
        let transform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Transform Buffer"),
            contents: bytemuck::cast_slice(&[transform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        
        // Create bind group layout for transform
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Transform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        
        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Transform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buffer.as_entire_binding(),
            }],
        });
        
        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        
        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });
        
        // Create render texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        Ok(Self {
            device,
            queue,
            render_pipeline,
            texture,
            texture_view,
            transform_buffer,
            transform_bind_group,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            clear_color: [CANVAS_CLEAR_LINEAR as f32; 3],
            width,
            height,
        })
    }
    
    /// Resize viewport
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        
        // Recreate texture with new size
        self.texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Viewport Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        
        self.texture_view = self.texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
    
    /// Render the drawing to the viewport texture
    pub fn render(&mut self, drawing: &Drawing) {
        // Update transform matrix
        let transform = ViewTransform::orthographic(
            self.width as f32,
            self.height as f32,
            self.zoom,
            self.pan_x,
            self.pan_y,
        );
        
        self.queue.write_buffer(
            &self.transform_buffer,
            0,
            bytemuck::cast_slice(&[transform]),
        );
        
        // Convert drawing entities to vertices
        let vertices = self.generate_vertices(drawing);
        let vertex_buffer = if vertices.is_empty() {
            None
        } else {
            Some(
                self.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Vertex Buffer"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        };
        
        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });
        
        // Render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color[0] as f64,
                            g: self.clear_color[1] as f64,
                            b: self.clear_color[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            
            if let Some(vb) = &vertex_buffer {
                render_pass.set_pipeline(&self.render_pipeline);
                render_pass.set_bind_group(0, &self.transform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.draw(0..vertices.len() as u32, 0..1);
            }
        }
        
        // Submit command buffer
        self.queue.submit(std::iter::once(encoder.finish()));
    }
    
    /// Convert drawing entities to vertex list.
    /// Only iterates visible entities; uses each entity's layer colour.
    fn generate_vertices(&self, drawing: &Drawing) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        let linetype_scale = if drawing.linetype_scale.is_finite() && drawing.linetype_scale > 0.0 {
            drawing.linetype_scale
        } else {
            1.0
        };

        for entity in drawing.visible_entities() {
            let layer = drawing.get_layer(entity.layer);
            // Resolve colour: entity override → layer colour → white fallback.
            let c = if let Some(ec) = entity.color {
                [ec[0] as f32 / 255.0, ec[1] as f32 / 255.0, ec[2] as f32 / 255.0]
            } else {
                layer
                    .map(|l| {
                        [
                            l.color[0] as f32 / 255.0,
                            l.color[1] as f32 / 255.0,
                            l.color[2] as f32 / 255.0,
                        ]
                    })
                    .unwrap_or([1.0, 1.0, 1.0])
            };
            let effective_linetype = if entity.linetype_by_layer {
                layer.map(|l| l.linetype).unwrap_or(Linetype::Continuous)
            } else {
                entity.linetype
            };
            let local_lt_scale = entity
                .linetype_scale
                .unwrap_or_else(|| layer.map(|l| l.linetype_scale).unwrap_or(1.0));
            let effective_lt_scale = (linetype_scale * local_lt_scale).max(1e-6);

            match &entity.kind {
                EntityKind::Line { start, end } => {
                    let pts = [Vec2::new(start.x, start.y), Vec2::new(end.x, end.y)];
                    Self::push_linetype_path(
                        &mut vertices,
                        &pts,
                        false,
                        c,
                        effective_linetype,
                        effective_lt_scale,
                    );
                }
                EntityKind::Circle { center, radius } => {
                    let pts = Self::sample_circle_points(Vec2::new(center.x, center.y), *radius);
                    Self::push_linetype_path(
                        &mut vertices,
                        &pts,
                        true,
                        c,
                        effective_linetype,
                        effective_lt_scale,
                    );
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let pts = Self::sample_arc_points(
                        Vec2::new(center.x, center.y),
                        *radius,
                        *start_angle,
                        *end_angle,
                    );
                    Self::push_linetype_path(
                        &mut vertices,
                        &pts,
                        false,
                        c,
                        effective_linetype,
                        effective_lt_scale,
                    );
                }
                EntityKind::Polyline { vertices: verts, closed } => {
                    if verts.len() < 2 {
                        continue;
                    }
                    let pts: Vec<Vec2> = verts.iter().map(|v| Vec2::new(v.x, v.y)).collect();
                    Self::push_linetype_path(
                        &mut vertices,
                        &pts,
                        *closed,
                        c,
                        effective_linetype,
                        effective_lt_scale,
                    );
                }
                EntityKind::DimAligned { start, end, offset, text_override, text_pos, arrow_length, arrow_half_width } => {
                    let sx = start.x as f32;
                    let sy = start.y as f32;
                    let ex = end.x as f32;
                    let ey = end.y as f32;

                    let dx = ex - sx;
                    let dy = ey - sy;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1e-6 {
                        continue;
                    }

                    let dir = [dx / len, dy / len];
                    let perp = [-dir[1], dir[0]]; // 90° CCW
                    let off = *offset as f32;
                    let sign = if off >= 0.0 { 1.0f32 } else { -1.0f32 };

                    // Dimension line endpoints
                    let dl1 = [sx + perp[0] * off, sy + perp[1] * off];
                    let dl2 = [ex + perp[0] * off, ey + perp[1] * off];

                    // Extension line colour (dimmed)
                    let ec = [c[0] * 0.75, c[1] * 0.75, c[2] * 0.75];
                    let gap = 1.0f32;

                    // Extension line 1: from start+gap toward dim-line side
                    let ext1_s = [sx + perp[0] * sign * gap, sy + perp[1] * sign * gap];
                    vertices.push(Vertex::new(ext1_s[0], ext1_s[1], ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(dl1[0], dl1[1], ec[0], ec[1], ec[2]));

                    // Extension line 2: from end+gap toward dim-line side
                    let ext2_s = [ex + perp[0] * sign * gap, ey + perp[1] * sign * gap];
                    vertices.push(Vertex::new(ext2_s[0], ext2_s[1], ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(dl2[0], dl2[1], ec[0], ec[1], ec[2]));

                    // Dimension line
                    vertices.push(Vertex::new(dl1[0], dl1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl2[0], dl2[1], c[0], c[1], c[2]));

                    // Arrows
                    let arrow_len = *arrow_length as f32;
                    let arrow_hw = *arrow_half_width as f32;
                    let span = ((dl2[0] - dl1[0]).powi(2) + (dl2[1] - dl1[1]).powi(2)).sqrt();
                    // If the extension lines are too close for inward arrows, place them outside
                    // and mirror direction so arrowheads point toward the dimension line.
                    let arrows_outside = span < arrow_len * 2.0;

                    // Arrow at dl1
                    let a1_s = if arrows_outside { -1.0f32 } else { 1.0f32 };
                    let a1_base = [dl1[0] + dir[0] * arrow_len * a1_s, dl1[1] + dir[1] * arrow_len * a1_s];
                    let a1_w1 = [a1_base[0] + perp[0] * arrow_hw, a1_base[1] + perp[1] * arrow_hw];
                    let a1_w2 = [a1_base[0] - perp[0] * arrow_hw, a1_base[1] - perp[1] * arrow_hw];
                    vertices.push(Vertex::new(dl1[0], dl1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w1[0], a1_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl1[0], dl1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w2[0], a1_w2[1], c[0], c[1], c[2]));

                    // Arrow at dl2
                    let a2_s = if arrows_outside { 1.0f32 } else { -1.0f32 };
                    let a2_base = [dl2[0] + dir[0] * arrow_len * a2_s, dl2[1] + dir[1] * arrow_len * a2_s];
                    let a2_w1 = [a2_base[0] + perp[0] * arrow_hw, a2_base[1] + perp[1] * arrow_hw];
                    let a2_w2 = [a2_base[0] - perp[0] * arrow_hw, a2_base[1] - perp[1] * arrow_hw];
                    vertices.push(Vertex::new(dl2[0], dl2[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w1[0], a2_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl2[0], dl2[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w2[0], a2_w2[1], c[0], c[1], c[2]));

                    // Text label is rendered by the egui overlay (draw_dim_entities),
                    // not by the wgpu vertex buffer.
                    let _ = (text_override, text_pos);
                }
                EntityKind::DimLinear { start, end, offset, text_override, text_pos, horizontal, arrow_length, arrow_half_width } => {
                    let sx = start.x as f32;
                    let sy = start.y as f32;
                    let ex = end.x as f32;
                    let ey = end.y as f32;
                    let off = *offset as f32;
                    let sign = if off >= 0.0 { 1.0f32 } else { -1.0f32 };
                    let gap = 1.0f32;
                    let arrow_len = *arrow_length as f32;
                    let arrow_hw = *arrow_half_width as f32;
                    let ec = [c[0] * 0.75, c[1] * 0.75, c[2] * 0.75];

                    let (dl1, dl2, ext1_s, ext2_s, dir, perp) = if *horizontal {
                        // Horizontal dim: measures X, dim line at mid_y + offset
                        let mid_y = (sy + ey) * 0.5;
                        let dim_y = mid_y + off;
                        let dl1 = [sx, dim_y];
                        let dl2 = [ex, dim_y];
                        let ext1_s = [sx, sy + sign * gap];
                        let ext2_s = [ex, ey + sign * gap];
                        (dl1, dl2, ext1_s, ext2_s, [1.0f32, 0.0f32], [0.0f32, 1.0f32])
                    } else {
                        // Vertical dim: measures Y, dim line at mid_x + offset
                        let mid_x = (sx + ex) * 0.5;
                        let dim_x = mid_x + off;
                        let dl1 = [dim_x, sy];
                        let dl2 = [dim_x, ey];
                        let ext1_s = [sx + sign * gap, sy];
                        let ext2_s = [ex + sign * gap, ey];
                        (dl1, dl2, ext1_s, ext2_s, [0.0f32, 1.0f32], [1.0f32, 0.0f32])
                    };

                    // Extension lines
                    vertices.push(Vertex::new(ext1_s[0], ext1_s[1], ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(dl1[0], dl1[1], ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(ext2_s[0], ext2_s[1], ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(dl2[0], dl2[1], ec[0], ec[1], ec[2]));

                    // Dimension line — orient so dl_a has lower coord value
                    let (dl_a, dl_b) = if dir[0] > 0.5 {
                        if dl1[0] <= dl2[0] { (dl1, dl2) } else { (dl2, dl1) }
                    } else {
                        if dl1[1] <= dl2[1] { (dl1, dl2) } else { (dl2, dl1) }
                    };
                    vertices.push(Vertex::new(dl_a[0], dl_a[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl_b[0], dl_b[1], c[0], c[1], c[2]));
                    let span = ((dl_b[0] - dl_a[0]).powi(2) + (dl_b[1] - dl_a[1]).powi(2)).sqrt();
                    let arrows_outside = span < arrow_len * 2.0;

                    // Arrow at dl_a
                    let a1_s = if arrows_outside { -1.0f32 } else { 1.0f32 };
                    let a1_base = [dl_a[0] + dir[0] * arrow_len * a1_s, dl_a[1] + dir[1] * arrow_len * a1_s];
                    let a1_w1 = [a1_base[0] + perp[0] * arrow_hw, a1_base[1] + perp[1] * arrow_hw];
                    let a1_w2 = [a1_base[0] - perp[0] * arrow_hw, a1_base[1] - perp[1] * arrow_hw];
                    vertices.push(Vertex::new(dl_a[0], dl_a[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w1[0], a1_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl_a[0], dl_a[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w2[0], a1_w2[1], c[0], c[1], c[2]));

                    // Arrow at dl_b
                    let a2_s = if arrows_outside { 1.0f32 } else { -1.0f32 };
                    let a2_base = [dl_b[0] + dir[0] * arrow_len * a2_s, dl_b[1] + dir[1] * arrow_len * a2_s];
                    let a2_w1 = [a2_base[0] + perp[0] * arrow_hw, a2_base[1] + perp[1] * arrow_hw];
                    let a2_w2 = [a2_base[0] - perp[0] * arrow_hw, a2_base[1] - perp[1] * arrow_hw];
                    vertices.push(Vertex::new(dl_b[0], dl_b[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w1[0], a2_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl_b[0], dl_b[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w2[0], a2_w2[1], c[0], c[1], c[2]));

                    let _ = (text_override, text_pos);
                }
                EntityKind::DimAngular {
                    vertex, line1_pt, line2_pt, radius,
                    text_override, text_pos,
                    arrow_length, arrow_half_width,
                } => {
                    use std::f32::consts::TAU;
                    let vx = vertex.x as f32;
                    let vy = vertex.y as f32;
                    let a1 = ((line1_pt.y - vertex.y) as f32).atan2((line1_pt.x - vertex.x) as f32);
                    let mut a2 = ((line2_pt.y - vertex.y) as f32).atan2((line2_pt.x - vertex.x) as f32);
                    if a2 <= a1 { a2 += TAU; }
                    let rad = *radius as f32;
                    if rad < 1e-6 { continue; }

                    let ec = [c[0] * 0.75, c[1] * 0.75, c[2] * 0.75];
                    let gap = 1.0f32;
                    let ext_extra = 2.0f32;

                    // Extension lines: from gap inside arc to slightly past arc
                    let r_inner = (rad - gap).max(0.0);
                    let r_outer = rad + ext_extra;
                    let cos1 = a1.cos(); let sin1 = a1.sin();
                    let cos2 = a2.cos(); let sin2 = a2.sin();
                    vertices.push(Vertex::new(vx + cos1 * r_inner, vy + sin1 * r_inner, ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(vx + cos1 * r_outer, vy + sin1 * r_outer, ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(vx + cos2 * r_inner, vy + sin2 * r_inner, ec[0], ec[1], ec[2]));
                    vertices.push(Vertex::new(vx + cos2 * r_outer, vy + sin2 * r_outer, ec[0], ec[1], ec[2]));

                    // Arc segments
                    let sweep = a2 - a1;
                    let steps = ((sweep * rad).abs().max(6.0) as usize).clamp(12, 96);
                    let arc_pts: Vec<[f32; 2]> = (0..=steps).map(|i| {
                        let t = i as f32 / steps as f32;
                        let a = a1 + sweep * t;
                        [vx + rad * a.cos(), vy + rad * a.sin()]
                    }).collect();
                    for pair in arc_pts.windows(2) {
                        vertices.push(Vertex::new(pair[0][0], pair[0][1], c[0], c[1], c[2]));
                        vertices.push(Vertex::new(pair[1][0], pair[1][1], c[0], c[1], c[2]));
                    }

                    // Arrowheads at arc endpoints (open V shape)
                    let arrow_len = *arrow_length as f32;
                    let arrow_hw = *arrow_half_width as f32;

                    // Arrow at a1: tangent direction CCW = [-sin(a1), cos(a1)], radial perp = [cos1, sin1]
                    let t1 = [-sin1, cos1];
                    let a1_tip = [vx + cos1 * rad, vy + sin1 * rad];
                    let a1_base = [a1_tip[0] + t1[0] * arrow_len, a1_tip[1] + t1[1] * arrow_len];
                    let a1_w1 = [a1_base[0] + cos1 * arrow_hw, a1_base[1] + sin1 * arrow_hw];
                    let a1_w2 = [a1_base[0] - cos1 * arrow_hw, a1_base[1] - sin1 * arrow_hw];
                    vertices.push(Vertex::new(a1_tip[0], a1_tip[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w1[0], a1_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_tip[0], a1_tip[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w2[0], a1_w2[1], c[0], c[1], c[2]));

                    // Arrow at a2: CW tangent = [sin(a2), -cos(a2)], radial perp = [cos2, sin2]
                    let t2 = [sin2, -cos2];
                    let a2_tip = [vx + cos2 * rad, vy + sin2 * rad];
                    let a2_base = [a2_tip[0] + t2[0] * arrow_len, a2_tip[1] + t2[1] * arrow_len];
                    let a2_w1 = [a2_base[0] + cos2 * arrow_hw, a2_base[1] + sin2 * arrow_hw];
                    let a2_w2 = [a2_base[0] - cos2 * arrow_hw, a2_base[1] - sin2 * arrow_hw];
                    vertices.push(Vertex::new(a2_tip[0], a2_tip[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w1[0], a2_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_tip[0], a2_tip[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w2[0], a2_w2[1], c[0], c[1], c[2]));

                    let _ = (text_override, text_pos);
                }
                EntityKind::DimRadial {
                    center,
                    radius,
                    leader_pt,
                    is_diameter,
                    text_override,
                    text_pos,
                    arrow_length,
                    arrow_half_width,
                } => {
                    let cx = center.x as f32;
                    let cy = center.y as f32;
                    let lx = leader_pt.x as f32;
                    let ly = leader_pt.y as f32;
                    let r = *radius as f32;
                    if r < 1e-6 {
                        continue;
                    }

                    let dx = lx - cx;
                    let dy = ly - cy;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 1e-6 {
                        continue;
                    }
                    let dir = [dx / len, dy / len];

                    // Arrowhead tips on the measured circle.
                    let tip_outer = [cx + dir[0] * r, cy + dir[1] * r];
                    let tip_inner = [cx - dir[0] * r, cy - dir[1] * r];

                    if *is_diameter {
                        // Diameter: full line through center with two arrows.
                        vertices.push(Vertex::new(tip_inner[0], tip_inner[1], c[0], c[1], c[2]));
                        vertices.push(Vertex::new(lx, ly, c[0], c[1], c[2]));
                    } else {
                        // Radius: center-to-leader line with one arrow at the circle edge.
                        vertices.push(Vertex::new(cx, cy, c[0], c[1], c[2]));
                        vertices.push(Vertex::new(lx, ly, c[0], c[1], c[2]));
                    }

                    let arrow_len = *arrow_length as f32;
                    let arrow_hw = *arrow_half_width as f32;

                    // Outer arrow points toward center (-dir).
                    let a1_base = [tip_outer[0] + dir[0] * arrow_len, tip_outer[1] + dir[1] * arrow_len];
                    let a1_w1 = [a1_base[0] - dir[1] * arrow_hw, a1_base[1] + dir[0] * arrow_hw];
                    let a1_w2 = [a1_base[0] + dir[1] * arrow_hw, a1_base[1] - dir[0] * arrow_hw];
                    vertices.push(Vertex::new(tip_outer[0], tip_outer[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w1[0], a1_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(tip_outer[0], tip_outer[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w2[0], a1_w2[1], c[0], c[1], c[2]));

                    if *is_diameter {
                        // Inner arrow also points toward center (+dir).
                        let a2_base = [tip_inner[0] - dir[0] * arrow_len, tip_inner[1] - dir[1] * arrow_len];
                        let a2_w1 = [a2_base[0] - dir[1] * arrow_hw, a2_base[1] + dir[0] * arrow_hw];
                        let a2_w2 = [a2_base[0] + dir[1] * arrow_hw, a2_base[1] - dir[0] * arrow_hw];
                        vertices.push(Vertex::new(tip_inner[0], tip_inner[1], c[0], c[1], c[2]));
                        vertices.push(Vertex::new(a2_w1[0], a2_w1[1], c[0], c[1], c[2]));
                        vertices.push(Vertex::new(tip_inner[0], tip_inner[1], c[0], c[1], c[2]));
                        vertices.push(Vertex::new(a2_w2[0], a2_w2[1], c[0], c[1], c[2]));
                    }

                    let _ = (text_override, text_pos);
                }
                EntityKind::Text { .. } => {
                    // Text entities are rendered by the egui overlay (painter.text),
                    // not by the wgpu vertex buffer.  Nothing to emit here.
                }
                EntityKind::Insert {
                    name,
                    position,
                    rotation,
                    scale_x,
                    scale_y,
                } => {
                    let Some(def) = drawing.get_block(name) else {
                        continue;
                    };
                    let sx = *scale_x;
                    let sy = *scale_y;
                    let ca = rotation.cos();
                    let sa = rotation.sin();
                    let tp = |x: f64, y: f64| -> Vec2 {
                        let lx = x * sx;
                        let ly = y * sy;
                        Vec2::new(
                            position.x + lx * ca - ly * sa,
                            position.y + lx * sa + ly * ca,
                        )
                    };
                    for be in &def.entities {
                        let blayer = drawing.get_layer(be.layer);
                        let bc = if let Some(ec) = be.color {
                            [ec[0] as f32 / 255.0, ec[1] as f32 / 255.0, ec[2] as f32 / 255.0]
                        } else {
                            blayer
                                .map(|l| {
                                    [
                                        l.color[0] as f32 / 255.0,
                                        l.color[1] as f32 / 255.0,
                                        l.color[2] as f32 / 255.0,
                                    ]
                                })
                                .unwrap_or(c)
                        };
                        let blt = if be.linetype_by_layer {
                            blayer.map(|l| l.linetype).unwrap_or(Linetype::Continuous)
                        } else {
                            be.linetype
                        };
                        let blts = be
                            .linetype_scale
                            .unwrap_or_else(|| blayer.map(|l| l.linetype_scale).unwrap_or(1.0));
                        let beff_scale = (linetype_scale * blts).max(1e-6);
                        match &be.kind {
                            EntityKind::Line { start, end } => {
                                let a = tp(start.x, start.y);
                                let b = tp(end.x, end.y);
                                Self::push_linetype_path(
                                    &mut vertices,
                                    &[a, b],
                                    false,
                                    bc,
                                    blt,
                                    beff_scale,
                                );
                            }
                            EntityKind::Circle { center, radius } => {
                                let cc = tp(center.x, center.y);
                                let rr = *radius * ((sx.abs() + sy.abs()) * 0.5).max(1e-9);
                                let pts = Self::sample_circle_points(cc, rr);
                                Self::push_linetype_path(
                                    &mut vertices,
                                    &pts,
                                    true,
                                    bc,
                                    blt,
                                    beff_scale,
                                );
                            }
                            EntityKind::Arc {
                                center,
                                radius,
                                start_angle,
                                end_angle,
                            } => {
                                let cc = tp(center.x, center.y);
                                let rr = *radius * ((sx.abs() + sy.abs()) * 0.5).max(1e-9);
                                let pts = Self::sample_arc_points(
                                    cc,
                                    rr,
                                    *start_angle + *rotation,
                                    *end_angle + *rotation,
                                );
                                Self::push_linetype_path(
                                    &mut vertices,
                                    &pts,
                                    false,
                                    bc,
                                    blt,
                                    beff_scale,
                                );
                            }
                            EntityKind::Polyline { vertices: verts, closed } => {
                                if verts.len() < 2 {
                                    continue;
                                }
                                let pts: Vec<Vec2> = verts.iter().map(|v| tp(v.x, v.y)).collect();
                                Self::push_linetype_path(
                                    &mut vertices,
                                    &pts,
                                    *closed,
                                    bc,
                                    blt,
                                    beff_scale,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        vertices
    }
    
    /// Get the rendered texture (for display in egui)
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Get the rendered texture view (for display registration in egui-wgpu).
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    /// Current viewport size in pixels.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns `clear_color` converted to sRGB bytes `[r, g, b]` (0–255).
    /// Use this to match the egui overlay mask colour to the wgpu background.
    pub fn bg_srgb(&self) -> [u8; 3] {
        let to_srgb = |v: f32| (v.powf(1.0 / 2.2).clamp(0.0, 1.0) * 255.0).round() as u8;
        [
            to_srgb(self.clear_color[0]),
            to_srgb(self.clear_color[1]),
            to_srgb(self.clear_color[2]),
        ]
    }
    
    /// Pan the viewport
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx / self.zoom;
        self.pan_y += dy / self.zoom;
    }
    
    /// Zoom the viewport (centered on current view)
    pub fn zoom_delta(&mut self, delta: f32) {
        self.zoom *= 1.0 + delta;
        self.zoom = self.zoom.clamp(0.1, 100.0);
    }
}

/// Convert viewport-local screen pixel coordinates to world coordinates.
///
/// `screen_x`/`screen_y` are expected in viewport pixel space where `(0, 0)` is top-left.
pub fn screen_to_world(screen_x: f32, screen_y: f32, viewport: &Viewport) -> Vec2 {
    let width = viewport.width.max(1) as f32;
    let height = viewport.height.max(1) as f32;

    let aspect = width / height;
    let half_height = 100.0 / viewport.zoom;
    let half_width = half_height * aspect;

    let left = -half_width + viewport.pan_x;
    let right = half_width + viewport.pan_x;
    let bottom = -half_height + viewport.pan_y;
    let top = half_height + viewport.pan_y;

    let world_x = left + (screen_x / width) * (right - left);
    let world_y = top - (screen_y / height) * (top - bottom);

    Vec2::new(world_x as f64, world_y as f64)
}

/// Convert world coordinates to viewport-local screen pixel coordinates.
///
/// Returns `(x, y)` in viewport pixel space where `(0, 0)` is top-left.
pub fn world_to_screen(world_x: f32, world_y: f32, viewport: &Viewport) -> (f32, f32) {
    let width = viewport.width.max(1) as f32;
    let height = viewport.height.max(1) as f32;

    let aspect = width / height;
    let half_height = 100.0 / viewport.zoom;
    let half_width = half_height * aspect;

    let left = -half_width + viewport.pan_x;
    let right = half_width + viewport.pan_x;
    let bottom = -half_height + viewport.pan_y;
    let top = half_height + viewport.pan_y;

    let screen_x = ((world_x - left) / (right - left)) * width;
    let screen_y = ((top - world_y) / (top - bottom)) * height;

    (screen_x, screen_y)
}
