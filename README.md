# CadKit - Modular CAD Platform

**Professional 2D/3D CAD with Python Scripting & AI Assistance**

## Vision
Build a modern, affordable CAD platform that combines:
- AutoCAD LT-level 2D drafting
- SketchUp-style direct 3D modeling
- Integrated CNC/CAM toolpath generation
- Cabinet design automation
- Python scripting for customization
- Local AI assistance for natural language commands

## Target Market
- Cabinet/case manufacturers (compete with Microvellum/Cabinet Vision)
- Small CNC shops (affordable CAD+CAM solution)
- Hobbyist makers (DIY-friendly with AI help)
- Manufacturing engineers (practical tools, not bloatware)

## Architecture
Modular Rust workspace with clear separation of concerns:
- **types**: Core data types (Vec2/Vec3, Guid, tolerances)
- **2d-core**: Drafting entities, layers, snaps
- **region-find**: Boundary detection (for hatch and push/pull)
- **direct-3d**: SketchUp-style push/pull operations
- **render-wgpu**: Viewport rendering
- **ui-egui**: User interface shell
- **scripting-python**: Embedded Python runtime (PyO3)
- **ai-engine**: Pluggable AI backend (Claude Desktop/API/local)
- **ai-python-bridge**: Exposes AI to Python scripts
- **plugin-host**: C-ABI plugin system for extensibility

## Coordinate System
- **X-axis**: Right (positive)
- **Y-axis**: Forward/Away (positive)
- **Z-axis**: Up (positive)
- **XY plane**: Ground/construction plane (Z=0 for 2D phase)
- **Right-handed system**

## Development Phases

### Phase 1: 2D Drafting Foundation (Year 1)
- [ ] Basic entities (Line, Arc, Circle, Polyline)
- [ ] Viewport with pan/zoom
- [ ] Object snaps (endpoint, midpoint, center, etc)
- [ ] Grid and coordinate display
- [ ] Layers and selection
- [ ] Basic edit operations (move, copy, rotate, delete)
- [ ] Save/load (native format)
- [ ] Python scripting bridge

### Phase 2: AI & Advanced 2D (Year 2)
- [ ] AI command-line interface ("move entity A forward 3 inches")
- [ ] Text and dimensions
- [ ] Blocks/references
- [ ] Region detection (hatch boundaries)
- [ ] Hatching
- [ ] DXF import/export
- [ ] More edit ops (offset, trim, extend, fillet, chamfer)

### Phase 3: Direct 3D Modeling (Year 3)
- [ ] Push/pull extrusion using region detection
- [ ] Boolean operations (union, subtract)
- [ ] 3D viewport navigation
- [ ] STL/OBJ export
- [ ] Basic rendering (shaded/wireframe)

### Phase 4: CNC/CAM Integration (Year 4)
- [ ] Toolpath generation (pocket, profile, drill)
- [ ] Post-processors (Mach3, Fanuc/KOMO, Planet CNC)
- [ ] Feed/speed calculations
- [ ] Material database
- [ ] G-code simulation
- [ ] Test on home CNC (30x24 Planet CNC)

### Phase 5: Cabinet Designer Module (Year 5-6)
- [ ] Parametric cabinet templates
- [ ] Cut list generation with nesting
- [ ] Hardware catalog integration
- [ ] Assembly drawings
- [ ] Pricing/quoting engine
- [ ] CNC output for nested cutting

### Phase 6: Parametric & Polish (Year 7)
- [ ] Feature history tree
- [ ] Constraints and relations
- [ ] Assembly mode
- [ ] Performance optimization
- [ ] Documentation
- [ ] Market and seek acquisition interest

## Technology Stack
- **Language**: Rust (performance, safety, packaging)
- **UI**: egui (immediate mode, cross-platform)
- **Rendering**: wgpu (modern graphics API)
- **Scripting**: PyO3 (embedded Python)
- **AI**: MCP protocol (Claude Desktop), local models (Phi-3)
- **File Format**: Serde JSON (dev), binary (production)

## Development Strategy
- **One crate at a time**: Work on focused modules, prevent context collapse
- **AI-assisted**: Use Claude Code/Desktop, rotate to Copilot/ChatGPT when rate-limited
- **Test-driven**: Home CNC for real-world validation
- **User-first**: Build for personal use, customers are bonus
- **Exit-ready**: Clean architecture for potential acquisition

## License
Proprietary - All Rights Reserved
See EULA.txt for terms

## Building
```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone <repo>
cd cadkit_project
cargo build --release

# Run
cargo run --bin cadkit
```

## Testing
```bash
# Run all tests
cargo test --workspace

# Test specific crate
cargo test -p cadkit-types
```

## Project Status
**Current Phase**: Foundation setup
**Next Milestone**: Draw a line in the viewport

---
*Built by Bill - 20+ years manufacturing experience, ready to ship what should exist.*
