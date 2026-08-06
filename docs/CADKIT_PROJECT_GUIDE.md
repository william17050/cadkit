# CadKit Project Guide

## Purpose
CadKit is a modular CAD application aimed at a practical 2D drafting MVP first, with room for later 3D, CAM, scripting, and AI workflows.

## Current Product Shape
- Desktop Rust application.
- `egui` provides the application shell and overlays.
- `wgpu` renders the drawing viewport.
- The app uses a CAD-style command line plus toolbar and property panels.
- The codebase already includes drawing, modify, layer, dimension, text, block, hatch, DXF, Python, and initial AI plumbing.

## Repository Layout
- `crates/types/`: shared primitives such as `Vec2`, `Vec3`, `Guid`, units, tolerances.
- `crates/2d-core/`: drawing model, entities, layers, blocks, persistence, DXF import/export.
- `crates/geometry/`: geometry primitives, intersections, region detection.
- `crates/render-wgpu/`: viewport rendering and transform logic.
- `crates/ui-egui/`: application shell, command parsing, interaction phases, overlays, panels, dialogs.
- `crates/scripting-python/`: embedded Python runtime and CAD API exposure.
- `scripts/`: repo helper scripts.
- `qa/`: handoffs, reports, test cases, fixtures, evidence, compatibility notes.
- `docs/`: project-facing operational documentation.

## Runtime Entry Points
- App entrypoint: `crates/ui-egui/src/main.rs`
- Main application type: `crates/ui-egui/src/app.rs`
- Command aliases: `crates/ui-egui/src/app/commands.rs`
- Interaction state enums: `crates/ui-egui/src/app/state.rs`
- File dialogs and import/export: `crates/ui-egui/src/app/io.rs`

## Working Model
CadKit behavior is currently centered in `CadKitApp`, which coordinates:
- document state from `cadkit-2d-core`
- viewport rendering from `cadkit-render-wgpu`
- command aliases and mode transitions
- selection, snaps, typed input, and preview behavior
- UI panels, dialogs, overlays, and file interactions

This means many user-visible features cross crate boundaries, but most interaction wiring lands in `crates/ui-egui`.

## Build And Test
- Build: `cargo build --workspace`
- Check: `cargo check --workspace`
- Run tests: `cargo test --workspace`
- Run app: `cargo run -p cadkit`
- Run with Python support helper: `./run.sh --py`

## Current Documentation Reliability
- `README.md` is the best high-level product summary in the repo today.
- `ROADMAP.md` is useful for milestone status and recent capability additions.
- `BUILD_INSTRUCTIONS.md` is historically useful but no longer reflects the current implementation state in several sections.

## QA Operating Principle
For behavioral QA, use the same path a human would use:
- start the built app
- use menu actions, toolbar buttons, command aliases, keyboard input, and pointer interactions
- observe rendered feedback, prompts, and resulting document state

Automation and static inspection help explain the system, but they do not replace playtesting.
