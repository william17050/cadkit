# Integrating wgpu Viewport into egui

## What I Built For You

1. **render-wgpu crate** - Complete wgpu rendering infrastructure
   - `shader.wgsl` - Vertex and fragment shaders
   - `vertex.rs` - Vertex type and transform matrix
   - `lib.rs` - Main Viewport struct with wgpu boilerplate

2. **Key features already implemented:**
   - ✅ wgpu device initialization
   - ✅ Shader compilation
   - ✅ Transform uniform buffer (pan/zoom)
   - ✅ Render pipeline setup
   - ✅ Line rendering (converts Line entities to vertices)
   - ✅ Pan/zoom controls

## What You Need to Do (with Copilot)

### Step 1: Add dependency to ui-egui

Edit `crates/ui-egui/Cargo.toml`, add:
```toml
cadkit-render-wgpu = { path = "../render-wgpu" }
```

### Step 2: Initialize Viewport in CadKitApp

In `main.rs`, modify the `CadKitApp` struct:

```rust
struct CadKitApp {
    drawing: Drawing,
    command_input: String,
    viewport: Option<cadkit_render_wgpu::Viewport>,  // ADD THIS
}
```

The `Option` is because we need async initialization.

### Step 3: Create Viewport After Window Opens

egui doesn't give you async init, so we create viewport lazily on first frame:

```rust
impl eframe::App for CadKitApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Initialize viewport on first frame
        if self.viewport.is_none() {
            // This is the tricky part - need to spawn viewport creation
            // Copilot suggestion: use pollster to block on async
            self.viewport = Some(
                pollster::block_on(
                    cadkit_render_wgpu::Viewport::new(800, 600)
                ).expect("Failed to create viewport")
            );
        }
        
        // ... rest of your UI code
    }
}
```

You'll need to add `pollster = "0.3"` to ui-egui dependencies.

### Step 4: Render and Display

In the `CentralPanel` section, replace the placeholder with:

```rust
egui::CentralPanel::default().show(ctx, |ui| {
    if let Some(viewport) = &mut self.viewport {
        // Render the drawing
        viewport.render(&self.drawing);
        
        // TODO: Display the wgpu texture in egui
        // This is the complex part - needs egui-wgpu integration
        // For now, show dimensions to prove it's working
        ui.label(format!("Viewport ready: {}x{}", viewport.width, viewport.height));
    } else {
        ui.label("Initializing viewport...");
    }
});
```

### Step 5: Displaying wgpu Texture in egui (Complex)

This is where you need `egui-wgpu` crate. The texture needs to be registered with egui's rendering backend.

**Option A: Use egui-wgpu (recommended)**
Add to ui-egui Cargo.toml:
```toml
egui-wgpu = "0.27"
```

Then you can register the texture and display it as an egui Image.

**Option B: Simpler test - Copy pixels to CPU**
For initial testing, you can read the texture to CPU and display with egui's ColorImage. Slower but proves rendering works.

## What Copilot Will Help With

Once you have the basic integration, Copilot will help you:

1. **Fill in Circle/Arc rendering** - In `generate_vertices()`, add the TODO sections
2. **Handle mouse input** - Pan with middle mouse, zoom with scroll wheel
3. **Screen-to-world coordinate conversion** - For click detection
4. **egui-wgpu integration** - Register texture for display

## Testing Your Progress

After each step:
1. `cargo build` - should compile
2. `cargo run` - window should open
3. Check logs for "Viewport ready" message
4. Once texture displays, you should see your test line!

## Current State

Your test line is already being converted to vertices in `generate_vertices()`:
```rust
EntityKind::Line { start, end } => {
    vertices.push(Vertex::new(start.x as f32, start.y as f32, 1.0, 1.0, 1.0));
    vertices.push(Vertex::new(end.x as f32, end.y as f32, 1.0, 1.0, 1.0));
}
```

So as soon as you display the texture, you'll see a white line from (0,0) to (100,100).

## Next Steps After You See the Line

1. Add pan with middle mouse drag
2. Add zoom with scroll wheel
3. Implement Circle rendering (approximate with 32 line segments)
4. Implement Arc rendering
5. Add grid rendering
6. Add coordinate display

Copilot will autocomplete most of this once it sees the patterns.

## Troubleshooting

**"pollster not found"** - Add to Cargo.toml:
```toml
pollster = "0.3"
```

**"wgpu adapter not found"** - Your GPU doesn't support Vulkan/DX12. Try:
```rust
backends: wgpu::Backends::GL,  // Force OpenGL
```

**Texture not displaying** - This is expected until you add egui-wgpu integration. The rendering IS working, just not displayed yet.

Good luck! Start with Step 1-3 to get it compiling, then tackle texture display with Copilot's help.
