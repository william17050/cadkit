# CadKit Current State

## Current Build
- Current commit hash: `ec1e40a`
- Branch: `refactor/main-split`
- Worktree: Dirty

## Latest Completed Work Item
- Work item ID: `CK-0005`
- Title: `Split Ortho and Polar directional constraints`
- QA status: `PASS`
- Commit: `ec1e40a`
- Current handoff/report: `qa/handoffs/CK-0005_H001.md`, `qa/reports/CK-0005_R001.md`

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
- Polar Bridge Control: COMPLETE
- Polar Angle Bridge Control: COMPLETE
- Wayland Behavioral Testing: COMPLETE via in-app bridge
- Preference Snapshot/Restore: PARTIAL

## Major Systems
- Viewport: Stable
- Selection: Stable
- Snapping: Stable
- Directional Constraints: Stable
- Drawing: Stable
- Modify: Partial
- Undo/Redo: Stable
- Layers: Partial
- DXF: Partial
- Python: Partial

## Next Recommended Work
- Add session-scoped snapshot/restore for preference-backed QA settings such as snap flags and other persisted toggles so bridge-driven tests cannot leak state across cycles.

## Last Updated
- 2026-08-06 22:34:00Z
