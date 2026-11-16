# Minimal Video Editor Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task in this session. The user delegated detailed design and requested implementation; continue without further design approval.

**Goal:** Build a lightweight macOS video editor with lossless cuts, keyboard operation, and manual audio sync; preserve a Windows-compatible architecture.

**Architecture:** A Tauri application serves a small vanilla TypeScript interface. Rust probes local files, streams keyframe metadata, and manages cancellable FFmpeg exports without shell interpolation. Native WebView media playback avoids decoding video into JavaScript.

**Tech Stack:** Tauri 2, Rust, TypeScript, Vite, Node test runner, FFmpeg/ffprobe.

**Spec:** `docs/specs/2026-11-13-minimal-video-editor-design.md`

## Global Constraints
- macOS first; Windows-compatible paths, processes, and shortcuts.
- No re-encoding, full-file JavaScript reads, permanent proxies, or background waveform/thumbnail generation.
- Source and existing destination files must never be overwritten.
- Audio offset is in seconds; positive means audio later.
- No remote assets or services at runtime.

## Review Focus
- Nonzero source timestamps and variable frame rates: normalized keyframes and bounded markers.
- Silent video and unsupported native preview codecs: usable editor and actionable preview error.
- File names with Unicode, spaces, and apostrophes: argument-based process spawning and escaped concat entries.
- Cancellation and concurrent export requests: one active export and no partial destination.
- Positive/negative audio offsets: preserved stream data and independently measured timing differences.

### Task 1: Editing model and app configuration
**Files:** package.json, tsconfig.json, vite.config.ts, index.html, src/editor.ts, tests/editor.test.ts, src-tauri/Cargo.toml, src-tauri/tauri.conf.json, src-tauri/capabilities/default.json.
**Interfaces:** `Range { start: number; end: number }`, `Editor` manages retained ranges, marker snapping, history, keyframe seeking, and audio delay.
- [x] Write tests for removing a middle range, keeping across deleted gaps, undo/redo including delay, snapping, invalid/empty edits, and next/previous keyframe boundaries.
- [x] Run `pnpm test` and observe missing implementation failure.
- [x] Implement the pure editing model and minimal Tauri/Vite configuration.
- [x] Run `pnpm test` and `pnpm check`; expect all tests/typechecks to pass.

### Task 2: Native media engine
**Files:** src-tauri/src/media.rs, src-tauri/src/export.rs, src-tauri/src/lib.rs, src-tauri/src/main.rs, src-tauri/build.rs, src-tauri/tests/media_integration.rs.
**Interfaces:** `probe_media(path) -> MediaInfo`, `scan_keyframes(path) -> Vec<f64>`, `start_export(request)` with progress events, `cancel_export()`; dialog commands select input/output; local asset URLs are scoped to selected media.
- [x] Add Rust validation/probe/export tests and run `cargo test` to observe failures before implementing the engine.
- [x] Implement executable discovery, safe async commands, metadata parsing, streamed packet scanning, stream-copy segment/concat/remux, cancellation, progress, and atomic no-clobber publish.
- [x] Use real FFmpeg fixtures to check retained duration, unchanged packet hashes, offsets in both directions, silent media, unusual paths, overwrite protection, and cancellation.
- [x] Run `cargo test`; expect unit and integration tests to pass.

### Task 3: Desktop editor interface
**Files:** src/main.ts, src/styles.css, src/bridge.ts, src/player.ts, src/icons.ts, tests/player.test.ts.
**Interfaces:** Typed native bridge consumes MediaInfo and Range, sends ExportRequest, listens for job-scoped progress. Player synchronizes audio to `video.currentTime - offset` only while playing.
- [x] Test audio timing decisions for positive/negative offsets, bounds, and paused playback before implementing the synchronizer.
- [x] Build Japanese editor UI: native open/drop, source metadata, playback, editable markers, keyframe timeline, range operations/history, offset controls, export/cancel, accessible shortcut dialog, visible loading/error/empty states.
- [x] Verify `pnpm test`, `pnpm check`, `pnpm build`, and desktop compilation.

### Task 4: Distribution, verification, and review
**Files:** README.md, .github/workflows/ci.yml, scripts/verify-media.mjs as needed.
- [x] Document setup, launch/build, FFmpeg discovery, shortcuts, precision and preview limitations.
- [x] Add macOS/Windows CI typechecks, editing tests, Rust checks/tests, and application build.
- [x] Run the full tests, formatter, clippy, release build, and smoke checks; record actual evidence and any environment limitations.
- [x] Request a fresh code review, address significant findings, then leave the implementation and runnable artifacts available in the user's workspace.

## Verification record (2026-11-13)

- `pnpm test`: 11 passing tests covering editing and audio timing.
- `pnpm check` / `pnpm build`: passing; production frontend approximately 47 kB JavaScript + 16 kB CSS, before compression.
- `cargo test --manifest-path src-tauri/Cargo.toml --offline`: 2 unit + 9 real FFmpeg integration tests passing.
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `cargo clippy --manifest-path src-tauri/Cargo.toml --offline --all-targets -- -D warnings`: passing.
- `ppnpm test:ui`: 2 passing Chromium UI tests using the real UI and mocked native IPC. These cover input protection, keyboard button activation, editing/history, audio offset, export payload, dialogs, and browser-only error behavior; not native dialogs or WebView media decoding.
- `pnpm desktop:build`: macOS application bundle built successfully, 4.52 MiB excluding external FFmpeg. Native process startup smoke-tested outside the filesystem sandbox.
- Fresh reviewer identified inconsistent per-segment timestamp origins, blocked Enter activation, and obsolete scans continuing in the background. Each received a regression test; all fixes are included. B-frame join spacing is now 2.000 seconds for the tested [0,2]+[4,6] fixture rather than 2.074707 seconds.
- macOS and Windows CI configurations added. Windows CI and interactive Windows behavior have not been run in this session. Signing/notarization and bundling FFmpeg are outside the first local development build.
