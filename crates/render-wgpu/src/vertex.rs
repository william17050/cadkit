//! Vertex type for 2D CAD rendering

use bytemuck::{Pod, Zeroable};

/// Vertex for 2D geometry
/// Contains position in world coordinates and color
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],  // X, Y in world coordinates
    pub color: [f32; 3],     // RGB
}

impl Vertex {
    pub fn new(x: f32, y: f32, r: f32, g: f32, b: f32) -> Self {
        Self {
            position: [x, y],
            color: [r, g, b],
        }
    }
    
    /// Vertex buffer layout descriptor for wgpu
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position at location 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Color at location 1
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

/// Transform matrix uniform buffer
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ViewTransform {
    pub view_proj: [[f32; 4]; 4],  // 4x4 matrix
}

impl ViewTransform {
    /// Create identity transform (no zoom/pan)
    pub fn identity() -> Self {
        Self {
            view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
    
    /// Create orthographic projection for 2D viewport
    /// screen_width, screen_height in pixels
    /// zoom: scale factor (1.0 = normal, 2.0 = zoomed in 2x)
    /// pan_x, pan_y: camera offset in world units
    pub fn orthographic(
        screen_width: f32,
        screen_height: f32,
        zoom: f32,
        pan_x: f32,
        pan_y: f32,
    ) -> Self {
        // Calculate viewport bounds in world space
        let aspect = screen_width / screen_height;
        let half_height = 100.0 / zoom; // Base view height
        let half_width = half_height * aspect;
        
        // Orthographic projection matrix
        let left = -half_width + pan_x;
        let right = half_width + pan_x;
        let bottom = -half_height + pan_y;
        let top = half_height + pan_y;
        
        let sx = 2.0 / (right - left);
        let sy = 2.0 / (top - bottom);
        let tx = -(right + left) / (right - left);
        let ty = -(top + bottom) / (top - bottom);
        
        Self {
            view_proj: [
                [sx, 0.0, 0.0, 0.0],
                [0.0, sy, 0.0, 0.0],
                [0.0, 0.0, -1.0, 0.0],
                [tx, ty, 0.0, 1.0],
            ],
        }
    }
}
