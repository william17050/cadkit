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

## Current 2D Feature Set (as of Aug 2026)
- **Drawing tools**: line, arc (3-point), circle, polyline with close, rectangle, polygon, ellipse, isocircle, boundary, hatch, text, and multiple dimension commands; command aliases and toolbar buttons.
- **Precision input**: absolute/relative (@x,y) and polar (@dist<angle); direct distance entry with live rubber-band; FROM offset workflow; orthogonal Ortho mode (F8); Polar tracking (F10); snap toggle (F3); ISO mode/isoplane.
- **Snaps**: endpoint, midpoint, center, quadrant, intersection, perpendicular, tangent, nearest, plus parallel/perpendicular tracking and visual snap glyphs.
- **Editing**: move, copy, rotate, scale, mirror, offset, trim, extend, stretch, fillet, chamfer, associative rectangular/polar array workflows — all with ghosted previews, undo/redo, and cancel via Esc or right-click.
- **Blocks**: block definitions (`BLOCK`/`BMAKE`), true block inserts (`INSERT`), explode (`X/EXPLODE`), and first-pass block editing workflow (`BEDIT`, `BSAVE`, `BCANCEL`).
- **Block snapping/selection**: insert geometry is selectable/snappable from transformed block geometry (not just insert origin); selected insert highlights full block geometry.
- **Dynamic blocks (developer first pass)**: `dynamic_v1` data model + per-insert override map + runtime regeneration pipeline (authored-base driven, ordered action execution subset).
- **Trim/Extend policy**: non-block entities can trim/extend against block geometry; direct trim/extend of insert internals requires explode or block edit workflow.
- **Dimensions**: linear, angular, radial, and diameter dimensions; live preview; DimStyle editing; egui text rendering; grip editing; DXF DIMENSION import/export.
- **Layers**: create, color, rename, set current, move entities between layers, toggle visibility, lock, and freeze; selection highlights by layer.
- **IO**: JSON save/load; DXF import/export with per-entity warnings; SVG/PDF export (paths-only/vector); auto-save recovery snapshots with startup restore prompt; file dialogs; window title reflects current file.
- **Rendering/UI**: wgpu viewport, dot grid, status bar, selection marquee (window/crossing), command log, left tool palette, top menu bar, right properties/layers panel.
- **Python/AI**: embedded Python console and script runner; initial natural-language command preview/execution path; local-model and MCP-aware AI plumbing.
- **QA automation**: in-app QA bridge with mode toggles, polar-angle control, snap-aware testing, and session-safe preference snapshot/restore for repeatable behavioral QA.

## Near-Term Product Focus
- Leaders and callouts.
- Hatch robustness, especially gap healing under tolerance.
- Blocks follow-up work: nested blocks and a fuller block library/palette.
- DXF and layer-system depth beyond the current stable drafting core.
- Continued polish of modify workflows and annotation ergonomics.

## Longer-Term (high level)
- Hatch patterns, leaders/callouts, multi-line text, dynamic/parametric blocks.
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
# AICMD Phi-3 backend expects `llama-cli` in PATH + a local GGUF model file path
```

## Testing
```bash
# Run all tests
cargo test --workspace

# Test specific crate
cargo test -p cadkit-types
```

## Linetype Controls (ByLayer + Override)
- **Global scale**: `LTS` or `LTSCALE` changes overall dash size multiplier.
- **Layer defaults**: In the right panel, active layer has `Layer LT` and `S` (layer LT scale).
- **Per-entity linetype**: In Properties, set `Linetype` to `ByLayer` or a direct value (`Continuous`, `Hidden`, `Center`).
- **Per-entity LT scale**: In Properties, set `LTScale` to `ByLayer` or type a numeric override.
- Effective dash size is: `global LTSCALE * (entity LTScale override or layer LT scale)`.

## Block Commands
- `BLOCK` / `BMAKE`: create block definition from selected entities.
- `BLOCKMAKE <name>`: developer shortcut to create block definition from current selection with inline name.
- `INSERT <name>`: place block reference by pick point.
- `INSERTBLOCK <name>`: developer alias for insert flow.
- `X` / `EXPLODE`: explode selected inserts/associative arrays into regular entities.
- `BEDIT <name>` or `BEDIT` with an insert selected: open block edit workspace.
- `BSAVE`: commit block edits back to definition.
- `BCANCEL`: discard block edits and restore drawing state.
- Dynamic instance test commands (selected insert): `DYNLIST`, `DYNSET <param> <value>`, `DYNCLEAR <param>`, `DYNCLEARALL`.
- Dynamic definition dev commands:
  - `DYNADDPARAM <block> <param> <axis:X|Y> <default> <min> <max> <step>`
  - `DYNLISTDEF <block>`
  - `DYNADDACTION <block> <param> <move|anchor|stretch|visibility>`
  - `DYNBINDSEL <block> <param> <behavior> <frame> <keepdefault|offset:v|v>`
  - `DYNMAKEGROUP <block> <group_name>`
  - `DYNBINDGROUP <block> <param> <group_name> <behavior> <frame> <keepdefault|offset:v|v>`

## Project Status
**Current**: Mature 2D drafting core with a strong editing, snapping, annotation, DXF/JSON IO, Python/AI, and QA automation foundation.
**Next Milestone**: Return to product-facing feature work on advanced 2D depth, likely leaders/callouts, hatch robustness, or broader block workflows.

---
*Built by Bill - 20+ years manufacturing experience, ready to ship what should exist.*
