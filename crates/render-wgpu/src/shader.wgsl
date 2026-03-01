// 2D CAD Viewport Shader (WGSL)

// Vertex shader transforms world coordinates to screen space
struct VertexInput {
    @location(0) position: vec2<f32>,  // World coordinates
    @location(1) color: vec3<f32>,     // RGB color
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

struct ViewTransform {
    view_proj: mat4x4<f32>,  // Combined view + projection matrix
}

@group(0) @binding(0)
var<uniform> transform: ViewTransform;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform 2D world coords to clip space
    let world_pos = vec4<f32>(in.position.x, in.position.y, 0.0, 1.0);
    out.clip_position = transform.view_proj * world_pos;
    out.color = in.color;
    
    return out;
}

// Fragment shader outputs color
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);  // RGB + alpha
}
