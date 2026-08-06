# AutoCAD Behavior Spec

## Intent
CadKit is not required to match AutoCAD implementation details exactly, but it should match AutoCAD-like behavioral expectations where that is the product goal and where the current feature set claims parity.

This document is the behavioral reference for testing and implementation discussions until a more granular command-by-command spec exists.

## Core Principle
When CadKit offers an AutoCAD-style command, alias, or workflow, the default expectation is:
- command naming and prompt sequencing should feel familiar
- escape and cancel behavior should be predictable
- point selection, typed input, snaps, and preview feedback should support drafting precision
- resulting geometry and selection behavior should be stable and unsurprising

## Priority Behaviors

### Command Lifecycle
- Commands should accept common AutoCAD-style aliases where implemented.
- `Esc` should cancel the active command or interaction mode cleanly.
- `Enter` should advance or confirm a command step where applicable.
- Right-click cancel behavior should match the app's current interaction model consistently.
- Starting an unrelated command should exit incompatible prior modes cleanly.

### Point Entry
- A point may come from direct clicking, typed absolute coordinates, typed relative coordinates, or typed polar coordinates where implemented.
- FROM-style offsets should preserve context and feed the active command rather than silently discarding it.
- Typed distance entry should produce the same result as the corresponding pointer-driven action when both are valid inputs.

### Object Snaps And Guidance
- Snap feedback should indicate what point type is being acquired.
- Snaps should influence the actual delivered point, not only the hover preview.
- Ortho constraints should combine predictably with snaps and typed input.
- Parallel, perpendicular, tangent, nearest, midpoint, endpoint, center, and intersection behavior should be judged by drafting usefulness, not just by code-path activation.

### Selection And Editing
- Selection should be stable and visually obvious.
- Window and crossing selection should behave consistently.
- Modify commands should preserve entity identity and layer intent where appropriate.
- Undo and redo should restore user-visible drawing state reliably across command flows.

### File And Document Behavior
- Open, save, import, export, and recovery prompts should be understandable and resistant to silent data loss.
- DXF round-trips should preserve expected geometry and supported annotation entities within the current feature scope.

## Behavioral Source Hierarchy
When evaluating behavior, use this order:
1. Explicit product owner direction from Bill
2. The newest applicable QA handoff and QA report for the work item
3. Current in-app prompts and existing user-facing behavior
4. This spec
5. AutoCAD reference behavior as a comparison baseline, not as an excuse to guess

## Non-Goals For This Spec
- It does not define geometric kernel implementation details.
- It does not promise parity for unimplemented AutoCAD features.
- It does not override explicit product decisions from Bill.

## Expected Testing Notes
Every behavioral report should call out:
- where CadKit matched expected CAD behavior
- where it felt awkward even if technically functional
- where it differed from AutoCAD in a way that may matter to users
