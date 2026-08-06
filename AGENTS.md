# CadKit Agent Guide

## Purpose
CadKit is a Rust-based desktop CAD application focused on practical 2D drafting today, with planned expansion into 3D, CAM, cabinet workflows, Python scripting, and AI-assisted commands.

## High-Level Architecture
- `crates/types`: shared math, identifiers, units, tolerances.
- `crates/2d-core`: drawing document model, entities, layers, blocks, DXF I/O.
- `crates/geometry`: intersection and region/boundary detection logic.
- `crates/render-wgpu`: viewport rendering, camera transforms, linetype sampling.
- `crates/ui-egui`: main application shell, command handling, interaction state, panels, overlays, file dialogs.
- `crates/scripting-python`: embedded Python integration.

Primary runtime entrypoint:
- `crates/ui-egui/src/main.rs`

Primary integration surface:
- `crates/ui-egui/src/app.rs`

## Build And Run
- Build workspace: `cargo build --workspace`
- Check workspace: `cargo check --workspace`
- Run app: `cargo run -p cadkit`
- Run app with Python helper launcher: `./run.sh --py`

## Tests And Validation
- Run tests: `cargo test --workspace`
- Format: `cargo fmt --all`
- Lint: `cargo clippy --workspace`
- Validate QA helper scripts: `python -m py_compile scripts/latest_handoff.py scripts/latest_report.py`

## Coding Boundaries And Conventions
- Prefer focused changes. Do not refactor broadly during feature or QA support work unless explicitly requested.
- Preserve current behavior unless the task requires a targeted fix.
- Keep source edits ASCII unless a file already requires another encoding.
- Use existing crate boundaries. Do not move responsibility between crates casually.
- `crates/ui-egui` is the command and interaction shell. `crates/2d-core` owns the drawing model.
- Treat `BUILD_INSTRUCTIONS.md` as historical unless updated; use the current workspace state and README as the operational source of truth.

## Roles

### Codex Engineer
- Implements features and fixes.
- Builds CadKit and runs automated checks.
- Performs limited smoke testing.
- Writes versioned handoffs in `qa/handoffs/`.
- May summarize behavioral findings, but must not mark behavioral QA findings as passed.

### Claude Code Tester
- Acts as behavioral playtester and QA reviewer.
- Launches and operates CadKit through the same user-facing controls available to a human.
- Records observations, captures evidence, and writes versioned reports in `qa/reports/`.
- Must not edit application source, scripts, or build configuration as part of testing.

### Bill
- Defines or approves work items.
- Serves as product owner and AutoCAD behavior expert.
- Reviews handoffs and reports.
- Provides final acceptance authority.

## File Ownership Rules
- Application source under `crates/`: Codex may modify when implementing approved work.
- QA evidence under `qa/evidence/`: primarily produced by Claude Code during testing.
- QA handoffs under `qa/handoffs/`: created and versioned by Codex.
- QA reports under `qa/reports/`: created and versioned by Claude Code.
- Shared process docs under `docs/` and templates under `qa/test_cases/`: Codex may maintain.
- Claude Code must not edit application source.

## Workflow Rules
- Begin each new work item by reading:
  - `AGENTS.md`
  - `CURRENT_STATE.md`
- Read only the newest applicable handoff or report for a work item unless historical investigation is required.
- Use the helper scripts in `scripts/` to resolve the newest handoff or report before reading older artifacts manually.
- Act through the human path; verify through the machine path.
- Behavioral validation must use normal user-facing commands, dialogs, keyboard, and pointer interactions.
- Codex may verify builds, tests, and deterministic automation, but that does not close behavioral QA findings.
- A behavioral issue is closed only after Claude Code retests it or Bill explicitly accepts it.

## QA Status Rules
- Codex must not mark behavioral QA findings as passed.
- Claude Code reports observed pass/fail outcomes for behavioral scenarios.
- Bill can accept risk explicitly, but acceptance should be recorded in the relevant handoff or report trail.

## Handoff And Report Discovery
- Current state dashboard: `CURRENT_STATE.md`
- Latest handoff: `python scripts/latest_handoff.py CK-0001`
- Latest report: `python scripts/latest_report.py CK-0001`

These scripts print only the resolved path on success and return a nonzero exit code on failure.
