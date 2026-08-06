# CK-0001-T002 - Initial Viewport Behavior

## Metadata
- Test ID: `CK-0001-T002`
- Related work item: `CK-0001`
- Purpose: `Assess initial viewport rendering, pointer feedback, zoom, and pan behavior.`

## Preconditions
- CadKit is already launched successfully.

## Fixture
- `None`

## Human-Equivalent Actions
1. Observe the viewport immediately after launch.
2. Move the pointer through the viewport.
3. Use the normal user control for zoom in and zoom out.
4. Use the normal user control for panning.
5. Repeat zoom and pan enough to notice visual consistency or instability.

## Expected Prompts
- Do not assume exact prompt text.
- Record any status-bar changes or command-line messages that appear during viewport interaction.

## Expected Visual Feedback
- The view should redraw when zooming and panning.
- The tester should record whether the grid, overlays, and geometry remain visually coherent.
- The tester should record whether zoom anchors to the pointer, viewport center, or another position.

## Expected Application State
- The application remains interactive and does not leave the viewport visually corrupted after basic camera movement.

## AutoCAD Reference Behavior
- AutoCAD can be used only as a comparison point for zoom/pan predictability and feel.

## Pass Criteria
- Zoom and pan function through normal user input without obvious corruption, freeze, or lost interactivity.

## Evidence To Capture
- Screenshot before and after camera movement
- Notes on zoom anchor behavior
- Notes on pan direction and sensitivity
