# CadKit Build Instructions

## Prerequisites

### Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### Verify Installation
```bash
rustc --version
cargo --version
```

## Building

### Build All Crates
```bash
cd cadkit_project
cargo build --workspace
```

### Build Release Version
```bash
cargo build --workspace --release
```

### Run the Application
```bash
cargo run --bin cadkit
```

Or after building:
```bash
./target/debug/cadkit
# or
./target/release/cadkit
```

## Testing

### Run All Tests
```bash
cargo test --workspace
```

### Test Specific Crate
```bash
cargo test -p cadkit-types
cargo test -p cadkit-2d-core
```

### Run Tests with Output
```bash
cargo test --workspace -- --nocapture
```

## Development Workflow

### Check for Errors (Fast)
```bash
cargo check --workspace
```

### Format Code
```bash
cargo fmt --all
```

### Lint Code
```bash
cargo clippy --workspace
```

## Current Status

### Implemented
- ✅ types crate: Vec2, Vec3, Guid, Units, Tolerances
- ✅ 2d-core crate: Line, Arc, Circle, Polyline entities
- ✅ 2d-core: Drawing document with layers
- ✅ 2d-core: Save/load to JSON
- ✅ ui-egui: Basic window with menu and panels

### Next Steps
1. Add actual viewport rendering (wgpu integration)
2. Implement interactive line drawing tool
3. Add grid and snap system
4. Implement selection mechanism

## Troubleshooting

### "Cannot find crate" errors
Make sure you're in the project root directory with Cargo.toml workspace file.

### egui/eframe compilation issues
These crates have platform-specific dependencies. On Linux you may need:
```bash
# Ubuntu/Debian
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev

# Fedora
sudo dnf install libxcb-devel
```

### Slow compilation
First build takes time. Subsequent builds are incremental and much faster.
Use `cargo check` during development for faster feedback.

## Project Structure
```
cadkit_project/
├── Cargo.toml          # Workspace configuration
├── crates/
│   ├── types/          # Core types (Vec2/Vec3/Guid)
│   ├── 2d-core/        # 2D entities and drawing
│   └── ui-egui/        # Main application UI
└── target/             # Build output (created by cargo)
```

## Notes for Bill

When you get home from work:
1. Open terminal in cadkit_project directory
2. Run `cargo build --workspace` (first time takes ~5 minutes)
3. Run `cargo run --bin cadkit` to see the window
4. You should see a basic UI with menus and "1 entities" (the test line)

The line isn't rendered yet - that's the next step. But this proves:
- Rust toolchain works
- Entity storage works
- UI framework works
- Save/load works

From here you can start adding real features one at a time.
