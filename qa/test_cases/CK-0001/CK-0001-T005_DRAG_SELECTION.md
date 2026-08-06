# CK-0001-T005 - Window or Crossing Selection

## Metadata
- Test ID: `CK-0001-T005`
- Related work item: `CK-0001`
- Purpose: `Determine whether drag-selection exists and, if so, how its selection semantics behave.`

## Preconditions
- Multiple visible entities exist with some fully enclosed by a potential drag rectangle and some only crossed by it.

## Fixture
- `None`

## Human-Equivalent Actions
1. Attempt drag selection in one direction across multiple entities.
2. Attempt drag selection in the opposite direction if the UI appears to support it.
3. Release the mouse and observe the result.
4. Press `Escape` during or after the drag-selection attempt where meaningful.

## Expected Prompts
- Do not assume exact prompt text.
- Record any messages that appear when starting, dragging, or releasing selection.

## Expected Visual Feedback
- Record whether a rectangle appears.
- Record when it first appears.
- Record whether its appearance differs by drag direction.
- Record which entities appear selected after release.

## Expected Application State
- Either drag-selection behavior is observable and documentable, or the absence of the feature is recorded clearly.

## AutoCAD Reference Behavior
- AutoCAD may be used only as a comparison point if CadKit visibly presents window/crossing-like behavior.

## Pass Criteria
- The tester can describe actual drag-selection behavior, including directionality and selection outcome, without assuming undocumented functionality.

## Evidence To Capture
- Screenshot or recording of the selection rectangle
- Notes on drag direction and release behavior
- Notes on `Escape` cancellation behavior
