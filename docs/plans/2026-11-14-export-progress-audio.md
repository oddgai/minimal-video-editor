# Export progress and source audio correction

> **For agentic workers:** Use superpowers:executing-plans to implement these tasks inline.

**Goal:** Show export progress in a modal and preserve available source audio when correcting sync across cuts.

**Architecture:** Reuse the native export-progress events, cancellation, and job IDs. Select corrected audio from the original source for each retained video range, before concatenating the ranges; keep packet copy for both streams.

**Tech Stack:** Tauri, TypeScript, Rust, FFmpeg, Playwright.

**Spec:** User requests in this conversation: an export progress dialog and a fix for silence introduced by audio correction. Detailed design was delegated earlier.

## Global constraints

- Use pnpm; keep macOS app lightweight and preserve Windows support.
- Preserve encoded video/audio packets and safe export cancellation/no-overwrite behavior.
- Positive offset delays audio; source audio at video time t is t minus offset.
- Pending user choice: retain video length or omit video edges where original audio does not exist. Do not implement a destructive duration change before that answer.

## Review focus

- Cancellation/error must close the modal and allow another export.
- Escape/backdrop must not hide a running export or discard its cancellation control.
- Per-job progress must ignore stale jobs; exporting must block editing shortcuts.
- Shifted audio must use source packets outside retained video ranges when available.
- Keep B-frame boundaries, nonzero source timestamps, and no-audio files working.

## Task 1: Export progress modal

Files: `src/main.ts`, `src/styles.css`, `tests/e2e/native-mock.ts`, `tests/e2e/editor.spec.ts`.

- [x] Add an IPC-boundary test holding an export open, delivering progress, and completing/cancelling/failing it.
- [x] Confirm the test fails because the current progress panel is not modal.
- [x] Convert the existing panel to a labelled dialog with stage, percentage, progress bar, output path and cancel button. Later user direction: block shortcuts and body operations while open, show OK after success and retain the dialog until acknowledged.
- [x] Run frontend tests/typecheck.

## Task 2: Correct source audio selection

Files: `src-tauri/src/export.rs`, `src-tauri/tests/media_integration.rs`, `README.md`.

- [x] Add real FFmpeg regression tests verifying shifted source audio packets and audible coverage at retained range edges/joins for both signs.
- [x] Confirm tests reproduce the current incorrect audio selection.
- [x] Implement source-relative audio selection before cuts, retaining existing video length. Preserve empty audio headers in intermediate MOV/MP4 segments and prune packet preroll before joins. Tests cover ±12 seconds, B frames, nonzero source origin, and whole empty edge chapters in MP4/MOV/MKV.
- [ ] Resolve source-edge preference from the pending answer; only omit video edges if the user chooses that behavior.
- [x] Run Rust integration tests, fmt/clippy and frontend regression tests: 17 Node, 10 Playwright, 16 Rust tests pass; typecheck/clippy/fmt pass.
- [x] Build the macOS app: release bundle succeeded, 4.57 MiB.
- [x] Review the final changes and report behavior and unavoidable source limitations. Source-edge clipping remains pending the user's preference.

## Task 3: Close and quit shortcuts (added by user)

Files: `src/main.ts`, `src/bridge.ts`, `src/player.ts`, `src-tauri/src/lib.rs`, frontend tests and README.

- [x] Add failing UI test for Cmd/Ctrl W clearing media, edits, offset and decoder sources, and reopening successfully; Cmd/Ctrl Q requests exit.
- [x] Implement unload/native close cleanup (session, scan, preview server, destination) and native quit command.
- [x] Replace predefined native close/quit accelerators with app command menu events, guarded by the progress modal.
- [x] Confirm W/Q are ignored during running and completed modal states. Review found completion-modal drag/drop bypass; include dialog openness in busy(), rerender after OK, and test native event/drop paths as well.

## Review

Fresh read-only review `export_changes_review`: no remaining Critical/Important findings after the modal busy guard. Independently exercised wholly missing source audio with ±12-second offsets for MP4 and MKV. Native menu behavior inspected, not interactively automated.

Source-edge choice remains unanswered as of verification. Existing video duration is preserved; only unnecessary silence caused by cutting audio before correction is fixed. No video edges have been silently removed.
