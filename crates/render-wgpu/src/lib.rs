//! 2D CAD Viewport Renderer using wgpu
//!
//! This module handles:
//! - wgpu device initialization
//! - Rendering entities to texture
//! - Pan/zoom camera control

pub mod font;
pub mod vertex;

use cadkit_2d_core::{Drawing, EntityKind};
use cadkit_types::Vec2;
use std::sync::Arc;
use vertex::{Vertex, ViewTransform};
use wgpu::util::DeviceExt;

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
    
    width: u32,
    height: u32,
}

impl Viewport {
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
                            r: 0.08,
                            g: 0.08,
                            b: 0.08,
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

        for entity in drawing.visible_entities() {
            // Resolve colour: entity override → layer colour → white fallback.
            let c = if let Some(ec) = entity.color {
                [ec[0] as f32 / 255.0, ec[1] as f32 / 255.0, ec[2] as f32 / 255.0]
            } else {
                drawing
                    .get_layer(entity.layer)
                    .map(|l| {
                        [
                            l.color[0] as f32 / 255.0,
                            l.color[1] as f32 / 255.0,
                            l.color[2] as f32 / 255.0,
                        ]
                    })
                    .unwrap_or([1.0, 1.0, 1.0])
            };

            match &entity.kind {
                EntityKind::Line { start, end } => {
                    vertices.push(Vertex::new(start.x as f32, start.y as f32, c[0], c[1], c[2]));
                    vertices.push(Vertex::new(end.x as f32, end.y as f32, c[0], c[1], c[2]));
                }
                EntityKind::Circle { center, radius } => {
                    let segments = 32;
                    let cx = center.x as f32;
                    let cy = center.y as f32;
                    let r = *radius as f32;

                    for i in 0..segments {
                        let t0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
                        let t1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

                        let x0 = cx + r * t0.cos();
                        let y0 = cy + r * t0.sin();
                        let x1 = cx + r * t1.cos();
                        let y1 = cy + r * t1.sin();

                        vertices.push(Vertex::new(x0, y0, c[0], c[1], c[2]));
                        vertices.push(Vertex::new(x1, y1, c[0], c[1], c[2]));
                    }
                }
                EntityKind::Arc { center, radius, start_angle, end_angle } => {
                    let cx = center.x as f32;
                    let cy = center.y as f32;
                    let r = *radius as f32;
                    let start = *start_angle as f32;
                    let end = *end_angle as f32;

                    let span = end - start;
                    if span.abs() <= f32::EPSILON {
                        continue;
                    }

                    let segments =
                        (((span.abs() / std::f32::consts::TAU) * 32.0).ceil()).max(1.0) as usize;

                    for i in 0..segments {
                        let t0 = start + span * (i as f32 / segments as f32);
                        let t1 = start + span * ((i + 1) as f32 / segments as f32);

                        let x0 = cx + r * t0.cos();
                        let y0 = cy + r * t0.sin();
                        let x1 = cx + r * t1.cos();
                        let y1 = cy + r * t1.sin();

                        vertices.push(Vertex::new(x0, y0, c[0], c[1], c[2]));
                        vertices.push(Vertex::new(x1, y1, c[0], c[1], c[2]));
                    }
                }
                EntityKind::Polyline { vertices: verts, closed } => {
                    if verts.len() < 2 {
                        continue;
                    }
                    let mut iter = verts.iter().peekable();
                    while let Some(a) = iter.next() {
                        if let Some(b) = iter.peek() {
                            vertices.push(Vertex::new(a.x as f32, a.y as f32, c[0], c[1], c[2]));
                            vertices.push(Vertex::new(b.x as f32, b.y as f32, c[0], c[1], c[2]));
                        }
                    }
                    if *closed {
                        let a = verts.last().unwrap();
                        let b = verts.first().unwrap();
                        vertices.push(Vertex::new(a.x as f32, a.y as f32, c[0], c[1], c[2]));
                        vertices.push(Vertex::new(b.x as f32, b.y as f32, c[0], c[1], c[2]));
                    }
                }
                EntityKind::DimLinear { start, end, offset, text_override, text_pos } => {
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

                    // Arrows (V-shape, 3 units long, 0.75 half-width)
                    let arrow_len = 3.0f32;
                    let arrow_hw = 0.75f32;

                    // Arrow at dl1 (pointing in +dir)
                    let a1_base = [dl1[0] + dir[0] * arrow_len, dl1[1] + dir[1] * arrow_len];
                    let a1_w1 = [a1_base[0] + perp[0] * arrow_hw, a1_base[1] + perp[1] * arrow_hw];
                    let a1_w2 = [a1_base[0] - perp[0] * arrow_hw, a1_base[1] - perp[1] * arrow_hw];
                    vertices.push(Vertex::new(dl1[0], dl1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w1[0], a1_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl1[0], dl1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a1_w2[0], a1_w2[1], c[0], c[1], c[2]));

                    // Arrow at dl2 (pointing in -dir)
                    let a2_base = [dl2[0] - dir[0] * arrow_len, dl2[1] - dir[1] * arrow_len];
                    let a2_w1 = [a2_base[0] + perp[0] * arrow_hw, a2_base[1] + perp[1] * arrow_hw];
                    let a2_w2 = [a2_base[0] - perp[0] * arrow_hw, a2_base[1] - perp[1] * arrow_hw];
                    vertices.push(Vertex::new(dl2[0], dl2[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w1[0], a2_w1[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(dl2[0], dl2[1], c[0], c[1], c[2]));
                    vertices.push(Vertex::new(a2_w2[0], a2_w2[1], c[0], c[1], c[2]));

                    // Text label
                    let dist = start.distance_to(end);
                    let label = match text_override {
                        Some(s) => s.clone(),
                        None => format!("{:.3}", dist),
                    };
                    let tx = text_pos.x as f32;
                    let ty = text_pos.y as f32;
                    let up = [perp[0] * sign, perp[1] * sign];
                    for (p1, p2) in font::text_segments(&label, [tx, ty], dir, up, 5.0) {
                        vertices.push(Vertex::new(p1[0], p1[1], c[0], c[1], c[2]));
                        vertices.push(Vertex::new(p2[0], p2[1], c[0], c[1], c[2]));
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
