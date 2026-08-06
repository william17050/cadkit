# CK-0001-T006 - Delete and Undo

## Metadata
- Test ID: `CK-0001-T006`
- Related work item: `CK-0001`
- Purpose: `Assess normal delete behavior and whether undo/redo restore expected visible state.`

## Preconditions
- At least one selectable entity exists.

## Fixture
- `None`

## Human-Equivalent Actions
1. Select an entity using the normal user-facing method.
2. Delete it using the ordinary user-facing method.
3. Undo the deletion.
4. Confirm whether the entity returns.
5. If redo exists through normal user-facing controls, test redo as well.

## Expected Prompts
- Do not assume exact delete or undo prompt wording.
- Record actual command-line or status messages if present.

## Expected Visual Feedback
- Record how deletion is shown.
- Record whether the restored entity looks identical after undo.
- Record retained selection and highlight behavior after delete, undo, and redo.

## Expected Application State
- The drawing state changes through delete and is restorable through undo if the feature works as currently implemented.

## AutoCAD Reference Behavior
- AutoCAD may be used only as a comparison point for user expectations around delete and undo consistency.

## Pass Criteria
- The tester can document the observable delete/undo/redo behavior and any retained-state issues clearly.

## Evidence To Capture
- Before/after screenshots
- Notes on delete method used
- Notes on undo and redo results
