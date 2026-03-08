# CadKit — Claude Code Guide

## Project
2D CAD app in Rust. wgpu offscreen texture → egui image widget + egui overlays on top.
GitHub: https://github.com/william17050/cadkit
Branch: `refactor/main-split`
Cargo: `~/.cargo/bin/cargo` (not on PATH)

## Crate Map
| Crate | Path | Role |
|---|---|---|
| `cadkit-types` | `crates/types/` | Vec2, Vec3, Guid |
| `cadkit-2d-core` | `crates/2d-core/` | Drawing model, Entity, EntityKind, DXF I/O |
| `cadkit-geometry` | `crates/geometry/` | 2D intersection math |
| `cadkit-render-wgpu` | `crates/render-wgpu/` | GPU canvas (Viewport struct) |
| `cadkit-ui-egui` | `crates/ui-egui/` | App shell — all UX lives here |

## Key Conventions
- All 2D coords: `Vec3::xy(x, y)` (z=0). Vec3 is Copy.
- Angles: f64 radians, CCW positive from +X axis
- All arcs stored CCW (`end_angle > start_angle`)
- f64 precision throughout
- `deliver_point(world: Vec2)` — central method for all point delivery (click, Enter, FROM)

## ui-egui File Map
- `src/app.rs` — CadKitApp struct, update(), all command logic, draw_* methods
- `src/app/state.rs` — all Phase/Dialog enums
- `src/app/commands.rs` — `execute_command_alias()` — add new commands here
- `src/app/ui_panels.rs` — toolbar, menu bar, properties panel, command line
- `src/app/overlays.rs` — snap pick distances, selection overlay
- `src/app/io.rs` — save/load/export/import

## Current Entity Kinds
```
Line, Circle, Arc, Polyline, Text, DimAligned, DimLinear, DimAngular, DimRadial, Insert
```

## Gotchas
- `compute_trim/extend` must be `&self` (returns result) — apply mutations after, using field splitting, to avoid `&mut self` + `&self.viewport` conflict
- Snap highlight fires even when `snap_enabled=false`; click handlers must call `pick_entity_point` unconditionally
- wgpu clear color is **linear** RGB; egui colours are **sRGB**. Use `viewport.bg_srgb()` for mask colour
- `CANVAS_BG_SRGB` constant in render-wgpu is the default; runtime colour lives in `viewport.clear_color`
- ACI colour picker already exists — see `layer_color_picking` and `entity_color_picker_open` dialogs for the pattern
- `execute_command_alias` calls `exit_dim()` for any unrecognised input — guard with `|| self.from_phase != FromPhase::Idle` to preserve dim context during FROM offset entry

## Next Tasks (priority order)
1. **Dynamic/parametric blocks** — parameter schema + per-insert overrides + evaluator
2. **Gap healing for boundary** — close near-miss loops under tolerance for boundary/hatch
3. **Polyline interior-join improvements** (PEDIT/JOIN currently endpoint-driven)
4. **Array persistence polish** — preserve associative arrays robustly across save/load/edit cycles
5. **Linetype roadmap follow-up** — linetype table + custom patterns + DXF LT table mapping

## Adding a New Command — Checklist
1. Add Phase enum variant(s) to `state.rs`
2. Add field(s) to `CadKitApp` struct + `Default` impl in `app.rs`
3. Add alias to `execute_command_alias()` in `commands.rs`
4. Add `exit_*()` method in `app.rs`; call it from `cancel_active_tool()`
5. Add click / Enter / ESC handlers in `app.rs` update()
6. Add toolbar button in `ui_panels.rs`
7. Add new `EntityKind` arm to every match in `app.rs`, `overlays.rs`, `render-wgpu/src/lib.rs`, `2d-core/src/lib.rs`
8. Update HELP window table in `app.rs`

## Roadmap Summary
Phase 2 (done): Dimensions/text/editing/layers/UI polish + linetype ByLayer/override
Phase 3: DXF completeness + additional formats
Phase 4: Python API + AI/MCP command line
Phase 5: Hatch, Blocks
Phase 6: 3D push/pull
Phase 7: CNC/CAM G-code
Phase 8: Cabinet designer (target market)

## Recently Completed
- BOUNDARY command/region work
  - planar half-edge region detection in geometry crate
  - robust segment noding/intersection splitting for mixed line/polyline regions
  - shared-edge/compartment behavior fixes for interior picks
- RECTANGLE command (`REC` / `RECTANGLE`) with diagonal-corners mode and dimensions mode (`w,h` + direction), with live rubber-band preview
- ELLIPSE command (`EL` / `ELLIPSE`) with center → radius → height workflow, rubber-band preview, ortho support
- POLYGON command (`POL` / `POLYGON`) with center/radius rubber-band and ortho-aware radius pick
- CHAMFER command with d or d1,d2 input and 0-distance support for sharp corners
- FILLET command shipping with line/polyline segment picks
- FILLET polyline rebuild behavior
  - same open polyline corner fillet rebuilt as one open polyline
  - same closed polyline corner fillet rebuilt as one closed polyline with sampled arc
  - mixed open polyline + line endpoint fillet can produce joined polyline output
- `J` / `JOIN` selection-based join flow for touching open polyline + segments
- `PE` / `PEDIT` edit flow: select base open polyline, then join touching line/arc at ends
- ARRAY command (`AR` / `ARRAY`) with rectangular + polar modes
- Rectangular ARRAY grip editing
  - all grips visible (`dx`, `dy`, `cols`, `rows`) with live entity ghost preview
  - click-to-activate, click-to-set/release grip workflow
  - typed exact values on active grips (spacing/count), then auto-release
  - count grips allow 1 row/column
- Associative rectangular arrays in-session
  - selecting any member selects the full array group
  - running `ARRAY` on a member re-enters array edit with stored spacing/count/base/direction
  - `E` during array grip edit explodes associative linkage and exits edit
- Linetype first pass (CAD-visible dash styles)
  - built-in linetypes: `Continuous`, `Hidden`, `Center`
  - global `LTSCALE` (`LTS`) multiplier
  - layer style now includes linetype + LT scale
  - entities support `ByLayer` linetype and per-entity override
  - entities support `ByLayer` LT scale and per-entity numeric override
- HATCH command + dialog expansion
  - pick-point hatch creation using region/boundary detection
  - built-in patterns: `ANSI31`, `ANSI32`, `ANSI37`, `Cross`, `Grid`
  - hatch controls: spacing, angle, LTScale, color inherit/override
  - ACI 1-255 color picker window for hatch color override
  - island detection with explicit `Detect Islands` toggle
  - circle/arc boundaries participate in island detection
- Blocks/references first pass
  - true `Insert` entity support in core model + render + properties
  - `INSERT` places persistent references (not auto-exploded)
  - insert explode path via `X/EXPLODE`
  - insert snap/selection/highlight through transformed block geometry
  - `TRIM`/`EXTEND` use insert geometry as cutting/boundary references
  - `TRIM`/`EXTEND` do not directly edit inserts (`explode` or `BEDIT` required)
- Block edit workflow first pass
  - `BEDIT` opens isolated block editing context
  - `BSAVE` commits definition updates
  - `BCANCEL` discards edits/restores drawing
