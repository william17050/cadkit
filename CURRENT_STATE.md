# CadKit Current State

## Current Build
- Current commit hash: `c46e9c2`
- Branch: `refactor/main-split`
- Worktree: Dirty

## Latest Completed Work Item
- Work item ID: `CK-0005`
- Title: `Split Ortho and Polar directional constraints`
- QA status: `PASS`
- Commit: `pending commit for current worktree`
- Current handoff/report: `qa/handoffs/CK-0005_H001.md`, `qa/reports/CK-0005_R001.md`

## Active Work Item
- None

## Open Engineering Issues
- `AUTOMATION-001` - OS-level input injection on Wayland remains unavailable; the in-app QA bridge is the current workaround.
- `TOOLING-001` - `ortho_increment_deg` still has no QA bridge action or command alias, so some polar-angle verification still depends on backed-up prefs edits.

## Open Product Decisions
- `UX-001` - Grid visibility/default versus persisted-state intent needs Bill's judgment.
- `UX-002` - Close-with-unsaved-changes prompt behavior needs Bill's judgment.

## QA Automation Status
- QA Bridge: COMPLETE
- Snap Automation: COMPLETE
- Ortho Bridge Control: COMPLETE
- Polar Bridge Control: COMPLETE
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
- Add bridge/session support for preference-backed QA settings, starting with `ortho_increment_deg`, so bridge-driven tests no longer require manual prefs edits.

## Last Updated
- 2026-08-06 22:20:00Z
