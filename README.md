# CadKit - Modular CAD Platform

**Pragmatic 2D CAD core with room to grow into 3D, CAM, and AI.**

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
- **geometry**: 2D intersection calculations (line/arc/circle/polyline)
- **region-find**: Boundary detection (for hatch and push/pull)
- **direct-3d**: SketchUp-style push/pull operations
- **render-wgpu**: Viewport rendering (wgpu LineList, stroke font)
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

## Current 2D Feature Set (as of Mar 2026)
- **Drawing tools**: line, arc (3-point), circle, polyline with close; command aliases and toolbar buttons.
- **Precision input**: absolute/relative (@x,y) and polar (@dist<angle); direct distance entry with live rubber-band; FROM offset workflow; ortho lock (F8); snap toggle (F3).
- **Snaps**: endpoint, midpoint, center, intersection (radius search).
- **Editing**: move, copy, rotate, offset, trim, extend — all with ghosted rubber-band previews; cancel via Esc or right-click; undo/redo stack.
- **Dimensions**: DIMLINEAR command (`dli`) — 3-click placement (first point, second point, line location); live preview with stroke text; readable text regardless of pick direction.
- **Layers**: create, color, rename, set current, toggle visibility; selection highlights by layer.
- **IO**: JSON save/load; DXF import/export with per-entity warnings; SVG/PDF export (paths-only/vector); auto-save recovery snapshots with startup restore prompt; file dialogs; window title reflects current file.
- **Rendering/UI**: wgpu viewport, dot grid, selection marquee (window/crossing), command log, left tool palette, top menu bar, right properties/layers panel.

## Near-Term Roadmap (Q2 2026)
- **Dimension polish**: egui native text rendering for dim labels (replacing stroke font); DimStyle dialog (text height, arrow size, extension gap, color); DXF DIMENSION entity export.
- **Text placement**: TEXT command for placing annotation text entities on the drawing.
- Additional snaps (perpendicular, tangent, nearest) and improved snap glyphs.
- Status bar: live cursor coordinates, active snap/ortho/layer indicators.
- Scale and mirror editing commands.
- Preference persistence (grid spacing, last file, snap/ortho state).

## Longer-Term (high level)
- Hatch patterns, leaders/callouts, multi-line text, blocks/references.
- Python bridge and AI command line.
- Push/pull 3D prototype, then CAM and cabinet workflows.

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
git clone https://github.com/william17050/cadkit
cd cadkit
cargo build --release

# Run
cargo run -p cadkit

# Run (Python scripting enabled)
./run.sh --py

# Sample script to test PYRUN
# (Inside CadKit command line: PYRUN -> pick scripts/test_pyrun.py)
# In-app Python console: type PYCON
# Python console autocomplete: Ctrl+Space (or Complete button)
# Natural-language to Python preview: type AICMD
# MCP status check: type MCP
# AICMD backend can use LM Studio local API (default URL: http://127.0.0.1:1234/v1/chat/completions)
# AICMD profile: Strict CAD Code (recommended) or General
# Insert API reference/examples into AICMD prompt: AIHELP
```

## Testing
```bash
# Run all tests
cargo test --workspace

# Test specific crate
cargo test -p cadkit-types
```

## Project Status
**Current**: Interactive 2D drafting MVP — command-line tools, snaps, layers, undo/redo, DXF IO, linear dimensions.
**Next Milestone**: Dim text via egui font + DimStyle dialog + TEXT placement command.

---
*Built by Bill - 20+ years manufacturing experience, ready to ship what should exist.*
