# CadKit Current State

## Current Build
- Current commit hash: `9a3c212`
- Branch: `refactor/main-split`
- Worktree: Dirty

## Latest Completed Work Item
- Work item ID: `CK-0002`
- Title: `Snap radius tightening and ortho snap preservation`
- QA status: `PASS`
- Commit: `fba5c7a`
- Current handoff/report: `qa/handoffs/CK-0002_H002.md`, `qa/reports/CK-0002_R002.md`

## Active Work Item
- `CK-0003` - Extended snap-mode behavioral verification

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
- Execute `CK-0003_H002` to confirm the new bridge Ortho control removes the prefs workaround, then resolve the perpendicular precedence question.

## Last Updated
- 2026-08-06 20:20:00Z
