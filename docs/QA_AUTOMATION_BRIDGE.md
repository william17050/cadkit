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
python scripts/cadkit_qa_bridge.py click-world qa/runtime/CK-0001 0 0
python scripts/cadkit_qa_bridge.py click-world qa/runtime/CK-0001 20 0
python scripts/cadkit_qa_bridge.py escape qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py drag-select-world qa/runtime/CK-0001 -5 -5 25 5
python scripts/cadkit_qa_bridge.py delete qa/runtime/CK-0001
python scripts/cadkit_qa_bridge.py undo qa/runtime/CK-0001
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
- snap/ortho/grid flags
- viewport size, zoom, and pan

## Current Scope
The first bridge pass is aimed at the `CK-0001` baseline scenarios:
- launch handling
- recovery-prompt handling
- viewport zoom and pan
- command-line command execution
- line creation via point delivery
- click selection
- window/crossing selection
- delete, undo, redo
- clean close request

It does not yet automate every interaction mode in CadKit. Unsupported modes return explicit errors in the result JSON rather than silently mutating state.
