# CadKit Project Memory

## Project Overview
2D CAD application in Rust at `/var/home/bazzite/Documents/cadkit_with_wgpu`.
See CLAUDE.md at project root for full context, conventions, and next tasks.
GitHub: https://github.com/william17050/cadkit

## Cargo
`~/.cargo/bin/cargo` — not on PATH, always use full path.

## cadkit-geometry crate (`crates/geometry/`)
- `src/primitives.rs` — `Line`, `Arc`, `Circle`, `Polyline` structs
- `src/utils.rs` — `normalize_angle`, `ccw_span`, `angle_in_arc`, `dot2`, `cross2`, `rot90`
- `src/intersect/mod.rs` — `Intersection` enum + `Intersects<T>` trait

### Arc angle convention
`ccw_span(-PI, PI)` returns 0 (degenerate) because `normalize_angle(2π) = 0`.
Full-circle arcs must use slightly-less-than-2π span (e.g., `-PI+0.001` to `PI-0.001`).

### Arc storage invariant
All arcs CCW (`end_angle > start_angle`). `create_arc_from_three_points` normalises CW arcs.

### Reused internals
- `circle_circle_pts` — `pub(crate)` in `circle_circle.rs`, reused by `arc_circle.rs`, `arc_arc.rs`
- `filter_for_arc` — `pub(crate)` in `arc_circle.rs`, reused by `arc_arc.rs`

## Snap highlight vs click snap
`pick_entity_point` fires even when `snap_enabled=false`. Click handlers must call it
unconditionally, not guarded by `snap_enabled`, or highlight shows but click misses.

## Enter key (AutoCAD LT model)
- Enter = confirm hover position for all point-picking states
- ESC / right-click = cancel only
- Empty Enter in SelectingEntities → advance to next phase
- `deliver_point(world: Vec2)` is the central routing method

## FROM tracking
- `FromPhase` enum: Idle / WaitingBase / WaitingOffset
- Available whenever `is_picking_point()` returns true
- `exit_from()` cancels FROM without cancelling parent command
- In dim commands, `FR` now tries to reuse first point as FROM base automatically.
- Known issue: `FR -> type distance -> Enter` inside dim second-point flow is still unreliable on some runs (can drop/cancel).

## TRIM command
- `compute_trim` is `&self` (returns TrimResult) — mutations applied after via field splitting
- Pattern required to avoid `&mut self` + `&self.viewport` borrow conflict — apply to all compute_* methods

## Dimension rendering
- DimLinear text rendered by egui overlay (`draw_dim_entities`), NOT wgpu vertex buffer
- Background mask rectangle gaps the dimension line — colour via `viewport.bg_srgb()`
- `<>` in text_override is replaced with measured distance at render time
- Auto-creates "Dim" layer ([0,180,220]) on first dimension placement

## Colour space
- wgpu `clear_color` is **linear** RGB (stored as `[f32;3]` on Viewport)
- egui colours are **sRGB** bytes
- `viewport.bg_srgb()` converts: `v.powf(1/2.2) * 255`
- Default linear 0.08 → sRGB ~81 (grey, not near-black)

## Grid system
- `grid_visible: bool` (default true) — controls background dots AND green cursor dot
- `grid_spacing: f64` (default 12.0) — runtime-configurable via DragValue in status bar
- Grid snap (`snap_to_grid`) gated on BOTH `snap_enabled && grid_visible`
- Command alias: `GR` / `GRID` toggles grid_visible

## Status bar layout
Status bar is inside `draw_command_line` (bottom TopBottomPanel) — NOT in CentralPanel.
Panel render order: menu_bar → left_toolbar → command_line → right_panel

## Snap click consistency
All click handlers apply the full snap chain matching hover preview:
1. `pick_entity_point` (entity snap)
2. `snap_intersection_point` (intersection)
3. `hover_snap_kind` / `hover_world_pos` (perp/tangent/nearest)
4. `snap_to_grid` (grid fallback, only when snap_enabled && grid_visible)

## DimLinear (H/V locked dimension)
Fully implemented: `EntityKind::DimLinear { horizontal: bool, .. }` in 2d-core.
Commands: `DLI` / `DIMLINEAR`, toolbar "↔ Dim Linear".
Axis lock during Placing: drag more vertically → horizontal dim (measures X); drag horizontally → vertical dim (measures Y).
Rendering: wgpu GPU geometry + egui text overlay (same as DimAligned). Always horizontal text.
Full move/copy/rotate/properties/snap/selection support.

## Dimension grips (selected dims)
- Selected dimensions show draggable grips:
  - Start (green), End (red), Offset (orange), Text (cyan)
- Hover affordance:
  - Hand cursor, active hover highlight, and small tooltip label (`Start/End/Offset/Text`)
- Offset grip is axis-constrained:
  - Aligned dims: normal direction only
  - Linear dims: axis-locked (Y for horizontal dims, X for vertical dims)
- Endpoint grip drag supports osnaps (endpoint/mid/center/intersection) and excludes self-entity from snap candidates to avoid jitter.
- Grid fallback is disabled during grip drag snap path.
- Known limitation: typed FROM-style distance during dim second-point stage is still flaky.

## Phase 2 status
Remaining work:
- DXF DIMENSION export (skipped with warning in dxf_io.rs ~line 223)
- Scale (SC), Mirror (MI) commands
- DimStyle preset management (optional, future) / style expansion

## Text entity
Fully implemented: `Text` EntityKind in 2d-core, `TEXT` command (alias `T`), egui overlay rendering,
EDITTEXT (`ET`) for content edits, move/copy/rotate support.
Workflow: PlacingPosition → EnteringHeight → EnteringRotation → TypingContent.
