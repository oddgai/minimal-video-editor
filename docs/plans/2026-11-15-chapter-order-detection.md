# Chapter order and fade detection

> **For agentic workers:** Execute inline; the independent FFmpeg detector module is delegated using superpowers:dispatching-parallel-agents. Detailed design was delegated by the user earlier.

**Goal:** Reorder chapters by dragging or Cmd/Ctrl Shift Left/Right, and split automatically at detected black/white transitions.

**Architecture:** Treat chapter array order as edited timeline order, with source ranges remaining disjoint. Use explicit active playback span for sequencing across reordered ranges. Export validates disjoint ranges independently of their output order. Fade detection is an explicit cancellable FFmpeg scan that returns source keyframes; apply splits only inside retained chapters as a single undoable edit.

## Constraints

- Keep the minimal English UI and existing playback/audio fixes, click-to-nearest-keyframe behavior, Cmd W/Q and export modal guards.
- Use pnpm; no dependencies, re-encoding of exports or source-file changes.
- Drag uses pointer events instead of native HTML file drag/drop, so importing files keeps working.
- Fade detection limits sampling/filter resolution and worker threads; video decoding is still needed, only when requested. Group black/white intervals to one cut and ignore source head/tail fades.
- Automatic splits preserve current deletion/order/offset; cancellation adds no chapters. Undo once restores the pre-detection edit.

## Tasks

- [x] Add failing model/Rust/browser tests for reordered mappings, navigation, playback, export order and undo.
- [x] Implement reorder/history, order-independent mappings/keyframes, playback span sequencing, pointer drag marker and keyboard reorder.
- [x] Accept native export ranges in output order while rejecting source overlaps; real packet and decoded frame sequence test passes.
- [x] Implement independent detection module and seven real synthetic media/cancellation tests (agent `fade_detection`).
- [x] Integrate scan IPC, progress/cancel dialog, compact Detect chapters control and bulk split history. Test progress/cancel/no-result/preservation behavior. Commit an active offset input before a native-menu detection request.
- [x] Run complete checks: 20 Node, 15 Playwright, 24 Rust tests pass; final frontend regression after input-commit and EOF fixes passes. Typecheck/fmt/clippy pass.
- [x] Focused independent review `order_detection_review`: confirmed edited EOF context issue; added failing EOF navigation/jump-dialog tests and fixed explicit end handling. No other Critical/Important findings.
- [x] Native WKWebView reorder probe passes with a real 12-second H.264/AAC fixture and −0.25-second audio offset: source EOF jumps back to the next edited clip, edited EOF pauses at source 7.999, restart begins at source 8.272. Final A/V error 0.0242 seconds; no media errors.
- [x] macOS release bundle built successfully, 4.62 MiB. Features and detection sensitivity documented in README.

## Detection sensitivity

Samples 160×90 grayscale at 5 fps with two decoder threads. At least 98.5% of pixels must reach near-black (≤12) or near-white (≥243), with visible content on both sides and gradual brightness context. Entire blackout/whiteout intervals produce one cut. Very fast/incomplete/overlaid transitions can be missed; plain hard scene cuts are intentionally ignored. Original file is read-only. Progress IPC updates are throttled to 0.5% increments.
