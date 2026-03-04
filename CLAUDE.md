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
Line, Circle, Arc, Polyline, Text, DimLinear (actually Aligned — rename pending)
```

## Gotchas
- `compute_trim/extend` must be `&self` (returns result) — apply mutations after, using field splitting, to avoid `&mut self` + `&self.viewport` conflict
- Snap highlight fires even when `snap_enabled=false`; click handlers must call `pick_entity_point` unconditionally
- wgpu clear color is **linear** RGB; egui colours are **sRGB**. Use `viewport.bg_srgb()` for mask colour
- `CANVAS_BG_SRGB` constant in render-wgpu is the default; runtime colour lives in `viewport.clear_color`
- ACI colour picker already exists — see `layer_color_picking` and `entity_color_picker_open` dialogs for the pattern

## Next Tasks (priority order)
1. **Rename `DimLinear` → `DimAligned`** everywhere (EntityKind, DXF, match arms, UI) — mechanical
2. **Add true `DimLinear`** — horizontal or vertical only; axis locked by drag direction during Placing phase
3. **DXF dimension export** — currently skipped with warning; use `dxf_rs` AlignedDimension / RotatedDimension
4. **DXF TEXT export** — Text entity not yet exported to DXF
5. **Layer lock enforcement** — `Layer.locked` field exists + UI toggle, but edits not gated on it
6. **Scale command** (`SC`) — like Move but applies uniform or XY scale factor
7. **Mirror command** (`MI`) — pick axis line, reflect selected entities
8. **DimStyle dialog** — text height, arrow size, extension line gap, colour, precision
9. **Preference persistence** — serde-serialize snap/ortho/grid flags to `~/.config/cadkit/prefs.json`

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
Phase 2 (now — mostly done): DimLinear rename, true DimLinear, Scale/Mirror, DXF dims/text, layer lock, DimStyle, prefs
Phase 3: DXF completeness, SVG/PDF
Phase 4: Python API + AI/MCP command line
Phase 5: Hatch, Blocks
Phase 6: 3D push/pull
Phase 7: CNC/CAM G-code
Phase 8: Cabinet designer (target market)
