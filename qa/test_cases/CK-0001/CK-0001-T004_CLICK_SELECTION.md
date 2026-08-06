# CK-0001-T004 - Basic Click Selection

## Metadata
- Test ID: `CK-0001-T004`
- Related work item: `CK-0001`
- Purpose: `Assess basic click-based selection, deselection, and any visible add/remove selection behavior.`

## Preconditions
- At least two selectable entities exist in the drawing.

## Fixture
- `None`

## Human-Equivalent Actions
1. Click one entity.
2. Click empty space.
3. Click another entity.
4. If a user-visible modifier or interaction suggests add-to-selection, test it.
5. If a user-visible modifier or interaction suggests remove-from-selection, test it.
6. Press `Escape` during or after selection activity.

## Expected Prompts
- Do not assume exact selection prompts.
- Record any command-line or status text that changes during selection.

## Expected Visual Feedback
- Record how selected entities are highlighted.
- Record whether deselection is visually obvious.
- Record whether retained selection state is clear or confusing.

## Expected Application State
- Selection state changes in response to user clicks, or the actual limitation is documented clearly.

## AutoCAD Reference Behavior
- AutoCAD may be used only as a comparison point for selection clarity and retained-selection behavior.

## Pass Criteria
- A tester can determine how click selection currently behaves and document it without guessing.

## Evidence To Capture
- Screenshots of selected and deselected states
- Notes on selection modifiers if any are discoverable
- Notes on `Escape` behavior
