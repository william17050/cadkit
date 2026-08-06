# Existing Feature Inventory

Status values used here:
- `Present`
- `Partial`
- `Unconfirmed`
- `Not found`

This inventory is based on direct repository inspection on 2026-08-06. It avoids guessing. Where practical, each item cites a source file, command alias, or current doc.

## Startup And Document Handling

| Item | Status | Evidence |
|---|---|---|
| Native desktop app entrypoint | Present | `crates/ui-egui/src/main.rs` launches `CadKitApp` with `eframe` and `wgpu` renderer. |
| Current file tracking and recent files | Present | `current_file` and `recent_files` fields in `crates/ui-egui/src/app.rs`; preference persistence in `AppPrefs`; README current feature set. |
| JSON save/load | Present | README current feature set; file I/O module `crates/ui-egui/src/app/io.rs`; drawing model persistence in `crates/2d-core/src/lib.rs`. |
| DXF import/export dialogs | Present | `crates/ui-egui/src/app/io.rs:216`, `crates/ui-egui/src/app/io.rs:329`; DXF implementation in `crates/2d-core/src/dxf_io.rs`. |
| SVG export | Present | README current feature set; file I/O support in `crates/ui-egui/src/app/io.rs`. |
| PDF export | Present | README current feature set; file I/O support in `crates/ui-egui/src/app/io.rs`. |
| Auto-save recovery snapshots | Present | README current feature set; `autosave_last_at` and `recovery_prompt_open` in `crates/ui-egui/src/app.rs`. |
| Multi-document tabs | Not found | No evidence in inspected UI entrypoints, app state, or README. |

## Viewport

| Item | Status | Evidence |
|---|---|---|
| `wgpu` viewport renderer | Present | `crates/render-wgpu/src/lib.rs`; `crates/ui-egui/src/main.rs`. |
| Pan and zoom camera state | Present | `Viewport` stores `zoom`, `pan_x`, `pan_y` in `crates/render-wgpu/src/lib.rs`. |
| Dot grid | Present | README rendering/UI section; `grid_visible` and `grid_spacing` state in `crates/ui-egui/src/app.rs`. |
| Canvas background color control | Present | `bgcolor_picker_open` in `crates/ui-egui/src/app.rs`; background color constants in `crates/render-wgpu/src/lib.rs`. |
| Alternate visual styles or shaded 3D modes | Not found | No evidence in current renderer or roadmap phase marked complete. |

## Selection

| Item | Status | Evidence |
|---|---|---|
| Click selection | Present | README current feature set; `selected_entities` and selection state in `crates/ui-egui/src/app.rs`. |
| Box selection with window/crossing | Present | README rendering/UI section; `selection_drag_start` and `selection_drag_current` in `crates/ui-egui/src/app.rs`. |
| Multi-select | Present | `ROADMAP.md` marks multi-select toggle complete in Phase 1.4. |
| Array group selection behavior | Present | `ROADMAP.md` notes associative array group selection behavior complete. |
| Subentity selection inside inserts without explode/edit | Partial | README states block insert geometry is selectable/snappable, but trim/extend of insert internals still requires explode or block edit. |

## Drawing Commands

| Item | Status | Evidence |
|---|---|---|
| LINE | Present | `crates/ui-egui/src/app/commands.rs:1108`; `ActiveTool::Line` in `state.rs`; README current feature set. |
| ARC (3-point) | Present | README current feature set; `ActiveTool::Arc` in `state.rs`; `create_arc_from_three_points` flow in `app.rs`. |
| CIRCLE | Present | `ActiveTool::Circle` in `state.rs`; README current feature set. |
| POLYLINE | Present | `crates/ui-egui/src/app/commands.rs:1139`; `ActiveTool::Polyline` in `state.rs`; README current feature set. |
| TEXT | Present | `crates/ui-egui/src/app/commands.rs:1725`; `EntityKind::Text` in `crates/2d-core/src/lib.rs`; roadmap marks complete. |
| DIMLINEAR | Present | `crates/ui-egui/src/app/commands.rs:1805`; `EntityKind::DimLinear` in `crates/2d-core/src/lib.rs`. |
| DIMANGULAR | Present | `crates/ui-egui/src/app/commands.rs:1820`; `EntityKind::DimAngular` in `crates/2d-core/src/lib.rs`. |
| DIMRADIUS and DIMDIAMETER | Present | `crates/ui-egui/src/app/commands.rs:1835`; `EntityKind::DimRadial` in `crates/2d-core/src/lib.rs`. |
| POLYGON | Present | `PolygonPhase` in `state.rs`; roadmap marks complete. |
| ELLIPSE | Present | `EllipsePhase` in `state.rs`; roadmap marks complete. |
| RECTANGLE | Present | `RectanglePhase` in `state.rs`; roadmap marks complete. |
| BOUNDARY | Present | `crates/ui-egui/src/app/commands.rs:1441`; geometry region detection in `crates/geometry/src/region.rs`. |
| HATCH | Present | `crates/ui-egui/src/app/commands.rs:1464`; `HatchPhase` in `state.rs`; roadmap marks first pass complete. |
| ISOCIRCLE / isometric drafting helpers | Present | `IsocirclePhase`, `IsoExtrudePhase`, `DwIsoSidePhase` in `state.rs`; app fields in `app.rs`. |

## Modify Commands

| Item | Status | Evidence |
|---|---|---|
| MOVE | Present | `crates/ui-egui/src/app/commands.rs:1289`; `MovePhase` in `state.rs`. |
| COPY | Present | `crates/ui-egui/src/app/commands.rs:1599`; `CopyPhase` in `state.rs`. |
| ROTATE | Present | `crates/ui-egui/src/app/commands.rs:1302`; `RotatePhase` in `state.rs`. |
| SCALE | Present | `ScalePhase` in `state.rs`; roadmap marks complete. |
| MIRROR | Present | `MirrorPhase` in `state.rs`; roadmap marks complete. |
| OFFSET | Present | `crates/ui-egui/src/app/commands.rs:1629`; `OffsetPhase` in `state.rs`. |
| TRIM | Present | `crates/ui-egui/src/app/commands.rs:1268`; `TrimPhase` in `state.rs`. |
| EXTEND | Present | `crates/ui-egui/src/app/commands.rs:1277`; `ExtendPhase` in `state.rs`. |
| FILLET | Present | `FilletPhase` in `state.rs`; roadmap marks complete. |
| CHAMFER | Present | `ChamferPhase` in `state.rs`; roadmap marks complete. |
| ARRAY | Present | `ArrayPhase` and `ArrayMode` in `state.rs`; roadmap marks rectangular and polar support complete. |
| PEDIT / JOIN | Present | `PeditPhase` in `state.rs`; roadmap marks complete. |
| STRETCH | Present | `StretchPhase` in `state.rs`; app state includes stretch fields. |
| EXPLODE | Present | README block commands section includes `X` / `EXPLODE`. |

## Object Snaps

| Item | Status | Evidence |
|---|---|---|
| Endpoint snap | Present | `snap_endpoint` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Midpoint snap | Present | `snap_midpoint` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Center snap | Present | `snap_center` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Quadrant snap | Present | `snap_quadrant` in `crates/ui-egui/src/app.rs`; default prefs include it. |
| Intersection snap | Present | `snap_intersection` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Parallel tracking | Present | `snap_parallel` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Perpendicular snap | Present | `snap_perpendicular` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Tangent snap | Present | `snap_tangent` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Nearest snap | Present | `snap_nearest` in `crates/ui-egui/src/app.rs`; roadmap marks complete. |
| Visual snap glyphs | Present | roadmap marks visual snap glyphs complete; overlay module exists at `crates/ui-egui/src/app/overlays.rs`. |

## Ortho And Typed Input

| Item | Status | Evidence |
|---|---|---|
| Ortho mode | Present | `ortho_enabled` in `crates/ui-egui/src/app.rs`; README current feature set. |
| Relative coordinates `@x,y` | Present | README current feature set. |
| Polar input `@dist<angle` | Present | README current feature set. |
| Direct distance entry | Present | README current feature set; `distance_input` in `crates/ui-egui/src/app.rs`. |
| FROM offset workflow | Present | README current feature set; `FromPhase` used from `commands.rs` and app state. |
| Custom ortho increment beyond 90 degrees | Partial | `ortho_increment_deg` exists in `crates/ui-egui/src/app.rs`, but direct user-facing control was not confirmed from inspected command docs. |

## Layers

| Item | Status | Evidence |
|---|---|---|
| Create, rename, set current layer | Present | `crates/ui-egui/src/app/commands.rs:1645`; layer state fields in `crates/ui-egui/src/app.rs`; README layers section. |
| Layer color edit | Present | README layers section; layer color picking fields in `crates/ui-egui/src/app.rs`. |
| Layer visibility toggle | Present | README layers section; roadmap marks visibility complete. |
| Layer locking | Present | roadmap marks locking enforcement complete. |
| Layer freeze | Present | roadmap marks freeze complete. |
| Layer linetype and scale | Present | README linetype controls section; `crates/2d-core/src/lib.rs` has layer linetype defaults. |

## Undo/Redo

| Item | Status | Evidence |
|---|---|---|
| Undo | Present | `crates/ui-egui/src/app/commands.rs:1717`; `undo_stack` in `crates/ui-egui/src/app.rs`. |
| Redo | Present | `crates/ui-egui/src/app/commands.rs:1721`; `redo_stack` in `crates/ui-egui/src/app.rs`. |
| Behavioral coverage across all command families | Unconfirmed | State exists, but broad playtest evidence is not yet in the repo. |

## Blocks And References

| Item | Status | Evidence |
|---|---|---|
| Block definition | Present | README block commands; `BlockPhase` in `state.rs`. |
| Insert block reference | Present | `crates/ui-egui/src/app/commands.rs:297`; `EntityKind::Insert` in `crates/2d-core/src/lib.rs`. |
| Explode block reference | Present | README block commands. |
| Block editing workflow | Present | README block commands; block edit fields in `crates/ui-egui/src/app.rs`. |
| Dynamic blocks first pass | Partial | README marks developer first pass only; data model is present in `crates/2d-core/src/lib.rs`; workflow is not complete product UX. |
| Nested blocks | Not found | `ROADMAP.md` leaves this unchecked. |

## DXF Import/Export

| Item | Status | Evidence |
|---|---|---|
| Line, arc, circle, polyline DXF support | Present | `crates/2d-core/src/dxf_io.rs`. |
| Text and MTEXT DXF support | Present | `crates/2d-core/src/dxf_io.rs`. |
| DIMENSION DXF import/export | Present | `crates/2d-core/src/dxf_io.rs`; roadmap marks complete. |
| INSERT imported as true editable block references | Partial | `ROADMAP.md` states block import as flattened geometry. |
| Unsupported-entity warnings | Present | README current feature set and `DxfImportResult` in `crates/2d-core/src/dxf_io.rs`. |

## Dialogs, Panels, And UI

| Item | Status | Evidence |
|---|---|---|
| Left tool palette | Present | README rendering/UI section; `crates/ui-egui/src/app/ui_panels.rs`. |
| Top menu bar | Present | README rendering/UI section; `crates/ui-egui/src/app/ui_panels.rs`. |
| Right properties and layers panel | Present | README rendering/UI section; `crates/ui-egui/src/app/ui_panels.rs`. |
| Command log / command line | Present | README rendering/UI section; `command_log` and `command_input` in `crates/ui-egui/src/app.rs`. |
| Status bar with cursor and mode indicators | Present | README near-term/current features and roadmap marks complete. |
| Python console window | Present | README build section and `python_console_open` state in `crates/ui-egui/src/app.rs`. |
| AI command window | Present | README build section and AI state fields in `crates/ui-egui/src/app.rs`. |
| Full in-repo behavioral QA reports | Not found | No existing versioned QA reports were present before this setup task. |
