# CadKit Current State

## Current Build
- Current commit hash: `8657467`
- Branch: `refactor/main-split`
- Worktree: Dirty

## Latest Completed Work Item
- Work item ID: `CK-0003`
- Title: `Extended snap-mode behavioral verification`
- QA status: `PASS`
- Commit: `8657467`
- Current handoff/report: `qa/handoffs/CK-0003_H003.md`, `qa/reports/CK-0003_R003.md`

## Active Work Item
- None

## Open Engineering Issues
- `AUTOMATION-001` - OS-level input injection on Wayland remains unavailable; the in-app QA bridge is the current workaround.

## Open Product Decisions
- `UX-001` - Grid visibility/default versus persisted-state intent needs Bill's judgment.
- `UX-002` - Close-with-unsaved-changes prompt behavior needs Bill's judgment.

## QA Automation Status
- QA Bridge: COMPLETE
- Snap Automation: COMPLETE
- Ortho Bridge Control: COMPLETE
- Wayland Behavioral Testing: COMPLETE via in-app bridge

## Major Systems
- Viewport: Stable
- Selection: Stable
- Snapping: Stable
- Drawing: Stable
- Modify: Partial
- Undo/Redo: Stable
- Layers: Partial
- DXF: Partial
- Python: Partial

## Next Recommended Work
- Add QA-session snapshot/restore for preference-backed settings such as `OSMODE` so bridge-driven testing never depends on manual prefs restoration.

## Last Updated
- 2026-08-06 20:44:03Z
