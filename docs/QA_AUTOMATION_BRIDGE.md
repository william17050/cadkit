# QA Automation Bridge

## Purpose
CadKit now includes an opt-in in-app QA automation bridge so behavioral QA can drive core workflows without relying on OS-level input injection.

This is intended for QA and testing only. Normal users should not enable it.

## Enable The Bridge
Launch CadKit with `CADKIT_QA_DIR` set to a writable directory:

```bash
export CADKIT_QA_DIR="$PWD/qa/runtime/CK-0001"
./target/debug/cadkit
```

When enabled, CadKit creates and uses:
- `commands/`
- `processed/`
- `results/`
- `state.json`

under the configured root directory.

## Command Model
External tools enqueue JSON command files into `commands/`. CadKit processes them in filename order, writes a matching result JSON into `results/`, archives the command into `processed/`, and continuously refreshes `state.json`.

The bridge is intentionally explicit and file-based so it works in constrained Linux and Windows environments without sockets or symlinks.

## Supported Commands
- `run_command`
- `ortho`
- `polar`
- `polar_angle`
- `prefs_snapshot`
- `prefs_restore`
- `hover_world_snapped`
- `click_world_snapped`
- `click_world`
- `drag_select_world`
- `escape`
- `delete`
- `undo`
- `redo`
- `zoom`
- `pan_screen`
- `recovery`
- `quit`
- `status` is represented by reading `state.json`

## Helper Script
Use [scripts/cadkit_qa_bridge.py](/home/bazzite/Documents/cadkit_with_wgpu/scripts/cadkit_qa_bridge.py:1) to enqueue commands and wait for results.

Examples:

```bash
python scripts/cadkit_qa_bridge.py status qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py recovery qa/runtime/CK-0001 discard
python scripts/cadkit_qa_bridge.py run-command qa/runtime/CK-0001 line
python scripts/cadkit_qa_bridge.py ortho qa/runtime/CK-0001 on
python scripts/cadkit_qa_bridge.py polar qa/runtime/CK-0001 off
python scripts/cadkit_qa_bridge.py polar-angle qa/runtime/CK-0001 45
python scripts/cadkit_qa_bridge.py prefs-snapshot qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py hover-world-snapped qa/runtime/CK-0001 19.2 0.3
python scripts/cadkit_qa_bridge.py click-world-snapped qa/runtime/CK-0001 19.2 0.3
python scripts/cadkit_qa_bridge.py click-world qa/runtime/CK-0001 0 0
python scripts/cadkit_qa_bridge.py click-world qa/runtime/CK-0001 20 0
python scripts/cadkit_qa_bridge.py escape qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py drag-select-world qa/runtime/CK-0001 -5 -5 25 5
python scripts/cadkit_qa_bridge.py delete qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py undo qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py prefs-restore qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py quit qa/runtime/CK-0001
```

## State Snapshot
`state.json` is the current machine-readable QA snapshot. It includes:
- recovery-prompt state
- active tool / point mode
- selected entity IDs
- entity count and entity kinds
- command log tail
- current layer
- snap/ortho/polar/grid flags and current polar angle
- whether a QA prefs snapshot is currently active
- resolved hover world and snap kind
- viewport size, zoom, and pan

## Current Scope
The bridge covers the `CK-0001` baseline scenarios plus snap-aware follow-up testing:
- launch handling
- recovery-prompt handling
- viewport zoom and pan
- ortho state control
- polar state control
- polar angle control
- preference snapshot capture/restore
- command-line command execution
- line creation via point delivery
- snap-aware hover and click resolution
- click selection
- window/crossing selection
- delete, undo, redo
- clean close request

## Snap-Aware Mode
`hover_world_snapped` and `click_world_snapped` take an approximate world-space target, project it into the viewport, and then run CadKit's internal snap-resolution pipeline before reporting or delivering the resolved point.

This is intended for validating snap correctness:
- endpoint
- midpoint
- center
- quadrant
- intersection
- parallel/perpendicular tracking
- perpendicular
- tangent
- nearest
- polar-assisted line-like snapping

## Current Limitations
- Raw `click_world` still bypasses snapping by design and is useful for exact-coordinate regression checks.
- Snap-aware actions validate CadKit's snap logic, but not literal desktop hover ergonomics such as cursor icons or OS cursor motion.
- It does not yet automate every interaction mode in CadKit. Unsupported modes return explicit errors in the result JSON rather than silently mutating state.

## QA Session Preference Safety
`prefs_snapshot` captures the current persisted QA-relevant preference state inside
the active QA runtime root. `prefs_restore` reapplies it and deletes the snapshot.
If a snapshot is still active when the bridge `quit` command is used, CadKit restores
it automatically before closing.
