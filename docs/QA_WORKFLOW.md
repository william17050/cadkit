# QA Workflow

## Loop
1. Bill defines or approves the work item.
2. Codex implements the change.
3. Codex builds CadKit and performs limited smoke testing.
4. Codex writes the next versioned handoff in `qa/handoffs/`.
5. Bill starts Claude Code and directs it to the newest handoff.
6. Claude Code operates CadKit as a playtester through normal user-facing controls.
7. Claude Code writes the next versioned report in `qa/reports/` and stores evidence in `qa/evidence/`.
8. Bill directs Codex to the newest report.
9. Codex fixes unresolved findings and writes the next handoff version.
10. The loop repeats until the work item is behaviorally acceptable.
11. Bill performs final acceptance.

## Artifact Rules
- Handoffs use `qa/handoffs/CK-XXXX_HNNN.md`
- Reports use `qa/reports/CK-XXXX_RNNN.md`
- Codex authors handoffs.
- Claude Code authors reports.
- Use the newest applicable handoff or report by default.

## Discovery Commands
- Latest handoff: `python scripts/latest_handoff.py CK-0001`
- Latest report: `python scripts/latest_report.py CK-0001`

## Operating Rules
- Act through the human path; verify through the machine path.
- Claude Code must not edit application source.
- Codex must not close behavioral findings as passed.
- Behavioral issues close only after Claude Code retests them or Bill explicitly accepts them.

## Recommended Session Shape

### Codex Session
- implement targeted changes
- build and run automated validation
- smoke test only what is necessary to avoid obviously broken handoffs
- record known limitations and explicit exclusions

### Claude Code Session
- read the newest handoff first
- launch the built executable identified in the handoff
- execute the required scenarios as a human would
- capture evidence for both failures and notable passes when helpful
- write the next versioned report with reproduction details and subjective interaction notes

## Evidence Guidance
- Store screenshots, screen recordings, exported files, and relevant fixtures under `qa/evidence/` and `qa/fixtures/`.
- Reference exact evidence file paths from reports.
- Prefer work-item-specific subfolders once real test volume grows.

## Escalation Guidance
- If Claude Code encounters a blocker caused by missing setup information, report it explicitly rather than guessing.
- If Codex identifies tooling or observability gaps that prevent efficient playtesting, document them separately from the requested functional work.
