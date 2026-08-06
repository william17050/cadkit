# Baseline Test Roadmap

This roadmap is intentionally practical and shallow for the first QA loop. It prioritizes core user stability and drafting behavior before broad feature expansion.

## 1. Launch And Basic Stability
- Verify the app launches from a fresh build.
- Confirm the main window, viewport, command input, tool palette, and right-side panels appear.
- Confirm no immediate crash when idle for a short period.
- Confirm basic close and reopen behavior.

## 2. Viewport Pan And Zoom
- Verify pointer-driven zoom works repeatedly without artifacts.
- Verify pan works and does not corrupt overlays or selection visuals.
- Verify the grid remains coherent while moving the camera.

## 3. Selection And Deselection
- Verify single-click selection.
- Verify empty-space deselection.
- Verify selection box behavior for both window and crossing cases.
- Verify visible highlight feedback is understandable.

## 4. LINE And Polyline Interaction
- Draw simple lines by clicks.
- Draw a polyline with multiple segments.
- Confirm preview rubber-band feedback while placing points.
- Confirm resulting entities remain selectable after creation.

## 5. Escape, Enter, And Command Cancellation
- Start commands and cancel with `Esc`.
- Confirm repeated `Esc` does not leave the app in a broken state.
- Confirm `Enter` advances or completes the command where expected.
- Confirm command switching cancels incompatible prior modes cleanly.

## 6. Ortho And Typed Input
- Toggle Ortho and verify constrained line placement.
- Test direct distance entry during line creation.
- Test absolute, relative, and polar input on a simple geometry case.
- Confirm visible prompts and resulting geometry match the typed input.

## 7. Object Snaps
- Verify endpoint, midpoint, center, and intersection on simple geometry.
- Verify snap feedback is visible and the delivered point matches the indicated snap.
- Verify Ortho plus snap interaction on at least one line workflow.

## 8. MOVE, COPY, And ROTATE
- Move a selected entity with a base point and destination.
- Copy an entity and confirm the original remains.
- Rotate an entity and confirm preview and final state are reasonable.
- Confirm locked layers are not accidentally modified if included in a test drawing later.

## 9. Undo And Redo
- Undo and redo after line creation.
- Undo and redo after a modify command.
- Confirm the visible drawing state and selection state remain coherent.

## 10. OFFSET, TRIM, And EXTEND
- Offset a simple line or polyline.
- Trim against obvious cutting geometry.
- Extend toward obvious boundary geometry.
- Confirm previews, prompts, and outcomes are understandable.

## 11. Layers
- Create a layer, set it current, and draw onto it.
- Change color or visibility and confirm viewport feedback updates.
- Verify selection/editing expectations on hidden or locked layers in a limited smoke pass.

## 12. DXF Round-Trip
- Import a small known DXF fixture.
- Verify geometry appears and remains selectable.
- Export the drawing back to DXF.
- Reimport the exported DXF and compare for obvious geometry loss or annotation regressions.

## Initial Execution Notes
- Start with tiny, human-readable fixtures.
- Capture evidence when behavior is surprising, not only when it fails.
- Keep the first loop focused on reproducible core interactions, not exhaustive edge cases.

## Suggested First Detailed Test Cases To Author
- Launch to idle viewport
- Draw single line and cancel next segment
- Polyline with three points and finish
- Endpoint snap during line continuation
- Move then undo/redo
- Offset simple rectangle edge
- Trim crossing lines
- Layer visibility toggle smoke test
- DXF import/export smoke round-trip
