# Chapter Detection Another 10x Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reduce cold fast-scan time by at least another 10x on the two user-provided videos while retaining the same detected boundaries.

**Architecture:** Use compressed MP4 sample-size minima near keyframes to plan a small set of windows. Inspect those windows with four concurrent H.264 decoders that skip non-reference frames, then run the existing full decoder only for windows where the fast pass finds no boundary. Precise full-video scanning remains available.

**Tech Stack:** Rust, FFmpeg, Tauri, pnpm; no new dependencies.

**Spec:** User goal `detect chapterをあと10倍高速化して`; prior benchmark artifacts under `.superpowers/tenfold/`.

## Global Constraints

- Preserve the precise full-scan path and its existing detector thresholds.
- Keep video files read-only and do not retain decoded frames after a scan.
- Cancellation must stop and reap every active FFmpeg process.
- Use pnpm for JavaScript package commands.
- Compare cold scans against the saved 4-thread baseline with the same two input files.

## Review Focus

- Concurrent cancellation: cancel must kill and reap all active children.
- Sparse-reference H.264 output: fast-pass boundaries must remain on the source timeline.
- Candidate-free supported MP4: return quickly in fast mode; precise mode must still inspect the whole file.
- Unsupported/fragmented MP4: retain the existing precise fallback.
- Candidate with no fast-pass boundary: exact local verification must recover it.

---

### Task 1: Select sparse candidate windows

**Files:**
- Modify: `src-tauri/src/detection_candidates.rs`
- Modify: `src-tauri/src/detection.rs`

**Interfaces:** `candidate_windows(path, info, keyframes)` returns zero or more windows from compact low-packet-size clusters near source keyframes.

- [x] Group sorted sample-size candidates, retain local minima at or below 0.3 bytes per macroblock when within 0.15 seconds before a keyframe, and retain the 0.3–0.7 range only for minima 0.10–0.15 seconds before a keyframe.
- [x] Merge selected event windows with 2 seconds of context and keep the existing dense-window fallback.
- [x] Verify the real-file planner produces no boundary candidates for video 1 and the seven expected locations for video 2.

### Task 2: Parallel coarse pass and local exact fallback

**Files:**
- Modify: `src-tauri/src/detection.rs`

**Interfaces:** `DetectionControl` tracks multiple child processes; window workers return detector boundaries or errors through a channel.

- [x] Refactor child tracking so cancellation kills all registered FFmpeg processes and each worker reaps its own child.
- [x] Run candidate windows in a pool capped at four workers, with `-skip_frame noref` for the coarse pass.
- [x] Re-run only windows with no coarse boundary using the existing full-frame detector; keep precise mode as one full-video scan.
- [x] Preserve monotonic progress events and boundary snapping/deduplication.
- [x] Verify cancel handling and full precision fallback through the existing Rust integration tests.

### Task 3: Measure and package

**Files:**
- Use: `src-tauri/examples/detection_bench.rs`
- Build: `src-tauri/target/release/bundle/macos/Minimal Video Editor.app`

- [x] Run Rust formatting, clippy, and the relevant existing Rust integration tests.
- [x] Measure cold scans for both user-provided files; ignore cached repeat timings.
- [x] Require exact boundary-list parity and at least 10x improvement over the previous fast-scan measurements (12.8024s and 28.6653s).
- [x] Build the macOS app bundle after measurement passes.
