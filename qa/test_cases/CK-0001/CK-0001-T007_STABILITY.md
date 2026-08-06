# CK-0001-T007 - Stability Pass

## Metadata
- Test ID: `CK-0001-T007`
- Related work item: `CK-0001`
- Purpose: `Exercise a short mixed interaction sequence and document crashes, freezes, artifacts, or stuck modes.`

## Preconditions
- CadKit is running and basic geometry creation has already been attempted successfully enough to continue.

## Fixture
- `None`

## Human-Equivalent Actions
1. Draw geometry.
2. Zoom.
3. Pan.
4. Select and deselect entities.
5. Delete an entity.
6. Undo the deletion.
7. Start a command and cancel it.

## Expected Prompts
- Do not assume exact prompt text.
- Record any messages that suggest stuck state, failed transitions, or command confusion.

## Expected Visual Feedback
- Record any flicker, stale rendering, highlight glitches, or obviously incorrect viewport state.

## Expected Application State
- The application remains responsive through the short mixed sequence, or the failure mode is documented precisely.

## AutoCAD Reference Behavior
- AutoCAD may be used only as a comparison point for overall interaction smoothness and command-state clarity.

## Pass Criteria
- The tester completes the mixed sequence and can clearly document any instability, retained state, or graphical artifact encountered.

## Evidence To Capture
- Short screen recording if practical
- Notes on any crash, freeze, or stuck mode
- Screenshots of visible artifacts if they occur
