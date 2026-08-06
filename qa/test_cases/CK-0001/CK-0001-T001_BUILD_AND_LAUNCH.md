# CK-0001-T001 - Build and Launch

## Metadata
- Test ID: `CK-0001-T001`
- Related work item: `CK-0001`
- Purpose: `Confirm that CadKit builds and launches into a responsive main window on the current Bazzite Linux environment.`

## Preconditions
- Repository is at the intended `CK-0001` test commit or a clearly documented later QA-only commit.
- Rust toolchain is installed.
- A graphical desktop session is available.

## Fixture
- `None`

## Human-Equivalent Actions
1. Run `python scripts/prepare_qa_build.py CK-0001 R001`.
2. Launch `./target/debug/cadkit` from a terminal.
3. Observe startup behavior and interact lightly with the window.
4. Close the window using the normal window-manager control.

## Expected Prompts
- Do not assume exact startup prompts.
- Record any terminal output, warnings, or visible startup dialogs exactly as observed.

## Expected Visual Feedback
- A main application window should appear.
- The viewport and primary UI chrome should be visible if startup succeeds.
- The UI should remain responsive to basic input long enough to begin testing.

## Expected Application State
- The application reaches an idle, usable initial state or fails in a way that can be clearly documented.

## AutoCAD Reference Behavior
- No direct AutoCAD reference. This is a CadKit desktop stability check.

## Pass Criteria
- The app builds successfully, launches, presents a usable main window, and closes without an obvious crash or hung process.

## Evidence To Capture
- Terminal output from build and launch
- Screenshot of the initial window
- Note on clean or unclean shutdown
